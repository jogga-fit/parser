use dioxus::prelude::*;

use crate::web::image::CropSelection;

#[derive(Clone, PartialEq)]
pub struct CropModalState {
    pub object_url: String,
    pub natural_width: u32,
    pub natural_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub title: String,
    pub circle_mask: bool,
}

#[component]
pub fn CropModal(
    state: CropModalState,
    on_cancel: EventHandler<()>,
    on_apply: EventHandler<CropSelection>,
) -> Element {
    let extra_height = 112.0_f64;
    let max_modal_width = window_inner_width()
        .map(|w| (w - 32.0).clamp(280.0, 540.0))
        .unwrap_or(540.0);
    let box_width = (max_modal_width * 0.88).round();
    let aspect_ratio = state.output_width as f64 / state.output_height as f64;
    let box_height = (box_width / aspect_ratio).round();
    let stage_height = box_height + extra_height;
    let crop_x = ((max_modal_width - box_width) / 2.0).round();
    let crop_y = ((stage_height - extra_height - box_height) / 2.0).round();

    let initial = initial_transform(
        state.natural_width as f64,
        state.natural_height as f64,
        crop_x,
        crop_y,
        box_width,
        box_height,
    );

    let mut scale = use_signal(|| initial.scale);
    let min_scale = use_signal(|| initial.scale);
    let mut image_x = use_signal(|| initial.x);
    let mut image_y = use_signal(|| initial.y);
    let mut dragging = use_signal(|| false);
    let mut last_pointer = use_signal(|| (0.0_f64, 0.0_f64));

    let clamp_image = move |mut x: f64, mut y: f64, current_scale: f64| {
        let image_width = state.natural_width as f64 * current_scale;
        let image_height = state.natural_height as f64 * current_scale;
        if x > crop_x {
            x = crop_x;
        }
        if y > crop_y {
            y = crop_y;
        }
        if x + image_width < crop_x + box_width {
            x = crop_x + box_width - image_width;
        }
        if y + image_height < crop_y + box_height {
            y = crop_y + box_height - image_height;
        }
        (x, y)
    };

    let mut begin_drag = move |client_x: f64, client_y: f64| {
        dragging.set(true);
        last_pointer.set((client_x, client_y));
    };

    let mut update_drag = move |client_x: f64, client_y: f64| {
        if !*dragging.read() {
            return;
        }
        let (last_x, last_y) = *last_pointer.read();
        let next_x = *image_x.read() + client_x - last_x;
        let next_y = *image_y.read() + client_y - last_y;
        let (clamped_x, clamped_y) = clamp_image(next_x, next_y, *scale.read());
        image_x.set(clamped_x);
        image_y.set(clamped_y);
        last_pointer.set((client_x, client_y));
    };

    let mut end_drag = move || dragging.set(false);

    let on_zoom = move |evt: Event<FormData>| {
        let slider_value = evt.value().parse::<f64>().unwrap_or(1.0).clamp(1.0, 5.0);
        let current_scale = *scale.read();
        let next_scale = *min_scale.read() * slider_value;
        let ratio = next_scale / current_scale;
        let center_x = crop_x + box_width / 2.0;
        let center_y = crop_y + box_height / 2.0;
        let next_x = center_x - ratio * (center_x - *image_x.read());
        let next_y = center_y - ratio * (center_y - *image_y.read());
        let (clamped_x, clamped_y) = clamp_image(next_x, next_y, next_scale);
        scale.set(next_scale);
        image_x.set(clamped_x);
        image_y.set(clamped_y);
    };

    let on_apply_click = move |_: Event<MouseData>| {
        let current_scale = *scale.read();
        let src_x = (crop_x - *image_x.read()) / current_scale;
        let src_y = (crop_y - *image_y.read()) / current_scale;
        let src_w = box_width / current_scale;
        let src_h = box_height / current_scale;
        on_apply.call(CropSelection {
            src_x,
            src_y,
            src_w,
            src_h,
            out_w: state.output_width,
            out_h: state.output_height,
        });
    };

    let stage_cursor = if *dragging.read() { "grabbing" } else { "grab" };
    let image_style = format!(
        "left:{:.2}px;top:{:.2}px;width:{:.2}px;height:{:.2}px;",
        *image_x.read(),
        *image_y.read(),
        state.natural_width as f64 * *scale.read(),
        state.natural_height as f64 * *scale.read()
    );
    let stage_style = format!("height:{stage_height:.0}px;");
    let crop_style = format!(
        "position:absolute;left:{crop_x:.0}px;top:{crop_y:.0}px;width:{box_width:.0}px;height:{box_height:.0}px;border:2px solid rgba(255,255,255,0.75);pointer-events:none;{}",
        if state.circle_mask {
            "border-radius:50%;"
        } else {
            ""
        }
    );
    let top_mask = format!(
        "position:absolute;left:0;top:0;width:100%;height:{crop_y:.0}px;background:rgba(0,0,0,0.55);pointer-events:none;"
    );
    let bottom_mask = format!(
        "position:absolute;left:0;top:{:.0}px;width:100%;height:{:.0}px;background:rgba(0,0,0,0.55);pointer-events:none;",
        crop_y + box_height,
        stage_height - (crop_y + box_height)
    );
    let left_mask = format!(
        "position:absolute;left:0;top:{crop_y:.0}px;width:{crop_x:.0}px;height:{box_height:.0}px;background:rgba(0,0,0,0.55);pointer-events:none;"
    );
    let right_mask = format!(
        "position:absolute;left:{:.0}px;top:{crop_y:.0}px;width:{:.0}px;height:{box_height:.0}px;background:rgba(0,0,0,0.55);pointer-events:none;",
        crop_x + box_width,
        max_modal_width - (crop_x + box_width)
    );

    rsx! {
        div {
            class: "cm-overlay",
            onclick: move |_| on_cancel.call(()),

            div {
                class: "cm-modal",
                onclick: move |e| e.stop_propagation(),

                div { class: "cm-header",
                    button {
                        class: "cm-btn cm-btn-cancel",
                        onclick: move |_| on_cancel.call(()),
                        "Cancel"
                    }
                    span { class: "cm-title", "{state.title}" }
                    div { style: "width:70px" }
                }

                div {
                    class: "cm-stage",
                    style: "{stage_style}cursor:{stage_cursor};",
                    onmousedown: move |e| begin_drag(e.data().client_coordinates().x, e.data().client_coordinates().y),
                    onmousemove: move |e| update_drag(e.data().client_coordinates().x, e.data().client_coordinates().y),
                    onmouseup: move |_| end_drag(),
                    onmouseleave: move |_| end_drag(),
                    img {
                        class: "cm-img",
                        src: "{state.object_url}",
                        alt: "",
                        draggable: "false",
                        style: "{image_style}",
                    }
                    div { style: "{top_mask}" }
                    div { style: "{bottom_mask}" }
                    div { style: "{left_mask}" }
                    div { style: "{right_mask}" }
                    div { style: "{crop_style}" }
                }

                div { class: "cm-zoom-bar",
                    span { class: "cm-zoom-icon", aria_hidden: "true", "−" }
                    input {
                        class: "cm-zoom-slider",
                        r#type: "range",
                        min: "1",
                        max: "5",
                        step: "0.01",
                        value: format!("{:.2}", *scale.read() / *min_scale.read()),
                        oninput: on_zoom,
                    }
                    span { class: "cm-zoom-icon", aria_hidden: "true", "+" }
                }

                div { class: "cm-footer",
                    button {
                        class: "cm-btn cm-btn-apply",
                        onclick: on_apply_click,
                        "Apply"
                    }
                }
            }
        }
    }
}

fn initial_transform(
    natural_width: f64,
    natural_height: f64,
    crop_x: f64,
    crop_y: f64,
    crop_width: f64,
    crop_height: f64,
) -> InitialTransform {
    let scale = (crop_width / natural_width).max(crop_height / natural_height);
    let x = crop_x - (natural_width * scale - crop_width) / 2.0;
    let y = crop_y - (natural_height * scale - crop_height) / 2.0;
    InitialTransform { scale, x, y }
}

fn window_inner_width() -> Option<f64> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|v| v.as_f64())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

struct InitialTransform {
    scale: f64,
    x: f64,
    y: f64,
}
