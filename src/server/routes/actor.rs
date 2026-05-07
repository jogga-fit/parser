use activitypub_federation::{
    axum::json::FederationJson, config::Data, protocol::context::WithContext, traits::Object,
};
use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
};

use crate::{
    db::queries::ActorQueries,
    server::{error::AppError, impls::actor::DbActor, state::AppState},
};

/// GET /users/{username}
///
/// Content negotiates: AP clients get the JSON Person, browsers get a redirect
/// to `/@{username}`. Non-owner usernames return 404 regardless of Accept.
pub async fn get_actor(
    Path(username): Path<String>,
    headers: HeaderMap,
    data: Data<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let accept = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // For browser requests, redirect to the handle URL — but only if this actor
    // actually exists locally. Non-owner usernames always return 404.
    if !accept.contains("activity+json") && !accept.contains("ld+json") {
        // Verify the actor is local before redirecting (single-owner invariant).
        ActorQueries::find_local_by_username(&data.db, &username).await?;
        return Ok(Redirect::temporary(&format!("/@{username}")).into_response());
    }

    let row = ActorQueries::find_local_by_username(&data.db, &username).await?;
    let db_actor = DbActor { row };
    let person = db_actor.into_json(&data).await?;
    let _ = StatusCode::OK;
    Ok(FederationJson(WithContext::new_default(person)).into_response())
}

