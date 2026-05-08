use dioxus::prelude::*;

use crate::web::{
    MeResult, NotificationItem,
    browser::mark_document_hydrated,
    components::ErrorBanner,
    hooks::use_client_only,
    pages::{
        clubs::{ClubPage, ClubsPage},
        credits::CreditsPage,
        home::HomePage,
        login::LoginPage,
        people::PeoplePage,
        post_detail::{ExerciseDetailPage, PostDetailPage},
        profile::ProfilePage,
        register::RegisterPage,
        reset_password::ResetPasswordPage,
        settings::{MigrationModal, SettingsPage},
    },
    server_fns::{
        dismiss_notification, get_notifications, get_owner_username, get_theme,
        get_unread_notification_count, mark_all_notifications_read,
    },
    state::{
        AuthSignal, MigrationModalSignal, ThemeSignal, clear_auth, load_auth, load_theme,
        save_theme,
    },
};

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[route("/")]
    Index {},
    #[route("/login")]
    Login {},
    #[route("/register")]
    Register {},
    #[route("/reset-password?:query")]
    ResetPassword { query: String },
    #[route("/home")]
    Home {},
    #[route("/profile")]
    Profile {},
    #[route("/people")]
    People {},
    #[route("/clubs")]
    Clubs {},
    #[route("/clubs/:handle")]
    ClubDetail { handle: String },
    #[route("/settings")]
    Settings {},
    // Post detail pages — must come before the /:username catch-all.
    #[route("/:username/notes/:note_id")]
    NoteDetail { username: String, note_id: String },
    #[route("/:username/exercises/:exercise_id")]
    ExerciseDetail {
        username: String,
        exercise_id: String,
    },
    #[route("/credits")]
    Credits {},
    // Must be last — catch-all for /@username profile pages.
    // Dioxus router can't parse a literal @ prefix in a dynamic segment,
    // so we use /:username and validate the @ in the component.
    #[route("/:username")]
    UserProfile { username: String },
    // Catch-all for any path that didn't match above.
    #[route("/:..segments")]
    PageNotFound { segments: Vec<String> },
}

/// Top-level app component. Provides auth context and the router.
#[component]
pub fn App() -> Element {
    let auth: AuthSignal = use_signal(load_auth);
    use_context_provider(|| auth);
    let mut theme: ThemeSignal = use_signal(load_theme);
    use_context_provider(|| theme);
    let migration_modal: MigrationModalSignal = use_signal(|| None::<MeResult>);
    use_context_provider(|| migration_modal);

    // Apply the theme attribute to <html> whenever the preference changes.
    // "system" resolves via prefers-color-scheme; also installs a media-query
    // listener so the theme tracks OS changes live.
    use_effect(move || {
        let pref = theme.read().clone();
        spawn(async move {
            if pref == "system" {
                let js = r#"
                    (function() {
                        var mq = window.matchMedia('(prefers-color-scheme: dark)');
                        document.documentElement.setAttribute('data-theme', mq.matches ? 'dark' : 'light');
                        if (window._joggaThemeHandler) {
                            mq.removeEventListener('change', window._joggaThemeHandler);
                        }
                        window._joggaThemeHandler = function(e) {
                            document.documentElement.setAttribute('data-theme', e.matches ? 'dark' : 'light');
                        };
                        mq.addEventListener('change', window._joggaThemeHandler);
                        window._joggaThemeMQ = mq;
                    })();
                "#;
                let _ = document::eval(js).await;
            } else {
                // Remove any system listener and apply explicit theme.
                let js = format!(
                    r#"
                    (function() {{
                        if (window._joggaThemeMQ && window._joggaThemeHandler) {{
                            window._joggaThemeMQ.removeEventListener('change', window._joggaThemeHandler);
                            window._joggaThemeMQ = null;
                            window._joggaThemeHandler = null;
                        }}
                        document.documentElement.setAttribute('data-theme', '{pref}');
                    }})();
                "#
                );
                let _ = document::eval(&js).await;
            }
        });
    });

    // Sync theme from server whenever auth changes (login / session restore).
    use_effect(move || {
        if let Some(u) = auth.read().clone() {
            let tok = u.token.clone();
            spawn(async move {
                if let Ok(t) = get_theme(tok).await {
                    save_theme(&t);
                    theme.set(t);
                }
            });
        }
    });

    // Mark the document as hydrated so e2e tests can wait for WASM to be ready.
    use_effect(move || {
        let _ = mark_document_hydrated();
    });

    rsx! {
        document::Link { rel: "preconnect", href: "https://fonts.googleapis.com" }
        document::Link { rel: "preconnect", href: "https://fonts.gstatic.com", crossorigin: "anonymous" }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=DM+Sans:ital,opsz,wght@0,9..40,300;0,9..40,400;0,9..40,500;0,9..40,600;0,9..40,700;1,9..40,400&family=Geist+Mono:wght@400;500&display=swap"
        }
        document::Link { rel: "stylesheet", href: asset!("/assets/main.css") }
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/@phosphor-icons/web@2.1.1/src/regular/style.css" }
        document::Link { rel: "stylesheet", href: "https://cdn.jsdelivr.net/npm/@phosphor-icons/web@2.1.1/src/fill/style.css" }
        SuspenseBoundary {
            fallback: |_| rsx! { div { class: "loading", "Loading…" } },
            Router::<Route> {}
        }
    }
}

