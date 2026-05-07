use web_sys::window;

use super::AuthUser;

const STORAGE_KEY: &str = "fedisport_auth";
const THEME_KEY: &str = "fedisport_theme";
const INSTANCE_KEY: &str = "jogga_instance";
pub const DEFAULT_INSTANCE: &str = "https://app.jogga.fit";

pub fn is_tauri() -> bool {
    window()
        .and_then(|w| w.location().hostname().ok())
        .map(|h| h == "tauri.localhost")
        .unwrap_or(false)
}

pub fn load_instance_url() -> String {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(INSTANCE_KEY).ok().flatten())
        .unwrap_or_else(|| DEFAULT_INSTANCE.to_string())
}

pub fn save_instance_url(url: &str) {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item(INSTANCE_KEY, url);
    }
}

pub fn load_auth() -> Option<AuthUser> {
    let storage = window()?.local_storage().ok()??;
    let raw = storage.get_item(STORAGE_KEY).ok()??;
    serde_json::from_str(&raw).ok()
}

pub fn save_auth(user: &AuthUser) {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        if let Ok(json) = serde_json::to_string(user) {
            let _ = storage.set_item(STORAGE_KEY, &json);
        }
    }
}

pub fn clear_auth() {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.remove_item(STORAGE_KEY);
    }
}

pub fn load_theme() -> String {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(THEME_KEY).ok().flatten())
        .filter(|v| v == "light" || v == "dark" || v == "system")
        .unwrap_or_else(|| "system".to_string())
}

/// Resolve "system" to "dark" (default server-side fallback); "light"/"dark" pass through.
/// Actual system resolution happens in JS via prefers-color-scheme in app.rs.
pub fn resolve_theme(pref: &str) -> String {
    if pref == "system" {
        "dark".to_string()
    } else {
        pref.to_string()
    }
}

pub fn save_theme(theme: &str) {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok()).flatten() {
        let _ = storage.set_item(THEME_KEY, theme);
    }
}
