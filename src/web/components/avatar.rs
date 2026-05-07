use dioxus::prelude::*;

/// Round avatar: shows photo if available, falls back to first character of `name`.
///
/// Size classes: pass `"avatar-sm"` or `"avatar-lg"` via `size`. Defaults to medium.
#[component]
pub fn Avatar(url: Option<String>, name: String, #[props(default)] size: String) -> Element {
    let extra = if size.is_empty() {
        String::new()
    } else {
        format!(" {size}")
    };
    let initial = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    rsx! {
        if let Some(url) = url {
            img {
                class: "avatar{extra} avatar-img",
                src: "{url}",
                alt: "{name}",
            }
        } else {
            div { class: "avatar{extra}", "{initial}" }
        }
    }
}