#[component]
fn Index() -> Element {
    let auth = use_context::<AuthSignal>();
    let nav = use_navigator();

    // use_resource resolves on the server (streaming SSR), so `owner` is
    // populated in the SSR pass and serialised for the client.
    let owner = use_resource(|| async { get_owner_username().await.ok() });

    // Client-side redirect: logged-in → Home, logged-out → owner profile.
    // Effects never run on SSR, so no use_client_only guard needed.
    use_effect(move || {
        if auth.read().is_some() {
            nav.push(Route::Home {});
            return;
        }
        if let Some(Some(ref username)) = *owner.read() {
            nav.push(Route::UserProfile {
                username: format!("@{username}"),
            });
        }
    });

    // SSR and initial WASM render: stream the owner's profile page directly
    // instead of a loading spinner. Owner is always available server-side.
    match owner.read().clone() {
        Some(Some(username)) => rsx! { ProfilePage { username } },
        _ => rsx! { div { class: "loading", "Loading…" } },
    }
}

#[component]
fn Login() -> Element {
    rsx! { LoginPage {} }
}

#[component]
fn Register() -> Element {
    rsx! { RegisterPage {} }
}

#[component]
fn ResetPassword(query: String) -> Element {
    rsx! { ResetPasswordPage { query } }
}

#[component]
fn Home() -> Element {
    let auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    let ready = use_client_only();
    use_effect(move || {
        if *ready.read() && auth.read().is_none() {
            nav.push(Route::Index {});
        }
    });
    if !*ready.read() {
        return rsx! { div { class: "loading", "Loading…" } };
    }
    rsx! { HomePage {} }
}

#[component]
fn Profile() -> Element {
    let auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    use_effect(move || {
        if let Some(u) = auth.read().as_ref() {
            nav.push(Route::UserProfile {
                username: format!("@{}", u.username),
            });
        } else {
            nav.push(Route::Login {});
        }
    });
    rsx! { div { class: "loading", "Loading…" } }
}

#[component]
fn People() -> Element {
    let auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    let ready = use_client_only();
    use_effect(move || {
        if *ready.read() && auth.read().is_none() {
            nav.push(Route::Index {});
        }
    });
    if !*ready.read() {
        return rsx! { div { class: "loading", "Loading…" } };
    }
    rsx! { PeoplePage {} }
}

#[component]
fn Clubs() -> Element {
    let auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    let ready = use_client_only();
    use_effect(move || {
        if *ready.read() && auth.read().is_none() {
            nav.push(Route::Index {});
        }
    });
    if !*ready.read() {
        return rsx! { div { class: "loading", "Loading…" } };
    }
    rsx! { ClubsPage {} }
}

#[component]
fn ClubDetail(handle: String) -> Element {
    let auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    let ready = use_client_only();
    use_effect(move || {
        if *ready.read() && auth.read().is_none() {
            nav.push(Route::Index {});
        }
    });
    if !*ready.read() {
        return rsx! { div { class: "loading", "Loading…" } };
    }
    rsx! { ClubPage { handle } }
}

#[component]
fn Settings() -> Element {
    let auth = use_context::<AuthSignal>();
    let nav = use_navigator();
    let ready = use_client_only();
    use_effect(move || {
        if *ready.read() && auth.read().is_none() {
            nav.push(Route::Login {});
        }
    });
    if !*ready.read() {
        return rsx! { div { class: "loading", "Loading…" } };
    }
    rsx! { SettingsPage {} }
}

