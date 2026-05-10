use web_sys::window;

use super::AuthUser;

const STORAGE_KEY: &str = "fedisport_auth";
const THEME_KEY: &str = "fedisport_theme";

/// True when running inside a Tauri webview.
/// Checks for `window.__TAURI__` which Tauri injects into pages that have
/// been granted capabilities (capabilities/default.json → remote.urls).
pub fn is_tauri() -> bool {
    let win = match window() {
        Some(w) => w,
        None => return false,
    };
    if let Ok(Some(storage)) = win.session_storage() {
        if storage.get_item("__tauri__").ok().flatten().as_deref() == Some("1") {
            return true;
        }
        let in_tauri = win.location().search().ok()
            .map(|s| s.contains("__tauri=1"))
            .unwrap_or(false);
        if in_tauri {
            let _ = storage.set_item("__tauri__", "1");
        }
        in_tauri
    } else {
        win.location().search().ok()
            .map(|s| s.contains("__tauri=1"))
            .unwrap_or(false)
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

/// Resolve "system" to "dark" (server-side fallback); "light"/"dark" pass through.
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
