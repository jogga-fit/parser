use activitypub_federation::{
    axum::inbox::{ActivityData, receive_activity},
    config::Data,
    protocol::context::WithContext,
};
use axum::{extract::Path, http::HeaderMap, http::StatusCode};
use tracing::debug;

use crate::{
    db::queries::ActorQueries,
    server::{
        error::AppError, impls::actor::DbActor, protocol::UserAcceptedActivities, state::AppState,
    },
};

/// POST /users/:username/inbox
#[tracing::instrument(skip_all, fields(username))]
pub async fn handle_inbox(
    Path(username): Path<String>,
    headers: HeaderMap,
    data: Data<AppState>,
    activity_data: ActivityData,
) -> Result<StatusCode, AppError> {
    if headers.get("signature").is_none() {
        return Err(AppError::Unauthorized);
    }

    ActorQueries::find_local_by_username(&data.db, &username)
        .await
        .map_err(|_| AppError::NotFound)?;

    debug!(username, "inbox: dispatching inbound activity");
    receive_activity::<WithContext<UserAcceptedActivities>, DbActor, AppState>(
        activity_data,
        &data,
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}

/// POST /inbox (shared inbox)
#[tracing::instrument(skip_all)]
pub async fn handle_shared_inbox(
    headers: HeaderMap,
    data: Data<AppState>,
    activity_data: ActivityData,
) -> Result<StatusCode, AppError> {
    if headers.get("signature").is_none() {
        return Err(AppError::Unauthorized);
    }

    debug!("shared_inbox: dispatching inbound activity");
    receive_activity::<WithContext<UserAcceptedActivities>, DbActor, AppState>(
        activity_data,
        &data,
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}