#[component]
fn NoteDetail(username: String, note_id: String) -> Element {
    // Only handle /@username paths.
    if username.strip_prefix('@').is_some() {
        rsx! { PostDetailPage { object_ap_id: note_id } }
    } else {
        rsx! {
            div { class: "page-content",
                ErrorBanner { message: format!("Page not found: /{username}/notes/{note_id}") }
            }
        }
    }
}

#[component]
fn ExerciseDetail(username: String, exercise_id: String) -> Element {
    if username.starts_with('@') {
        rsx! { ExerciseDetailPage { object_ap_id: exercise_id } }
    } else {
        rsx! {
            div { class: "page-content",
                ErrorBanner { message: format!("Page not found: /{username}/exercises/{exercise_id}") }
            }
        }
    }
}

#[component]
fn UserProfile(username: String) -> Element {
    // Strip leading @ so both /@alice and /alice work.
    // ProfilePage handles not-found via its own DNF component.
    let u = username.trim_start_matches('@').to_string();
    rsx! { ProfilePage { key: "{u}", username: u } }
}

#[component]
fn PageNotFound(segments: Vec<String>) -> Element {
    let nav = use_navigator();
    let owner = use_resource(|| async { get_owner_username().await.ok() });
    rsx! {
        div { class: "not-found-page",
            div { class: "not-found-blob-wrap",
                div { class: "nf-blob nf-blob-a" }
                div { class: "nf-blob nf-blob-b" }
            }
            div { class: "not-found-card",
                div { class: "nf-illustration",
                    i { class: "ph ph-question nf-runner" }
                    span { class: "nf-arrow", "←" }
                    i { class: "ph ph-flag-checkered nf-flag" }
                }
                p { class: "nf-label", "did not qualify" }
                h1 { class: "nf-title", "DNQ" }
                p { class: "nf-desc", "We don't know what you are looking for." }
                if let Some(Some(u)) = owner.read().as_ref() {
                    p { class: "nf-hint", "This is a dedicated server for "
                        code { "@{u}" }
                        "."
                    }
                }
                button {
                    class: "btn btn-primary",
                    onclick: move |_| { nav.push(Route::Index {}); },
                    "Back to home"
                }
            }
        }
    }
}

#[component]
fn Credits() -> Element {
    rsx! { CreditsPage {} }
}

