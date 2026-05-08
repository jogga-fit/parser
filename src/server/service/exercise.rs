//! Exercise / content service functions: upload, update post, announce.

use activitypub_federation::{config::Data, kinds::activity::UpdateType};
use chrono::Utc;
use serde_json::json;
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::queries::{
    ActivityQueries, AnnounceQueries, DeliveryQueries, ExerciseQueries, FollowQueries,
    MediaAttachmentQueries, ObjectQueries,
    activity::NewActivity,
    exercise::NewExercise,
    object::NewObject,
};
use crate::server::{
    error::{AppError, InternalError},
protocol::{context::FEDISPORT_CONTEXT, update::Update},
    state::AppState,
};

use super::helpers::fetch_local_actor;

pub struct ExerciseUploadRequest {
    pub activity_type: String,
    pub file_bytes: Vec<u8>,
    pub file_type: String,
    pub file_name: String,
    pub visibility: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image_urls: Vec<String>,
    pub hidden_stats: Vec<String>,
}

pub struct ExerciseUploadResult {
    pub id: Uuid,
    pub ap_id: String,
}

pub const VALID_VISIBILITY: &[&str] = &["public", "unlisted", "followers", "private"];

/// Allowed activity types — must exactly match the DB CHECK constraint in exercises table:
/// `activity_type IN ('run', 'ride', 'swim', 'walk', 'hike')`
pub const VALID_ACTIVITY_TYPES: &[&str] = &["run", "ride", "swim", "walk", "hike"];

pub(super) fn is_valid_activity_type(s: &str) -> bool {
    VALID_ACTIVITY_TYPES.contains(&s)
}

