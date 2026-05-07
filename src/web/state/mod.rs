use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AuthUser {
    pub token: String,
    pub username: String,
    #[serde(default)]
    pub ap_id: String,
}

/// Global auth state. `None` = not logged in.
pub type AuthSignal = Signal<Option<AuthUser>>;

/// Global theme signal ("system" | "dark" | "light"). Provided at the `App` level.
pub type ThemeSignal = Signal<String>;

/// Migration modal — `Some(profile)` while open, `None` while closed.
/// Provided at `App` level; opened by `MigrationRow`, rendered by `AppShell`.
pub type MigrationModalSignal = Signal<Option<crate::web::MeResult>>;

#[cfg(target_arch = "wasm32")]
#[path = "platform_wasm.rs"]
mod platform;

#[cfg(not(target_arch = "wasm32"))]
#[path = "platform_ssr.rs"]
mod platform;

pub use platform::{clear_auth, load_auth, load_theme, resolve_theme, save_auth, save_theme};

#[cfg(target_arch = "wasm32")]
pub use platform::{DEFAULT_INSTANCE, is_tauri, load_instance_url, save_instance_url};