#[component]
pub fn AppShell(children: Element) -> Element {
    let mut auth = use_context::<AuthSignal>();
    let mut theme = use_context::<ThemeSignal>();
    let mut migration_modal = use_context::<MigrationModalSignal>();
    let ready = use_client_only();
    let nav = use_navigator();

    // Suppress hydration mismatch: username is "" on SSR and first WASM render,
    // then fills in after hydration.
    let username = if *ready.read() {
        auth.read()
            .as_ref()
            .map(|u| u.username.clone())
            .unwrap_or_default()
    } else {
        String::new()
    };

    let is_authed = *ready.read() && auth.read().is_some();

    let avatar_initial = username
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string());

    // Notification state — hoisted so sidebar bell and mobile header bell share it.
    let mut notif_open: Signal<bool> = use_signal(|| false);
    let mut notifications: Signal<Vec<NotificationItem>> = use_signal(Vec::new);
    let mut notif_loading: Signal<bool> = use_signal(|| false);
    let mut unread_count: Signal<usize> = use_signal(|| 0);

    // Account sheet state (mobile header avatar tap).
    let mut acct_open: Signal<bool> = use_signal(|| false);

    use_effect(move || {
        if is_authed {
            let t = auth
                .read()
                .as_ref()
                .map(|u| u.token.clone())
                .unwrap_or_default();
            spawn(async move {
                if let Ok(n) = get_unread_notification_count(t).await {
                    unread_count.set(n as usize);
                }
            });
        }
    });

    // Toggle body.has-modal so the sidebar compositor layer is lowered (z-index: 0)
    // when the modal is open, allowing the fixed backdrop (z-index: 9001) to paint
    // above it in Chrome's GPU compositing order.
    use_effect(move || {
        let open = migration_modal.read().is_some();
        spawn(async move {
            let js = if open {
                "document.body.classList.add('has-modal')"
            } else {
                "document.body.classList.remove('has-modal')"
            };
            let _ = document::eval(js).await;
        });
    });

    rsx! {
        div { class: "app-shell",
            nav { class: "sidebar",
                div { class: "sidebar-brand",
                    i { class: "ph ph-person-simple-run nav-icon" }
                    span { class: "brand-name", "Jogga:" }
                }

                if is_authed {
                    Link { class: "nav-item", active_class: "active", to: Route::Home {},
                        i { class: "ph ph-house nav-icon" }
                        span { class: "nav-label", "Feed" }
                    }
                    Link { class: "nav-item", active_class: "active", to: Route::People {},
                        i { class: "ph ph-users nav-icon" }
                        span { class: "nav-label", "People" }
                    }
                    Link { class: "nav-item", active_class: "active", to: Route::Clubs {},
                        i { class: "ph ph-users-three nav-icon" }
                        span { class: "nav-label", "Clubs" }
                    }
                }

                if is_authed {
                    Link { class: "nav-item", active_class: "active", to: Route::UserProfile { username: format!("@{username}") },
                        i { class: "ph ph-user nav-icon" }
                        span { class: "nav-label", "Profile" }
                    }
                }

                div { class: "sidebar-spacer" }

                if is_authed {
                    NotificationBell { notif_open, notifications, notif_loading, own_username: username.clone(), unread_count }
                    Link { class: "nav-item", active_class: "active", to: Route::Settings {},
                        i { class: "ph ph-gear nav-icon" }
                        span { class: "nav-label", "Settings" }
                    }
                    button {
                        class: "nav-item sign-out-btn",
                        onclick: move |_| {
                            clear_auth();
                            auth.set(None);
                            save_theme("system");
                            theme.set("system".to_string());
                            nav.push(Route::Login {});
                        },
                        i { class: "ph ph-sign-out nav-icon" }
                        span { class: "nav-label", "Sign out" }
                    }
                } else {
                    Link { class: "nav-item", to: Route::Login {},
                        i { class: "ph ph-sign-in nav-icon" }
                        span { class: "nav-label", "Sign in" }
                    }
                }
            }

            header { class: "mobile-header",
                div { class: "mobile-header-brand",
                    i { class: "ph ph-person-simple-run" }
                    span { "Jogga:" }
                }
                div { class: "mobile-header-actions",
                    if is_authed {
                        button {
                            class: "mobile-header-btn",
                            onclick: move |_| {
                                acct_open.set(false);
                                let next = !*notif_open.read();
                                notif_open.set(next);
                                if next && notifications.read().is_empty() {
                                    let t = auth.read().as_ref().map(|u| u.token.clone()).unwrap_or_default();
                                    notif_loading.set(true);
                                    spawn(async move {
                                        if let Ok(items) = get_notifications(t.clone()).await {
                                            notifications.set(items);
                                        }
                                        notif_loading.set(false);
                                        let _ = mark_all_notifications_read(t).await;
                                        unread_count.set(0);
                                    });
                                }
                            },
                            i { class: "ph ph-bell" }
                            if *unread_count.read() > 0 {
                                span { class: "mobile-header-badge",
                                    if *unread_count.read() > 99 { "99+" } else { "{unread_count}" }
                                }
                            }
                        }
                        button {
                            class: if *acct_open.read() { "mobile-header-avatar mobile-header-avatar-open" } else { "mobile-header-avatar" },
                            onclick: move |_| {
                                notif_open.set(false);
                                let next = !*acct_open.read();
                                acct_open.set(next);
                            },
                            "{avatar_initial}"
                        }
                    } else {
                        Link { class: "mobile-header-signin", to: Route::Login {},
                            i { class: "ph ph-sign-in" }
                            span { "Sign in" }
                        }
                    }
                }
            }

            main { class: "main-content", {children} }

            if is_authed && *notif_open.read() {
                div { class: "notif-sheet-backdrop", onclick: move |_| notif_open.set(false) }
                div { class: "notif-sheet",
                    div { class: "notif-sheet-handle" }
                    div { class: "notif-header",
                        span { class: "notif-title", "Notifications" }
                        button { class: "notif-close", onclick: move |_| notif_open.set(false), "×" }
                    }
                    div { class: "notif-sheet-body",
                        NotifItemList {
                            notifications,
                            notif_loading,
                            token: auth.read().as_ref().map(|u| u.token.clone()).unwrap_or_default(),
                            own_username: username.clone(),
                            notif_open,
                        }
                    }
                }
            }

            if is_authed && *acct_open.read() {
                div { class: "notif-sheet-backdrop", onclick: move |_| acct_open.set(false) }
                div { class: "notif-sheet",
                    div { class: "notif-sheet-handle" }
                    div { class: "acct-sheet-user",
                        div { class: "acct-sheet-avatar", "{avatar_initial}" }
                        div {
                            div { class: "acct-sheet-name", "@{username}" }
                        }
                    }
                    div { class: "acct-sheet-nav",
                        Link {
                            class: "acct-sheet-item",
                            to: Route::UserProfile { username: format!("@{username}") },
                            onclick: move |_| acct_open.set(false),
                            i { class: "ph ph-user" }
                            span { "Profile" }
                            i { class: "ph ph-caret-right acct-sheet-arrow" }
                        }
                        div { class: "acct-sheet-divider" }
                        Link {
                            class: "acct-sheet-item",
                            to: Route::Settings {},
                            onclick: move |_| acct_open.set(false),
                            i { class: "ph ph-gear" }
                            span { "Settings" }
                            i { class: "ph ph-caret-right acct-sheet-arrow" }
                        }
                        div { class: "acct-sheet-divider" }
                        button {
                            class: "acct-sheet-item acct-sheet-signout",
                            onclick: move |_| {
                                acct_open.set(false);
                                clear_auth();
                                auth.set(None);
                                save_theme("system");
                                theme.set("system".to_string());
                                nav.push(Route::Login {});
                            },
                            i { class: "ph ph-sign-out" }
                            span { "Sign out" }
                        }
                    }
                }
            }

            if is_authed {
                nav { class: "bottom-nav",
                    Link { class: "bottom-nav-item", active_class: "active", to: Route::Home {},
                        i { class: "ph ph-house nav-icon" }
                        span { class: "nav-label", "Feed" }
                    }
                    Link { class: "bottom-nav-item", active_class: "active", to: Route::People {},
                        i { class: "ph ph-users nav-icon" }
                        span { class: "nav-label", "People" }
                    }
                    Link { class: "bottom-nav-item", active_class: "active", to: Route::Clubs {},
                        i { class: "ph ph-users-three nav-icon" }
                        span { class: "nav-label", "Clubs" }
                    }
                }
            }

            FediverseBadge {}
        }

        // Migration modal — rendered outside the grid so the fixed backdrop
        // covers the sidebar without compositing layer conflicts.
        if let Some(profile) = migration_modal.read().clone() {
            MigrationModal {
                profile,
                on_close: move |_| migration_modal.set(None),
            }
        }
    }
}

