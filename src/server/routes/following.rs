use activitypub_federation::config::Data;
use axum::{Json, extract::Path, http::HeaderMap, http::StatusCode, response::IntoResponse};
use serde_json::json;
use url::Url;

use crate::{
    db::queries::{ActorQueries, FollowQueries},
    server::{
        error::AppError,
        routes::helpers::{ap_json_headers, require_ap_accept},
        state::AppState,
    },
};

pub async fn get_following(
    Path(username): Path<String>,
    req_headers: HeaderMap,
    data: Data<AppState>,
) -> impl IntoResponse {
    require_ap_accept(&req_headers)?;

    let actor = ActorQueries::find_local_by_username(&data.db, &username).await?;
    let count = FollowQueries::count_following(&data.db, actor.id).await? as u64;
    let domain = data.domain();
    let scheme = data.app_data().config.instance.scheme();
    let url: Url = format!("{scheme}://{domain}/users/{username}/following").parse()?;

    Ok::<_, AppError>((
        StatusCode::OK,
        ap_json_headers(),
        Json(json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "OrderedCollection",
            "id": url,
            "totalItems": count,
        })),
    ))
}
