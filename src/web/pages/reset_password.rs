use dioxus::prelude::*;

use crate::web::{
    app::Route,
    components::ErrorBanner,
    pages::otp_password_form::OtpPasswordForm,
    server_fns::{get_me, get_owner_username, otp_verify, password_reset_init},
    sfn_msg,
    state::{AuthSignal, AuthUser, save_auth},
};

fn percent_decode(s: &str) -> String {
    let mut buf: Vec<u8> = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            buf.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                buf.push(((h as u8) << 4) | (l as u8));
                i += 3;
            } else {
                buf.push(bytes[i]);
                i += 1;
            }
        } else {
            buf.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn parse_query(query: &str) -> (String, Option<String>, Option<String>) {
    let query = query.strip_prefix('?').unwrap_or(query);
    let mut otp_id = String::new();
    let mut otp_code = None::<String>;
    let mut display_name = None::<String>;

    for part in query.split('&') {
        if let Some(v) = part.strip_prefix("code=") {
            let decoded = percent_decode(v);
            if let Some((id, code)) = decoded.split_once('.') {
                otp_id = id.to_string();
                otp_code = Some(code.to_string());
            } else {
                otp_id = decoded;
            }
        } else if let Some(v) = part.strip_prefix("dn=") {
            let decoded = percent_decode(v);
            if !decoded.is_empty() {
                display_name = Some(decoded);
            }
        }
    }

    (otp_id, otp_code, display_name)
}

fn effective_query(query: &str) -> String {
    let trimmed = query.strip_prefix('?').unwrap_or(query);
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    browser_query()
}

fn browser_query() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.location().search().ok())
            .unwrap_or_default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    String::new()
}

