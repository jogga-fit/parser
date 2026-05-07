//! Private helpers shared across service submodules.

use activitypub_federation::config::Data;
use uuid::Uuid;

use crate::db::queries::ActorQueries;
use crate::server::{error::AppError, impls::actor::DbActor, state::AppState};

pub(super) async fn fetch_local_actor(
    data: &Data<AppState>,
    actor_id: Uuid,
) -> Result<DbActor, AppError> {
    let row = ActorQueries::find_by_id(&data.db, actor_id).await?;
    Ok(DbActor { row })
}

pub(super) fn actor_inbox_url(
    actor: &crate::db::models::ActorRow,
) -> Result<url::Url, AppError> {
    actor
        .shared_inbox_url
        .as_deref()
        .unwrap_or(&actor.inbox_url)
        .parse()
        .map_err(AppError::from)
}
