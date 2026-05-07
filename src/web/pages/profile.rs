use dioxus::prelude::*;

use super::feed_card::FeedCard;
use crate::web::{
    ActorInfo, ConnectionsResult,
    app::{AppShell, Route},
    browser::copy_to_clipboard,
    components::{CropModal, CropModalState, ErrorBanner},
    image::{
        CropSelection, clear_file_input, compress_avatar_from_input,
        prepare_selected_image_from_input, revoke_object_url,
    },
    server_fns::{
        check_following, follow_person, get_actor_connections, get_actor_info, get_actor_posts,
        unfollow_actor, update_profile,
    },
    sfn_msg, sleep_ms,
    state::AuthSignal,
};

#[component]
pub fn ProfilePage(username: String) -> Element {
    let auth = use_context::<AuthSignal>();

    // Peek-compare: sync prop into signal so use_resource re-runs on navigation.
    // (use_effect only fires on signal changes, not prop-change re-renders.)
    let plain = username.trim_start_matches('@').to_string();
    let mut plain_sig = use_signal(|| plain.clone());
    if *plain_sig.peek() != plain {
        plain_sig.set(plain.clone());
    }

    let info = use_resource(move || {
        let p = plain_sig();
        async move { get_actor_info(p).await }
    });

    // Gate on `ready` so SSR and initial client render produce identical output.
    // After hydration the effect fires and re-renders with correct state.
    let mut ready = use_signal(|| false);
    use_effect(move || {
        ready.set(true);
    });
    let is_own = *ready.read()
        && auth
            .read()
            .as_ref()
            .map(|u| {
                let p = plain_sig.read();
                let local = p.split_once('@').map(|(u, _)| u).unwrap_or(&p);
                u.username == local
            })
            .unwrap_or(false);
    let is_logged_in = *ready.read()
        && auth
            .read()
            .as_ref()
            .map(|u| !u.token.is_empty())
            .unwrap_or(false);

    let content = match info.read().as_ref() {
        None => rsx! { div { class: "loading-spinner", "Loading…" } },
        Some(Err(_)) => rsx! { NotFound { username: plain_sig() } },
        Some(Ok(actor)) => rsx! {
            ProfileCard {
                actor: actor.clone(),
                is_own,
                is_logged_in,
            }
        },
    };

    // Always render AppShell — sidebar fills in reactively after hydration.
    rsx! {
        AppShell {
            div { class: "page-content", {content} }
        }
    }
}

