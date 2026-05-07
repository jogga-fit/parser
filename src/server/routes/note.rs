use activitypub_federation::{axum::json::FederationJson, config::Data, traits::Object};
use axum::{
    Json,
    extract::Path,
    http::{
        HeaderMap, StatusCode,
        header::{ACCEPT, CONTENT_TYPE},
    },
    response::{IntoResponse, Redirect},
};
use serde_json::json;

use crate::{
    db::{DbError, queries::ObjectQueries},
    server::{error::AppError, impls::note::DbNote, state::AppState},
};

/// GET /notes/{id}
///
/// Content-negotiates:
/// - `Accept: application/activity+json` → return Note AP JSON
/// - Else → redirect to `/@{username}/notes/{id}`
pub async fn get_note(
    Path(b58): Path<String>,
    headers: HeaderMap,
    data: Data<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let accept = headers
        .get(ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let note_ap_id_suffix = format!("/notes/{b58}");
    let like_pattern = format!("%{note_ap_id_suffix}");
    let obj = sqlx::query!(
        r#"SELECT ap_id, attributed_to, visibility FROM objects WHERE ap_id LIKE ? AND object_type = 'Note'"#,
        like_pattern
    )
    .fetch_optional(&data.db)
    .await
    .map_err(DbError::Sqlx)?
    .ok_or(AppError::NotFound)?;

    if obj.visibility == "private" {
        return Err(AppError::Forbidden);
    }

    if accept.contains("activity+json") || accept.contains("ld+json") {
        let ap_id_url = obj.ap_id.parse().map_err(|_| AppError::NotFound)?;
        let db_note = DbNote::read_from_id(ap_id_url, &data)
            .await?
            .ok_or(AppError::NotFound)?;
        let note = db_note.into_json(&data).await?;
        return Ok(FederationJson(note).into_response());
    }

    let username = obj
        .attributed_to
        .rsplit('/')
        .next()
        .unwrap_or("unknown")
        .to_owned();
    let redirect_url = format!("/@{username}/notes/{b58}");
    Ok(Redirect::to(&redirect_url).into_response())
}

/// GET /notes/{id}/replies
pub async fn get_note_replies(
    Path(b58): Path<String>,
    data: Data<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let note_ap_id_suffix = format!("/notes/{b58}");
    let like_pattern = format!("%{note_ap_id_suffix}");
    let obj = sqlx::query!(
        r#"SELECT ap_id FROM objects WHERE ap_id LIKE ? AND object_type = 'Note'"#,
        like_pattern
    )
    .fetch_optional(&data.db)
    .await
    .map_err(DbError::Sqlx)?
    .ok_or(AppError::NotFound)?;

    let reply_ids = ObjectQueries::find_reply_ap_ids(&data.db, &obj.ap_id).await?;

    let scheme = data.app_data().config.instance.scheme();
    let domain = data.domain();
    let collection_id = format!("{scheme}://{domain}/notes/{b58}/replies");

    let body = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "OrderedCollection",
        "id": collection_id,
        "totalItems": reply_ids.len(),
        "orderedItems": reply_ids,
    });

    Ok((
        StatusCode::OK,
        [(CONTENT_TYPE, "application/activity+json")],
        Json(body),
    ))
}
