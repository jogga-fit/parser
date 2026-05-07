use chrono::Timelike as _;
use dioxus::prelude::*;
use parser::ParsedActivity;

use super::feed_card::FeedCard;
use crate::web::{
    UploadExerciseMeta,
    app::AppShell,
    components::{ErrorBanner, RouteMapFromCoords},
    format::{fmt_distance, fmt_duration, fmt_elevation, fmt_pace},
    hooks::{is_auth_error, use_auth_guard},
    image::compress_post_images_from_input,
    server_fns::{get_feed, get_public_feed, upload_exercise_fn},
    sfn_msg,
    state::AuthSignal,
};

/// Stats/map that can be toggled hidden.
const MAP_KEY: &str = "map";

/// Stats that can be toggled hidden. Key matches ParsedActivity field names; label is display text.
const STAT_TOGGLES: &[(&str, &str)] = &[
    ("avg_heart_rate_bpm", "Heart rate"),
    ("max_heart_rate_bpm", "Max HR"),
    ("avg_power_w", "Avg power"),
    ("max_power_w", "Max power"),
    ("normalized_power_w", "NP"),
    ("avg_cadence_rpm", "Cadence"),
];

/// Mirror the server-side title generation so the field is pre-filled client-side.
fn auto_title(pa: &parser::ParsedActivity, activity_type: &str) -> String {
    let hour = pa.started_at.map(|dt| dt.hour()).unwrap_or(12);
    let time_of_day = match hour {
        5..=11 => "Morning",
        12..=13 => "Lunch",
        14..=17 => "Afternoon",
        18..=20 => "Evening",
        _ => "Night",
    };
    let cap_type = {
        let mut c = activity_type.chars();
        c.next()
            .map(|ch| ch.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default()
    };
    format!("{time_of_day} {cap_type}")
}

/// All known activity types (value, display label).
pub const ACTIVITY_TYPES: &[(&str, &str)] = &[
    ("run", "Run"),
    ("trail-run", "Trail Run"),
    ("virtual-run", "Virtual Run"),
    ("ride", "Ride"),
    ("gravel-ride", "Gravel Ride"),
    ("mountain-bike-ride", "Mountain Bike"),
    ("e-bike-ride", "E-Bike Ride"),
    ("e-mountain-bike-ride", "E-Mountain Bike"),
    ("virtual-ride", "Virtual Ride"),
    ("velomobile", "Velomobile"),
    ("handcycle", "Handcycle"),
    ("swim", "Swim"),
    ("walk", "Walk"),
    ("hike", "Hike"),
    ("snowshoe", "Snowshoe"),
    ("alpine-ski", "Alpine Ski"),
    ("backcountry-ski", "Backcountry Ski"),
    ("nordic-ski", "Nordic Ski"),
    ("snowboard", "Snowboard"),
    ("ice-skate", "Ice Skate"),
    ("inline-skate", "Inline Skate"),
    ("skateboard", "Skateboard"),
    ("rowing", "Rowing"),
    ("virtual-row", "Virtual Row"),
    ("kayaking", "Kayaking"),
    ("canoeing", "Canoeing"),
    ("stand-up-paddling", "Stand-Up Paddling"),
    ("surf", "Surf"),
    ("windsurf", "Windsurf"),
    ("kitesurf", "Kitesurf"),
    ("sail", "Sail"),
    ("rock-climbing", "Rock Climbing"),
    ("weight-training", "Weight Training"),
    ("crossfit", "CrossFit"),
    ("hiit", "HIIT"),
    ("elliptical", "Elliptical"),
    ("stair-stepper", "Stair Stepper"),
    ("yoga", "Yoga"),
    ("pilates", "Pilates"),
    ("workout", "Workout"),
    ("golf", "Golf"),
    ("soccer", "Soccer"),
    ("tennis", "Tennis"),
    ("squash", "Squash"),
    ("racquetball", "Racquetball"),
    ("badminton", "Badminton"),
    ("pickleball", "Pickleball"),
    ("table-tennis", "Table Tennis"),
    ("wheelchair", "Wheelchair"),
];

/// Map a sport string from a GPX/FIT file to a known activity type value.
fn sport_to_activity_type(sport: &str) -> Option<&'static str> {
    let s = sport.to_lowercase();
    // Trail run before generic run.
    if s.contains("trail") && s.contains("run") {
        Some("trail-run")
    } else if s.contains("virtual") && s.contains("run") {
        Some("virtual-run")
    } else if s.contains("run") || s == "running" {
        Some("run")
    } else if s.contains("virtual") && (s.contains("ride") || s.contains("cycl")) {
        Some("virtual-ride")
    } else if s.contains("mountain") && s.contains("bik") {
        if s.contains("electric") || s.contains("e-") || s.starts_with("e ") {
            Some("e-mountain-bike-ride")
        } else {
            Some("mountain-bike-ride")
        }
    } else if s.contains("gravel") {
        Some("gravel-ride")
    } else if s.contains("electric") && (s.contains("bik") || s.contains("cycl")) {
        Some("e-bike-ride")
    } else if s.contains("cycl") || s.contains("bik") || s == "ride" {
        Some("ride")
    } else if s.contains("velomobile") {
        Some("velomobile")
    } else if s.contains("handcycle") {
        Some("handcycle")
    } else if s.contains("swim") {
        Some("swim")
    } else if s.contains("hik") {
        Some("hike")
    } else if s.contains("walk") {
        Some("walk")
    } else if s.contains("snowshoe") {
        Some("snowshoe")
    } else if s.contains("alpine")
        || s.contains("downhill")
        || s.contains("ski") && !s.contains("nordic") && !s.contains("cross")
    {
        Some("alpine-ski")
    } else if s.contains("nordic") || s.contains("cross-country") {
        Some("nordic-ski")
    } else if s.contains("backcountry") {
        Some("backcountry-ski")
    } else if s.contains("snowboard") {
        Some("snowboard")
    } else if s.contains("ice") && s.contains("skat") {
        Some("ice-skate")
    } else if s.contains("inline") || s.contains("roller") {
        Some("inline-skate")
    } else if s.contains("skateboard") {
        Some("skateboard")
    } else if s.contains("virtual") && s.contains("row") {
        Some("virtual-row")
    } else if s.contains("row") {
        Some("rowing")
    } else if s.contains("kayak") {
        Some("kayaking")
    } else if s.contains("canoe") {
        Some("canoeing")
    } else if s.contains("paddl") || s.contains("sup") {
        Some("stand-up-paddling")
    } else if s.contains("windsurf") {
        Some("windsurf")
    } else if s.contains("kite") {
        Some("kitesurf")
    } else if s.contains("surf") {
        Some("surf")
    } else if s.contains("sail") {
        Some("sail")
    } else if s.contains("climb") {
        Some("rock-climbing")
    } else if s.contains("weight") || s.contains("strength") || s.contains("lifting") {
        Some("weight-training")
    } else if s.contains("crossfit") {
        Some("crossfit")
    } else if s.contains("hiit") || s.contains("interval") {
        Some("hiit")
    } else if s.contains("elliptical") {
        Some("elliptical")
    } else if s.contains("stair") {
        Some("stair-stepper")
    } else if s.contains("yoga") {
        Some("yoga")
    } else if s.contains("pilates") {
        Some("pilates")
    } else if s.contains("golf") {
        Some("golf")
    } else if s.contains("soccer") || s.contains("football") {
        Some("soccer")
    } else if s.contains("tennis") {
        Some("tennis")
    } else if s.contains("squash") {
        Some("squash")
    } else if s.contains("racquetball") {
        Some("racquetball")
    } else if s.contains("badminton") {
        Some("badminton")
    } else if s.contains("pickleball") {
        Some("pickleball")
    } else if s.contains("table") {
        Some("table-tennis")
    } else if s.contains("wheelchair") {
        Some("wheelchair")
    } else if s.contains("workout") || s.contains("training") {
        Some("workout")
    } else {
        None
    }
}

