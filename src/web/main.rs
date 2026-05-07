fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    crate::web::server::run();

    #[cfg(target_arch = "wasm32")]
    {
        // Always apply the stored instance URL so the browser dev build and the
        // Tauri app can both target a different backend via Settings → Instance.
        let url = crate::web::state::load_instance_url();
        dioxus_fullstack::set_server_url(url.leak());
        dioxus::launch(crate::web::app::App);
    }
}
