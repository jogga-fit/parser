use dioxus::prelude::*;

use crate::web::{
    FeedItem, ThreadItem,
    app::{AppShell, Route},
    components::{Avatar, MediaCollage},
    format::{fmt_distance, fmt_duration, fmt_elevation, fmt_pace},
    pages::{
        exercise_icon_label,
        feed_card::{EditPostModal, FeedCard},
    },
    server_fns::{create_reply, delete_object, get_thread, like_object, unlike_object},
    sfn_msg,
    state::AuthSignal,
};

#[component]
pub fn ExerciseDetailPage(object_ap_id: String) -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth.read().as_ref().map(|u| u.token.clone());
    let nav = use_navigator();

    let ap_id = object_ap_id.clone();
    let tok = token.clone();
    let thread = use_resource(move || {
        let id = ap_id.clone();
        let t = tok.clone();
        async move { get_thread(id, t).await }
    });

    rsx! {
        AppShell {
            div { class: "page-content thread-page",
                div { class: "thread-back",
                    Link { class: "back-link", to: Route::Home {},
                        "← Feed"
                    }
                }
                match &*thread.read() {
                    None => rsx! { div { class: "loading", "Loading…" } },
                    Some(Err(e)) => rsx! {
                        div { class: "error-banner", "{sfn_msg(e)}" }
                    },
                    Some(Ok((parent, replies))) => rsx! {
                        ExerciseDetailCard {
                            item: parent.clone(),
                            token: token.clone(),
                            on_deleted: move |_| { nav.push(Route::Home {}); },
                            on_edited: {
                                let mut thread = thread;
                                move |_| thread.restart()
                            },
                        }

                        div { class: "thread-divider",
                            if replies.is_empty() {
                                "No comments yet"
                            } else if replies.len() == 1 {
                                "1 comment"
                            } else {
                                "{replies.len()} comments"
                            }
                        }

                        {replies.iter().map(|r| rsx! {
                            ReplyItem {
                                key: "{r.ap_id}",
                                item: r.clone(),
                                token: token.clone(),
                                on_deleted: {
                                    let mut thread = thread;
                                    move |_| thread.restart()
                                },
                            }
                        })}

                        if token.is_some() {
                            ReplyComposer {
                                in_reply_to: parent.object_ap_id.clone(),
                                token: token.clone().unwrap_or_default(),
                                on_posted: {
                                    let mut thread = thread;
                                    move |_| thread.restart()
                                },
                            }
                        }
                    },
                }
            }
        }
    }
}

