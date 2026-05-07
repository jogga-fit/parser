use activitypub_federation::config::Data;
use axum::{
    Json,
    extract::{Path, Query},
    http::HeaderMap,
    http::StatusCode,
    response::IntoResponse,
};
use http::{
    HeaderValue,
    header::{ACCEPT, CONTENT_TYPE},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    db::queries::{ActivityQueries, ActorQueries},
    server::{error::AppError, state::AppState},
};

#[derive(Deserialize)]
pub struct OutboxQuery {
    page: Option<bool>,
    min_id: Option<uuid::Uuid>,
}

pub async fn get_outbox(
    Path(username): Path<String>,
    Query(q): Query<OutboxQuery>,
    req_headers: HeaderMap,
    data: Data<AppState>,
) -> impl IntoResponse {
    let accept = req_headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !accept.contains("activity+json") && !accept.contains("ld+json") {
        return Err(AppError::BadRequest(
            "requires Accept: application/activity+json".into(),
        ));
    }

    let actor = ActorQueries::find_local_by_username(&data.db, &username).await?;
    let outbox_url = format!("{}/outbox", actor.ap_id);
    let total = ActivityQueries::count_outbox(&data.db, actor.id).await? as u64;

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/activity+json"),
    );

    let body = if q.page.unwrap_or(false) {
        let before_time = if let Some(min_id) = q.min_id {
            Some(ActivityQueries::outbox_cursor_time(&data.db, actor.id, min_id).await?)
        } else {
            None
        };
        let activities = ActivityQueries::get_outbox(&data.db, actor.id, 20, before_time).await?;
        let items: Vec<_> = activities.into_iter().map(|a| a.ap_json.0).collect();
        json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "OrderedCollectionPage",
            "id": format!("{outbox_url}?page=true"),
            "partOf": outbox_url,
            "orderedItems": items,
        })
    } else {
        json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "OrderedCollection",
            "id": outbox_url,
            "totalItems": total,
            "first": if total > 0 { Some(format!("{outbox_url}?page=true")) } else { None },
        })
    };

    Ok((StatusCode::OK, headers, Json(body)))
}