#[component]
fn ProfileCard(actor: ActorInfo, is_own: bool, is_logged_in: bool) -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let mut editing = use_signal(|| false);
    let mut display_name = use_signal(|| actor.display_name.clone().unwrap_or_default());
    let mut bio = use_signal(|| actor.bio.clone().unwrap_or_default());
    let mut saving = use_signal(|| false);
    let mut saved = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut avatar_url = use_signal(|| actor.avatar_url.clone());
    let mut crop_modal: Signal<Option<CropModalState>> = use_signal(|| None);

    // Reset actor-derived signals when navigating to a different profile.
    // Keyed on username@domain so in-progress edits survive same-profile re-renders.
    let actor_key = format!("{}@{}", actor.username, actor.domain);
    let mut last_actor_key = use_signal(|| actor_key.clone());
    if *last_actor_key.peek() != actor_key {
        last_actor_key.set(actor_key);
        editing.set(false);
        display_name.set(actor.display_name.clone().unwrap_or_default());
        bio.set(actor.bio.clone().unwrap_or_default());
        avatar_url.set(actor.avatar_url.clone());
    }
    let mut photo_error = use_signal(|| Option::<String>::None);
    let mut connections_tab: Signal<Option<&'static str>> = use_signal(|| None);

    // Determine if the viewer may open the connections modal.
    // - Public profile or own profile: always yes.
    // - Private profile + logged in: only if viewer is an accepted follower.
    // - Private profile + not logged in: no.
    // Track ap_id + public_profile as signals so the resource re-runs on profile navigation.
    let mut actor_ap_id_sig = use_signal(|| actor.ap_id.clone());
    let mut actor_public_sig = use_signal(|| actor.public_profile);
    if *actor_ap_id_sig.peek() != actor.ap_id {
        actor_ap_id_sig.set(actor.ap_id.clone());
    }
    if *actor_public_sig.peek() != actor.public_profile {
        actor_public_sig.set(actor.public_profile);
    }
    let token_cv = token.clone();
    let can_view_connections = use_resource(move || {
        let own = is_own;
        let public = actor_public_sig();
        let t = token_cv.clone();
        let aid = actor_ap_id_sig();
        async move {
            if own || public {
                return true;
            }
            if t.is_empty() {
                return false;
            }
            check_following(t, aid).await.ok().flatten() == Some(true)
        }
    });
    // For public / own profiles use true as the initial value to avoid a flash of
    // non-interactive stats while the resource resolves.
    let can_view = can_view_connections
        .read()
        .as_ref()
        .copied()
        .unwrap_or(is_own || actor.public_profile);

    // Fetch posts client-side; visibility depends on auth.
    // Peek-compare so use_resource re-runs when navigating to a different profile.
    let mut actor_username_sig = use_signal(|| actor.username.clone());
    if *actor_username_sig.peek() != actor.username {
        actor_username_sig.set(actor.username.clone());
    }
    let token_posts = token.clone();
    let posts = use_resource(move || {
        let u = actor_username_sig();
        let t = if token_posts.is_empty() {
            None
        } else {
            Some(token_posts.clone())
        };
        async move { get_actor_posts(u, t).await }
    });

    let follow_handle = format!("@{}@{}", actor.username, actor.domain);

    let token_save = token.clone();
    let on_save = move |_: Event<MouseData>| {
        let t = token_save.clone();
        let dn = display_name.read().clone();
        let b = bio.read().clone();
        saving.set(true);
        saved.set(false);
        error.set(None);
        spawn(async move {
            let dn_opt = if dn.trim().is_empty() { None } else { Some(dn) };
            let bio_opt = if b.trim().is_empty() { None } else { Some(b) };
            match update_profile(t, dn_opt, bio_opt).await {
                Ok(()) => {
                    saved.set(true);
                    editing.set(false);
                }
                Err(e) => error.set(Some(sfn_msg(&e))),
            }
            saving.set(false);
        });
    };

    let t_avatar = token.clone();
    let on_avatar_change = move |_| {
        spawn(async move {
            photo_error.set(None);
            match prepare_selected_image_from_input("avatar-file-input").await {
                Ok(image) => crop_modal.set(Some(CropModalState {
                    object_url: image.object_url,
                    natural_width: image.natural_width,
                    natural_height: image.natural_height,
                    output_width: 400,
                    output_height: 400,
                    title: "Crop avatar".to_string(),
                    circle_mask: true,
                })),
                Err(err) => photo_error.set(Some(err)),
            }
        });
    };

    let t_avatar_apply = t_avatar.clone();
    let on_crop_apply = move |crop: CropSelection| {
        let _t = t_avatar_apply.clone();
        spawn(async move {
            photo_error.set(None);
            match compress_avatar_from_input("avatar-file-input", crop).await {
                Ok(_image) => {
                    photo_error.set(Some(
                        "Avatar upload not supported on this instance".to_string(),
                    ));
                }
                Err(err) => photo_error.set(Some(err)),
            }
            if let Some(current) = crop_modal.take() {
                revoke_object_url(&current.object_url);
            }
        });
    };

    let on_crop_cancel = move |_| {
        let _ = clear_file_input("avatar-file-input");
        if let Some(current) = crop_modal.take() {
            revoke_object_url(&current.object_url);
        }
    };

    let initial = actor
        .username
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());
    let display = if display_name.read().is_empty() {
        actor.username.clone()
    } else {
        display_name.read().clone()
    };

    rsx! {
        div { class: "card profile-card",
            div { class: "profile-header-row",
                div { class: "avatar-upload-wrap",
                    div { class: "avatar-inner",
                        if let Some(url) = avatar_url.read().as_ref() {
                            img {
                                class: "avatar avatar-lg avatar-img",
                                src: "{url}",
                                alt: "{actor.username}",
                            }
                        } else {
                            div { class: "avatar avatar-lg", "{initial}" }
                        }
                        if is_own && *editing.read() {
                            label {
                                class: "avatar-overlay-btn",
                                r#for: "avatar-file-input",
                                "📷"
                            }
                        }
                    }
                }
                if is_own {
                    input {
                        r#type: "file",
                        id: "avatar-file-input",
                        accept: "image/jpeg,image/png,image/webp",
                        style: "display:none",
                        onchange: on_avatar_change,
                    }
                }
                div { class: "profile-header-actions",
                    if is_own && !*editing.read() {
                        button {
                            class: "btn btn-ghost profile-edit-btn",
                            onclick: move |_| { editing.set(true); saved.set(false); },
                            "Edit profile"
                        }
                    }
                    // Renders nothing for is_own — FollowButton guards itself.
                    FollowButton {
                        is_own,
                        is_logged_in,
                        token: token.clone(),
                        ap_id: actor.ap_id.clone(),
                        handle: follow_handle.clone(),
                    }
                }
            }

            div { class: "profile-body",
                if is_own {
                    if let Some(err) = photo_error.read().as_ref() {
                        ErrorBanner { message: err.clone() }
                    }
                }

                if is_own && *editing.read() {
                    div { class: "profile-edit-form",
                        div { class: "form-group",
                            label { "Display name" }
                            input {
                                r#type: "text",
                                placeholder: &*actor.username,
                                value: "{display_name}",
                                oninput: move |e| { display_name.set(e.value()); saved.set(false); },
                            }
                        }
                        div { class: "form-group",
                            label { "Bio" }
                            textarea {
                                placeholder: "Tell people about yourself…",
                                rows: "3",
                                value: "{bio}",
                                oninput: move |e| { bio.set(e.value()); saved.set(false); },
                            }
                        }
                        if let Some(err) = error.read().as_ref() {
                            ErrorBanner { message: err.clone() }
                        }
                        div { class: "settings-row",
                            button {
                                class: "btn btn-primary",
                                disabled: *saving.read(),
                                onclick: on_save,
                                if *saving.read() { "Saving…" } else { "Save" }
                            }
                            button {
                                class: "btn btn-ghost",
                                onclick: move |_| { editing.set(false); error.set(None); },
                                "Cancel"
                            }
                        }
                    }
                } else {
                    h2 { class: "profile-name", "{display}" }
                    p { class: "profile-handle", "@{actor.username}@{actor.domain}" }
                    if !bio.read().is_empty() {
                        p { class: "profile-bio", "{bio}" }
                    }

                    div { class: "profile-stats",
                        if can_view {
                            button {
                                class: "profile-stat",
                                onclick: move |_| { connections_tab.set(Some("following")); },
                                span { class: "profile-stat-count", "{actor.following_count}" }
                                span { class: "profile-stat-label", "Following" }
                            }
                        } else {
                            span { class: "profile-stat",
                                span { class: "profile-stat-count", "{actor.following_count}" }
                                span { class: "profile-stat-label", "Following" }
                            }
                        }
                        span { class: "profile-stat-sep" }
                        if can_view {
                            button {
                                class: "profile-stat",
                                onclick: move |_| { connections_tab.set(Some("followers")); },
                                span { class: "profile-stat-count", "{actor.followers_count}" }
                                span { class: "profile-stat-label", "Followers" }
                            }
                        } else {
                            span { class: "profile-stat",
                                span { class: "profile-stat-count", "{actor.followers_count}" }
                                span { class: "profile-stat-label", "Followers" }
                            }
                        }
                    }

                    if is_own && *saved.read() {
                        span { class: "saved-badge", "✓ Saved" }
                    }
                }

            }
        }

        match posts.read().as_ref() {
            None => rsx! { div { class: "loading-spinner", "Loading posts…" } },
            Some(Err(e)) => rsx! {
                div { class: "profile-empty-posts", "{crate::web::sfn_msg(e)}" }
            },
            Some(Ok(items)) if items.is_empty() => rsx! {
                if !actor.public_profile && !is_own {
                    div { class: "card private-profile-notice",
                        i { class: "ph ph-lock private-profile-icon" }
                        div { class: "private-profile-text",
                            strong { "This profile is private." }
                            span { " Follow to see their posts." }
                        }
                    }
                } else {
                    div { class: "profile-empty-posts", "No posts yet." }
                }
            },
            Some(Ok(items)) => rsx! {
                {items.iter().map(|item| {
                    let tok = if token.is_empty() { None } else { Some(token.clone()) };
                    rsx! {
                        FeedCard {
                            key: "{item.id}",
                            item: item.clone(),
                            token: tok,
                            on_deleted: {
                                let mut posts = posts;
                                move |_| posts.restart()
                            },
                            on_edited: {
                                let mut posts = posts;
                                move |_| posts.restart()
                            },
                        }
                    }
                })}
            },
        }

        if connections_tab().is_some() {
            ConnectionsModal {
                username: actor.username.clone(),
                token: if token.is_empty() { None } else { Some(token.clone()) },
                connections_tab,
            }
        }

        if let Some(crop_state) = crop_modal() {
            CropModal {
                state: crop_state,
                on_cancel: on_crop_cancel,
                on_apply: on_crop_apply,
            }
        }
    }
}

