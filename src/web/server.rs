use std::{path::Path, sync::OnceLock};

use crate::server::auth::generate_placeholder_password;

pub use activitypub_federation::{
    config::FederationConfig,
    fetch::object_id::ObjectId,
    kinds::activity::{CreateType, DeleteType, UpdateType},
};
use axum::{Router, extract::DefaultBodyLimit};
pub use bs58;
pub use chrono::Utc;
use dioxus::prelude::{DioxusRouterExt, ServeConfig};
pub use sqlx;
use tower_http::{compression::CompressionLayer, timeout::TimeoutLayer, trace::TraceLayer};
use tracing_subscriber::{EnvFilter, fmt};
pub use uuid::Uuid;

pub use crate::{
    db::queries::{
        AccountQueries, ActivityQueries, ActorQueries, ExerciseQueries, FollowQueries, LikeQueries,
        MediaAttachmentQueries, NotificationQueries, ObjectQueries, activity::NewActivity,
        object::NewObject,
    },
    server::state::AppState,
    server::{
        impls::actor::DbActor,
        protocol::{
            accept::Accept,
            create::Create,
            delete::{Delete, DeleteObject},
            follow::Follow,
            reject::Reject,
            update::Update,
        },
    },
};

static FEDERATION_CONFIG: OnceLock<FederationConfig<AppState>> = OnceLock::new();

#[allow(dead_code)]
pub(crate) fn request_data() -> activitypub_federation::config::Data<AppState> {
    FEDERATION_CONFIG
        .get()
        .expect("not initialized")
        .to_request_data()
}

pub async fn run_server(config_path: Option<&Path>) {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let config =
        crate::server::config::AppConfig::load(config_path).expect("failed to load config");

    let state = crate::server::state::AppState::new(&config)
        .await
        .expect("failed to build AppState");

    // Auto-seed owner on first boot if config specifies one and no local actors exist.
    let local_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM actors WHERE is_local = 1")
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);
    if local_count == 0 && !config.owner.username.is_empty() {
        let initial_password = generate_placeholder_password();
        match crate::server::service::seed_owner(
            &state.db,
            &config.owner.username,
            &initial_password,
            &config.instance.domain,
            config.instance.scheme(),
            &config.owner.contact,
        )
        .await
        {
            Ok(()) => {
                if config.owner.contact.is_empty() {
                    // No contact configured — log the generated password so the
                    // admin can use it to sign in and change it immediately.
                    tracing::warn!(
                        username = %config.owner.username,
                        password = %initial_password,
                        "First boot: owner account seeded with generated password. \
                         Sign in and change it immediately."
                    );
                } else {
                    tracing::warn!(
                        username = %config.owner.username,
                        contact = %config.owner.contact,
                        "First boot: owner account seeded. Sending password-reset OTP."
                    );
                    match crate::server::service::do_password_reset_init(
                        &state,
                        &config.owner.contact,
                    )
                    .await
                    {
                        Ok(_) => tracing::warn!(
                            contact = %config.owner.contact,
                            "First boot: password-reset OTP sent. Follow the link to set your password."
                        ),
                        Err(e) => tracing::warn!(
                            error = %e,
                            contact = %config.owner.contact,
                            password = %initial_password,
                            "First boot: OTP delivery failed. Use the generated password above to sign in."
                        ),
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "Auto-seed skipped"),
        }
    }

    let (ap_router, fed_config) = crate::server::app::build_router(state)
        .await
        .expect("failed to build AP router");

    FEDERATION_CONFIG.set(fed_config.clone()).ok();

    tokio::spawn(crate::server::delivery::run_delivery_worker(fed_config));

    // The Dioxus SSR router MUST use `DefaultBodyLimit::disable()` because
    // `serve_dioxus_application` installs a catch-all handler for server functions
    // that may receive chunked/multipart bodies. Axum's DefaultBodyLimit is not
    // negotiated per-route by the Dioxus integration, so any global limit here
    // would silently reject valid server-function calls.
    //
    // Upload size enforcement is handled at two layers:
    //   1. The `/api/exercises/upload` route in `ap_router` applies a 10 MB
    //      `DefaultBodyLimit::max` *before* this Dioxus catch-all is reached.
    //   2. The reverse proxy (nginx / Cloudflare) enforces an outer request size
    //      limit (typically 10–50 MB) for all inbound traffic.
    let dx_router: axum::Router<()> = Router::new()
        .serve_dioxus_application(
            ServeConfig::new().enable_out_of_order_streaming(),
            crate::web::app::App,
        )
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(30),
        ))
        .layer(DefaultBodyLimit::disable());

    let full_router = axum::Router::new().merge(ap_router).merge(dx_router);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind failed");

    tracing::info!(addr = %addr, "Jogga: (fedisport) listening");
    axum::serve(listener, full_router.into_make_service())
        .await
        .expect("serve error");
}
