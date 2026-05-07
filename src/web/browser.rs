#[cfg(target_arch = "wasm32")]
mod imp {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::window;

    pub async fn copy_to_clipboard(text: &str) -> Result<(), String> {
        let navigator = window()
            .ok_or_else(|| "window unavailable".to_string())?
            .navigator();
        let clipboard = navigator.clipboard();
        let promise = clipboard.write_text(text);
        JsFuture::from(promise).await.map_err(js_err)?;
        Ok(())
    }

    pub fn mark_document_hydrated() -> Result<(), String> {
        let document = window()
            .and_then(|w| w.document())
            .ok_or_else(|| "document unavailable".to_string())?;
        let body = document
            .body()
            .ok_or_else(|| "document body unavailable".to_string())?;
        body.set_attribute("data-hydrated", "true")
            .map_err(js_err)?;
        Ok(())
    }

    fn js_err(err: wasm_bindgen::JsValue) -> String {
        err.as_string().unwrap_or_else(|| format!("{err:?}"))
    }
}

#[cfg(target_arch = "wasm32")]
pub use imp::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub async fn copy_to_clipboard(_text: &str) -> Result<(), String> {
        Err("clipboard unavailable outside the browser".to_string())
    }

    pub fn mark_document_hydrated() -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use imp::*;
