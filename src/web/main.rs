fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    crate::web::server::run();

    #[cfg(target_arch = "wasm32")]
    dioxus::launch(crate::web::app::App);
}