#[component]
fn ConnectionsModal(
    username: String,
    token: Option<String>,
    mut connections_tab: Signal<Option<&'static str>>,
) -> Element {
    let nav = use_navigator();
    let mut fetched: Signal<Option<ConnectionsResult>> = use_signal(|| None);
    let mut loading = use_signal(|| false);

    // Fetch once when the modal first opens; skip on subsequent tab switches.
    let username_fetch = username.clone();
    let token_fetch = token.clone();
    use_effect(move || {
        let tab = connections_tab();
        if tab.is_some() && fetched.read().is_none() && !*loading.read() {
            loading.set(true);
            let u = username_fetch.clone();
            let t = token_fetch.clone();
            spawn(async move {
                if let Ok(result) = get_actor_connections(u, t).await {
                    fetched.set(Some(result));
                }
                loading.set(false);
            });
        }
    });

    let active_tab = connections_tab().unwrap_or("following");

    // Clone data out of the signal so we can use it in RSX without a live borrow.
    let data: Option<ConnectionsResult> = fetched.read().clone();

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| { connections_tab.set(None); },

            div {
                class: "modal-card connections-modal",
                onclick: move |e| { e.stop_propagation(); },

                div { class: "modal-header",
                    div { class: "modal-tabs",
                        button {
                            class: if active_tab == "following" { "modal-tab modal-tab-active" } else { "modal-tab" },
                            onclick: move |_| { connections_tab.set(Some("following")); },
                            "Following"
                        }
                        button {
                            class: if active_tab == "followers" { "modal-tab modal-tab-active" } else { "modal-tab" },
                            onclick: move |_| { connections_tab.set(Some("followers")); },
                            "Followers"
                        }
                    }
                    button {
                        class: "modal-close",
                        onclick: move |_| { connections_tab.set(None); },
                        "×"
                    }
                }

                div { class: "modal-body",
                    if *loading.read() || data.is_none() {
                        div { class: "loading-spinner", "Loading…" }
                    } else {
                        match data {
                            None => rsx! {},
                            Some(ref result) => {
                                let items: &[crate::web::ConnectionItem] = if active_tab == "following" {
                                    &result.following
                                } else {
                                    &result.followers
                                };
                                if items.is_empty() {
                                    rsx! {
                                        div { class: "connections-empty",
                                            if active_tab == "following" { "Not following anyone yet." }
                                            else { "No followers yet." }
                                        }
                                    }
                                } else {
                                    rsx! {
                                        {items.iter().map(|item| {
                                            let nav2 = nav;
                                            let uname = item.username.clone();
                                            let is_local = item.is_local;
                                            let domain = item.domain.clone();
                                            let handle = if is_local {
                                                format!("@{uname}")
                                            } else {
                                                format!("@{uname}@{domain}")
                                            };
                                            let initial = item.username
                                                .chars()
                                                .next()
                                                .map(|c| c.to_uppercase().to_string())
                                                .unwrap_or_else(|| "?".into());
                                            let display = item.display_name
                                                .clone()
                                                .unwrap_or_else(|| item.username.clone());
                                            // Only local profiles are navigable; remote ones
                                            // are not hosted here so routing would 404.
                                            let nav_handle = if is_local {
                                                Some(format!("@{uname}"))
                                            } else {
                                                None
                                            };
                                            rsx! {
                                                div {
                                                    key: "{item.ap_id}",
                                                    class: if nav_handle.is_some() { "connection-row connection-row-link" } else { "connection-row" },
                                                    onclick: move |_| {
                                                        if let Some(ref h) = nav_handle {
                                                            connections_tab.set(None);
                                                            nav2.push(Route::UserProfile {
                                                                username: h.clone(),
                                                            });
                                                        }
                                                    },
                                                    div { class: "avatar avatar-sm", "{initial}" }
                                                    div { class: "connection-info",
                                                        span { class: "connection-name", "{display}" }
                                                        span { class: "connection-handle", "{handle}" }
                                                    }
                                                }
                                            }
                                        })}
                                    }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

// Separate component so Dioxus 0.7 always allocates a dynamic template slot.

#[component]
fn FollowButton(
    is_own: bool,
    is_logged_in: bool,
    token: String,
    ap_id: String,
    handle: String,
) -> Element {
    // None = not following, Some(false) = pending, Some(true) = accepted
    let mut follow_status = use_signal(|| Option::<Option<bool>>::None);
    let mut in_flight = use_signal(|| false);
    let mut follow_error = use_signal(|| Option::<String>::None);
    let mut copied = use_signal(|| false);

    // Peek-compare: sync props into signals so use_resource / RSX re-run on navigation.
    let mut is_own_sig = use_signal(|| is_own);
    let mut is_logged_in_sig = use_signal(|| is_logged_in);
    let mut ap_id_sig = use_signal(|| ap_id.clone());

    if *is_own_sig.peek() != is_own {
        is_own_sig.set(is_own);
    }
    if *is_logged_in_sig.peek() != is_logged_in {
        is_logged_in_sig.set(is_logged_in);
    }
    if *ap_id_sig.peek() != ap_id {
        ap_id_sig.set(ap_id.clone());
    }

    // Fetch follow status. Re-runs automatically on profile navigation (ap_id change).
    let token_check = token.clone();
    let mut follow_resource = use_resource(move || {
        let logged_in = is_logged_in_sig();
        let own = is_own_sig();
        let t = token_check.clone();
        let id = ap_id_sig();
        async move {
            if logged_in && !own {
                check_following(t, id).await.ok()
            } else {
                None
            }
        }
    });

    // Sync resource into follow_status only when no mutation is in flight.
    // When in_flight is true the optimistic value owns follow_status.
    // When the resource is restarting (outer None = loading) we don't override.
    if !*in_flight.peek() {
        if let Some(resource_val) = follow_resource.read().as_ref() {
            // resource_val: &Option<Option<bool>> — inner None = not following,
            // Some(false) = pending, Some(true) = accepted.
            if *follow_status.peek() != *resource_val {
                follow_status.set(*resource_val);
            }
        }
    }

    let token_follow = token.clone();
    let ap_id_follow = ap_id.clone();
    let on_follow = move |_: Event<MouseData>| {
        let t = token_follow.clone();
        let id = ap_id_follow.clone();
        in_flight.set(true);
        follow_error.set(None);
        spawn(async move {
            match follow_person(t, id).await {
                Ok(()) => {
                    // Optimistic: show pending (Some(false)) — server will confirm
                    follow_status.set(Some(Some(false)));
                    follow_resource.restart();
                }
                Err(e) => follow_error.set(Some(sfn_msg(&e))),
            }
            in_flight.set(false);
        });
    };

    let token_unfollow = token.clone();
    let ap_id_unfollow = ap_id.clone();
    let on_unfollow = move |_: Event<MouseData>| {
        let t = token_unfollow.clone();
        let id = ap_id_unfollow.clone();
        in_flight.set(true);
        follow_error.set(None);
        spawn(async move {
            match unfollow_actor(t, id).await {
                Ok(()) => {
                    follow_status.set(Some(None));
                    follow_resource.restart();
                }
                Err(e) => follow_error.set(Some(sfn_msg(&e))),
            }
            in_flight.set(false);
        });
    };

    let copy_handle = handle.clone();
    let on_copy_handle = move |_: Event<MouseData>| {
        let h = copy_handle.clone();
        spawn(async move {
            if copy_to_clipboard(&h).await.is_ok() {
                copied.set(true);
                sleep_ms(1_500).await;
                copied.set(false);
            }
        });
    };

    rsx! {
        if !*is_own_sig.read() {
            if *is_logged_in_sig.read() {
                match *follow_status.read() {
                    None => rsx! {
                        button { class: "btn btn-ghost", disabled: true, "…" }
                    },
                    Some(None) => rsx! {
                        button {
                            class: "btn btn-primary",
                            disabled: *in_flight.read(),
                            onclick: on_follow,
                            if *in_flight.read() { "Following…" } else { "Follow" }
                        }
                    },
                    Some(Some(false)) => rsx! {
                        button {
                            class: "btn btn-ghost follow-pending-btn",
                            disabled: *in_flight.read(),
                            onclick: on_unfollow,
                            if *in_flight.read() { "…" } else { "Pending…" }
                        }
                    },
                    Some(Some(true)) => rsx! {
                        button {
                            class: "btn btn-ghost",
                            disabled: *in_flight.read(),
                            onclick: on_unfollow,
                            if *in_flight.read() { "Unfollowing…" } else { "Unfollow" }
                        }
                    },
                }
            } else {
                button {
                    class: if *copied.read() { "btn btn-ghost" } else { "btn btn-primary" },
                    onclick: on_copy_handle,
                    if *copied.read() { "✓ Copied!" } else { "Follow" }
                }
            }
            if let Some(err) = follow_error.read().as_ref() {
                span { class: "error-banner", "{err}" }
            }
        }
    }
}

#[component]
fn NotFound(username: String) -> Element {
    let nav = use_navigator();
    rsx! {
        div { class: "not-found-page",
            div { class: "not-found-blob-wrap",
                div { class: "nf-blob nf-blob-a" }
                div { class: "nf-blob nf-blob-b" }
            }
            div { class: "not-found-card",
                div { class: "nf-illustration",
                    i { class: "ph ph-person-simple-run nf-runner" }
                    span { class: "nf-arrow", "←" }
                    i { class: "ph ph-flag-checkered nf-flag" }
                }
                p { class: "nf-label", "did not find" }
                h1 { class: "nf-title", "DNF" }
                p { class: "nf-handle", "@{username}" }
                p { class: "nf-desc", "This athlete isn't on this instance." }
                p { class: "nf-hint",
                    "Following someone from another server? Try "
                    code { "@{username}@theirdomain" }
                    " in the Follow box."
                }
                button {
                    class: "btn btn-primary",
                    onclick: move |_| { nav.push(Route::Home {}); },
                    "Back to feed"
                }
            }
        }
    }
}
