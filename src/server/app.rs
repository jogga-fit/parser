use activitypub_federation::config::{FederationConfig, FederationMiddleware};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{Method, StatusCode},
    routing::{get, post},
};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::server::{
    routes::{
        actor, api, exercise, followers, following, health, inbox, nodeinfo, note, outbox,
        webfinger,
    },
    state::AppState,
};

const INBOX_BODY_LIMIT: usize = 1024 * 1024;       // 1 MB
const UPLOAD_BODY_LIMIT: usize = 10 * 1024 * 1024; // 10 MB
const DEFAULT_BODY_LIMIT: usize = 5 * 1024 * 1024; // 5 MB

/// Build the Axum router wrapped in the federation middleware.
pub async fn build_router(
    state: AppState,
) -> Result<(Router, FederationConfig<AppState>), Box<dyn std::error::Error + Send + Sync>> {
    let domain = state.config.instance.domain.clone();
    let debug_flag = cfg!(debug_assertions);

    let federation_config = FederationConfig::builder()
        .domain(domain)
        .app_data(state)
        .debug(debug_flag)
        .build()
        .await?;

    let router = Router::new()
        // ActivityPub / federation endpoints
        .route("/.well-known/webfinger", get(webfinger::handle))
        .route("/.well-known/nodeinfo", get(nodeinfo::well_known))
        .route("/nodeinfo/2.1", get(nodeinfo::nodeinfo))
        .route("/users/{username}", get(actor::get_actor))
        .route("/users/{username}/followers", get(followers::get_followers))
        .route("/users/{username}/following", get(following::get_following))
        .route("/users/{username}/outbox", get(outbox::get_outbox))
        .route(
            "/users/{username}/inbox",
            post(inbox::handle_inbox).route_layer(DefaultBodyLimit::max(INBOX_BODY_LIMIT)),
        )
        .route(
            "/inbox",
            post(inbox::handle_shared_inbox).route_layer(DefaultBodyLimit::max(INBOX_BODY_LIMIT)),
        )
        // Note AP endpoints
        .route("/notes/{id}", get(note::get_note))
        .route("/notes/{id}/replies", get(note::get_note_replies))
        // Exercise AP + GeoJSON endpoints
        .route("/exercises/{id}", get(exercise::get_exercise))
        .route(
            "/api/exercises/{id}/route",
            get(exercise::get_exercise_route),
        )
        .route(
            "/api/exercises/{id}/stats",
            get(exercise::get_exercise_stats),
        )
        // Health check — used by mobile app when adding a server
        .route("/api/health", get(health::health))
        // Write API — authentication
        .route("/api/v1/accounts/token", post(api::token))
        .route(
            "/api/v1/accounts/me",
            get(api::get_me)
                .patch(api::update_me)
                .delete(api::delete_me),
        )
        .route("/api/v1/accounts/privacy", post(api::update_privacy))
        .route("/api/v1/accounts/{username}", get(api::get_account))
        // Read API — followers / following
        .route("/api/v1/accounts/me/followers", get(api::list_my_followers))
        .route("/api/v1/accounts/me/following", get(api::list_my_following))
        // Write API — follow / unfollow
        .route("/api/v1/follows", post(api::follow))
        .route("/api/v1/follow", post(api::follow))
        .route("/api/v1/unfollow", post(api::unfollow))
        // Write API — account migration
        .route("/api/v1/accounts/me/move", post(api::move_account))
        .route(
            "/api/v1/accounts/me/aliases",
            post(api::add_alias).delete(api::remove_alias),
        )
        // Write API — password reset
        .route(
            "/api/v1/accounts/password-reset/init",
            post(api::password_reset_init),
        )
        .route(
            "/api/v1/accounts/password-reset/verify",
            post(api::password_reset_verify),
        )
        // Write API — GPX/FIT upload
        .route(
            "/api/exercises/upload",
            post(api::upload_exercise).route_layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        // Write API — likes
        .route("/api/v1/likes", post(api::like))
        .route("/api/v1/unlikes", post(api::unlike))
        // Write API — notifications
        .route(
            "/api/v1/notifications",
            get(api::list_notifications).post(api::mark_notifications_read),
        )
        // Middleware
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT))
        .layer(FederationMiddleware::new(federation_config.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(30),
        ))
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::DELETE,
                    Method::PUT,
                    Method::PATCH,
                ])
                .allow_headers(Any),
        );

    Ok((router, federation_config))
}
