use dioxus::prelude::*;

use crate::web::{
    app::Route,
    state::{AuthSignal, clear_auth},
};

/// Returns a signal that is `false` on both SSR and the initial WASM render,
/// then flips to `true` after the component mounts on the client.
///
/// Use this wherever you need to suppress SSR/WASM hydration mismatches caused
/// by client-only state (e.g. auth loaded from localStorage).
pub fn use_client_only() -> Signal<bool> {
    let mut ready = use_signal(|| false);
    use_effect(move || {
        ready.set(true);
    });
    ready
}

/// True when a server-function error indicates an expired or invalid token.
pub fn is_auth_error(e: &ServerFnError) -> bool {
    e.to_string().contains("invalid token")
}

/// Watches a reactive predicate and redirects to the login page when it
/// returns `true` (i.e. when a server function returns an auth error).
///
/// # Example
/// ```rust,ignore
/// use_auth_guard(move || matches!(*feed.read(), Some(Err(ref e)) if is_auth_error(e)));
/// ```
pub fn use_auth_guard(is_expired: impl Fn() -> bool + 'static) {
    let mut auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    use_effect(move || {
        if is_expired() {
            clear_auth();
            auth.set(None);
            nav.push(Route::Login {});
        }
    });
}
