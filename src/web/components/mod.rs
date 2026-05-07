use dioxus::prelude::*;

pub mod avatar;
pub mod crop_modal;
pub mod error_banner;
pub mod media_carousel;
pub mod route_map;

pub use avatar::Avatar;
pub use crop_modal::{CropModal, CropModalState};
pub use error_banner::ErrorBanner;
pub use media_carousel::{CarouselOverlay, MediaCollage};
pub use route_map::{RouteMap, RouteMapFromCoords, RouteSection};

/// A labelled toggle switch row used throughout the Settings page.
#[component]
pub fn SettingToggle(
    label: String,
    description: String,
    checked: bool,
    disabled: bool,
    onchange: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        div { class: "toggle-row",
            div { class: "toggle-info",
                span { class: "toggle-label", "{label}" }
                span { class: "toggle-desc", "{description}" }
            }
            label { class: "toggle-switch",
                input {
                    r#type: "checkbox",
                    checked,
                    disabled,
                    onchange,
                }
                span { class: "toggle-thumb" }
            }
        }
    }
}
