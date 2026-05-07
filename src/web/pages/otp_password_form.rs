use dioxus::prelude::*;

/// Shared OTP + new-password form used by the registration verify step and
/// the password-reset verify step.
///
/// The caller owns the signals and the submit handler so that server call
/// logic (and its error/loading state) stays with the page component.
#[component]
pub fn OtpPasswordForm(
    /// Dev-mode OTP hint rendered only in debug builds when `Some`.
    dev_code: Option<String>,
    /// Signal bound to the one-time-code input.
    mut otp: Signal<String>,
    /// Signal bound to the new-password input.
    mut password: Signal<String>,
    /// Signal bound to the confirm-password input.
    mut password2: Signal<String>,
    /// Whether the form is currently submitting (disables the button).
    loading: bool,
    /// Called when the user clicks the submit button.
    on_submit: EventHandler<Event<MouseData>>,
    /// Button label in the idle state, e.g. "Create account".
    submit_label: String,
    /// Button label while `loading` is true, e.g. "Creating account…".
    loading_label: String,
    /// Password field label, e.g. "Password" or "New password".
    password_label: String,
    /// Confirm-password field label.
    password2_label: String,
    /// Optional resend-code callback (password reset only). When `Some`, a
    /// "Resend code" link is rendered below the submit button.
    #[props(optional)]
    on_resend: Option<EventHandler<()>>,
) -> Element {
    rsx! {
        if cfg!(debug_assertions) {
            if let Some(code) = dev_code {
                div { class: "dev-hint",
                    "Dev mode — your code is: "
                    strong { "{code}" }
                }
            }
        }
        div { class: "form-group",
            label { "Verification code" }
            label { class: "otp-boxes",
                {
                    let len = otp.read().len();
                    let active_idx: usize = len.min(5);
                    let is_full = len >= 6;
                    rsx! {
                        for i in 0..6usize {
                            div {
                                key: "{i}",
                                class: if i == active_idx && !is_full { "otp-cell otp-cell-active" } else { "otp-cell" },
                                { otp.read().chars().nth(i).map(|c| c.to_string()).unwrap_or_default() }
                            }
                        }
                    }
                }
                input {
                    id: "otp-hidden",
                    class: "otp-hidden-input",
                    r#type: "text",
                    inputmode: "numeric",
                    autocomplete: "one-time-code",
                    value: "{otp}",
                    oninput: move |e| {
                        let filtered: String = e
                            .value()
                            .chars()
                            .filter(|c| c.is_ascii_digit())
                            .take(6)
                            .collect();
                        otp.set(filtered);
                    },
                }
            }
        }
        div { class: "form-group",
            label { r#for: "pwd", "{password_label}" }
            input {
                id: "pwd",
                r#type: "password",
                placeholder: "At least 8 characters",
                autocomplete: "new-password",
                value: "{password}",
                oninput: move |e| password.set(e.value()),
            }
            span { class: "form-hint", "Make it hard to guess." }
        }
        div { class: "form-group",
            label { r#for: "pwd2", "{password2_label}" }
            input {
                id: "pwd2",
                r#type: "password",
                placeholder: "Same password again",
                autocomplete: "new-password",
                value: "{password2}",
                oninput: move |e| password2.set(e.value()),
            }
        }
        button {
            class: "btn btn-primary btn-full",
            disabled: loading,
            onclick: on_submit,
            if loading { "{loading_label}" } else { "{submit_label}" }
        }
        if let Some(resend) = on_resend {
            div { class: "auth-footer",
                button {
                    class: "btn-link",
                    onclick: move |_| resend.call(()),
                    "Resend code"
                }
            }
        }
    }
}