/// Shared OTP-verify + password page for both registration and password-reset.
///
/// Query params:
/// - `code=<uuid>` — OTP request ID; user types OTP manually
/// - `code=<uuid>.<6digits>` — ID + OTP pre-filled (magic link)
/// - `dn=<display_name>` — optional; forwarded from registration step 1
#[component]
pub fn ResetPasswordPage(query: String) -> Element {
    let mut auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    let owner = use_resource(|| async { get_owner_username().await.ok() });

    let (initial_otp_id, initial_pre_otp, initial_display_name) =
        parse_query(&effective_query(&query));

    let mut error = use_signal(|| Option::<String>::None);
    let mut loading = use_signal(|| false);
    let mut invalid_otp = use_signal(|| false);

    let mut otp_id = use_signal(|| initial_otp_id);
    let mut display_name = use_signal(|| initial_display_name);
    let mut otp_code = use_signal(|| initial_pre_otp.unwrap_or_default());
    let password = use_signal(String::new);
    let password2 = use_signal(String::new);

    // Re-request state — shown when otp_id is missing or the OTP has expired.
    let mut rerequest_contact = use_signal(String::new);
    let mut rerequest_loading = use_signal(|| false);
    let mut rerequest_sent = use_signal(|| false);
    let mut rerequest_error = use_signal(|| Option::<String>::None);

    let query_for_effect = query.clone();
    use_effect(move || {
        let (id, code, dn) = parse_query(&effective_query(&query_for_effect));
        if !id.is_empty() && *otp_id.read() != id {
            otp_id.set(id);
        }
        if let Some(code) = code {
            if *otp_code.read() != code {
                otp_code.set(code);
            }
        }
        if display_name.read().clone() != dn {
            display_name.set(dn);
        }
    });

    let on_submit = move |_: Event<MouseData>| {
        let code = otp_code.read().clone();
        let pwd = password.read().clone();
        let pwd2 = password2.read().clone();
        let id = otp_id.read().clone();
        let dn = display_name.read().clone();

        if code.len() < 6 || pwd.is_empty() {
            error.set(Some("Please fill in all fields.".into()));
            return;
        }
        if pwd != pwd2 {
            error.set(Some("Passwords don't match.".into()));
            return;
        }

        loading.set(true);
        error.set(None);

        spawn(async move {
            match otp_verify(id, code, pwd, dn).await {
                Ok(result) => {
                    let (username, ap_id) = if let Some(u) = result.username {
                        (u, result.ap_id.unwrap_or_default())
                    } else {
                        match get_me(result.token.clone()).await {
                            Ok(me) => (me.username, me.ap_id),
                            Err(e) => {
                                error.set(Some(sfn_msg(&e)));
                                loading.set(false);
                                return;
                            }
                        }
                    };
                    let user = AuthUser {
                        token: result.token,
                        username,
                        ap_id,
                    };
                    save_auth(&user);
                    auth.set(Some(user));
                    nav.push(Route::Home {});
                }
                Err(e) => {
                    let msg = sfn_msg(&e);
                    if msg.contains("invalid")
                        || msg.contains("expired")
                        || msg.contains("already used")
                    {
                        invalid_otp.set(true);
                    } else {
                        error.set(Some(msg));
                    }
                }
            }
            loading.set(false);
        });
    };

    // Show re-request form when there's no otp_id (bare /reset-password) or
    // after the user submits and the OTP is found to be expired/used.
    let no_code = otp_id.read().is_empty();
    let show_rerequest = no_code || *invalid_otp.read();

    let on_rerequest = move |_: Event<MouseData>| {
        let contact = rerequest_contact.read().trim().to_string();
        if contact.is_empty() {
            rerequest_error.set(Some("Please enter your email or phone.".into()));
            return;
        }
        rerequest_loading.set(true);
        rerequest_error.set(None);
        spawn(async move {
            match password_reset_init(contact).await {
                Ok(_) => rerequest_sent.set(true),
                Err(e) => rerequest_error.set(Some(sfn_msg(&e))),
            }
            rerequest_loading.set(false);
        });
    };

    rsx! {
        div { class: "auth-page",
            div { class: "auth-card",
                div { class: "auth-header",
                    div { class: "auth-logo", i { class: "ph ph-person-simple-run" } }
                    h1 { "Jogga:" }
                    if let Some(Some(u)) = owner.read().as_ref() {
                        p { class: "auth-instance", "Dedicated server for "
                            code { "@{u}" }
                        }
                    }
                    p { class: "auth-subtitle",
                        if show_rerequest { "Reset your password" } else { "Set your password" }
                    }
                }

                if let Some(err) = error.read().as_ref() {
                    ErrorBanner { message: err.clone() }
                }

                if show_rerequest {
                    if *invalid_otp.read() {
                        p { class: "auth-hint otp-expired-hint",
                            "This verification link has expired or has already been used."
                        }
                    } else {
                        p { class: "auth-hint",
                            "No reset code found. Enter your email or phone to receive a new link."
                        }
                    }

                    if *rerequest_sent.read() {
                        p { class: "auth-hint auth-hint-success",
                            "Reset link sent — check your inbox and follow the link."
                        }
                    } else {
                        if let Some(err) = rerequest_error.read().as_ref() {
                            ErrorBanner { message: err.clone() }
                        }
                        div { class: "auth-field",
                            label { r#for: "rerequest-contact", "Email or phone" }
                            input {
                                id: "rerequest-contact",
                                r#type: "text",
                                autocomplete: "email",
                                placeholder: "you@example.com",
                                value: "{rerequest_contact}",
                                oninput: move |e| rerequest_contact.set(e.value()),
                            }
                        }
                        button {
                            class: "btn btn-primary btn-full",
                            disabled: *rerequest_loading.read(),
                            onclick: on_rerequest,
                            if *rerequest_loading.read() { "Sending…" } else { "Send reset link" }
                        }
                    }

                    div { class: "auth-footer",
                        Link { to: Route::Login {}, "Back to sign in" }
                    }
                } else {
                    OtpPasswordForm {
                        dev_code: None,
                        otp: otp_code,
                        password,
                        password2,
                        loading: *loading.read(),
                        on_submit,
                        submit_label: "Continue".to_string(),
                        loading_label: "Verifying…".to_string(),
                        password_label: "Password".to_string(),
                        password2_label: "Confirm password".to_string(),
                    }
                    div { class: "auth-footer",
                        Link { to: Route::Login {}, "Back to sign in" }
                    }
                }
            }
        }
    }
}
