use dioxus::prelude::*;

use super::route_map::RouteSection;

/// Unified media display component.
///
/// **Feed mode** (`interactive = false`, default) — scan:
///   All media shown as a clickable collage grid. Each cell click fires
///   `on_open_overlay(slide_index)`. The caller is responsible for rendering
///   `CarouselOverlay` *outside* any `overflow:hidden` ancestor so that
///   `position:fixed` is not contained by a Chrome compositing layer.
///
/// **Detail mode** (`interactive = true`) — study:
///   Map rendered full-width and interactive immediately. Images shown as a
///   collage strip below; clicking opens an images-only lightbox.
#[component]
pub fn MediaCollage(
    route_url: Option<String>,
    image_urls: Vec<String>,
    token: Option<String>,
    #[props(default = false)] interactive: bool,
    /// CSS height for the map tile. Default "240px".
    #[props(default = String::from("240px"))]
    map_height: String,
    /// Called with the initial slide index when a feed-mode tile is clicked.
    /// When provided, no overlay is rendered internally — the caller renders
    /// `CarouselOverlay` itself (outside any `overflow:hidden` card wrapper).
    /// When omitted, an internal overlay is rendered (for standalone use).
    #[props(default)]
    on_open_overlay: Option<EventHandler<usize>>,
) -> Element {
    let has_map = route_url.is_some();
    let img_count = image_urls.len();

    if !has_map && img_count == 0 {
        return rsx! {};
    }

    if interactive {
        return rsx! {
            DetailLayout {
                route_url,
                image_urls,
                token,
                map_height,
            }
        };
    }

    // Feed mode: if the caller owns the overlay, just emit the grid.
    if on_open_overlay.is_some() {
        return rsx! {
            FeedCollage {
                route_url,
                image_urls,
                token,
                map_height,
                on_open: move |i| { if let Some(h) = on_open_overlay.as_ref() { h.call(i); } },
            }
        };
    }

    // Fallback (no external handler): manage overlay internally.
    let mut carousel_idx: Signal<Option<usize>> = use_signal(|| None);

    rsx! {
        FeedCollage {
            route_url: route_url.clone(),
            image_urls: image_urls.clone(),
            token: token.clone(),
            map_height,
            on_open: move |i| carousel_idx.set(Some(i)),
        }
        if let Some(idx) = *carousel_idx.read() {
            CarouselOverlay {
                route_url: route_url.clone(),
                image_urls: image_urls.clone(),
                token: token.clone(),
                initial_index: idx,
                on_close: move |_| carousel_idx.set(None),
            }
        }
    }
}

#[component]
fn FeedCollage(
    route_url: Option<String>,
    image_urls: Vec<String>,
    token: Option<String>,
    map_height: String,
    /// Called with the slide index when a tile is clicked.
    on_open: EventHandler<usize>,
) -> Element {
    let has_map = route_url.is_some();
    let img_count = image_urls.len();

    // At most 2 image slots in the right column when map is present,
    // 4 slots when images-only.
    let max_img_tiles: usize = if has_map { 2 } else { 4 };
    let show_imgs = img_count.min(max_img_tiles);
    let overflow = img_count.saturating_sub(max_img_tiles);

    let grid_class = if has_map {
        match show_imgs {
            0 => "media-collage media-collage-map-only",
            1 => "media-collage media-collage-map-1",
            _ => "media-collage media-collage-map-2",
        }
    } else {
        match show_imgs {
            1 => "media-collage media-collage-1",
            2 => "media-collage media-collage-2",
            3 => "media-collage media-collage-3",
            _ => "media-collage media-collage-4",
        }
    };

    rsx! {
        div { class: "{grid_class}",
            if let Some(ref url) = route_url {
                div {
                    class: "collage-cell collage-map-tile",
                    onclick: move |_| on_open.call(0),
                    RouteSection {
                        route_url: url.clone(),
                        token: token.clone(),
                        map_height: map_height.clone(),
                        interactive: false,
                    }
                    // Hover-only overlay — signals the tile is tappable without
                    // cluttering the map with buttons in the resting state.
                    div { class: "collage-map-hover-hint" }
                }
            }

            {(0..show_imgs).map(|i| {
                let url = image_urls[i].clone();
                let slide_idx = if has_map { i + 1 } else { i };
                let is_last = i + 1 == show_imgs && overflow > 0;
                rsx! {
                    div {
                        key: "{i}",
                        class: "collage-cell",
                        onclick: move |_| on_open.call(slide_idx),
                        img {
                            class: "collage-img",
                            src: "{url}",
                            alt: "",
                            loading: "lazy",
                        }
                        if is_last {
                            div { class: "collage-overflow-badge", "+{overflow}" }
                        }
                    }
                }
            })}
        }
    }
}

