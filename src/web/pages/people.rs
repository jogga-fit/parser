use dioxus::prelude::*;

use crate::web::{
    FollowerItem, FollowingItem,
    app::{AppShell, Route},
    components::{Avatar, ErrorBanner},
    hooks::{is_auth_error, use_auth_guard},
    server_fns::{
        accept_follow_request, follow_person, get_followers, get_following, get_pending_followers,
        kick_follower, reject_follow_request, unfollow_actor,
    },
    sfn_msg, sleep_ms,
    state::AuthSignal,
};

#[component]
pub fn PeoplePage() -> Element {
    let auth = use_context::<AuthSignal>();
    let is_logged_in = auth.read().is_some();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let token2 = token.clone();
    let token4 = token.clone();
    let mut following = use_resource(move || {
        let t = token.clone();
        async move {
            if t.is_empty() {
                return Ok(vec![]);
            }
            get_following(t).await
        }
    });

    let mut followers = use_resource(move || {
        let t = token2.clone();
        async move {
            if t.is_empty() {
                return Ok(vec![]);
            }
            get_followers(t).await
        }
    });

    let mut pending = use_resource(move || {
        let t = token4.clone();
        async move {
            if t.is_empty() {
                return Ok(vec![]);
            }
            get_pending_followers(t).await
        }
    });

    // Only redirect to login when we expected auth to work and got an auth error.
    // Never redirect logged-out users — their resources return Ok(vec![]) immediately.
    use_auth_guard(move || {
        is_logged_in
            && (matches!(*following.read(), Some(Err(ref e)) if is_auth_error(e))
                || matches!(*followers.read(), Some(Err(ref e)) if is_auth_error(e)))
    });

    rsx! {
        AppShell {
            div { class: "page-content",
                h1 { class: "settings-title", "People" }

                if is_logged_in {
                    FollowCard {
                        token: auth.read().as_ref().map(|u| u.token.clone()).unwrap_or_default(),
                        on_success: move || following.restart(),
                    }
                }

                if is_logged_in {
                    {
                        let following_items = following.read().as_ref().and_then(|r| r.as_ref().ok()).cloned().unwrap_or_default();
                        let follower_items  = followers.read().as_ref().and_then(|r| r.as_ref().ok()).cloned().unwrap_or_default();
                        let pending_items   = pending.read().as_ref().and_then(|r| r.as_ref().ok()).cloned().unwrap_or_default();
                        let tok_fw = auth.read().as_ref().map(|u| u.token.clone()).unwrap_or_default();
                        rsx! {
                            ConnectionsCard {
                                token: tok_fw,
                                following_items,
                                follower_items,
                                pending_items,
                                on_following_change: move || following.restart(),
                                on_follower_change: move || {
                                    followers.restart();
                                    pending.restart();
                                },
                            }
                        }
                    }
                }

            }
        }
    }
}

