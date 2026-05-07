use dioxus::prelude::*;

/// Inline error message styled with the `error-banner` CSS class.
#[component]
pub fn ErrorBanner(message: String) -> Element {
    rsx! {
        div { class: "error-banner", "{message}" }
    }
}