/// Owned snapshot of parsed stats for use in RSX without holding a signal borrow.
#[derive(Clone)]
struct ParsedStats {
    distance_m: f64,
    duration_s: i32,
    elevation_gain_m: Option<f64>,
    avg_pace_s_per_km: Option<f64>,
    avg_heart_rate_bpm: Option<i32>,
    max_heart_rate_bpm: Option<i32>,
    avg_power_w: Option<f64>,
    max_power_w: Option<f64>,
    normalized_power_w: Option<f64>,
    avg_cadence_rpm: Option<f64>,
    device: Option<String>,
}

fn stat_is_present(pa: &parser::ParsedActivity, key: &str) -> bool {
    match key {
        "avg_heart_rate_bpm" => pa.avg_heart_rate_bpm.is_some(),
        "max_heart_rate_bpm" => pa.max_heart_rate_bpm.is_some(),
        "avg_power_w" => pa.avg_power_w.is_some(),
        "max_power_w" => pa.max_power_w.is_some(),
        "normalized_power_w" => pa.normalized_power_w.is_some(),
        "avg_cadence_rpm" => pa.avg_cadence_rpm.is_some(),
        _ => false,
    }
}

/// A photo compressed in-browser by the WASM client and ready to upload.
#[derive(Clone, PartialEq)]
struct PendingImage {
    name: String,
    compressed_b64: String,
    preview_url: String, // data:image/jpeg;base64,...
}

