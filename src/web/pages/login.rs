use dioxus::prelude::*;

use crate::web::{
    app::Route,
    components::ErrorBanner,
    pages::otp_password_form::OtpPasswordForm,
    server_fns::{get_me, get_owner_username, login, password_reset_init, password_reset_verify},
    sfn_msg,
    state::{AuthSignal, AuthUser, save_auth},
};

#[derive(Clone, PartialEq)]
enum View {
    SignIn,
    ResetContact,
    ResetCode {
        otp_id: String,
        dev_code: Option<String>,
    },
}

#[component]
pub fn LoginPage() -> Element {
    let mut auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    let owner = use_resource(|| async { get_owner_username().await.ok() });

    let mut view = use_signal(|| View::SignIn);
    let mut error = use_signal(|| Option::<String>::None);
    let mut loading = use_signal(|| false);

    // Sign-in fields
    let mut login_val = use_signal(String::new);
    let mut password = use_signal(String::new);

    // Reset fields
    let mut reset_contact = use_signal(String::new);
    let reset_code = use_signal(String::new);
    let new_password = use_signal(String::new);
    let new_password2 = use_signal(String::new);

    // Shared logic: fetch profile for a token, persist auth, and navigate home.
    let complete_login = move |token: String| async move {
        match get_me(token.clone()).await {
            Ok(me) => {
                let user = AuthUser {
                    token,
                    username: me.username,
                    ap_id: me.ap_id,
                };
                save_auth(&user);
                auth.set(Some(user));
                nav.push(Route::Home {});
            }
            Err(e) => error.set(Some(sfn_msg(&e))),
        }
    };

    let on_signin = move |_: Event<MouseData>| {
        let l = login_val.read().clone();
        let p = password.read().clone();
        if l.is_empty() || p.is_empty() {
            error.set(Some("Please fill in all fields.".into()));
            return;
        }
        loading.set(true);
        error.set(None);
        spawn(async move {
            match login(l, p).await {
                Ok(result) => complete_login(result.token).await,
                Err(e) => error.set(Some(sfn_msg(&e))),
            }
            loading.set(false);
        });
    };

    let on_reset_request = move |_: Event<MouseData>| {
        let c = reset_contact.read().clone();
        if c.is_empty() {
            error.set(Some("Enter your email or phone number.".into()));
            return;
        }
        loading.set(true);
        error.set(None);
        spawn(async move {
            match password_reset_init(c).await {
                Ok(r) => view.set(View::ResetCode {
                    otp_id: r.otp_id,
                    dev_code: r.code,
                }),
                Err(e) => error.set(Some(sfn_msg(&e))),
            }
            loading.set(false);
        });
    };

    let on_reset_verify = move |_: Event<MouseData>| {
        let View::ResetCode { otp_id, .. } = view.read().clone() else {
            return;
        };
        let c = reset_code.read().clone();
        let p = new_password.read().clone();
        let p2 = new_password2.read().clone();
        if c.is_empty() || p.is_empty() {
            error.set(Some("Please fill in all fields.".into()));
            return;
        }
        if p != p2 {
            error.set(Some("Passwords do not match.".into()));
            return;
        }
        loading.set(true);
        error.set(None);
        spawn(async move {
            match password_reset_verify(otp_id, c, p).await {
                Ok(result) => complete_login(result.token).await,
                Err(e) => error.set(Some(sfn_msg(&e))),
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
                    if let Some(Some(u)) = owner.read().as_ref() {
                        p { class: "auth-instance", "Dedicated server for "
                            code { "@{u}" }
                        }
                    }
                    p { class: "auth-subtitle",
                        match *view.read() {
                            View::SignIn => "Sign in to your account",
                            View::ResetContact => "Reset your password",
                            View::ResetCode { .. } => "Enter your code",
                        }
                    }
                }

                if let Some(err) = error.read().as_ref() {
                    ErrorBanner { message: err.clone() }
                }

                match view.read().clone() {
                    View::SignIn => rsx! {
                        div { class: "form-group",
                            label { r#for: "login-field", "Username, email, or phone" }
                            input {
                                id: "login-field",
                                r#type: "text",
                                placeholder: "username, you@example.com, or +1234567890",
                                autocomplete: "username",
                                value: "{login_val}",
                                oninput: move |e| login_val.set(e.value()),
                            }
                        }
                        div { class: "form-group",
                            label { r#for: "password", "Password" }
                            input {
                                id: "password",
                                r#type: "password",
                                placeholder: "••••••••",
                                autocomplete: "current-password",
                                value: "{password}",
                                oninput: move |e| password.set(e.value()),
                            }
                        }
                        button {
                            class: "btn btn-primary btn-full",
                            disabled: *loading.read(),
                            onclick: on_signin,
                            if *loading.read() { "Signing in…" } else { "Sign in" }
                        }
                        div { class: "auth-footer",
                            button {
                                class: "btn-link",
                                onclick: move |_| {
                                    error.set(None);
                                    view.set(View::ResetContact);
                                },
                                "Forgot password?"
                            }
                        }
                    },

                    View::ResetContact => rsx! {
                        p { class: "auth-hint",
                            "Enter the email or phone number linked to your account. \
                             We'll send you a one-time code."
                        }
                        div { class: "form-group",
                            label { r#for: "reset-contact", "Email or phone" }
                            input {
                                id: "reset-contact",
                                r#type: "text",
                                placeholder: "you@example.com",
                                autocomplete: "email",
                                value: "{reset_contact}",
                                oninput: move |e| reset_contact.set(e.value()),
                            }
                        }
                        button {
                            class: "btn btn-primary btn-full",
                            disabled: *loading.read() || reset_contact.read().trim().is_empty(),
                            onclick: on_reset_request,
                            if *loading.read() { "Sending…" } else { "Send code" }
                        }
                        div { class: "auth-footer",
                            button {
                                class: "btn-link",
                                onclick: move |_| {
                                    error.set(None);
                                    view.set(View::SignIn);
                                },
                                "Back to sign in"
                            }
                        }
                    },

                    View::ResetCode { dev_code, otp_id: _ } => rsx! {
                        OtpPasswordForm {
                            dev_code,
                            otp: reset_code,
                            password: new_password,
                            password2: new_password2,
                            loading: *loading.read(),
                            on_submit: on_reset_verify,
                            submit_label: "Set new password".to_string(),
                            loading_label: "Updating…".to_string(),
                            password_label: "New password".to_string(),
                            password2_label: "Confirm new password".to_string(),
                            on_resend: move |_| {
                                error.set(None);
                                view.set(View::ResetContact);
                            },
                        }
                    },
                }
            }
        }
    }
}