#[component]
fn ConnectionsCard(
    token: String,
    following_items: Vec<FollowingItem>,
    follower_items: Vec<FollowerItem>,
    pending_items: Vec<FollowerItem>,
    on_following_change: EventHandler<()>,
    on_follower_change: EventHandler<()>,
) -> Element {
    let has_pending = !pending_items.is_empty();
    let mut active_tab: Signal<&'static str> = use_signal(|| "following");

    let n_following = following_items.len();
    let n_followers = follower_items.len();
    let n_pending = pending_items.len();

    rsx! {
        div { class: "card connections-card",
            div { class: "modal-header",
                div { class: "modal-tabs",
                    button {
                        class: if *active_tab.read() == "following" { "modal-tab modal-tab-active" } else { "modal-tab" },
                        onclick: move |_| active_tab.set("following"),
                        "Following"
                        span { class: "tab-count", "{n_following}" }
                    }
                    button {
                        class: if *active_tab.read() == "followers" { "modal-tab modal-tab-active" } else { "modal-tab" },
                        onclick: move |_| active_tab.set("followers"),
                        "Followers"
                        span { class: "tab-count", "{n_followers}" }
                    }
                    if has_pending {
                        button {
                            class: if *active_tab.read() == "requests" { "modal-tab modal-tab-active" } else { "modal-tab" },
                            onclick: move |_| active_tab.set("requests"),
                            "Requests"
                            span { class: "tab-count tab-count-accent", "{n_pending}" }
                        }
                    }
                }
            }
            div { class: "connections-list",
                match active_tab() {
                    "following" => rsx! {
                        FollowingTab {
                            token: token.clone(),
                            items: following_items.clone(),
                            on_change: move || on_following_change.call(()),
                        }
                    },
                    "followers" => rsx! {
                        FollowersTab {
                            token: token.clone(),
                            items: follower_items.clone(),
                            on_change: move || on_follower_change.call(()),
                        }
                    },
                    _ => rsx! {
                        RequestsTab {
                            token: token.clone(),
                            items: pending_items.clone(),
                            on_change: move || on_follower_change.call(()),
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn FollowingTab(token: String, items: Vec<FollowingItem>, on_change: EventHandler<()>) -> Element {
    let mut confirming = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);
    let nav = use_navigator();

    if items.is_empty() {
        return rsx! {
            div { class: "connections-empty", "Not following anyone yet." }
        };
    }

    rsx! {
        {items.iter().map(|item| {
            let display = item.display_name.as_deref().unwrap_or(&item.username).to_string();
            let ap_id = item.ap_id.clone();
            let ap_id_confirm = ap_id.clone();
            let ap_id_unfollow = ap_id.clone();
            let tok = token.clone();
            let username = item.username.clone();
            let domain = item.domain.clone();
            let is_local = item.is_local;
            let handle = if is_local { format!("@{username}") } else { format!("@{username}@{domain}") };
            let username_nav = username.clone();
            let is_confirming = confirming.read().as_deref() == Some(ap_id.as_str());
            let accepted = item.accepted;
            let avatar = item.avatar_url.clone();
            rsx! {
                div { class: "follow-list-item", key: "{ap_id}",
                    div {
                        class: if is_local { "follow-list-identity" } else { "follow-list-identity follow-list-identity-remote" },
                        onclick: move |_| {
                            if is_local {
                                nav.push(Route::UserProfile { username: format!("@{username_nav}") });
                            }
                        },
                        Avatar { url: avatar, name: display.clone(), size: "avatar-sm" }
                        div { class: "connection-info",
                            span { class: "connection-name", "{display}" }
                            span { class: "connection-handle", "{handle}" }
                        }
                    }
                    if !accepted {
                        span { class: "follow-badge pending", "pending" }
                    }
                    if is_confirming {
                        div { class: "unfollow-confirm",
                            span { class: "unfollow-confirm-text", "Unfollow?" }
                            button {
                                class: "btn btn-sm btn-danger",
                                disabled: *busy.read(),
                                onclick: move |_| {
                                    let t = tok.clone();
                                    let aid = ap_id_unfollow.clone();
                                    busy.set(true);
                                    spawn(async move {
                                        let _ = unfollow_actor(t, aid).await;
                                        busy.set(false);
                                        confirming.set(None);
                                        on_change.call(());
                                    });
                                },
                                if *busy.read() { "…" } else { "Confirm" }
                            }
                            button {
                                class: "btn btn-sm btn-ghost",
                                onclick: move |_| confirming.set(None),
                                "Cancel"
                            }
                        }
                    } else {
                        button {
                            class: "btn btn-sm btn-ghost unfollow-btn",
                            onclick: move |_| confirming.set(Some(ap_id_confirm.clone())),
                            "Unfollow"
                        }
                    }
                }
            }
        })}
    }
}

#[component]
fn FollowersTab(token: String, items: Vec<FollowerItem>, on_change: EventHandler<()>) -> Element {
    let nav = use_navigator();

    if items.is_empty() {
        return rsx! {
            div { class: "connections-empty", "No followers yet." }
        };
    }

    rsx! {
        {items.iter().map(|item| {
            let display = item.display_name.as_deref().unwrap_or(&item.username).to_string();
            let ap_id = item.ap_id.clone();
            let tok = token.clone();
            let username = item.username.clone();
            let domain = item.domain.clone();
            let is_local = item.is_local;
            let handle = if is_local { format!("@{username}") } else { format!("@{username}@{domain}") };
            let username_nav = username.clone();
            let avatar = item.avatar_url.clone();
            rsx! {
                div { class: "follow-list-item", key: "{ap_id}",
                    div {
                        class: if is_local { "follow-list-identity" } else { "follow-list-identity follow-list-identity-remote" },
                        onclick: move |_| {
                            if is_local {
                                nav.push(Route::UserProfile { username: format!("@{username_nav}") });
                            }
                        },
                        Avatar { url: avatar, name: display.clone(), size: "avatar-sm" }
                        div { class: "connection-info",
                            span { class: "connection-name", "{display}" }
                            span { class: "connection-handle", "{handle}" }
                        }
                    }
                    button {
                        class: "btn btn-sm btn-ghost remove-follower-btn",
                        title: "Remove follower",
                        onclick: move |_| {
                            let t = tok.clone();
                            let aid = ap_id.clone();
                            spawn(async move {
                                let _ = kick_follower(t, aid).await;
                                on_change.call(());
                            });
                        },
                        "✕"
                    }
                }
            }
        })}
    }
}

#[component]
fn RequestsTab(token: String, items: Vec<FollowerItem>, on_change: EventHandler<()>) -> Element {
    let nav = use_navigator();

    if items.is_empty() {
        return rsx! {
            div { class: "connections-empty", "No pending requests." }
        };
    }

    rsx! {
        {items.iter().map(|item| {
            let display = item.display_name.as_deref().unwrap_or(&item.username).to_string();
            let ap_id = item.ap_id.clone();
            let follow_ap_id = item.follow_ap_id.clone().unwrap_or_default();
            let tok_a = token.clone();
            let tok_r = token.clone();
            let ap_id_r = ap_id.clone();
            let follow_ap_id_r = follow_ap_id.clone();
            let username = item.username.clone();
            let domain = item.domain.clone();
            let is_local = item.is_local;
            let handle = if is_local { format!("@{username}") } else { format!("@{username}@{domain}") };
            let username_nav = username.clone();
            let avatar = item.avatar_url.clone();
            rsx! {
                div { class: "follow-list-item", key: "pending-{ap_id}",
                    div {
                        class: if is_local { "follow-list-identity" } else { "follow-list-identity follow-list-identity-remote" },
                        onclick: move |_| {
                            if is_local {
                                nav.push(Route::UserProfile { username: format!("@{username_nav}") });
                            }
                        },
                        Avatar { url: avatar, name: display.clone(), size: "avatar-sm" }
                        div { class: "connection-info",
                            span { class: "connection-name", "{display}" }
                            span { class: "connection-handle", "{handle}" }
                        }
                    }
                    div { class: "pending-actions",
                        button {
                            class: "btn btn-sm btn-accept",
                            title: "Accept",
                            onclick: move |_| {
                                let t = tok_a.clone();
                                let aid = ap_id.clone();
                                let fid = follow_ap_id.clone();
                                spawn(async move {
                                    let _ = accept_follow_request(t, aid, fid).await;
                                    on_change.call(());
                                });
                            },
                            "✓"
                        }
                        button {
                            class: "btn btn-sm btn-reject",
                            title: "Reject",
                            onclick: move |_| {
                                let t = tok_r.clone();
                                let aid = ap_id_r.clone();
                                let fid = follow_ap_id_r.clone();
                                spawn(async move {
                                    let _ = reject_follow_request(t, aid, fid).await;
                                    on_change.call(());
                                });
                            },
                            "✗"
                        }
                    }
                }
            }
        })}
    }
}

#[component]
fn FollowCard(token: String, on_success: EventHandler<()>) -> Element {
    let mut target = use_signal(String::new);
    let mut following_signal = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut done = use_signal(|| false);

    let on_follow = {
        let token = token.clone();
        move |_: Event<MouseData>| {
            let t = token.clone();
            let ap_id = target.read().trim().to_string();
            if ap_id.is_empty() {
                return;
            }
            following_signal.set(true);
            error.set(None);
            done.set(false);
            spawn(async move {
                match follow_person(t, ap_id).await {
                    Ok(()) => {
                        target.set(String::new());
                        done.set(true);
                        sleep_ms(1_200).await;
                        on_success.call(());
                    }
                    Err(e) => error.set(Some(sfn_msg(&e))),
                }
                following_signal.set(false);
            });
        }
    };

    rsx! {
        div { class: "card follow-card",
            div { class: "follow-card-header",
                i { class: "ph ph-magnifying-glass follow-card-icon" }
                span { class: "follow-card-title", "Follow someone" }
            }
            div { class: "follow-input-row",
                input {
                    class: "follow-input",
                    r#type: "text",
                    placeholder: "@username@instance.example  or  https://instance.example/users/username",
                    value: "{target}",
                    oninput: move |e| {
                        target.set(e.value());
                        done.set(false);
                        error.set(None);
                    },
                }
                button {
                    class: "btn btn-primary",
                    disabled: *following_signal.read() || target.read().trim().is_empty(),
                    onclick: on_follow,
                    if *following_signal.read() { "Following…" } else { "Follow" }
                }
            }
            if let Some(err) = error.read().as_ref() {
                ErrorBanner { message: err.clone() }
            }
            if *done.read() {
                div { class: "follow-success", "✓ Follow request sent!" }
            }
        }
    }
}