#[tracing::instrument(skip(state, req, actor), fields(username = actor.username))]
pub async fn do_upload_exercise(
    state: &AppState,
    actor: &crate::db::models::ActorRow,
    req: ExerciseUploadRequest,
) -> Result<ExerciseUploadResult, AppError> {
    use parser::ParsedActivity;

    if !is_valid_activity_type(&req.activity_type) {
        return Err(AppError::BadRequest(format!(
            "invalid activityType: must be one of {}",
            VALID_ACTIVITY_TYPES.join(", ")
        )));
    }
    if !VALID_VISIBILITY.contains(&req.visibility.as_str()) {
        return Err(AppError::BadRequest(format!(
            "invalid visibility: must be one of {}",
            VALID_VISIBILITY.join(", ")
        )));
    }

    let file_bytes = req.file_bytes;
    let parsed = if req.file_type == "fit" {
        let bytes = file_bytes.clone();
        tokio::task::spawn_blocking(move || ParsedActivity::from_fit(&bytes))
            .await
            .map_err(|e| AppError::Internal(InternalError::Unexpected(e.to_string())))?
            .map_err(|_| AppError::BadRequest("FIT file could not be parsed".into()))?
    } else {
        ParsedActivity::from_gpx(&file_bytes).map_err(|_| {
            AppError::BadRequest("GPX file could not be parsed: invalid format".into())
        })?
    };

    let started_at = parsed
        .started_at
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    let route = if parsed.route_coords.is_empty() {
        None
    } else {
        let coords: Vec<serde_json::Value> = parsed
            .route_coords
            .iter()
            .map(|p| {
                if let Some(ele) = p.ele {
                    json!([p.lon, p.lat, ele])
                } else {
                    json!([p.lon, p.lat])
                }
            })
            .collect();
        Some(json!({ "type": "LineString", "coordinates": coords }))
    };

    let ex_id = Uuid::now_v7();
    let ex_b58 = crate::server::id::encode(ex_id);
    let domain = &state.config.instance.domain;
    let scheme = state.config.instance.scheme();
    let actor_ap_id = actor.ap_id.to_string();
    let exercise_ap_id = format!("{scheme}://{domain}/exercises/{ex_b58}");
    let route_url_str = format!("{scheme}://{domain}/api/exercises/{ex_b58}/route");
    let stats_url_str = format!("{scheme}://{domain}/api/exercises/{ex_b58}/stats");

    let title = req.title.filter(|t| !t.trim().is_empty()).or_else(|| {
        let hour = started_at
            .format("%H")
            .to_string()
            .parse::<u8>()
            .unwrap_or(12);
        let time_of_day = match hour {
            5..=11 => "Morning",
            12..=13 => "Lunch",
            14..=17 => "Afternoon",
            18..=20 => "Evening",
            _ => "Night",
        };
        let cap_type = {
            let t = &req.activity_type;
            let mut c = t.chars();
            c.next()
                .map(|ch| ch.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        };
        Some(format!("{time_of_day} {cap_type}"))
    });

    let mut ap_json = json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            FEDISPORT_CONTEXT
        ],
        "type": "Exercise",
        "id": exercise_ap_id,
        "attributedTo": actor_ap_id,
        "activityType": req.activity_type,
        "startedAt": started_at.to_rfc3339(),
        "routeUrl": route_url_str,
        "statsUrl": stats_url_str,
        "published": started_at.to_rfc3339(),
    });

    if let Some(ref t) = title {
        ap_json["name"] = json!(t);
    }
    if let Some(ref desc) = req.description {
        if !desc.trim().is_empty() {
            ap_json["content"] = json!(desc);
        }
    }
    if !req.image_urls.is_empty() {
        let attachments: Vec<serde_json::Value> = req
            .image_urls
            .iter()
            .map(|url| json!({"type": "Image", "url": url, "mediaType": "image/jpeg"}))
            .collect();
        ap_json["attachment"] = json!(attachments);
    }

    // jogga has no S3 storage — gpx_url is always None.
    let description = req.description.as_deref().filter(|s| !s.trim().is_empty());

    let obj = NewObject {
        ap_id: &exercise_ap_id,
        object_type: "Exercise",
        attributed_to: &actor_ap_id,
        actor_id: Some(actor.id),
        content: description,
        content_map: None,
        summary: title.as_deref(),
        sensitive: false,
        in_reply_to: None,
        published: Some(started_at),
        url: None,
        ap_json: ap_json.clone(),
        visibility: &req.visibility,
    };

    let ex = NewExercise {
        id: ex_id,
        actor_id: actor.id,
        activity_type: req.activity_type.clone(),
        started_at,
        duration_s: parsed.duration_s,
        distance_m: parsed.distance_m,
        elevation_gain_m: parsed.elevation_gain_m,
        avg_pace_s_per_km: parsed.avg_pace_s_per_km,
        avg_heart_rate_bpm: parsed.avg_heart_rate_bpm,
        max_heart_rate_bpm: parsed.max_heart_rate_bpm,
        avg_cadence_rpm: parsed.avg_cadence_rpm,
        avg_power_w: parsed.avg_power_w,
        max_power_w: parsed.max_power_w,
        normalized_power_w: parsed.normalized_power_w,
        title: title.clone(),
        file_type: req.file_type.clone(),
        device: parsed.device.clone(),
        gpx_url: None,
        route,
        visibility: req.visibility.clone(),
        hidden_stats: req.hidden_stats.clone(),
    };

    let activity_ap_id = format!("{actor_ap_id}/activities/{}", Uuid::now_v7());
    let activity_json = json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            FEDISPORT_CONTEXT
        ],
        "type":      "Create",
        "id":        activity_ap_id,
        "actor":     actor_ap_id,
        "published": started_at.to_rfc3339(),
        "object":    ap_json,
    });

    let activity = {
        let mut tx = state
            .db
            .begin()
            .await
            .map_err(|e| AppError::from(crate::db::DbError::Sqlx(e)))?;

        let object_id = ExerciseQueries::insert_with_object(&mut tx, &obj, &ex).await?;

        for (i, url) in req.image_urls.iter().enumerate() {
            MediaAttachmentQueries::insert(&mut tx, &exercise_ap_id, url, i as i16).await?;
        }

        let activity = ActivityQueries::insert_tx(
            &mut tx,
            &NewActivity {
                ap_id: activity_ap_id.clone(),
                activity_type: "Create".to_owned(),
                actor_id: actor.id,
                object_ap_id: exercise_ap_id.clone(),
                target_ap_id: None,
                object_id: Some(object_id),
                ap_json: activity_json,
            },
        )
        .await?;

        ActivityQueries::add_to_outbox(&mut tx, actor.id, activity.id).await?;

        tx.commit()
            .await
            .map_err(|e| AppError::from(crate::db::DbError::Sqlx(e)))?;

        activity
    };

    // Enqueue delivery to followers.
    let db = state.db.clone();
    let actor_id = actor.id;
    let activity_id = activity.id;
    tokio::spawn(async move {
        let inbox_urls = match FollowQueries::list_follower_inbox_urls(&db, actor_id).await {
            Ok(urls) => urls,
            Err(e) => {
                warn!(actor_id=%actor_id, err=%e, "exercise upload: follower inbox lookup failed");
                return;
            }
        };
        if !inbox_urls.is_empty() {
            if let Err(e) = DeliveryQueries::insert_deliveries(&db, activity_id, &inbox_urls).await
            {
                warn!(err=%e, "exercise upload: delivery insert failed");
            }
        }
    });

    info!(
        username = actor.username,
        exercise_id = %ex_id,
        activity_type = req.activity_type,
        "exercise uploaded"
    );

    Ok(ExerciseUploadResult {
        id: ex_id,
        ap_id: exercise_ap_id,
    })
}

pub struct UpdatePostRequest {
    pub content: Option<String>,
    pub title: Option<String>,
    pub hidden_stats: Vec<String>,
    pub removed_image_urls: Vec<String>,
}

