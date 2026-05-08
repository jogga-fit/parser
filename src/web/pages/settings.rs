use dioxus::prelude::*;

use crate::web::{
    MeResult,
    app::{AppShell, Route},
    components::{ErrorBanner, SettingToggle},
    hooks::{is_auth_error, use_auth_guard},
    server_fns::{
        add_alias, delete_account, get_me, move_account, remove_alias, set_privacy_settings,
        set_theme,
    },
    sfn_msg,
    state::{AuthSignal, ThemeSignal, clear_auth, save_theme},
};

#[component]
pub fn SettingsPage() -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let token_for_me = token.clone();
    let me = use_resource(move || {
        let t = token_for_me.clone();
        async move { get_me(t).await }
    });

    use_auth_guard(move || matches!(*me.read(), Some(Err(ref e)) if is_auth_error(e)));

    rsx! {
        AppShell {
            div { class: "page-content",
                h1 { class: "settings-title", "Settings" }
                match &*me.read() {
                    None => rsx! { div { class: "loading-spinner", "Loading…" } },
                    Some(Err(_)) => rsx! { ErrorBanner { message: "Could not load settings. Please try again.".to_string() } },
                    Some(Ok(profile)) => rsx! {
                        AppearanceSection { profile: profile.clone() }
                        PrivacySection { profile: profile.clone() }
                        IntegrationsSection {}
                        MigrationRow { profile: profile.clone() }

                        DangerZoneSection { username: profile.username.clone() }
                    },
                }
            }
        }
    }
}

#[component]
fn AppearanceSection(profile: MeResult) -> Element {
    let auth = use_context::<AuthSignal>();
    let mut theme_signal = use_context::<ThemeSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    // Start from the server-stored preference; fall back to the local signal value.
    let initial = {
        let sig = theme_signal.read().clone();
        if sig == "system" || sig == "dark" || sig == "light" {
            sig
        } else {
            profile.theme.clone()
        }
    };
    let mut current_pref = use_signal(|| initial);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let make_pick = move |pref: &'static str| {
        let t = token.clone();
        move |_: MouseEvent| {
            let prev = current_pref.read().clone();
            if pref == prev {
                return;
            }
            current_pref.set(pref.to_string());
            saving.set(true);
            error.set(None);
            let t2 = t.clone();
            spawn(async move {
                match set_theme(t2, pref.to_string()).await {
                    Ok(()) => {
                        save_theme(pref);
                        theme_signal.set(pref.to_string());
                    }
                    Err(e) => {
                        current_pref.set(prev);
                        error.set(Some(sfn_msg(&e)));
                    }
                }
                saving.set(false);
            });
        }
    };

    let pref = current_pref.read().clone();
    let disabled = *saving.read();

    rsx! {
        section { class: "settings-section",
            h2 { class: "settings-section-title", "Appearance" }
            p { class: "settings-section-desc", "Synced across all your devices." }

            div { class: "theme-picker",
                ThemeCard {
                    id: "system",
                    label: "System",
                    active: pref == "system",
                    disabled,
                    onclick: make_pick("system"),
                    div { class: "theme-preview theme-preview--system",
                        div { class: "theme-preview-topbar" }
                        div { class: "theme-preview-body",
                            div { class: "theme-preview-card" }
                            div { class: "theme-preview-card theme-preview-card--sm" }
                        }
                    }
                }
                ThemeCard {
                    id: "light",
                    label: "Light",
                    active: pref == "light",
                    disabled,
                    onclick: make_pick("light"),
                    div { class: "theme-preview theme-preview--light",
                        div { class: "theme-preview-topbar" }
                        div { class: "theme-preview-body",
                            div { class: "theme-preview-card" }
                            div { class: "theme-preview-card theme-preview-card--sm" }
                        }
                    }
                }
                ThemeCard {
                    id: "dark",
                    label: "Dark",
                    active: pref == "dark",
                    disabled,
                    onclick: make_pick("dark"),
                    div { class: "theme-preview theme-preview--dark",
                        div { class: "theme-preview-topbar" }
                        div { class: "theme-preview-body",
                            div { class: "theme-preview-card" }
                            div { class: "theme-preview-card theme-preview-card--sm" }
                        }
                    }
                }
            }

            if let Some(err) = error.read().as_ref() {
                ErrorBanner { message: err.clone() }
            }
        }
    }
}

