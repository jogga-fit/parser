use dioxus::prelude::*;

use crate::web::{
    FollowingItem,
    app::{AppShell, Route},
    components::ErrorBanner,
    hooks::use_auth_guard,
    pages::feed_card::FeedCard,
    server_fns::{follow_actor, get_club_feed, list_joined_clubs, unfollow_actor},
    sfn_msg, sleep_ms,
    state::AuthSignal,
};

#[component]
pub fn ClubsPage() -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let mut clubs = use_resource(move || {
        let t = token.clone();
        async move { list_joined_clubs(t).await }
    });

    use_auth_guard(
        move || matches!(*clubs.read(), Some(Err(ref e)) if crate::web::hooks::is_auth_error(e)),
    );

    let auth2 = auth;

    rsx! {
        AppShell {
            div { class: "page-content",
                div { class: "clubs-header",
                    h1 { class: "settings-title", "Clubs" }
                }

                FindClubCard {
                    token: auth2.read().as_ref().map(|u| u.token.clone()).unwrap_or_default(),
                    on_joined: move || clubs.restart(),
                }

                div { "data-testid": "clubs",
                match clubs.read().as_ref() {
                    None => rsx! { div { class: "loading-spinner", "Loading clubs…" } },
                    Some(Err(e)) => rsx! { ErrorBanner { message: sfn_msg(e) } },
                    Some(Ok(items)) if items.is_empty() => rsx! {
                        div { class: "empty-state",
                            div { class: "empty-icon", i { class: "ph ph-users-three" } }
                            h3 { "No clubs yet" }
                            p {
                                "Join remote clubs by handle using the form above."
                            }
                        }
                    },
                    Some(Ok(items)) => rsx! {
                        div { class: "clubs-grid",
                            {items.iter().map(|club| {
                                rsx! {
                                    ClubCard {
                                        key: "{club.ap_id}",
                                        club: club.clone(),
                                        token: auth2.read().as_ref().map(|u| u.token.clone()).unwrap_or_default(),
                                        on_change: move || clubs.restart(),
                                    }
                                }
                            })}
                        }
                    },
                }
                } // end data-testid="clubs"
            }
        }
    }
}

