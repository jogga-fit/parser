use dioxus::prelude::*;

use crate::web::{
    app::Route,
    components::ErrorBanner,
    pages::otp_password_form::OtpPasswordForm,
    server_fns::{get_me, otp_verify},
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

    rsx! {
        div { class: "auth-page",
            div { class: "auth-card",
                div { class: "auth-header",
                    div { class: "auth-logo", i { class: "ph ph-person-simple-run" } }
                    h1 { "Jogga:" }
                    p { class: "auth-subtitle",
                        if *invalid_otp.read() { "Code expired" } else { "Set your password" }
                    }
                }

                if let Some(err) = error.read().as_ref() {
                    ErrorBanner { message: err.clone() }
                }

                if *invalid_otp.read() {
                    p { class: "auth-hint",
                        "This verification link has expired or already been used. \
                         Please request a new code."
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