#[component]
fn FediverseBadge() -> Element {
    rsx! {
        a {
            class: "fediverse-badge",
            href: "https://github.com/jogga-fit/core",
            target: "_blank",
            rel: "noopener noreferrer",
            "Built with ❤️ on the fediverse"
        }
    }
}

#[component]
fn NotificationBell(
    mut notif_open: Signal<bool>,
    notifications: Signal<Vec<NotificationItem>>,
    mut notif_loading: Signal<bool>,
    own_username: String,
    mut unread_count: Signal<usize>,
) -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    // Fetch (and mark read) whenever the panel is opened.
    {
        let tok = token.clone();
        let mut notifications = notifications;
        use_effect(move || {
            let is_open = *notif_open.read();
            if is_open {
                let t = tok.clone();
                notif_loading.set(true);
                spawn(async move {
                    if let Ok(items) = get_notifications(t.clone()).await {
                        notifications.set(items);
                    }
                    notif_loading.set(false);
                    let _ = mark_all_notifications_read(t).await;
                    unread_count.set(0);
                });
            }
        });
    }

    let bell_class = if *unread_count.read() > 0 {
        "nav-item notif-bell notif-bell-unread"
    } else {
        "nav-item notif-bell"
    };

    rsx! {
        // The entire section (button + inline list) is a flex-column block inside the sidebar.
        div { class: "notif-section",
            button {
                class: bell_class,
                onclick: move |_| {
                    let next = !*notif_open.read();
                    notif_open.set(next);
                },
                title: "Notifications",
                i { class: "ph ph-bell nav-icon" }
                span { class: "nav-label", "Notifications" }
                if *unread_count.read() > 0 {
                    span { class: "notif-badge",
                        if *unread_count.read() > 99 { "99+" } else { "{unread_count}" }
                    }
                }
            }

            // Inline list — only on desktop (CSS hides on mobile).
            if *notif_open.read() {
                div { class: "notif-inline-list",
                    NotifItemList { notifications, notif_loading, token: token.clone(), own_username: own_username.clone(), notif_open }
                }
            }
        }
    }
}