#[tracing::instrument(skip(data, req), fields(actor_id = %actor_id, object = object_ap_id))]
pub async fn do_update_post(
    data: &Data<AppState>,
    actor_id: Uuid,
    object_ap_id: &str,
    req: UpdatePostRequest,
) -> Result<(), AppError> {
    let state = data.app_data();
    let local_actor = fetch_local_actor(data, actor_id).await?;

    let obj = ObjectQueries::find_by_ap_id(&state.db, object_ap_id)
        .await
        .map_err(|_| AppError::NotFound)?;

    if obj.attributed_to != local_actor.ap_url().to_string() {
        return Err(AppError::Forbidden);
    }

    let mut ap_json = obj.ap_json.0.clone();

    let clean_content = req
        .content
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    match clean_content {
        Some(c) => {
            ap_json["content"] = json!(c);
        }
        None => {
            ap_json.as_object_mut().map(|m| m.remove("content"));
        }
    }

    let clean_title: Option<&str> = if obj.object_type == "Exercise" {
        req.title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    if obj.object_type == "Exercise" {
        match clean_title {
            Some(t) => {
                ap_json["name"] = json!(t);
            }
            None => {
                ap_json.as_object_mut().map(|m| m.remove("name"));
            }
        }

        let scheme = state.config.instance.scheme();
        let domain = &state.config.instance.domain;
        let ex_b58 = object_ap_id.rsplit('/').next().unwrap_or("");
        ap_json["routeUrl"] = json!(format!("{scheme}://{domain}/api/exercises/{ex_b58}/route"));
        ap_json["statsUrl"] = json!(format!("{scheme}://{domain}/api/exercises/{ex_b58}/stats"));

        ExerciseQueries::update_edit(&state.db, object_ap_id, clean_title, &req.hidden_stats)
            .await?;
    }

    if !req.removed_image_urls.is_empty() {
        if let Some(arr) = ap_json.get("attachment").and_then(|a| a.as_array()) {
            let kept: Vec<serde_json::Value> = arr
                .iter()
                .filter(|v| {
                    let url = v.get("url").and_then(|u| u.as_str()).unwrap_or("");
                    !req.removed_image_urls.contains(&url.to_string())
                })
                .cloned()
                .collect();
            if kept.is_empty() {
                ap_json.as_object_mut().map(|m| m.remove("attachment"));
            } else {
                ap_json["attachment"] = json!(kept);
            }
        }
        MediaAttachmentQueries::delete_urls(&state.db, object_ap_id, &req.removed_image_urls)
            .await
            .ok();
    }

    ObjectQueries::update_post(
        &state.db,
        object_ap_id,
        clean_content,
        clean_title,
        ap_json.clone(),
    )
    .await?;

    let scheme = state.config.instance.scheme();
    let domain = data.domain();
    let update_id: url::Url = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse()
        .map_err(AppError::from)?;

    let update_activity = Update {
        kind: UpdateType::Update,
        id: update_id,
        actor: local_actor.ap_url(),
        object: ap_json,
    };

    let inbox_urls = FollowQueries::list_follower_inbox_urls(&state.db, actor_id)
        .await
        .unwrap_or_default();

    if !inbox_urls.is_empty() {
        let inboxes: Vec<url::Url> = inbox_urls
            .into_iter()
            .filter_map(|u| u.parse().ok())
            .collect();
        let actor_clone = local_actor.clone();
        let data_clone = data.clone();
        tokio::spawn(async move {
            if let Err(e) = actor_clone
                .send(update_activity, inboxes, &data_clone)
                .await
            {
                warn!(err=%e, "do_update_post: federation send failed");
            }
        });
    }

    info!(
        actor = local_actor.row.username,
        object = object_ap_id,
        "post updated"
    );
    Ok(())
}

/// Announce a boost.
pub async fn do_announce(
    data: &Data<AppState>,
    actor_id: Uuid,
    object_ap_id: &str,
) -> Result<(), AppError> {
    let local_actor = fetch_local_actor(data, actor_id).await?;

    let scheme = data.app_data().config.instance.scheme();
    let domain = data.domain();
    let announce_id: url::Url = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse()
        .map_err(AppError::from)?;

    let object_url: url::Url = object_ap_id
        .parse()
        .map_err(|_| AppError::BadRequest("invalid object URL".into()))?;

    let announce = crate::server::protocol::announce::Announce {
        kind: activitypub_federation::kinds::activity::AnnounceType::Announce,
        id: announce_id.clone(),
        actor: local_actor.ap_url(),
        object: object_url,
    };

    let inbox_urls = FollowQueries::list_follower_inbox_urls(&data.db, actor_id)
        .await
        .unwrap_or_default();

    let inboxes: Vec<url::Url> = inbox_urls
        .into_iter()
        .filter_map(|u| u.parse().ok())
        .collect();
    if !inboxes.is_empty() {
        local_actor.send(announce.clone(), inboxes, data).await?;
    }

    AnnounceQueries::upsert(
        &data.db,
        Uuid::now_v7(),
        local_actor.ap_url().as_str(),
        object_ap_id,
        announce_id.as_str(),
    )
    .await
    .map_err(AppError::from)?;

    Ok(())
}
