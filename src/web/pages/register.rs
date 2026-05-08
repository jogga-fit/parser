use dioxus::prelude::*;

use crate::{
    web::{
        app::Route,
        components::ErrorBanner,
        server_fns::register_init,
        sfn_msg,
    },
};

#[component]
pub fn RegisterPage() -> Element {
    let nav = use_navigator();

    let mut error = use_signal(|| Option::<String>::None);
    let mut loading = use_signal(|| false);

    let mut username = use_signal(String::new);
    let mut contact = use_signal(String::new);
    let mut display_name = use_signal(String::new);

    let on_init = move |_: Event<MouseData>| {
        let u = username.read().clone();
        let c = contact.read().clone();

        if u.is_empty() || c.is_empty() {
            error.set(Some("Please fill in all fields.".into()));
            return;
        }

        let (email, phone) = if c.contains('@') {
            (Some(c), None)
        } else {
            (None, Some(c))
        };

        loading.set(true);
        error.set(None);

        let dn = display_name.read().clone();

        spawn(async move {
            match register_init(u, email, phone).await {
                Ok(r) => {
                    #[allow(unused_mut)]
                    let mut code_param = r.otp_id.clone();
                    #[cfg(debug_assertions)]
                    if let Some(c) = &r.code {
                        code_param.push('.');
                        code_param.push_str(c);
                    }
                    let mut query = format!("code={code_param}");
                    if !dn.is_empty() {
                        let encoded: String = dn
                            .chars()
                            .flat_map(|c| {
                                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                                    format!("{c}").chars().collect::<Vec<_>>()
                                } else if c == ' ' {
                                    vec!['+']
                                } else {
                                    let mut buf = [0u8; 4];
                                    let s = c.encode_utf8(&mut buf);
                                    s.bytes()
                                        .flat_map(|b| {
                                            format!("%{b:02X}").chars().collect::<Vec<_>>()
                                        })
                                        .collect::<Vec<_>>()
                                }
                            })
                            .collect();
                        query.push_str(&format!("&dn={encoded}"));
                    }
                    nav.push(Route::ResetPassword { query });
                }
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
                    p { class: "auth-subtitle", "Create your account" }
                }

                if let Some(err) = error.read().as_ref() {
                    ErrorBanner { message: err.clone() }
                }

                div { class: "form-group",
                    label { r#for: "username", "Username" }
                    input {
                        id: "username",
                        r#type: "text",
                        placeholder: "coolrunner42",
                        autocomplete: "username",
                        value: "{username}",
                        oninput: move |e| username.set(e.value()),
                    }
                    span { class: "form-hint", "Letters, numbers, underscores. 1–30 chars." }
                }
                div { class: "form-group",
                    label { r#for: "contact", "Email or phone" }
                    input {
                        id: "contact",
                        r#type: "text",
                        placeholder: "you@example.com or +1234567890",
                        value: "{contact}",
                        oninput: move |e| contact.set(e.value()),
                    }
                    span { class: "form-hint", "Used for verification only." }
                }
                div { class: "form-group",
                    label { r#for: "display_name", "Display name (optional)" }
                    input {
                        id: "display_name",
                        r#type: "text",
                        placeholder: "Alex Runner",
                        value: "{display_name}",
                        oninput: move |e| display_name.set(e.value()),
                    }
                }
                button {
                    class: "btn btn-primary btn-full",
                    disabled: *loading.read(),
                    onclick: on_init,
                    if *loading.read() { "Sending code…" } else { "Send verification code" }
                }

                div { class: "auth-footer",
                    "Already have an account? "
                    Link { to: Route::Login {}, "Sign in" }
                }
            }
        }
    }
}
