#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use base64::Engine as _;
    use futures_channel::oneshot;
    use js_sys::Uint8Array;
    use serde::Deserialize;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        Blob, CanvasRenderingContext2d, Document, File, HtmlCanvasElement, HtmlInputElement,
        ImageBitmap, Url, window,
    };

    const JPEG_MIME: &str = "image/jpeg";

    #[derive(Clone)]
    pub struct CompressedImage {
        pub name: String,
        pub b64: String,
        pub preview_url: String,
        pub content_type: String,
    }

    #[derive(Clone)]
    pub struct SelectedImage {
        pub object_url: String,
        pub natural_width: u32,
        pub natural_height: u32,
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct CropSelection {
        pub src_x: f64,
        pub src_y: f64,
        pub src_w: f64,
        pub src_h: f64,
        pub out_w: u32,
        pub out_h: u32,
    }

    fn document() -> Result<Document, String> {
        window()
            .and_then(|w| w.document())
            .ok_or_else(|| "document unavailable".to_string())
    }

    fn file_input(input_id: &str) -> Result<HtmlInputElement, String> {
        document()?
            .get_element_by_id(input_id)
            .ok_or_else(|| format!("missing input: {input_id}"))?
            .dyn_into::<HtmlInputElement>()
            .map_err(|_| format!("input is not a file input: {input_id}"))
    }

    fn clear_input_value(input: &HtmlInputElement) {
        input.set_value("");
    }

    fn canvas_2d(
        width: u32,
        height: u32,
    ) -> Result<(HtmlCanvasElement, CanvasRenderingContext2d), String> {
        let canvas = document()?
            .create_element("canvas")
            .map_err(js_err)?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "failed to create canvas".to_string())?;
        canvas.set_width(width);
        canvas.set_height(height);
        let ctx = canvas
            .get_context("2d")
            .map_err(js_err)?
            .ok_or_else(|| "2d canvas context unavailable".to_string())?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| "failed to create 2d canvas context".to_string())?;
        Ok((canvas, ctx))
    }

    async fn load_bitmap(file: &File) -> Result<ImageBitmap, String> {
        let promise = window()
            .ok_or_else(|| "window unavailable".to_string())?
            .create_image_bitmap_with_blob(file)
            .map_err(js_err)?;
        JsFuture::from(promise)
            .await
            .map_err(js_err)?
            .dyn_into::<ImageBitmap>()
            .map_err(|_| "failed to decode image".to_string())
    }

    async fn canvas_blob(canvas: &HtmlCanvasElement, quality: f64) -> Result<Blob, String> {
        let (tx, rx) = oneshot::channel();
        let sender = Rc::new(RefCell::new(Some(tx)));
        let cb_sender = Rc::clone(&sender);
        let callback = Closure::once(move |blob: Option<Blob>| {
            if let Some(tx) = cb_sender.borrow_mut().take() {
                let _ = tx.send(blob);
            }
        });
        canvas
            .to_blob_with_type_and_encoder_options(
                callback.as_ref().unchecked_ref(),
                JPEG_MIME,
                &JsValue::from_f64(quality),
            )
            .map_err(js_err)?;
        callback.forget();
        rx.await
            .map_err(|_| "image encode cancelled".to_string())?
            .ok_or_else(|| "canvas toBlob failed".to_string())
    }

    async fn blob_to_b64(blob: &Blob) -> Result<String, String> {
        let promise = blob.array_buffer();
        let buf = JsFuture::from(promise).await.map_err(js_err)?;
        let bytes = Uint8Array::new(&buf).to_vec();
        Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    async fn encode_canvas(
        canvas: &HtmlCanvasElement,
        quality: f64,
    ) -> Result<CompressedImage, String> {
        let blob = canvas_blob(canvas, quality).await?;
        let b64 = blob_to_b64(&blob).await?;
        Ok(CompressedImage {
            name: String::new(),
            preview_url: format!("data:{JPEG_MIME};base64,{b64}"),
            b64,
            content_type: JPEG_MIME.to_string(),
        })
    }

    async fn compress_file_inner(
        file: &File,
        max_px: u32,
        quality: f64,
    ) -> Result<CompressedImage, String> {
        let bitmap = load_bitmap(file).await?;
        let mut width = bitmap.width();
        let mut height = bitmap.height();

        if width > max_px || height > max_px {
            if width >= height {
                height = ((height as f64) * (max_px as f64) / (width as f64)).round() as u32;
                width = max_px;
            } else {
                width = ((width as f64) * (max_px as f64) / (height as f64)).round() as u32;
                height = max_px;
            }
        }

        let (canvas, ctx) = canvas_2d(width, height)?;
        ctx.draw_image_with_image_bitmap_and_dw_and_dh(
            &bitmap,
            0.0,
            0.0,
            width as f64,
            height as f64,
        )
        .map_err(js_err)?;
        encode_canvas(&canvas, quality).await
    }

    pub async fn compress_post_images_from_input(
        input_id: &str,
        max_new: usize,
    ) -> Result<Vec<CompressedImage>, String> {
        let input = file_input(input_id)?;
        let files = input
            .files()
            .ok_or_else(|| "file input has no files".to_string())?;
        let count = files.length().min(max_new as u32);
        let mut out = Vec::with_capacity(count as usize);

        for idx in 0..count {
            let file = files
                .get(idx)
                .ok_or_else(|| "missing selected file".to_string())?;
            if file.size() > 5.0 * 1024.0 * 1024.0 {
                clear_input_value(&input);
                return Err(format!("File too large (max 5 MB): {}", file.name()));
            }
            let compressed = compress_file_inner(&file, 1200, 0.85).await?;
            out.push(CompressedImage {
                name: file.name(),
                preview_url: compressed.preview_url,
                b64: compressed.b64,
                content_type: compressed.content_type,
            });
        }

        clear_input_value(&input);
        Ok(out)
    }

    pub fn clear_file_input(input_id: &str) -> Result<(), String> {
        clear_input_value(&file_input(input_id)?);
        Ok(())
    }

    pub async fn compress_avatar_from_input(
        input_id: &str,
        crop: CropSelection,
    ) -> Result<CompressedImage, String> {
        let input = file_input(input_id)?;
        let files = input
            .files()
            .ok_or_else(|| "file input has no files".to_string())?;
        let file = files.get(0).ok_or_else(|| "no file selected".to_string())?;

        let bitmap = load_bitmap(&file).await?;
        let (canvas, ctx) = canvas_2d(crop.out_w, crop.out_h)?;
        ctx.draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &bitmap,
            crop.src_x,
            crop.src_y,
            crop.src_w,
            crop.src_h,
            0.0,
            0.0,
            crop.out_w as f64,
            crop.out_h as f64,
        )
        .map_err(js_err)?;
        let result = encode_canvas(&canvas, 0.85).await?;
        clear_input_value(&input);
        Ok(result)
    }

    pub async fn prepare_selected_image_from_input(
        input_id: &str,
    ) -> Result<SelectedImage, String> {
        let input = file_input(input_id)?;
        let files = input
            .files()
            .ok_or_else(|| "file input has no files".to_string())?;
        let file = files.get(0).ok_or_else(|| "no file selected".to_string())?;
        if file.size() > 50.0 * 1024.0 * 1024.0 {
            clear_input_value(&input);
            return Err("Source file too large (max 50 MB)".to_string());
        }
        let bitmap = load_bitmap(&file).await?;
        let object_url = Url::create_object_url_with_blob(&file).map_err(js_err)?;
        Ok(SelectedImage {
            object_url,
            natural_width: bitmap.width(),
            natural_height: bitmap.height(),
        })
    }

    pub fn revoke_object_url(url: &str) {
        let _ = Url::revoke_object_url(url);
    }

    fn js_err(err: JsValue) -> String {
        err.as_string().unwrap_or_else(|| format!("{err:?}"))
    }
}