#[component]
fn NotifItemList(
    notifications: Signal<Vec<NotificationItem>>,
    notif_loading: Signal<bool>,
    token: String,
    own_username: String,
    mut notif_open: Signal<bool>,
) -> Element {
    let mut notifications = notifications;
    let nav = use_navigator();

    rsx! {
        if *notif_loading.read() {
            div { class: "notif-empty", "Loading…" }
        } else if notifications.read().is_empty() {
            div { class: "notif-empty",
                i { class: "ph ph-bell" }
                span { "No notifications yet" }
            }
        } else {
            {
                let items: Vec<NotificationItem> = notifications.read().clone();
                items.into_iter().map(|n| {
                    let nid = n.id.clone();
                    let tok_dismiss = token.clone();
                    let initial = n.from_username.chars().next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_else(|| "?".into());
                    let avatar = n.from_avatar_url.clone();
                    let name = n.from_display_name.as_deref()
                        .unwrap_or(&n.from_username)
                        .to_owned();
                    let text = notification_text(&n);
                    let ts = relative_time(&n.created_at);
                    let row_class = if n.is_read {
                        "notif-item notif-item-clickable"
                    } else {
                        "notif-item notif-item-unread notif-item-clickable"
                    };
                    // Resolve nav target from notification kind.
                    let dest: Route = match n.kind.as_str() {
                        "follow_request" => Route::People {},
                        "new_follower" | "follow_accepted" => {
                            Route::UserProfile { username: format!("@{}", n.from_username) }
                        }
                        "reply" | "like" => {
                            // Navigate to the parent post thread.
                            if let Some(ap_id) = n.object_ap_id.as_deref() {
                                let segments: Vec<&str> = ap_id.trim_end_matches('/').split('/').collect();
                                let object_id = segments.last().unwrap_or(&"").to_string();
                                let is_note = segments.len() >= 2
                                    && *segments.get(segments.len() - 2).unwrap_or(&"") == "notes";
                                if is_note {
                                    Route::NoteDetail {
                                        username: format!("@{own_username}"),
                                        note_id: object_id,
                                    }
                                } else {
                                    Route::ExerciseDetail {
                                        username: format!("@{own_username}"),
                                        exercise_id: object_id,
                                    }
                                }
                            } else {
                                Route::UserProfile { username: format!("@{own_username}") }
                            }
                        }
                        _ => Route::UserProfile { username: format!("@{own_username}") },
                    };
                    rsx! {
                        div { key: "{nid}", class: row_class,
                            onclick: move |_| {
                                notif_open.set(false);
                                nav.push(dest.clone());
                            },
                            if let Some(url) = avatar.as_ref() {
                                img { class: "avatar avatar-sm avatar-img notif-avatar", src: "{url}", alt: "{name}" }
                            } else {
                                div { class: "avatar avatar-sm notif-avatar", "{initial}" }
                            }
                            div { class: "notif-body",
                                span { class: "notif-name", "{name}" }
                                span { class: "notif-text", " {text}" }
                                span { class: "notif-ts", "{ts}" }
                            }
                            button {
                                class: "notif-dismiss",
                                title: "Dismiss",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    let id = nid.clone();
                                    let t = tok_dismiss.clone();
                                    notifications.write().retain(|n| n.id != id);
                                    spawn(async move {
                                        let _ = dismiss_notification(t, id).await;
                                    });
                                },
                                "×"
                            }
                        }
                    }
                })
            }
        }
    }
}

fn notification_text(n: &NotificationItem) -> String {
    match n.kind.as_str() {
        "follow_request" => "sent you a follow request".into(),
        "new_follower" => "started following you".into(),
        "follow_accepted" => "accepted your follow request".into(),
        "like" => {
            let label = n.object_title.as_deref().unwrap_or("your post");
            format!("liked {label}")
        }
        "reply" => "replied to your post".into(),
        other => format!("({other})"),
    }
}

/// Format an ISO-8601 timestamp as a short absolute date/time.
fn relative_time(iso: &str) -> String {
    // Format: 2026-04-14T12:34:56+00:00  or  2026-04-14T12:34:56Z
    let trimmed = iso.trim_end_matches('Z').split('+').next().unwrap_or(iso);
    let parts: Vec<&str> = trimmed.splitn(2, 'T').collect();
    if parts.len() == 2 {
        let date = parts[0];
        let time = parts[1].get(..5).unwrap_or(parts[1]);
        format!("{date} {time}")
    } else {
        iso.get(..16).unwrap_or(iso).to_owned()
    }
}
