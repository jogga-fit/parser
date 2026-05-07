use activitypub_federation::{
    config::Data,
    fetch::webfinger::{Webfinger, build_webfinger_response, extract_webfinger_name},
};
use axum::{Json, extract::Query};
use serde::Deserialize;

use crate::{
    db::queries::ActorQueries,
    server::{error::AppError, state::AppState},
};

#[derive(Deserialize)]
pub struct WebFingerQuery {
    resource: String,
}

pub async fn handle(
    Query(q): Query<WebFingerQuery>,
    data: Data<AppState>,
) -> Result<Json<Webfinger>, AppError> {
    let name = extract_webfinger_name(&q.resource, &data)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    let actor = ActorQueries::find_local_by_username(&data.db, name).await?;

    let scheme = data.app_data().config.instance.scheme();
    // jogga has no clubs — all actors are at /users/:username.
    let actor_id: url::Url = format!("{}://{}/users/{}", scheme, data.domain(), actor.username)
        .parse()
        .map_err(AppError::from)?;

    Ok(Json(build_webfinger_response(q.resource, actor_id)))
}