#[cfg(target_arch = "wasm32")]
pub use imp::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use serde::Deserialize;

    #[derive(Clone)]
    pub struct CompressedImage {
        pub name: String,
        pub b64: String,
        pub preview_url: String,
        pub content_type: String,
    }

    #[derive(Clone)]
    pub struct SelectedImage {
        pub object_url: String,
        pub natural_width: u32,
        pub natural_height: u32,
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct CropSelection {
        pub src_x: f64,
        pub src_y: f64,
        pub src_w: f64,
        pub src_h: f64,
        pub out_w: u32,
        pub out_h: u32,
    }

    pub async fn compress_post_images_from_input(
        _input_id: &str,
        _max_new: usize,
    ) -> Result<Vec<CompressedImage>, String> {
        Err("image compression is only available in the browser".to_string())
    }

    pub fn clear_file_input(_input_id: &str) -> Result<(), String> {
        Ok(())
    }

    pub async fn compress_avatar_from_input(
        _input_id: &str,
        _crop: CropSelection,
    ) -> Result<CompressedImage, String> {
        Err("image compression is only available in the browser".to_string())
    }

    pub async fn prepare_selected_image_from_input(
        _input_id: &str,
    ) -> Result<SelectedImage, String> {
        Err("image selection is only available in the browser".to_string())
    }

    pub fn revoke_object_url(_url: &str) {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use imp::*;