#[component]
pub fn HomePage() -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth.read().as_ref().map(|u| u.token.clone());
    let is_logged_in = token.is_some();
    let token_feed = token.clone();

    let mut feed = use_resource(move || {
        let t = token_feed.clone();
        async move {
            if let Some(tok) = t {
                get_feed(tok).await
            } else {
                get_public_feed().await
            }
        }
    });

    // Only redirect to login when we expected auth to work and got an auth error.
    // Never redirect logged-out users — get_public_feed() does not return "invalid token".
    use_auth_guard(move || {
        is_logged_in && matches!(*feed.read(), Some(Err(ref e)) if is_auth_error(e))
    });

    let initial = auth
        .read()
        .as_ref()
        .and_then(|u| u.username.chars().next())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());

    rsx! {
        AppShell {
            div { class: "page-content",
                if is_logged_in {
                    ComposeCard {
                        initial: initial.clone(),
                        on_posted: move |_| feed.restart(),
                    }
                }
                div { class: "feed",
                    match &*feed.read() {
                        None => rsx! {
                            div { class: "loading-spinner", "Loading feed…" }
                        },
                        Some(Err(_)) => rsx! {
                            ErrorBanner { message: "Could not load feed. Please try again.".to_string() }
                        },
                        Some(Ok(items)) if is_logged_in => {
                            if items.is_empty() {
                                rsx! {
                                    div { class: "empty-state",
                                        div { class: "empty-icon", i { class: "ph ph-flag-checkered" } }
                                        h3 { "Nothing here yet" }
                                        p { "Post your first activity above, or go to " Link { to: crate::web::app::Route::People {}, "People" } " to follow someone." }
                                    }
                                }
                            } else {
                                rsx! {
                                    {items.iter().map(|item| rsx! {
                                        FeedCard {
                                            key: "{item.id}",
                                            item: item.clone(),
                                            token: token.clone(),
                                            on_deleted: { let mut feed = feed; move |_| feed.restart() },
                                            on_edited: { let mut feed = feed; move |_| feed.restart() },
                                        }
                                    })}
                                }
                            }
                        },
                        Some(Ok(items)) => {
                            // Guest view — always show the sign-in gate at the end of the feed.
                            const GUEST_LIMIT: usize = 4;
                            let visible: Vec<_> = items.iter().take(GUEST_LIMIT).collect();
                            let preview: Vec<_> = items.iter().skip(GUEST_LIMIT).take(2).collect();
                            rsx! {
                                if visible.is_empty() {
                                    div { class: "empty-state",
                                        div { class: "empty-icon", i { class: "ph ph-flag-checkered" } }
                                        h3 { "Nothing here yet" }
                                        p { "No public activity on this server yet." }
                                    }
                                }
                                {visible.iter().map(|item| rsx! {
                                    FeedCard {
                                        key: "{item.id}",
                                        item: (*item).clone(),
                                        token: None,
                                    }
                                })}
                                div { class: "feed-gate-section",
                                    if !preview.is_empty() {
                                        div { class: "feed-gate-blur-wrap",
                                            {preview.iter().map(|item| rsx! {
                                                FeedCard {
                                                    key: "preview-{item.id}",
                                                    item: (*item).clone(),
                                                    token: None,
                                                }
                                            })}
                                        }
                                    }
                                    div { class: "feed-gate-overlay",
                                        div { class: "feed-gate-card",
                                            i { class: "ph ph-person-simple-run feed-gate-icon" }
                                            h3 { class: "feed-gate-title", "Join the community" }
                                            p { class: "feed-gate-body", "Sign in to follow athletes and see your personalised feed." }
                                            div { class: "feed-gate-actions",
                                                Link { class: "btn btn-primary feed-gate-btn", to: crate::web::app::Route::Login {}, "Sign in" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn ComposeCard(initial: String, on_posted: EventHandler<()>) -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let mut posting = use_signal(|| false);
    let mut compose_error = use_signal(|| Option::<String>::None);

    // Shared image state
    let mut pending_images: Signal<Vec<PendingImage>> = use_signal(Vec::new);
    let mut images_loading = use_signal(|| false);

    // Activity mode state
    let mut activity_type = use_signal(|| "run".to_string());
    let mut activity_file_name = use_signal(|| Option::<String>::None);
    let mut activity_file_bytes = use_signal(|| Option::<Vec<u8>>::None);
    let mut activity_title = use_signal(String::new);
    let mut activity_desc = use_signal(String::new);
    let mut activity_visibility = use_signal(|| "public".to_string());

    // Parsed activity — populated after file selection for stat preview and hidden_stats UI.
    let mut parsed_activity: Signal<Option<parser::ParsedActivity>> = use_signal(|| None);
    // Stats that the user has chosen to hide (subset of stats present in the parsed file).
    let mut hidden_stats: Signal<Vec<String>> = use_signal(Vec::new);
    let mut parse_error: Signal<Option<String>> = use_signal(|| None);

    let on_images_changed = move |_: Event<FormData>| {
        if *images_loading.read() {
            return;
        }
        images_loading.set(true);
        compose_error.set(None);
        let max_new = 8usize.saturating_sub(pending_images.read().len());
        if max_new == 0 {
            images_loading.set(false);
            return;
        }
        spawn(async move {
            match compress_post_images_from_input("post-image-input", max_new).await {
                Ok(images) => {
                    for image in images {
                        if pending_images.read().len() < 8 {
                            pending_images.write().push(PendingImage {
                                name: image.name,
                                compressed_b64: image.b64,
                                preview_url: image.preview_url,
                            });
                        }
                    }
                }
                Err(err) => compose_error.set(Some(err)),
            }
            images_loading.set(false);
        });
    };

    let upload_token = token.clone();
    let on_upload_activity = move |_: Event<MouseData>| {
        let file_bytes = match activity_file_bytes.read().clone() {
            Some(b) => b,
            None => {
                compose_error.set(Some("Please select a GPX or FIT file.".into()));
                return;
            }
        };
        let file_name = activity_file_name
            .read()
            .clone()
            .unwrap_or_else(|| "activity.gpx".into());
        let atype = activity_type.read().clone();
        let title = {
            let t = activity_title.read().clone();
            if t.trim().is_empty() { None } else { Some(t) }
        };
        let desc = {
            let d = activity_desc.read().clone();
            if d.trim().is_empty() { None } else { Some(d) }
        };
        let vis = activity_visibility.read().clone();
        let hs = hidden_stats.read().clone();
        let t = upload_token.clone();
        let images = pending_images.read().clone();

        posting.set(true);
        compose_error.set(None);

        spawn(async move {
            // Image uploads not supported on this instance (no object storage).
            let uploaded: Vec<String> = Vec::new();
            let _ = images; // images captured but not uploaded
            match upload_exercise_fn(
                t,
                file_bytes,
                file_name,
                UploadExerciseMeta {
                    activity_type: atype,
                    visibility: vis,
                    title,
                    description: desc,
                    image_urls: uploaded,
                    hidden_stats: hs,
                },
            )
            .await
            {
                Ok(_) => {
                    activity_file_name.set(None);
                    activity_file_bytes.set(None);
                    activity_title.set(String::new());
                    activity_desc.set(String::new());
                    pending_images.write().clear();
                    on_posted.call(());
                }
                Err(e) => compose_error.set(Some(sfn_msg(&e))),
            }
            posting.set(false);
        });
    };

    let file_ready = activity_file_bytes.read().is_some();
    let imgs = pending_images.read().clone();
    let img_count = imgs.len();

    rsx! {
        div { class: "card compose-card",
            input {
                r#type: "file",
                id: "post-image-input",
                accept: "image/*",
                multiple: true,
                style: "display:none",
                onchange: on_images_changed,
            }

            if let Some(err) = compose_error.read().as_ref() {
                ErrorBanner { message: err.clone() }
            }

            div { class: "compose-body",
                        // File upload — always visible
                        label {
                            class: "file-drop-zone",
                            r#for: "activity-file",
                            if let Some(name) = activity_file_name.read().as_ref() {
                                div { class: "file-selected",
                                    i { class: "ph ph-file file-icon" }
                                    span { class: "file-name", "{name}" }
                                    button {
                                        class: "file-remove",
                                        onclick: move |e| {
                                            e.prevent_default();
                                            activity_file_name.set(None);
                                            activity_file_bytes.set(None);
                                            parsed_activity.set(None);
                                            hidden_stats.write().clear();
                                            parse_error.set(None);
                                            activity_title.set(String::new());
                                        },
                                        "×"
                                    }
                                }
                            } else {
                                div { class: "file-prompt",
                                    i { class: "ph ph-folder-open file-icon-lg" }
                                    span { class: "file-prompt-text", "Drop your GPX or FIT file here, or click to browse" }
                                    span { class: "file-hint", ".gpx · .fit — from Garmin, Wahoo, Strava, Komoot…" }
                                }
                            }
                            input {
                                r#type: "file",
                                id: "activity-file",
                                accept: ".gpx,.fit,application/gpx+xml,application/octet-stream",
                                style: "display:none",
                                onchange: move |e| {
                                    let files = e.files();
                                    if let Some(f) = files.into_iter().next() {
                                        let name = f.name();
                                        let is_fit = name.to_lowercase().ends_with(".fit");
                                        activity_file_name.set(Some(name.clone()));
                                        parsed_activity.set(None);
                                        hidden_stats.write().clear();
                                        parse_error.set(None);
                                        spawn(async move {
                                            if let Ok(bytes) = f.read_bytes().await {
                                                let bytes_vec = bytes.to_vec();
                                                let parse_result = if is_fit {
                                                    ParsedActivity::from_fit(&bytes_vec)
                                                } else {
                                                    ParsedActivity::from_gpx(&bytes_vec)
                                                };
                                                match parse_result {
                                                    Ok(pa) => {
                                                        // Auto-select activity type from sport in file.
                                                        let mapped_type = pa.sport.as_deref()
                                                            .and_then(sport_to_activity_type);
                                                        if let Some(t) = mapped_type {
                                                            activity_type.set(t.to_string());
                                                        }
                                                        // Pre-fill title if user hasn't typed one yet.
                                                        if activity_title.read().trim().is_empty() {
                                                            let type_for_title = mapped_type
                                                                .map(str::to_string)
                                                                .unwrap_or_else(|| activity_type.read().clone());
                                                            activity_title.set(auto_title(&pa, &type_for_title));
                                                        }
                                                        parsed_activity.set(Some(pa));
                                                    }
                                                    Err(e) => parse_error.set(Some(format!("Preview unavailable: {e}"))),
                                                }
                                                activity_file_bytes.set(Some(bytes_vec));
                                            }
                                        });
                                    }
                                },
                            }
                        }

                        // Everything below only shown after a file is selected
                        if activity_file_name.read().is_some() {
                            // Activity type selector (revealed after upload)
                            div { class: "activity-type-picker",
                                label { r#for: "activity-type-select", "Activity type" }
                                select {
                                    id: "activity-type-select",
                                    class: "activity-type-select",
                                    value: "{activity_type.read()}",
                                    onchange: move |e| activity_type.set(e.value()),
                                    for (val, label) in ACTIVITY_TYPES {
                                        option {
                                            key: "{val}",
                                            value: "{val}",
                                            selected: *activity_type.read() == *val,
                                            "{label}"
                                        }
                                    }
                                }
                            }

                            // Route map preview + hidden stats toggles (shown after parse)
                            {
                                let has_route = parsed_activity.read().as_ref()
                                    .is_some_and(|pa| !pa.route_coords.is_empty());
                                let map_hidden = hidden_stats.read().contains(&MAP_KEY.to_string());
                                let present: Vec<(&str, &str)> = {
                                    let pa_guard = parsed_activity.read();
                                    STAT_TOGGLES.iter()
                                        .copied()
                                        .filter(|(key, _)| pa_guard.as_ref().is_some_and(|pa| stat_is_present(pa, key)))
                                        .collect()
                                };
                                let show_toggles = has_route || !present.is_empty();
                                let preview: Option<ParsedStats> = {
                                    let pa = parsed_activity.read();
                                    pa.as_ref().map(|pa| ParsedStats {
                                        distance_m: pa.distance_m,
                                        duration_s: pa.duration_s,
                                        elevation_gain_m: pa.elevation_gain_m,
                                        avg_pace_s_per_km: pa.avg_pace_s_per_km,
                                        avg_heart_rate_bpm: pa.avg_heart_rate_bpm,
                                        max_heart_rate_bpm: pa.max_heart_rate_bpm,
                                        avg_power_w: pa.avg_power_w,
                                        max_power_w: pa.max_power_w,
                                        normalized_power_w: pa.normalized_power_w,
                                        avg_cadence_rpm: pa.avg_cadence_rpm,
                                        device: pa.device.clone(),
                                    })
                                };
                                rsx! {
                                    if has_route && !map_hidden {
                                        {
                                            let coords: Vec<dioxus_leaflet::LatLng> = parsed_activity.read()
                                                .as_ref()
                                                .map(|pa| pa.route_coords.iter()
                                                    .map(|p| dioxus_leaflet::LatLng::new(p.lat, p.lon))
                                                    .collect())
                                                .unwrap_or_default();
                                            rsx! {
                                                RouteMapFromCoords {
                                                    coords,
                                                    map_height: "180px".to_string(),
                                                }
                                            }
                                        }
                                    }
                                    if let Some(ref ps) = preview {
                                        div { class: "stats-grid",
                                            if ps.distance_m > 0.0 {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{fmt_distance(ps.distance_m)}" }
                                                    span { class: "stat-label", "Distance" }
                                                }
                                            }
                                            if ps.duration_s > 0 {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{fmt_duration(ps.duration_s)}" }
                                                    span { class: "stat-label", "Time" }
                                                }
                                            }
                                            if let Some(e) = ps.elevation_gain_m {
                                                if e > 0.0 {
                                                    div { class: "stat-cell",
                                                        span { class: "stat-value", "{fmt_elevation(e)}" }
                                                        span { class: "stat-label", "Elevation" }
                                                    }
                                                }
                                            }
                                            if let Some(p) = ps.avg_pace_s_per_km {
                                                if p > 0.0 {
                                                    div { class: "stat-cell",
                                                        span { class: "stat-value", "{fmt_pace(p)}" }
                                                        span { class: "stat-label", "Avg Pace" }
                                                    }
                                                }
                                            }
                                            if let Some(hr) = ps.avg_heart_rate_bpm {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{hr} bpm" }
                                                    span { class: "stat-label", "Avg HR" }
                                                }
                                            }
                                            if let Some(hr) = ps.max_heart_rate_bpm {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{hr} bpm" }
                                                    span { class: "stat-label", "Max HR" }
                                                }
                                            }
                                            if let Some(pwr) = ps.avg_power_w {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{pwr:.0}W" }
                                                    span { class: "stat-label", "Avg Power" }
                                                }
                                            }
                                            if let Some(pwr) = ps.max_power_w {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{pwr:.0}W" }
                                                    span { class: "stat-label", "Max Power" }
                                                }
                                            }
                                            if let Some(np) = ps.normalized_power_w {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{np:.0}W" }
                                                    span { class: "stat-label", "NP" }
                                                }
                                            }
                                            if let Some(cad) = ps.avg_cadence_rpm {
                                                div { class: "stat-cell",
                                                    span { class: "stat-value", "{cad:.0} rpm" }
                                                    span { class: "stat-label", "Cadence" }
                                                }
                                            }
                                        }
                                        if let Some(ref d) = ps.device {
                                            p { class: "activity-device",
                                                i { class: "ph ph-device-mobile-camera" }
                                                " {d}"
                                            }
                                        }
                                    }
                                    if show_toggles {
                                        div { class: "compose-field-row",
                                            label { class: "compose-label", "Hide from post" }
                                            div { class: "type-chip-row",
                                                if has_route {
                                                    button {
                                                        r#type: "button",
                                                        class: if map_hidden { "type-chip type-chip-active" } else { "type-chip" },
                                                        onclick: move |_| {
                                                            let mut hs = hidden_stats.write();
                                                            let k = MAP_KEY.to_string();
                                                            if hs.contains(&k) { hs.retain(|s| s != &k); } else { hs.push(k); }
                                                        },
                                                        i { class: "ph ph-map-trifold" }
                                                        " Map"
                                                    }
                                                }
                                                {present.into_iter().map(|(key, label)| {
                                                    let k = key.to_string();
                                                    let k2 = k.clone();
                                                    let label_s = label.to_string();
                                                    rsx! {
                                                        button {
                                                            key: "{k2}",
                                                            r#type: "button",
                                                            class: if hidden_stats.read().contains(&k) { "type-chip type-chip-active" } else { "type-chip" },
                                                            onclick: move |_| {
                                                                let mut hs = hidden_stats.write();
                                                                if hs.contains(&k2) {
                                                                    hs.retain(|s| s != &k2);
                                                                } else {
                                                                    hs.push(k2.clone());
                                                                }
                                                            },
                                                            "{label_s}"
                                                        }
                                                    }
                                                })}
                                            }
                                        }
                                    }
                                }
                            }

                            // Note-style compose area: title + description + footer
                            div { class: "note-compose-wrap",
                                input {
                                    r#type: "text",
                                    class: "activity-title-input",
                                    placeholder: "Activity name — auto-generated if left blank",
                                    value: "{activity_title}",
                                    oninput: move |e| activity_title.set(e.value()),
                                }
                                textarea {
                                    class: "compose-input note-compose-input activity-desc-input",
                                    placeholder: "How did it feel? Any notes about the route…",
                                    rows: "3",
                                    value: "{activity_desc}",
                                    oninput: move |e| activity_desc.set(e.value()),
                                }
                                if img_count > 0 || *images_loading.read() {
                                    NoteImageStrip {
                                        images: imgs.clone(),
                                        loading: *images_loading.read(),
                                        on_remove: move |idx| { pending_images.write().remove(idx); },
                                    }
                                }
                                div { class: "note-compose-footer",
                                    div { class: "note-compose-footer-left",
                                        if img_count < 8 && !*images_loading.read() {
                                            label {
                                                class: "note-attach-btn",
                                                r#for: "post-image-input",
                                                title: "Add photos",
                                                i { class: "ph ph-paperclip" }
                                            }
                                        }
                                        div { class: "type-chip-row",
                                            for (val, label) in [("public", "Public"), ("followers", "Followers"), ("private", "Private")] {
                                                button {
                                                    key: "{val}",
                                                    r#type: "button",
                                                    class: if *activity_visibility.read() == val { "type-chip type-chip-sm type-chip-active" } else { "type-chip type-chip-sm" },
                                                    onclick: {
                                                        let v = val.to_string();
                                                        move |_| activity_visibility.set(v.clone())
                                                    },
                                                    "{label}"
                                                }
                                            }
                                        }
                                    }
                                    div { class: "note-compose-footer-right",
                                        button {
                                            class: "btn btn-primary btn-sm",
                                            disabled: *posting.read() || !file_ready,
                                            onclick: on_upload_activity,
                                            if *posting.read() { "Uploading…" } else { "Post" }
                                        }
                                    }
                                }
                            }
                        }
            }
        }
    }
}

/// Compact horizontal strip of image thumbnails for the activity composer (no add-photo button).
#[component]
fn NoteImageStrip(
    images: Vec<PendingImage>,
    loading: bool,
    on_remove: EventHandler<usize>,
) -> Element {
    rsx! {
        div { class: "note-image-strip",
            {images.iter().enumerate().map(|(i, img)| rsx! {
                div { key: "{i}", class: "compose-thumb-wrap note-thumb",
                    img {
                        class: "compose-thumb",
                        src: "{img.preview_url}",
                        alt: "{img.name}",
                    }
                    button {
                        class: "compose-thumb-remove",
                        r#type: "button",
                        title: "Remove",
                        onclick: move |_| on_remove.call(i),
                        "×"
                    }
                }
            })}
            if loading {
                div { class: "compose-thumb-wrap note-thumb compose-thumb-loading",
                    div { class: "loading-spinner-sm" }
                }
            }
        }
    }
}