#[component]
fn DetailLayout(
    route_url: Option<String>,
    image_urls: Vec<String>,
    token: Option<String>,
    map_height: String,
) -> Element {
    let img_count = image_urls.len();
    let mut lightbox_idx: Signal<Option<usize>> = use_signal(|| None);

    rsx! {
        div { class: "media-collage-detail",
            // Map hero — always interactive, full-width.
            if let Some(ref url) = route_url {
                RouteSection {
                    route_url: url.clone(),
                    token: token.clone(),
                    map_height: map_height.clone(),
                    interactive: true,
                }
            }

            // Image collage strip — only if images exist.
            if img_count > 0 {
                ImageGrid {
                    urls: image_urls.clone(),
                    on_click: move |i| lightbox_idx.set(Some(i)),
                }
            }

            // Images-only lightbox.
            if let Some(idx) = *lightbox_idx.read() {
                ImageLightbox {
                    urls: image_urls.clone(),
                    initial_index: idx,
                    on_close: move |_| lightbox_idx.set(None),
                }
            }
        }
    }
}

/// Image-only collage grid (used in detail layout + images-only feed cards).
#[component]
fn ImageGrid(urls: Vec<String>, on_click: EventHandler<usize>) -> Element {
    let count = urls.len();
    let show = count.min(4);
    let overflow = count.saturating_sub(4);

    let grid_class = match show {
        1 => "media-collage media-collage-1",
        2 => "media-collage media-collage-2",
        3 => "media-collage media-collage-3",
        _ => "media-collage media-collage-4",
    };

    rsx! {
        div { class: "{grid_class}",
            {(0..show).map(|i| {
                let url = urls[i].clone();
                let is_last = i + 1 == show && overflow > 0;
                rsx! {
                    div {
                        key: "{i}",
                        class: "collage-cell",
                        onclick: move |_| on_click.call(i),
                        img {
                            class: "collage-img",
                            src: "{url}",
                            alt: "",
                            loading: "lazy",
                        }
                        if is_last {
                            div { class: "collage-overflow-badge", "+{overflow}" }
                        }
                    }
                }
            })}
        }
    }
}

/// Full-screen carousel overlay (feed mode). Map slide is interactive.
/// Hidden slides stay mounted via opacity trick so Leaflet doesn't remount.
///
/// Render this OUTSIDE any `overflow:hidden` ancestor (e.g. outside the feed
/// card div) so that Chrome does not contain the `position:fixed` element
/// within the card's compositing layer.
#[component]
pub fn CarouselOverlay(
    route_url: Option<String>,
    image_urls: Vec<String>,
    token: Option<String>,
    initial_index: usize,
    on_close: EventHandler<()>,
) -> Element {
    let has_map = route_url.is_some();
    let slide_count = if has_map { 1 } else { 0 } + image_urls.len();
    let mut current = use_signal(|| initial_index);
    let idx = *current.read();
    let is_map_active = has_map && idx == 0;

    // Pointer-based swipe: record x on pointerdown, resolve on pointerup.
    let mut swipe_start: Signal<Option<f64>> = use_signal(|| None);

    // Class signals whether the active slide is the map so CSS can fade controls.
    let overlay_class = if is_map_active {
        "carousel-overlay carousel-map-active"
    } else {
        "carousel-overlay"
    };

    rsx! {
        div {
            class: "{overlay_class}",
            tabindex: "-1",
            autofocus: true,
            onclick: move |_| on_close.call(()),
            onkeydown: move |e| {
                match e.key() {
                    Key::Escape => on_close.call(()),
                    Key::ArrowLeft => {
                        let i = *current.read();
                        if i > 0 { current.set(i - 1); }
                    }
                    Key::ArrowRight => {
                        let i = *current.read();
                        if i + 1 < slide_count { current.set(i + 1); }
                    }
                    _ => {}
                }
            },
            onpointerdown: move |e| {
                swipe_start.set(Some(e.client_coordinates().x));
            },
            onpointerup: move |e| {
                let start = *swipe_start.read();
                swipe_start.set(None);
                if let Some(sx) = start {
                    let delta = e.client_coordinates().x - sx;
                    let i = *current.read();
                    if delta > 50.0 && i > 0 {
                        current.set(i - 1);
                    } else if delta < -50.0 && i + 1 < slide_count {
                        current.set(i + 1);
                    }
                }
            },

            // Close button — fades on map slide, visible on hover.
            button {
                class: "carousel-close-btn",
                onclick: move |_| on_close.call(()),
                i { class: "ph ph-x" }
            }

            div {
                class: "carousel-overlay-content",
                onclick: move |e| e.stop_propagation(),

                // Map slide (always slide 0 when present).
                // Full-bleed: height 100vh, no border-radius.
                if let Some(ref url) = route_url {
                    div {
                        class: if idx == 0 { "carousel-overlay-slide carousel-overlay-slide-active" }
                               else { "carousel-overlay-slide carousel-overlay-slide-hidden" },
                        RouteSection {
                            route_url: url.clone(),
                            token: token.clone(),
                            map_height: "100vh".to_string(),
                            interactive: true,
                        }
                    }
                }

                // Image slides.
                {image_urls.iter().enumerate().map(|(i, url)| {
                    let slide_idx = if has_map { i + 1 } else { i };
                    let url = url.clone();
                    rsx! {
                        div {
                            key: "{i}",
                            class: if idx == slide_idx { "carousel-overlay-slide carousel-overlay-slide-active" }
                                   else { "carousel-overlay-slide carousel-overlay-slide-hidden" },
                            img {
                                class: "carousel-overlay-img",
                                src: "{url}",
                                alt: "",
                            }
                        }
                    }
                })}

                // Prev / next arrows — overlaid, faded on map slide.
                if slide_count > 1 {
                    if idx > 0 {
                        button {
                            class: "carousel-nav carousel-nav-prev",
                            onclick: move |e| {
                                e.stop_propagation();
                                let i = *current.read();
                                if i > 0 { current.set(i - 1); }
                            },
                            i { class: "ph ph-caret-left" }
                        }
                    }
                    if idx + 1 < slide_count {
                        button {
                            class: "carousel-nav carousel-nav-next",
                            onclick: move |e| {
                                e.stop_propagation();
                                let i = *current.read();
                                if i + 1 < slide_count { current.set(i + 1); }
                            },
                            i { class: "ph ph-caret-right" }
                        }
                    }
                    // Indicator row: dots + counter.
                    div { class: "carousel-indicator",
                        {(0..slide_count).map(|i| rsx! {
                            button {
                                key: "{i}",
                                class: if i == idx { "carousel-dot carousel-dot-active" } else { "carousel-dot" },
                                onclick: move |_| current.set(i),
                                aria_label: "Slide {i + 1}",
                            }
                        })}
                        span { class: "carousel-counter", "{idx + 1} / {slide_count}" }
                    }
                }
            }
        }
    }
}