#[component]
fn FindClubCard(token: String, on_joined: EventHandler<()>) -> Element {
    let mut target = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut done = use_signal(|| false);

    let on_join = move |_: Event<MouseData>| {
        let t = token.clone();
        let handle = target.read().trim().to_string();
        if handle.is_empty() {
            return;
        }
        busy.set(true);
        error.set(None);
        done.set(false);
        spawn(async move {
            match follow_actor(t, handle).await {
                Ok(()) => {
                    target.set(String::new());
                    done.set(true);
                    sleep_ms(1_200).await;
                    done.set(false);
                    on_joined.call(());
                }
                Err(e) => error.set(Some(sfn_msg(&e))),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "card follow-card",
            div { class: "follow-card-header",
                i { class: "ph ph-magnifying-glass follow-card-icon" }
                span { class: "follow-card-title", "Find a club on another instance" }
            }
            div { class: "follow-input-row",
                input {
                    class: "follow-input",
                    r#type: "text",
                    placeholder: "@clubname@instance.example",
                    value: "{target}",
                    oninput: move |e| {
                        target.set(e.value());
                        done.set(false);
                        error.set(None);
                    },
                }
                button {
                    class: "btn btn-primary",
                    disabled: *busy.read() || target.read().trim().is_empty(),
                    onclick: on_join,
                    if *busy.read() { "Joining…" } else { "Find & join" }
                }
            }
            if let Some(err) = error.read().as_ref() {
                ErrorBanner { message: err.clone() }
            }
            if *done.read() {
                div { class: "follow-success", "✓ Join request sent!" }
            }
        }
    }
}

#[component]
fn ClubCard(club: FollowingItem, token: String, on_change: EventHandler<()>) -> Element {
    let nav = use_navigator();
    let handle = club.username.clone();
    let domain = club.domain.clone();
    let display = club
        .display_name
        .as_deref()
        .unwrap_or(&club.username)
        .to_string();
    let initial = display
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());

    let handle_at = if domain.is_empty() {
        format!("@{handle}")
    } else {
        format!("@{handle}@{domain}")
    };
    let remote_club_url = if domain.is_empty() {
        None
    } else {
        Some(format!("https://{domain}/clubs/{handle}"))
    };

    let ap_id = club.ap_id.clone();
    let route_handle = format!("{handle}@{domain}");

    rsx! {
        div { class: "card club-card",
            div { class: "club-card-header",
                div { class: "avatar avatar-md club-avatar", "{initial}" }
                div { class: "club-card-info",
                    button {
                        class: "club-name-btn",
                        onclick: move |_| { nav.push(Route::ClubDetail { handle: route_handle.clone() }); },
                        "{display}"
                    }
                    div { class: "club-meta",
                        if let Some(url) = remote_club_url {
                            a {
                                class: "connection-handle",
                                href: "{url}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "{handle_at}"
                            }
                        } else {
                            span { class: "connection-handle", "{handle_at}" }
                        }
                        if club.accepted {
                            span { class: "club-badge club-badge-open", "Member" }
                        } else {
                            span { class: "club-badge club-badge-exclusive", "Pending" }
                        }
                    }
                }
            }

            div { class: "club-card-actions",
                LeaveButton {
                    token: token.clone(),
                    ap_id: ap_id.clone(),
                    accepted: club.accepted,
                    on_change: move || on_change.call(()),
                }
            }
        }
    }
}

#[component]
fn LeaveButton(
    token: String,
    ap_id: String,
    accepted: bool,
    on_change: EventHandler<()>,
) -> Element {
    let mut busy = use_signal(|| false);
    let mut confirming = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let leave_label = if accepted { "Leave" } else { "Cancel request" };
    let busy_label = if accepted {
        "Leaving…"
    } else {
        "Cancelling…"
    };
    let role_label = if accepted { "Member" } else { "Pending" };

    rsx! {
        div { class: "club-leave-group",
            if *confirming.read() {
                div { class: "unfollow-confirm",
                    span { class: "unfollow-confirm-text", "{leave_label}?" }
                    button {
                        class: "btn btn-sm btn-reject",
                        disabled: *busy.read(),
                        onclick: move |_| {
                            let t = token.clone();
                            let id = ap_id.clone();
                            busy.set(true);
                            error.set(None);
                            spawn(async move {
                                match unfollow_actor(t, id).await {
                                    Ok(()) => on_change.call(()),
                                    Err(e) => error.set(Some(sfn_msg(&e))),
                                }
                                busy.set(false);
                                confirming.set(false);
                            });
                        },
                        if *busy.read() { "{busy_label}" } else { "Yes, leave" }
                    }
                    button {
                        class: "btn btn-sm btn-ghost",
                        onclick: move |_| confirming.set(false),
                        "Cancel"
                    }
                }
            } else {
                button {
                    class: "btn btn-ghost btn-sm club-leave-btn",
                    onclick: move |_| confirming.set(true),
                    span { "{role_label}" }
                    i { class: "ph ph-x club-leave-icon" }
                }
            }
            if let Some(err) = error.read().as_ref() {
                span { class: "club-join-error", "{err}" }
            }
        }
    }
}

#[component]
pub fn ClubPage(handle: String) -> Element {
    let auth = use_context::<AuthSignal>();

    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let handle_clone = handle.clone();
    let club_res = use_resource(move || {
        let t = token.clone();
        let h = handle_clone.clone();
        async move {
            list_joined_clubs(t).await.map(|items| {
                items.into_iter().find(|c| {
                    let full = format!("{}@{}", c.username, c.domain);
                    full == h || c.username == h
                })
            })
        }
    });

    let nav = use_navigator();

    rsx! {
        AppShell {
            div { class: "page-content",
                div { class: "breadcrumb",
                    button {
                        class: "breadcrumb-back",
                        onclick: move |_| { nav.push(Route::Clubs {}); },
                        i { class: "ph ph-arrow-left" }
                        " Clubs"
                    }
                }
                match club_res.read().as_ref() {
                    None => rsx! { div { class: "loading-spinner", "Loading…" } },
                    Some(Err(e)) => rsx! { ErrorBanner { message: sfn_msg(e) } },
                    Some(Ok(None)) => rsx! {
                        div { class: "not-found-card",
                            h1 { "Not a member" }
                            p { "You have not joined @{handle}." }
                            p {
                                "Use the "
                                button {
                                    class: "link-btn",
                                    onclick: move |_| { nav.push(Route::Clubs {}); },
                                    "Clubs"
                                }
                                " page to find and join clubs."
                            }
                        }
                    },
                    Some(Ok(Some(club))) => rsx! {
                        ClubDetailCard {
                            club: club.clone(),
                            token: auth.read().as_ref().map(|u| u.token.clone()).unwrap_or_default(),
                            on_change: move || { nav.push(Route::Clubs {}); },
                        }
                        ClubFeed {
                            token: auth.read().as_ref().map(|u| u.token.clone()).unwrap_or_default(),
                            club: club.clone(),
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn ClubFeed(token: String, club: FollowingItem) -> Element {
    let ap_id = club.ap_id.clone();
    let token_clone = token.clone();
    let posts = use_resource(move || {
        let t = token_clone.clone();
        let id = ap_id.clone();
        async move { get_club_feed(t, id).await }
    });

    let remote_url = if !club.domain.is_empty() {
        // Build a link to the club on its home instance.
        // Fedisport uses /clubs/<username> routing.
        Some(format!("https://{}/clubs/{}", club.domain, club.username))
    } else {
        None
    };

    rsx! {
        div { class: "club-feed",
            match posts.read().as_ref() {
                None => rsx! { div { class: "loading-spinner", "Loading posts…" } },
                Some(Err(e)) => rsx! { ErrorBanner { message: sfn_msg(e) } },
                Some(Ok(items)) if items.is_empty() => rsx! {
                    div { class: "empty-state",
                        p { "No posts received from this club yet." }
                    }
                },
                Some(Ok(items)) => rsx! {
                    {items.iter().map(|item| rsx! {
                        FeedCard {
                            key: "{item.id}",
                            item: item.clone(),
                            token: Some(token.clone()),
                        }
                    })}
                },
            }
            if let Some(url) = remote_url {
                if !matches!(posts.read().as_ref(), None) {
                    p { class: "club-feed-remote-notice",
                        "Showing only posts received by this server. "
                        a {
                            href: "{url}",
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "View all posts on {club.domain}"
                            i { class: "ph ph-arrow-square-out", style: "margin-left: 4px; font-size: 0.85em;" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ClubDetailCard(club: FollowingItem, token: String, on_change: EventHandler<()>) -> Element {
    let display = club
        .display_name
        .as_deref()
        .unwrap_or(&club.username)
        .to_string();
    let initial = display
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into());
    let handle_at = if club.domain.is_empty() {
        format!("@{}", club.username)
    } else {
        format!("@{}@{}", club.username, club.domain)
    };
    let remote_club_url = if club.domain.is_empty() {
        None
    } else {
        Some(format!("https://{}/clubs/{}", club.domain, club.username))
    };

    rsx! {
        div { class: "card profile-card",
            div { class: "profile-header-row",
                div { class: "avatar avatar-lg club-avatar", "{initial}" }
                div { class: "profile-header-actions",
                    LeaveButton {
                        token: token.clone(),
                        ap_id: club.ap_id.clone(),
                        accepted: club.accepted,
                        on_change: move || on_change.call(()),
                    }
                }
            }
            div { class: "profile-body",
                h2 { class: "profile-name", "{display}" }
                if let Some(url) = remote_club_url {
                    a {
                        class: "profile-handle",
                        href: "{url}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        "{handle_at}"
                    }
                } else {
                    p { class: "profile-handle", "{handle_at}" }
                }
                div { class: "profile-stats",
                    if club.accepted {
                        span { class: "club-badge club-badge-open", "Member" }
                    } else {
                        span { class: "club-badge club-badge-exclusive", "Pending approval" }
                    }
                }
            }
        }
    }
}
