use dioxus::prelude::*;
use dioxus_leaflet::{LatLng, Map, MapOptions, MapPosition, PathOptions, Polyline, TileLayer};

use crate::web::server_fns::get_exercise_route_fn;

/// Build SVG polyline points from route coords. Returns `None` if there aren't
/// enough elevation samples or the route is completely flat (< 1 m range).
/// Downsamples to at most `max_pts` points to keep the SVG small.
fn elevation_polyline(coords: &[(f64, f64, Option<f64>)], max_pts: usize) -> Option<String> {
    let elevations: Vec<f64> = coords.iter().filter_map(|(_, _, e)| *e).collect();
    if elevations.len() < 2 {
        return None;
    }
    let step = elevations.len().div_ceil(max_pts);
    let sampled: Vec<f64> = elevations.iter().step_by(step).copied().collect();
    let min_e = sampled.iter().copied().fold(f64::INFINITY, f64::min);
    let max_e = sampled.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = max_e - min_e;
    if range < 1.0 {
        return None;
    }
    let n = sampled.len();
    let pts = sampled
        .iter()
        .enumerate()
        .map(|(i, &e)| {
            let x = (i as f64) / (n - 1) as f64 * 200.0;
            // y: 0 = top; leave 2px padding top and bottom within the 40px viewBox.
            let y = 38.0 - ((e - min_e) / range) * 36.0;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    Some(pts)
}

/// Renders a Leaflet map with the exercise route polyline.
///
/// `route_url` is the full URL of the GeoJSON route endpoint
/// (e.g. `https://domain/api/exercises/{uuid}/route`).
/// The component extracts the UUID from the URL and calls the
/// `get_exercise_route_fn` server function to fetch coordinates.
/// Nothing is rendered if there is no route or the fetch fails.
#[component]
pub fn RouteMap(
    route_url: String,
    token: Option<String>,
    #[props(default = true)] interactive: bool,
) -> Element {
    let url = route_url.clone();
    let tok = token.clone();
    let route_resource = use_resource(move || {
        let u = url.clone();
        let t = tok.clone();
        async move { get_exercise_route_fn(u, t).await }
    });

    match route_resource() {
        None => rsx! { div { class: "route-map-placeholder" } },
        Some(Err(_)) | Some(Ok(None)) => rsx! {},
        Some(Ok(Some(coords))) if coords.is_empty() => rsx! {},
        Some(Ok(Some(coords))) => {
            let latlngs: Vec<LatLng> = coords
                .iter()
                .map(|&(lat, lon, _)| LatLng::new(lat, lon))
                .collect();
            rsx! { RouteMapFromCoords { coords: latlngs, interactive } }
        }
    }
}

/// Renders a Leaflet map directly from a pre-computed `Vec<LatLng>`.
/// Use this when you already have coordinates (e.g. from a parsed activity)
/// and don't need to fetch them from the server.
/// `map_height` overrides the default CSS height of the map (default `"200px"`).
#[component]
pub fn RouteMapFromCoords(
    coords: Vec<LatLng>,
    #[props(default = String::from("200px"))] map_height: String,
    #[props(default = true)] interactive: bool,
) -> Element {
    if coords.is_empty() {
        return rsx! {};
    }
    let n = coords.len() as f64;
    let (lat_sum, lng_sum) = coords
        .iter()
        .fold((0.0f64, 0.0f64), |(la, lo), c| (la + c.lat, lo + c.lng));
    let center = MapPosition::new(lat_sum / n, lng_sum / n, 13.0);
    rsx! { RouteMapInner { coords, center, height: map_height, interactive } }
}

/// Renders a Leaflet map + elevation sparkline from a route URL.
/// Fetches the coordinates once and shares them between both sub-components.
/// `map_height` overrides the default CSS height of the map (default `"200px"`).
#[component]
pub fn RouteSection(
    route_url: String,
    token: Option<String>,
    #[props(default = String::from("200px"))] map_height: String,
    #[props(default = true)] interactive: bool,
) -> Element {
    let url = route_url.clone();
    let tok = token.clone();
    let route_resource = use_resource(move || {
        let u = url.clone();
        let t = tok.clone();
        async move { get_exercise_route_fn(u, t).await }
    });

    match route_resource() {
        None => rsx! { div { class: "route-map-placeholder" } },
        Some(Err(_)) | Some(Ok(None)) => rsx! {},
        Some(Ok(Some(coords))) if coords.is_empty() => rsx! {},
        Some(Ok(Some(coords))) => {
            let latlngs: Vec<LatLng> = coords
                .iter()
                .map(|&(lat, lon, _)| LatLng::new(lat, lon))
                .collect();
            rsx! {
                RouteMapFromCoords { coords: latlngs, map_height: map_height.clone(), interactive }
                ElevationSparkline { coords: coords.clone() }
            }
        }
    }
}

/// Inline SVG elevation profile. Renders nothing when elevation data is absent
/// or the route is flat.
#[component]
fn ElevationSparkline(coords: Vec<(f64, f64, Option<f64>)>) -> Element {
    let Some(pts) = elevation_polyline(&coords, 200) else {
        return rsx! {};
    };
    // Build the closing path for the filled area: line + floor + back to start.
    let first_x = pts
        .split_once(',')
        .map(|(x, _)| x)
        .unwrap_or("0")
        .to_string();
    let last_x = pts
        .rsplit(' ')
        .next()
        .and_then(|p| p.split_once(','))
        .map(|(x, _)| x)
        .unwrap_or("200")
        .to_string();
    let fill_pts = format!("{pts} {last_x},40 {first_x},40");
    rsx! {
        svg {
            class: "elevation-sparkline",
            view_box: "0 0 200 40",
            preserve_aspect_ratio: "none",
            xmlns: "http://www.w3.org/2000/svg",
            polygon {
                points: "{fill_pts}",
                class: "sparkline-fill",
            }
            polyline {
                points: "{pts}",
                class: "sparkline-line",
                fill: "none",
            }
        }
    }
}

#[component]
fn RouteMapInner(
    coords: Vec<LatLng>,
    center: MapPosition,
    height: String,
    interactive: bool,
) -> Element {
    let coords_signal = use_signal(|| vec![coords.clone()]);
    // Orange route line matching the app's primary colour, weight 4 for visibility.
    let opts_signal = use_signal(|| PathOptions {
        fill: false,
        color: dioxus_leaflet::Color::from_rgb8(0xc1, 0x44, 0x0e),
        fill_color: dioxus_leaflet::Color::from_rgb8(0xc1, 0x44, 0x0e),
        weight: 4,
        opacity: 0.9,
        ..PathOptions::default()
    });
    let map_opts = if interactive {
        MapOptions::default()
    } else {
        MapOptions::minimal()
    };
    let tile_layer = TileLayer {
        url: "https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png".to_string(),
        attribution: "&copy; <a href=\"https://www.openstreetmap.org/copyright\">OpenStreetMap</a> contributors &copy; <a href=\"https://carto.com/attributions\">CARTO</a>".to_string(),
        max_zoom: 19,
        subdomains: vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()],
    };

    rsx! {
        div { class: "route-map",
            Map {
                initial_position: center,
                height: "{height}",
                // Dark Matter tiles — dark grayish base that blends with the app's dark theme.
                options: map_opts.with_tile_layer(tile_layer),
                Polyline {
                    coordinates: coords_signal.boxed(),
                    options: opts_signal.boxed(),
                }
            }
        }
    }
}
