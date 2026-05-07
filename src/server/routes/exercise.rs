use activitypub_federation::{axum::json::FederationJson, config::Data, traits::Object};
use axum::{
    Json,
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde_json::{Value, json};

use crate::{
    db::queries::{ExerciseQueries, FollowQueries},
    server::{
        auth::OptionalAuth, error::AppError, id, impls::exercise::DbExercise,
        protocol::context::WithFedisportContext, state::AppState,
    },
};

/// GET /exercises/{id}
pub async fn get_exercise(
    Path(b58): Path<String>,
    headers: HeaderMap,
    data: Data<AppState>,
) -> Result<
    FederationJson<WithFedisportContext<crate::server::protocol::exercise::Exercise>>,
    AppError,
> {
    let accept = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !accept.contains("activity+json") && !accept.contains("ld+json") {
        return Err(AppError::NotAcceptable);
    }

    let id = id::decode(&b58)?;
    let db_exercise = DbExercise::find_by_id(id, &data)
        .await?
        .ok_or(AppError::NotFound)?;

    if db_exercise.row.visibility != "public" {
        return Err(AppError::NotFound);
    }

    let exercise = db_exercise.into_json(&data).await?;
    Ok(FederationJson(WithFedisportContext::new(exercise)))
}

/// GET /api/exercises/{id}/route
pub async fn get_exercise_route(
    Path(b58): Path<String>,
    OptionalAuth(maybe_auth): OptionalAuth,
    data: Data<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let id = id::decode(&b58)?;
    let row = ExerciseQueries::find_with_route(&data.db, id).await?;

    match row.visibility.as_str() {
        "public" => {}
        "followers" => {
            let auth = maybe_auth.ok_or(AppError::NotFound)?;
            let follows = FollowQueries::is_following(&data.db, auth.actor.id, row.actor_id)
                .await
                .map_err(AppError::from)?;
            if !follows && auth.actor.id != row.actor_id {
                return Err(AppError::NotFound);
            }
        }
        _ => {
            let auth = maybe_auth.ok_or(AppError::NotFound)?;
            if auth.actor.id != row.actor_id {
                return Err(AppError::NotFound);
            }
        }
    }

    let route = row.route.ok_or(AppError::NotFound)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::header::HeaderValue::from_static("application/geo+json"),
    );

    Ok((StatusCode::OK, headers, Json(route)))
}

/// GET /api/exercises/{id}/stats
pub async fn get_exercise_stats(
    Path(b58): Path<String>,
    OptionalAuth(maybe_auth): OptionalAuth,
    data: Data<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let id = id::decode(&b58)?;
    let row = ExerciseQueries::find_metadata_by_id(&data.db, id).await?;

    match row.visibility.as_str() {
        "public" => {}
        "followers" => {
            let auth = maybe_auth.ok_or(AppError::NotFound)?;
            let follows = FollowQueries::is_following(&data.db, auth.actor.id, row.actor_id)
                .await
                .map_err(AppError::from)?;
            if !follows && auth.actor.id != row.actor_id {
                return Err(AppError::NotFound);
            }
        }
        _ => {
            let auth = maybe_auth.ok_or(AppError::NotFound)?;
            if auth.actor.id != row.actor_id {
                return Err(AppError::NotFound);
            }
        }
    }

    let hidden: std::collections::HashSet<&str> =
        row.hidden_stats.iter().map(|s| s.as_str()).collect();

    let mut stats = serde_json::Map::new();

    macro_rules! emit {
        ($field:expr, $key:literal, $hide_key:literal) => {
            if !hidden.contains($hide_key) {
                if let Some(v) = $field {
                    stats.insert($key.to_string(), json!(v));
                }
            }
        };
    }

    emit!(Some(row.distance_m), "distance", "distance_m");
    emit!(Some(row.duration_s), "duration", "duration_s");
    emit!(row.elevation_gain_m, "elevationGain", "elevation_gain_m");
    emit!(row.device.as_deref(), "device", "device");
    emit!(row.avg_pace_s_per_km, "avgPace", "avg_pace_s_per_km");
    emit!(row.avg_heart_rate_bpm, "avgHeartRate", "avg_heart_rate_bpm");
    emit!(row.max_heart_rate_bpm, "maxHeartRate", "max_heart_rate_bpm");
    emit!(row.avg_power_w, "avgPower", "avg_power_w");
    emit!(row.max_power_w, "maxPower", "max_power_w");
    emit!(
        row.normalized_power_w,
        "normalizedPower",
        "normalized_power_w"
    );
    emit!(row.avg_cadence_rpm, "avgCadence", "avg_cadence_rpm");

    Ok(Json(Value::Object(stats)))
}