#[component]
fn ThemeCard(
    id: &'static str,
    label: &'static str,
    active: bool,
    disabled: bool,
    onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            class: if active { "theme-card theme-card--active" } else { "theme-card" },
            "data-testid": "theme-option-{id}",
            "aria-pressed": if active { "true" } else { "false" },
            disabled,
            onclick: move |e| onclick.call(e),
            {children}
            span { class: "theme-card-label",
                if active {
                    i { class: "ph ph-check-circle", style: "color: var(--primary); margin-right: 4px;" }
                }
                "{label}"
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct Integration {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    icon: &'static str,
}

const INTEGRATIONS: &[Integration] = &[
    Integration {
        id: "amazfit",
        name: "Amazfit / Zepp",
        description: "Cloud sync via the Zepp Health API.",
        icon: "ph-watch",
    },
    Integration {
        id: "garmin",
        name: "Garmin Connect",
        description: "Cloud sync via OAuth and activity webhooks.",
        icon: "ph-watch",
    },
    Integration {
        id: "strava",
        name: "Strava",
        description: "Cloud sync via OAuth and webhooks.",
        icon: "ph-bicycle",
    },
    Integration {
        id: "apple-health",
        name: "Apple Health",
        description: "Import workouts and health metrics from Apple devices.",
        icon: "ph-apple-logo",
    },
    Integration {
        id: "wahoo",
        name: "Wahoo",
        description: "Sync rides and training sessions from Wahoo.",
        icon: "ph-lightning",
    },
];

#[component]
fn IntegrationsSection() -> Element {
    rsx! {
        section { class: "settings-section",
            h2 { class: "settings-section-title", "Integrations" }
            p { class: "settings-section-desc",
                "Cloud sync integrations are coming soon."
            }
            div { class: "integrations-grid",
                {INTEGRATIONS.iter().map(|i| {
                    rsx! { IntegrationCard { key: "{i.id}", integration: i.clone() } }
                })}
            }
        }
    }
}

#[component]
fn IntegrationCard(integration: Integration) -> Element {
    rsx! {
        div { class: "integration-card card",
            div { class: "integration-icon", i { class: "ph {integration.icon}" } }
            div { class: "integration-info",
                span { class: "integration-name", "{integration.name}" }
                span { class: "integration-desc", "{integration.description}" }
            }
            span { class: "badge-coming-soon", "Coming soon" }
        }
    }
}

#[component]
fn DangerZoneSection(username: String) -> Element {
    let mut auth = use_context::<AuthSignal>();
    let mut theme = use_context::<ThemeSignal>();
    let nav = use_navigator();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let mut confirming = use_signal(|| false);
    let mut confirm_input = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let on_delete = move |_: Event<MouseData>| {
        let t = token.clone();
        loading.set(true);
        error.set(None);
        spawn(async move {
            match delete_account(t).await {
                Ok(()) => {
                    clear_auth();
                    auth.set(None);
                    save_theme("system");
                    theme.set("system".to_string());
                    nav.push(Route::Login {});
                }
                Err(e) => {
                    error.set(Some(sfn_msg(&e)));
                    loading.set(false);
                }
            }
        });
    };

    rsx! {
        section { class: "settings-section settings-danger-zone",
            h2 { class: "settings-section-title", "Danger zone" }

            if !*confirming.read() {
                div { class: "danger-row",
                    div { class: "danger-info",
                        span { class: "danger-label", "Delete account" }
                        span { class: "danger-desc",
                            "Permanently delete your account and all your data. This cannot be undone."
                        }
                    }
                    button {
                        class: "btn btn-danger",
                        onclick: move |_| {
                            confirm_input.set(String::new());
                            error.set(None);
                            confirming.set(true);
                        },
                        "Delete account"
                    }
                }
            } else {
                div { class: "danger-confirm",
                    p { class: "danger-confirm-prompt",
                        "Type " strong { "@{username}" } " to confirm deletion."
                    }
                    div { class: "form-group",
                        input {
                            id: "delete-confirm",
                            r#type: "text",
                            placeholder: "@{username}",
                            autocomplete: "off",
                            value: "{confirm_input}",
                            oninput: move |e| confirm_input.set(e.value()),
                        }
                    }
                    if let Some(err) = error.read().as_ref() {
                        ErrorBanner { message: err.clone() }
                    }
                    div { class: "danger-confirm-actions",
                        button {
                            class: "btn btn-danger",
                            disabled: *loading.read() || *confirm_input.read() != format!("@{username}"),
                            onclick: on_delete,
                            if *loading.read() { "Deleting…" } else { "Permanently delete account" }
                        }
                        button {
                            class: "btn btn-ghost",
                            disabled: *loading.read(),
                            onclick: move |_| confirming.set(false),
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PrivacySection(profile: MeResult) -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let mut public_profile = use_signal(|| profile.public_profile);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let on_toggle_public = {
        let t = token.clone();
        move |_| {
            let next_pub = !*public_profile.read();
            public_profile.set(next_pub);
            saving.set(true);
            error.set(None);
            let t = t.clone();
            spawn(async move {
                if let Err(e) = set_privacy_settings(t, next_pub).await {
                    public_profile.set(!next_pub);
                    error.set(Some(sfn_msg(&e)));
                }
                saving.set(false);
            });
        }
    };

    rsx! {
        section { class: "settings-section",
            h2 { class: "settings-section-title", "Privacy" }
            p { class: "settings-section-desc", "Control who can see your profile and activity data." }

            SettingToggle {
                label: "Public profile".to_string(),
                description: "Anyone can view your profile and activities.".to_string(),
                checked: *public_profile.read(),
                disabled: *saving.read(),
                onchange: on_toggle_public,
            }

            if let Some(err) = error.read().as_ref() {
                ErrorBanner { message: err.clone() }
            }
        }
    }
}

#[component]
fn MigrationRow(profile: MeResult) -> Element {
    let mut modal_signal = use_context::<crate::web::state::MigrationModalSignal>();
    let migrated = profile.moved_to.clone();

    rsx! {
        section { class: "settings-section",
            div { class: "settings-migration-row",
                div { class: "settings-migration-info",
                    span { class: "settings-migration-label",
                        i { class: "ph ph-arrow-square-right" }
                        " Account migration"
                    }
                    if let Some(target) = migrated.as_ref() {
                        span { class: "settings-migration-desc", "Your account has been moved." }
                        div { class: "migration-moved-badge",
                            i { class: "ph ph-arrow-right" }
                            a {
                                href: "{target}",
                                target: "_blank",
                                rel: "noopener noreferrer",
                                "{target}"
                            }
                        }
                    } else {
                        span { class: "settings-migration-desc",
                            "Move your followers to another ActivityPub instance."
                        }
                    }
                }
                button {
                    class: "btn btn-secondary btn-sm",
                    "data-testid": "migration-manage-btn",
                    onclick: move |_| modal_signal.set(Some(profile.clone())),
                    if migrated.is_some() { "View" } else { "Manage" }
                }
            }
        }
    }
}

#[component]
pub fn MigrationModal(profile: MeResult, on_close: EventHandler<()>) -> Element {
    let auth = use_context::<AuthSignal>();
    let token = auth
        .read()
        .as_ref()
        .map(|u| u.token.clone())
        .unwrap_or_default();

    let migrated = profile.moved_to.clone();
    let aliases = use_signal(|| profile.also_known_as.clone());

    // Wizard step: 1 = add alias, 2 = move account
    let mut step = use_signal(|| {
        if profile.also_known_as.is_empty() {
            1u8
        } else {
            2u8
        }
    });

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| on_close.call(()),

            div {
                class: "migration-modal-card",
                onclick: move |e| e.stop_propagation(),

                div { class: "migration-modal-header",
                    span { class: "migration-modal-title",
                        i { class: "ph ph-arrow-square-right" }
                        " Account migration"
                    }
                    button {
                        class: "modal-close",
                        aria_label: "Close",
                        onclick: move |_| on_close.call(()),
                        i { class: "ph ph-x" }
                    }
                }

                div { class: "migration-modal-body", "data-testid": "migration-section",
                    if let Some(target) = migrated.as_ref() {
                        div { class: "migration-success", "data-testid": "migration-success",
                            span { class: "migration-success-title",
                                i { class: "ph ph-check-circle" }
                                " Account migrated"
                            }
                            p { "Your followers have been redirected to your new account." }
                            div { class: "migration-moved-badge", "data-testid": "migration-moved-to",
                                i { class: "ph ph-arrow-right" }
                                a {
                                    href: "{target}",
                                    target: "_blank",
                                    rel: "noopener noreferrer",
                                    "{target}"
                                }
                            }
                        }
                    } else {
                        // Wizard step indicators
                        div { class: "wizard-steps-bar",
                            div {
                                class: if *step.read() == 1 { "wizard-step active" } else { "wizard-step done" },
                                "data-testid": "migration-step-alias",
                                role: "button",
                                onclick: move |_| step.set(1),
                                div { class: "wizard-step-circle",
                                    if *step.read() > 1 {
                                        i { class: "ph ph-check" }
                                    } else {
                                        "1"
                                    }
                                }
                                span { class: "wizard-step-label", "Add alias" }
                            }
                            div { class: "wizard-step-connector" }
                            div {
                                class: if *step.read() == 2 { "wizard-step active" } else { "wizard-step inactive" },
                                "data-testid": "migration-step-move",
                                div { class: "wizard-step-circle", "2" }
                                span { class: "wizard-step-label", "Move account" }
                            }
                        }

                        if *step.read() == 1 {
                            AliasesSubsection {
                                token: token.clone(),
                                aliases,
                            }
                            div { class: "wizard-step-actions",
                                button {
                                    class: "btn btn-primary btn-sm",
                                    disabled: aliases.read().is_empty(),
                                    onclick: move |_| step.set(2),
                                    "Next: Move Account"
                                    i { class: "ph ph-arrow-right" }
                                }
                            }
                        } else {
                            MoveAccountSubsection {
                                token: token.clone(),
                                username: profile.username.clone(),
                                has_alias: !aliases.read().is_empty(),
                            }
                            div { class: "wizard-step-actions wizard-step-actions--back",
                                button {
                                    class: "btn btn-ghost btn-sm",
                                    onclick: move |_| step.set(1),
                                    i { class: "ph ph-arrow-left" }
                                    " Manage aliases"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AliasesSubsection(token: String, aliases: Signal<Vec<String>>) -> Element {
    let mut input = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let token_for_add = token.clone();
    let on_add = move |_: Event<MouseData>| {
        let new_alias = input.read().trim().to_string();
        if new_alias.is_empty() {
            return;
        }
        let t = token_for_add.clone();
        let mut aliases = aliases;
        busy.set(true);
        error.set(None);
        spawn(async move {
            match add_alias(t, new_alias.clone()).await {
                Ok(()) => {
                    aliases.write().push(new_alias);
                    input.set(String::new());
                }
                Err(e) => error.set(Some(sfn_msg(&e))),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "settings-subsection", "data-testid": "aliases-section",
            h3 { class: "settings-subsection-title", "This account's aliases" }
            p { class: "settings-section-desc",
                "List accounts on other instances that you own. Your new account must add "
                strong { "this" }
                " account as an alias there before migration will be accepted."
            }

            if aliases.read().is_empty() {
                p { class: "settings-empty", "data-testid": "aliases-empty", "No aliases yet." }
            } else {
                ul { class: "settings-list", "data-testid": "aliases-list",
                    for alias in aliases.read().iter().cloned() {
                        AliasRow {
                            key: "{alias}",
                            token: token.clone(),
                            alias: alias.clone(),
                            aliases: aliases,
                        }
                    }
                }
            }

            div { class: "form-group",
                label { class: "label", r#for: "alias-input", "Add alias" }
                input {
                    id: "alias-input",
                    "data-testid": "alias-input",
                    class: "input",
                    r#type: "text",
                    autocomplete: "off",
                    placeholder: "@user@other.example or https://other.example/users/me",
                    value: "{input}",
                    oninput: move |e| input.set(e.value()),
                }
            }
            if let Some(err) = error.read().as_ref() {
                ErrorBanner { message: err.clone() }
            }
            button {
                class: "btn btn-secondary btn-sm",
                "data-testid": "alias-add-btn",
                disabled: *busy.read() || input.read().trim().is_empty(),
                onclick: on_add,
                if *busy.read() { "Adding…" } else { "Add alias" }
            }
        }
    }
}

#[component]
fn AliasRow(token: String, alias: String, aliases: Signal<Vec<String>>) -> Element {
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let alias_for_remove = alias.clone();
    let on_remove = move |_: Event<MouseData>| {
        let t = token.clone();
        let a = alias_for_remove.clone();
        let mut aliases = aliases;
        busy.set(true);
        error.set(None);
        spawn(async move {
            match remove_alias(t, a.clone()).await {
                Ok(()) => {
                    aliases.write().retain(|x| x != &a);
                }
                Err(e) => {
                    error.set(Some(sfn_msg(&e)));
                    busy.set(false);
                }
            }
        });
    };

    rsx! {
        li { class: "settings-list-row", "data-testid": "alias-row",
            span { class: "settings-list-text", "{alias}" }
            button {
                class: "btn btn-ghost btn-sm",
                "data-testid": "alias-remove-btn",
                disabled: *busy.read(),
                onclick: on_remove,
                if *busy.read() { "Removing…" } else { "Remove" }
            }
            if let Some(err) = error.read().as_ref() {
                ErrorBanner { message: err.clone() }
            }
        }
    }
}

#[component]
fn MoveAccountSubsection(token: String, username: String, has_alias: bool) -> Element {
    let mut target = use_signal(String::new);
    let mut confirming = use_signal(|| false);
    let mut confirm_input = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut success = use_signal(|| false);

    let on_move = move |_: Event<MouseData>| {
        let t = token.clone();
        let dest = target.read().trim().to_string();
        busy.set(true);
        error.set(None);
        spawn(async move {
            match move_account(t, dest).await {
                Ok(()) => {
                    success.set(true);
                    confirming.set(false);
                }
                Err(e) => error.set(Some(sfn_msg(&e))),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "settings-subsection settings-danger-zone", "data-testid": "move-section",
            h3 { class: "settings-subsection-title", "Move this account" }
            p { class: "settings-section-desc",
                "Redirect your followers to another account on any ActivityPub instance. The destination must list this account in its aliases first. "
                strong { "Irreversible." }
            }

            if *success.read() {
                div { class: "migration-success", "data-testid": "move-success",
                    span { class: "migration-success-title",
                        i { class: "ph ph-paper-plane-right" }
                        " Move queued"
                    }
                    p { "Your followers are being redirected in the background." }
                }
            } else if !*confirming.read() {
                if !has_alias {
                    div { class: "migration-callout",
                        i { class: "ph ph-warning migration-callout-icon" }
                        span {
                            "Add at least one alias above first — the destination must list this account in its aliases."
                        }
                    }
                }
                div { class: "form-group",
                    label { class: "label", r#for: "move-target", "New account" }
                    input {
                        id: "move-target",
                        "data-testid": "move-target-input",
                        class: "input",
                        r#type: "text",
                        autocomplete: "off",
                        placeholder: "@user@new.example or https://new.example/users/me",
                        value: "{target}",
                        disabled: !has_alias,
                        oninput: move |e| target.set(e.value()),
                    }
                }
                if let Some(err) = error.read().as_ref() {
                    ErrorBanner { message: err.clone() }
                }
                button {
                    class: "btn btn-danger",
                    "data-testid": "move-start-btn",
                    disabled: !has_alias || target.read().trim().is_empty(),
                    onclick: move |_| {
                        confirm_input.set(String::new());
                        error.set(None);
                        confirming.set(true);
                    },
                    "Move account"
                }
            } else {
                div { class: "danger-confirm", "data-testid": "move-confirm-dialog",
                    p { class: "danger-confirm-prompt",
                        "Type " strong { "@{username}" } " to confirm. This cannot be undone."
                    }
                    div { class: "form-group",
                        input {
                            id: "move-confirm",
                            "data-testid": "move-confirm-input",
                            class: "input",
                            r#type: "text",
                            placeholder: "@{username}",
                            autocomplete: "off",
                            value: "{confirm_input}",
                            oninput: move |e| confirm_input.set(e.value()),
                        }
                    }
                    if let Some(err) = error.read().as_ref() {
                        ErrorBanner { message: err.clone() }
                    }
                    div { class: "danger-confirm-actions",
                        button {
                            class: "btn btn-danger",
                            "data-testid": "move-confirm-btn",
                            disabled: *busy.read() || *confirm_input.read() != format!("@{username}"),
                            onclick: on_move,
                            if *busy.read() { "Moving…" } else { "Confirm move" }
                        }
                        button {
                            class: "btn btn-ghost",
                            "data-testid": "move-cancel-btn",
                            disabled: *busy.read(),
                            onclick: move |_| confirming.set(false),
                            "Cancel"
                        }
                    }
                }
            }
        }
    }
}