/// Full-screen images-only lightbox (used in detail layout).
#[component]
fn ImageLightbox(urls: Vec<String>, initial_index: usize, on_close: EventHandler<()>) -> Element {
    let mut current = use_signal(|| initial_index);
    let count = urls.len();
    let idx = *current.read();
    let url = urls.get(idx).cloned().unwrap_or_default();
    let mut swipe_start: Signal<Option<f64>> = use_signal(|| None);

    rsx! {
        div {
            class: "image-lightbox",
            tabindex: "-1",
            autofocus: true,
            onclick: move |_| on_close.call(()),
            onkeydown: move |e| {
                match e.key() {
                    Key::Escape => on_close.call(()),
                    Key::ArrowLeft => { let i = *current.read(); if i > 0 { current.set(i - 1); } }
                    Key::ArrowRight => { let i = *current.read(); if i + 1 < count { current.set(i + 1); } }
                    _ => {}
                }
            },
            onpointerdown: move |e| { swipe_start.set(Some(e.client_coordinates().x)); },
            onpointerup: move |e| {
                let start = *swipe_start.read();
                swipe_start.set(None);
                if let Some(sx) = start {
                    let delta = e.client_coordinates().x - sx;
                    let i = *current.read();
                    if delta > 50.0 && i > 0 { current.set(i - 1); }
                    else if delta < -50.0 && i + 1 < count { current.set(i + 1); }
                }
            },

            button {
                class: "carousel-close-btn",
                onclick: move |_| on_close.call(()),
                i { class: "ph ph-x" }
            }

            div {
                class: "lightbox-content",
                onclick: move |e| e.stop_propagation(),

                img { class: "lightbox-img", src: "{url}", alt: "" }

                if count > 1 {
                    if idx > 0 {
                        button {
                            class: "carousel-nav carousel-nav-prev",
                            onclick: move |_| { let i = *current.read(); if i > 0 { current.set(i - 1); } },
                            i { class: "ph ph-caret-left" }
                        }
                    }
                    if idx + 1 < count {
                        button {
                            class: "carousel-nav carousel-nav-next",
                            onclick: move |_| { let i = *current.read(); if i + 1 < count { current.set(i + 1); } },
                            i { class: "ph ph-caret-right" }
                        }
                    }
                    div { class: "carousel-indicator",
                        {(0..count).map(|i| rsx! {
                            button {
                                key: "{i}",
                                class: if i == idx { "carousel-dot carousel-dot-active" } else { "carousel-dot" },
                                onclick: move |_| current.set(i),
                                aria_label: "Slide {i + 1}",
                            }
                        })}
                        span { class: "carousel-counter", "{idx + 1} / {count}" }
                    }
                }
            }
        }
    }
}