/// Full-detail exercise card used on the ExerciseDetailPage.
/// Shows the larger route map, elevation sparkline, and complete stats panel.
#[component]
fn ExerciseDetailCard(
    item: FeedItem,
    token: Option<String>,
    on_deleted: EventHandler<()>,
    on_edited: EventHandler<()>,
) -> Element {
    let published = item
        .published
        .get(..16)
        .unwrap_or(&item.published)
        .replace('T', " ");

    let (post_icon, post_label) = exercise_icon_label(item.exercise_type.as_deref().unwrap_or(""));

    // Pace: use server-provided value when available, fall back to computing it.
    let pace_str: Option<String> =
        item.avg_pace_s_per_km
            .map(fmt_pace)
            .or_else(|| match (item.duration_s, item.distance_m) {
                (Some(dur), Some(dist)) if dist > 0.0 => {
                    Some(fmt_pace(dur as f64 / (dist / 1000.0)))
                }
                _ => None,
            });

    let mut liked = use_signal(|| item.viewer_has_liked);
    let mut like_count = use_signal(|| item.like_count);
    let mut liking = use_signal(|| false);

    let object_ap_id = item.object_ap_id.clone();
    let tok = token.clone();

    let toggle_like = move |_| {
        if *liking.read() {
            return;
        }
        let Some(tok) = tok.clone() else { return };
        let oid = object_ap_id.clone();
        let currently_liked = *liked.read();
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
                on_deleted.call(());
            }
            deleting.set(false);
        });
    };

    rsx! {
        div { class: "card exercise-detail-card",

            // Carousel header: route map + photos at the top, edge-to-edge.
            if item.route_url.is_some() || !item.image_urls.is_empty() {
                MediaCollage {
                    route_url: item.route_url.clone(),
                    image_urls: item.image_urls.clone(),
                    token: token.clone(),
                    map_height: "300px".to_string(),
                    interactive: true,
                }
            }

            // Author + activity type header
            div { class: "feed-card-header edc-section",
                div { class: "feed-actor-col",
                    {if item.actor_is_local {
                        let route = Route::UserProfile { username: format!("@{}", item.actor_username) };
                        rsx! {
                            Link {
                                class: "feed-avatar-wrap feed-actor-link",
                                to: route,
                                Avatar { url: item.actor_avatar_url.clone(), name: item.actor_username.clone() }
                                span { class: "feed-username", "@{item.actor_username}" }
                            }
                        }
                    } else {
                        let url = item.actor_ap_id.clone();
                        let domain = item.actor_domain.clone();
                        let uname = item.actor_username.clone();
                        rsx! {
                            a {
                                class: "feed-avatar-wrap feed-actor-link",
                                href: "{url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                Avatar { url: item.actor_avatar_url.clone(), name: item.actor_username.clone() }
                                span { class: "feed-username", "@{uname}@{domain}" }
                            }
                        }
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

            if *edit_open.read() {
                if let Some(tok) = token.clone() {
                    EditPostModal {
                        item: item.clone(),
                        token: tok,
                        on_saved: move |_| {
                            edit_open.set(false);
                            on_edited.call(());
                        },
                        on_cancel: move |_| edit_open.set(false),
                    }
                }
            }

            // Title + device badge
            div { class: "exercise-detail-header edc-section",
                if let Some(ref title) = item.title {
                    h2 { class: "exercise-detail-title", "{title}" }
                }
                if let Some(ref dev) = item.device {
                    span { class: "exercise-device-badge",
                        i { class: "ph ph-watch" }
                        "{dev}"
                    }
                }
            }

            // Full stats panel
            div { class: "stats-grid stats-grid-detail edc-section",
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
                if let Some(ref p) = pace_str {
                    div { class: "stat-cell",
                        span { class: "stat-value", "{p}" }
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

            // Description
            if let Some(ref content) = item.content {
                if !content.is_empty() {
                    div { class: "feed-content edc-section",
                        p { dangerous_inner_html: content.clone() }
                    }
                }
            }

            // Action bar
            div { class: "feed-card-actions edc-section",
                button {
                    class: heart_class,
                    disabled: token.is_none() || *liking.read(),
                    onclick: toggle_like,
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
            }
        }
    }
}

#[component]
pub fn PostDetailPage(object_ap_id: String) -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth.read().as_ref().map(|u| u.token.clone());
    let nav = use_navigator();

    let ap_id = object_ap_id.clone();
    let tok = token.clone();
    let thread = use_resource(move || {
        let id = ap_id.clone();
        let t = tok.clone();
        async move { get_thread(id, t).await }
    });

    rsx! {
        AppShell {
            div { class: "page-content thread-page",
                // Back link
                div { class: "thread-back",
                    Link { class: "back-link", to: Route::Home {},
                        "← Feed"
                    }
                }
                match &*thread.read() {
                    None => rsx! { div { class: "loading", "Loading…" } },
                    Some(Err(e)) => rsx! {
                        div { class: "error-banner", "{sfn_msg(e)}" }
                    },
                    Some(Ok((parent, replies))) => rsx! {
                        // Parent post
                        FeedCard {
                            item: parent.clone(),
                            token: token.clone(),
                            on_deleted: move |_| { nav.push(Route::Home {}); },
                        }

                        // Reply count header
                        div { class: "thread-divider",
                            if replies.is_empty() {
                                "No comments yet"
                            } else if replies.len() == 1 {
                                "1 comment"
                            } else {
                                "{replies.len()} comments"
                            }
                        }

                        // Reply list
                        {replies.iter().map(|r| rsx! {
                            ReplyItem {
                                key: "{r.ap_id}",
                                item: r.clone(),
                                token: token.clone(),
                                on_deleted: {
                                    let mut thread = thread;
                                    move |_| thread.restart()
                                },
                            }
                        })}

                        // Composer — only when authenticated.
                        // Use the resolved AP ID from the parent (not the raw UUID route prop).
                        // TODO: support @mention tagging — tagged users should receive a
                        //       "mention" notification (NotificationQueries::insert kind="mention").
                        if token.is_some() {
                            ReplyComposer {
                                in_reply_to: parent.object_ap_id.clone(),
                                token: token.clone().unwrap_or_default(),
                                on_posted: {
                                    let mut thread = thread;
                                    move |_| thread.restart()
                                },
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn ReplyItem(item: ThreadItem, token: Option<String>, on_deleted: EventHandler<()>) -> Element {
    let published = item
        .published
        .get(..16)
        .unwrap_or(&item.published)
        .replace('T', " ");

    let mut liked = use_signal(|| item.viewer_has_liked);
    let mut like_count = use_signal(|| item.like_count);
    let mut liking = use_signal(|| false);

    let ap_id = item.ap_id.clone();
    let tok = token.clone();

    let toggle_like = move |_| {
        if *liking.read() {
            return;
        }
        let Some(tok) = tok.clone() else { return };
        let oid = ap_id.clone();
        let currently_liked = *liked.read();
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
    let delete_ap_id = item.ap_id.clone();
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
                on_deleted.call(());
            }
            deleting.set(false);
        });
    };

    rsx! {
        div { class: "card reply-item",
            div { class: "feed-card-header",
                div { class: "feed-avatar-wrap",
                    Avatar { url: item.author_avatar_url.clone(), name: item.author_username.clone() }
                    span { class: "feed-username", "@{item.author_username}" }
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
            if let Some(content) = &item.content {
                if !content.is_empty() {
                    div { class: "feed-content",
                        p { dangerous_inner_html: content.clone() }
                    }
                }
            }
            div { class: "feed-card-actions",
                button {
                    class: heart_class,
                    disabled: token.is_none() || *liking.read(),
                    onclick: toggle_like,
                    title: if *liked.read() { "Unlike" } else { "Like" },
                    if *liked.read() {
                        i { class: "ph-fill ph-heart like-heart" }
                    } else {
                        i { class: "ph ph-heart like-heart" }
                    }
                    if *like_count.read() > 0 {
                        span { class: "like-count", "{like_count}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ReplyComposer(in_reply_to: String, token: String, on_posted: EventHandler<()>) -> Element {
    let mut content = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut error: Signal<Option<String>> = use_signal(|| None);

    let submit = {
        let in_reply_to = in_reply_to.clone();
        let tok = token.clone();
        move |_| {
            let text = content.read().trim().to_string();
            if text.is_empty() || *submitting.read() {
                return;
            }
            let irt = in_reply_to.clone();
            let t = tok.clone();
            submitting.set(true);
            error.set(None);
            spawn(async move {
                match create_reply(t, text, irt).await {
                    Ok(_) => {
                        content.set(String::new());
                        on_posted.call(());
                    }
                    Err(e) => {
                        error.set(Some(sfn_msg(&e)));
                    }
                }
                submitting.set(false);
            });
        }
    };

    rsx! {
        div { class: "reply-composer",
            if let Some(err) = error.read().as_ref() {
                div { class: "error-banner", "{err}" }
            }
            textarea {
                class: "reply-textarea",
                placeholder: "Write a reply…",
                disabled: *submitting.read(),
                value: "{content}",
                oninput: move |e| content.set(e.value()),
            }
            button {
                class: "btn btn-primary",
                disabled: content.read().trim().is_empty() || *submitting.read(),
                onclick: submit,
                if *submitting.read() { "Posting…" } else { "Reply" }
            }
        }
    }
}
