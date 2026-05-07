use super::AuthUser;

pub fn load_auth() -> Option<AuthUser> {
    None
}

pub fn save_auth(_user: &AuthUser) {}

pub fn clear_auth() {}

pub fn load_theme() -> String {
    "dark".to_string()
}

pub fn save_theme(_theme: &str) {}

pub fn resolve_theme(pref: &str) -> String {
    if pref == "system" {
        "dark".to_string()
    } else {
        pref.to_string()
    }
}
