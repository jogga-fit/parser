use axum::http::{HeaderMap, HeaderValue, header};

use crate::server::error::AppError;

/// Reject requests that don't advertise an ActivityPub-compatible Accept header.
pub fn require_ap_accept(headers: &HeaderMap) -> Result<(), AppError> {
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !accept.contains("activity+json") && !accept.contains("ld+json") {
        return Err(AppError::BadRequest(
            "requires Accept: application/activity+json".into(),
        ));
    }
    Ok(())
}

/// Build a `HeaderMap` with `Content-Type: application/activity+json`.
pub fn ap_json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/activity+json"),
    );
    headers
}
