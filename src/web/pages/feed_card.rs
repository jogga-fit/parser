use dioxus::prelude::*;

use crate::web::{
    FeedItem,
    app::Route,
    components::{Avatar, CarouselOverlay, MediaCollage},
    format::{fmt_distance, fmt_duration, fmt_elevation, fmt_pace},
    pages::exercise_icon_label,
    server_fns::{delete_object, like_object, unlike_object, update_post},
    sfn_msg,
};

const STAT_TOGGLES: &[(&str, &str)] = &[
    ("avg_heart_rate_bpm", "Heart rate"),
    ("max_heart_rate_bpm", "Max HR"),
    ("avg_power_w", "Avg power"),
    ("max_power_w", "Max power"),
    ("normalized_power_w", "NP"),
    ("avg_cadence_rpm", "Cadence"),
];

enum ActorDest {
    Local(Route),
    Remote(String),
}

#[component]
pub fn FeedCard(
    item: FeedItem,
    token: Option<String>,
    #[props(default)] on_deleted: Option<EventHandler<()>>,
    #[props(default)] on_edited: Option<EventHandler<()>>,
) -> Element {
    let published = item
        .published
        .get(..16)
        .unwrap_or(&item.published)
        .replace('T', " ");

    let (post_icon, post_label) = match item.object_type.as_str() {
        "Exercise" => {
            let (icon, label) = exercise_icon_label(item.exercise_type.as_deref().unwrap_or(""));
            (icon, label.to_string())
        }
        _ => ("ph-chat-circle", "Note".to_string()),
    };

    let mut liked = use_signal(|| item.viewer_has_liked);
    let mut like_count = use_signal(|| item.like_count);
    let mut liking = use_signal(|| false);

    let object_ap_id = item.object_ap_id.clone();
    let tok = token.clone();

    let mut toggle_like = move |_| {
        if *liking.read() {
            return;
        }
        let Some(tok) = tok.clone() else { return };
        let oid = object_ap_id.clone();
        let currently_liked = *liked.read();
        // Optimistic update.
        liked.set(!currently_liked);
        let new_count = *like_count.read() + if currently_liked { -1 } else { 1 };
        like_count.set(new_count);
        liking.set(true);
        spawn(async move {
            let result = if currently_liked {
                unlike_object(tok, oid).await
            } else {
                like_object(tok, oid).await
            };
            if result.is_err() {
                // Roll back optimistic update on failure.
                liked.set(currently_liked);
                let rolled_back = *like_count.read() + if currently_liked { 1 } else { -1 };
                like_count.set(rolled_back);
            }
            liking.set(false);
        });
    };

    let heart_class = if *liked.read() {
        "like-btn like-btn-active"
    } else {
        "like-btn"
    };

    let mut deleting = use_signal(|| false);
    let mut menu_open = use_signal(|| false);
    let mut edit_open = use_signal(|| false);
    let delete_ap_id = item.object_ap_id.clone();
    let delete_tok = token.clone();
    let is_owner = item.viewer_is_owner;

    let do_delete = move |_| {
        menu_open.set(false);
        let tok = match delete_tok.clone() {
            Some(t) => t,
            None => return,
        };
        let oid = delete_ap_id.clone();
        deleting.set(true);
        spawn(async move {
            if delete_object(tok, oid).await.is_ok() {
                if let Some(cb) = on_deleted {
                    cb.call(());
                }
            }
            deleting.set(false);
        });
    };

    let actor_profile_dest = if item.actor_is_local {
        ActorDest::Local(Route::UserProfile {
            username: format!("@{}", item.actor_username),
        })
    } else {
        ActorDest::Remote(item.actor_ap_id.clone())
    };

    let via_club = item.via_club_handle.clone();
    let via_club_display = item.via_club_display.clone();

    // Detail route — where the whole card navigates on click.
    let uuid = item
        .object_ap_id
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_owned();
    let actor_handle = if item.actor_is_local {
        format!("@{}", item.actor_username)
    } else {
        format!("@{}@{}", item.actor_username, item.actor_domain)
    };
    let detail_route = match item.object_type.as_str() {
        "Exercise" => Route::ExerciseDetail {
            username: actor_handle.clone(),
            exercise_id: uuid.clone(),
        },
        _ => Route::NoteDetail {
            username: actor_handle.clone(),
            note_id: uuid.clone(),
        },
    };
    let nav = use_navigator();

    // Carousel overlay state — hoisted outside the card so position:fixed
    // isn't contained by the card's overflow:hidden compositing layer.
    let mut overlay_open: Signal<Option<usize>> = use_signal(|| None);

    rsx! {
        div {
            class: "card feed-card feed-card-clickable",
            onclick: move |_| { nav.push(detail_route.clone()); },
            div { class: "feed-card-header",
                div { class: "feed-actor-col",
                    {match actor_profile_dest {
                        ActorDest::Local(ref route) => rsx! {
                            Link {
                                class: "feed-avatar-wrap feed-actor-link",
                                to: route.clone(),
                                onclick: move |e: MouseEvent| e.stop_propagation(),
                                Avatar { url: item.actor_avatar_url.clone(), name: item.actor_username.clone() }
                                span { class: "feed-username",
                                    "@{item.actor_username}"
                                    if let Some(ref club_display) = via_club_display {
                                        span { class: "feed-via-club",
                                            " › "
                                            Link {
                                                class: "feed-club-link",
                                                to: Route::ClubDetail { handle: via_club.clone().unwrap_or_default() },
                                                onclick: move |e: MouseEvent| e.stop_propagation(),
                                                "{club_display}"
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        ActorDest::Remote(ref url) => rsx! {
                            a {
                                class: "feed-avatar-wrap feed-actor-link",
                                href: "{url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                onclick: move |e: MouseEvent| e.stop_propagation(),
                                Avatar { url: item.actor_avatar_url.clone(), name: item.actor_username.clone() }
                                span { class: "feed-username",
                                    "@{item.actor_username}@{item.actor_domain}"
                                    if let Some(ref club_display) = via_club_display {
                                        span { class: "feed-via-club",
                                            " › "
                                            Link {
                                                class: "feed-club-link",
                                                to: Route::ClubDetail { handle: via_club.clone().unwrap_or_default() },
                                                onclick: move |e: MouseEvent| e.stop_propagation(),
                                                "{club_display}"
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }}
                    span { class: "post-type-badge",
                        i { class: "ph {post_icon}" }
                        span { "{post_label}" }
                    }
                }
                span { class: "post-time", "{published}" }
                if is_owner && token.is_some() {
                    div { class: "post-menu-wrap",
                        button {
                            class: "post-menu-trigger",
                            disabled: *deleting.read(),
                            onclick: move |e| { e.stop_propagation(); let v = *menu_open.read(); menu_open.set(!v); },
                            i { class: "ph ph-dots-three" }
                        }
                        if *menu_open.read() {
                            div {
                                class: "post-menu-backdrop",
                                onclick: move |_| menu_open.set(false),
                            }
                            div { class: "post-menu-dropdown",
                                button {
                                    class: "post-menu-item",
                                    onclick: move |_| { menu_open.set(false); edit_open.set(true); },
                                    i { class: "ph ph-pencil" }
                                    "Edit"
                                }
                                button {
                                    class: "post-menu-item post-menu-item-danger",
                                    onclick: do_delete,
                                    i { class: "ph ph-trash" }
                                    "Delete"
                                }
                            }
                        }
                    }
                }
            }

            // Exercise title
            if item.object_type == "Exercise" {
                if let Some(ref title) = item.title {
                    h3 { class: "exercise-title", "{title}" }
                }
            }

            // Exercise stats grid
            if item.object_type == "Exercise" {
                div { class: "stats-grid",
                    if let Some(d) = item.distance_m {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{fmt_distance(d)}" }
                            span { class: "stat-label", "Distance" }
                        }
                    }
                    if let Some(dur) = item.duration_s {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{fmt_duration(dur as i32)}" }
                            span { class: "stat-label", "Time" }
                        }
                    }
                    if let Some(p) = item.avg_pace_s_per_km {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{fmt_pace(p)}" }
                            span { class: "stat-label", "Pace" }
                        }
                    }
                    if let Some(e) = item.elevation_gain_m {
                        if e > 0.0 {
                            div { class: "stat-cell",
                                span { class: "stat-value", "{fmt_elevation(e)}" }
                                span { class: "stat-label", "Elevation" }
                            }
                        }
                    }
                    if let Some(hr) = item.avg_heart_rate_bpm {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{hr} bpm" }
                            span { class: "stat-label", "Avg HR" }
                        }
                    }
                    if let Some(hr) = item.max_heart_rate_bpm {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{hr} bpm" }
                            span { class: "stat-label", "Max HR" }
                        }
                    }
                    if let Some(pwr) = item.avg_power_w {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{pwr:.0} W" }
                            span { class: "stat-label", "Avg Power" }
                        }
                    }
                    if let Some(pwr) = item.max_power_w {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{pwr:.0} W" }
                            span { class: "stat-label", "Max Power" }
                        }
                    }
                    if let Some(np) = item.normalized_power_w {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{np:.0} W" }
                            span { class: "stat-label", "NP" }
                        }
                    }
                    if let Some(cad) = item.avg_cadence_rpm {
                        div { class: "stat-cell",
                            span { class: "stat-value", "{cad:.0} rpm" }
                            span { class: "stat-label", "Cadence" }
                        }
                    }
                }
            }

            if let Some(content) = &item.content {
                if !content.is_empty() {
                    div { class: "feed-content",
                        p { dangerous_inner_html: content.clone() }
                    }
                }
            }

            // Media collage: map + photos — shown after content, before actions.
            // Stop propagation so collage cell clicks open the overlay, not navigate.
            // on_open_overlay callback hoists overlay state outside the card so
            // position:fixed isn't contained by this card's overflow:hidden layer.
            if item.route_url.is_some() || !item.image_urls.is_empty() {
                div { onclick: move |e| e.stop_propagation(),
                    MediaCollage {
                        route_url: item.route_url.clone(),
                        image_urls: item.image_urls.clone(),
                        token: token.clone(),
                        on_open_overlay: move |idx| overlay_open.set(Some(idx)),
                    }
                }
            }

            if *edit_open.read() {
                if let Some(tok) = token.clone() {
                    EditPostModal {
                        item: item.clone(),
                        token: tok,
                        on_saved: move |_| {
                            edit_open.set(false);
                            if let Some(cb) = on_edited { cb.call(()); }
                        },
                        on_cancel: move |_| edit_open.set(false),
                    }
                }
            }

            // Action bar: like + comment count + delete
            div { class: "feed-card-actions",
                button {
                    class: heart_class,
                    disabled: token.is_none() || *liking.read(),
                    onclick: move |e: MouseEvent| { e.stop_propagation(); toggle_like(e); },
                    title: if *liked.read() { "Unlike" } else { "Like" },
                    aria_label: if *liked.read() { "Unlike" } else { "Like" },
                    if *liked.read() {
                        i { class: "ph-fill ph-heart like-heart" }
                    } else {
                        i { class: "ph ph-heart like-heart" }
                    }
                    if *like_count.read() > 0 {
                        span { class: "like-count", "{like_count}" }
                    }
                }
                // Comment count — links to post detail page.
                // Not shown for replies (posts that are themselves comments).
                if item.in_reply_to.is_none() {
                    Link {
                        class: "reply-count-link",
                        to: detail_route.clone(),
                        onclick: move |e: MouseEvent| e.stop_propagation(),
                        aria_label: if item.reply_count > 0 { format!("{} replies", item.reply_count) } else { "Reply".to_string() },
                        i { class: "ph ph-chat-circle reply-icon" }
                        if item.reply_count > 0 {
                            span { class: "reply-count", "{item.reply_count}" }
                        }
                    }
                }
            }
        }
        // Carousel overlay rendered OUTSIDE the card div so that position:fixed
        // escapes the card's overflow:hidden compositing layer (Chrome bug).
        if let Some(idx) = *overlay_open.read() {
            CarouselOverlay {
                route_url: item.route_url.clone(),
                image_urls: item.image_urls.clone(),
                token: token.clone(),
                initial_index: idx,
                on_close: move |_| overlay_open.set(None),
            }
        }
    }
}

#[component]
pub fn EditPostModal(
    item: FeedItem,
    token: String,
    on_saved: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    let is_exercise = item.object_type == "Exercise";

    let mut title = use_signal(|| item.title.clone().unwrap_or_default());
    let mut content = use_signal(|| item.content.clone().unwrap_or_default());
    let mut hidden_stats: Signal<Vec<String>> = use_signal(|| item.hidden_stats.clone());
    let mut removed_urls: Signal<Vec<String>> = use_signal(Vec::new);
    let mut saving = use_signal(|| false);
    let mut error: Signal<Option<String>> = use_signal(|| None);

    let has_map = item.route_url.is_some() || item.hidden_stats.contains(&"map".to_string());

    let relevant_stats: Vec<(&'static str, &'static str)> = STAT_TOGGLES
        .iter()
        .copied()
        .filter(|(key, _)| {
            let k = key.to_string();
            match *key {
                "avg_heart_rate_bpm" => {
                    item.avg_heart_rate_bpm.is_some() || item.hidden_stats.contains(&k)
                }
                "max_heart_rate_bpm" => {
                    item.max_heart_rate_bpm.is_some() || item.hidden_stats.contains(&k)
                }
                "avg_power_w" => item.avg_power_w.is_some() || item.hidden_stats.contains(&k),
                "max_power_w" => item.max_power_w.is_some() || item.hidden_stats.contains(&k),
                "normalized_power_w" => {
                    item.normalized_power_w.is_some() || item.hidden_stats.contains(&k)
                }
                "avg_cadence_rpm" => {
                    item.avg_cadence_rpm.is_some() || item.hidden_stats.contains(&k)
                }
                _ => false,
            }
        })
        .collect();

    let show_toggles = is_exercise && (has_map || !relevant_stats.is_empty());

    let tok = token.clone();
    let object_ap_id = item.object_ap_id.clone();

    let do_save = move |_| {
        if *saving.read() {
            return;
        }
        let t = tok.clone();
        let oid = object_ap_id.clone();
        let c = content.read().trim().to_string();
        let tl = if is_exercise {
            Some(title.read().trim().to_string())
        } else {
            None
        };
        let hs = if is_exercise {
            hidden_stats.read().clone()
        } else {
            vec![]
        };
        let removed = removed_urls.read().clone();
        saving.set(true);
        error.set(None);
        spawn(async move {
            let content_arg = if c.is_empty() { None } else { Some(c) };
            match update_post(t, oid, content_arg, tl, hs, removed).await {
                Ok(_) => on_saved.call(()),
                Err(e) => {
                    error.set(Some(sfn_msg(&e)));
                    saving.set(false);
                }
            }
        });
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| on_cancel.call(()),
            div {
                class: "modal-card edit-modal",
                onclick: move |e| e.stop_propagation(),

                // Header
                div { class: "modal-header edit-modal-header",
                    span { class: "edit-modal-title",
                        if is_exercise { "Edit Activity" } else { "Edit Post" }
                    }
                    button { class: "modal-close", onclick: move |_| on_cancel.call(()), "×" }
                }

                // Body
                div { class: "edit-modal-body",
                    if let Some(ref err) = *error.read() {
                        div { class: "error-banner", "{err}" }
                    }

                    if is_exercise {
                        div { class: "edit-field",
                            label { class: "edit-label", "Title" }
                            input {
                                r#type: "text",
                                class: "activity-title-input",
                                value: "{title}",
                                oninput: move |e| title.set(e.value()),
                                disabled: *saving.read(),
                            }
                        }
                    }

                    div { class: "edit-field",
                        label { class: "edit-label",
                            if is_exercise { "Description" } else { "Content" }
                        }
                        textarea {
                            class: "reply-textarea",
                            rows: "4",
                            value: "{content}",
                            oninput: move |e| content.set(e.value()),
                            disabled: *saving.read(),
                        }
                    }

                    if show_toggles {
                        div { class: "edit-field",
                            label { class: "edit-label", "Hide from post" }
                            div { class: "type-chip-row",
                                if has_map {
                                    button {
                                        r#type: "button",
                                        class: if hidden_stats.read().contains(&"map".to_string()) { "type-chip type-chip-active" } else { "type-chip" },
                                        onclick: move |_| {
                                            let mut hs = hidden_stats.write();
                                            let k = "map".to_string();
                                            if hs.contains(&k) { hs.retain(|s| s != &k); } else { hs.push(k); }
                                        },
                                        i { class: "ph ph-map-trifold" }
                                        " Map"
                                    }
                                }
                                {relevant_stats.iter().map(|(key, label)| {
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
                                                if hs.contains(&k2) { hs.retain(|s| s != &k2); } else { hs.push(k2.clone()); }
                                            },
                                            "{label_s}"
                                        }
                                    }
                                })}
                            }
                        }
                    }

                    if !item.image_urls.is_empty() {
                        div { class: "edit-field",
                            label { class: "edit-label", "Photos" }
                            div { class: "edit-image-strip",
                                {item.image_urls.iter().map(|url| {
                                    let u = url.clone();
                                    let u2 = url.clone();
                                    let u3 = url.clone();
                                    rsx! {
                                        div {
                                            key: "{u3}",
                                            class: if removed_urls.read().contains(&u) { "edit-thumb-wrap edit-thumb-removed" } else { "edit-thumb-wrap" },
                                            img { class: "collage-img edit-thumb", src: "{u2}", alt: "" }
                                            button {
                                                class: "compose-thumb-remove",
                                                r#type: "button",
                                                title: if removed_urls.read().contains(&u) { "Restore" } else { "Remove" },
                                                onclick: move |_| {
                                                    let mut rv = removed_urls.write();
                                                    if rv.contains(&u) { rv.retain(|x| x != &u); } else { rv.push(u.clone()); }
                                                },
                                                if removed_urls.read().contains(&u3) { "↩" } else { "×" }
                                            }
                                        }
                                    }
                                })}
                            }
                        }
                    }
                }

                // Footer
                div { class: "edit-modal-footer",
                    button {
                        class: "btn btn-ghost btn-sm",
                        onclick: move |_| on_cancel.call(()),
                        disabled: *saving.read(),
                        "Cancel"
                    }
                    button {
                        class: "btn btn-primary btn-sm",
                        onclick: do_save,
                        disabled: *saving.read(),
                        if *saving.read() { "Saving…" } else { "Save" }
                    }
                }
            }
        }
    }
}
