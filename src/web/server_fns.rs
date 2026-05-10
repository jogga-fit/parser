use dioxus::prelude::*;

use crate::web::{
    ActorInfo, ConnectionsResult, DirectoryItem, FeedItem, FollowerItem, FollowingItem,
    LoginResult, MeResult, OtpVerifyResult, RegisterInitResult, ThreadItem, UploadExerciseMeta,
    UploadExerciseResult,
};

/// Convert an `AppError` to a `ServerFnError` with a safe, user-facing message.
#[cfg(feature = "server")]
fn into_sfn_err(e: crate::server::error::AppError) -> ServerFnError {
    use crate::server::error::AppError;
    let msg: String = match &e {
        AppError::BadRequest(m) => m.clone(),
        AppError::NotFound => "not found".into(),
        AppError::Unauthorized => "unauthorized".into(),
        AppError::Forbidden => "forbidden".into(),
        AppError::NotAcceptable => "not acceptable".into(),
        AppError::Conflict(m) => m.clone(),
        AppError::Gone(m) => m.clone(),
        AppError::NotAvailable(m) => m.clone(),
        AppError::Internal(ie) => {
            tracing::error!(error = %ie, "server function internal error");
            "internal server error".into()
        }
    };
    ServerFnError::new(msg)
}

/// Extract the `BadRequest` message or fall back to a static string.
#[cfg(feature = "server")]
fn bad_request_or(e: crate::server::error::AppError, fallback: &'static str) -> ServerFnError {
    use crate::server::error::AppError;
    match e {
        AppError::BadRequest(m) => ServerFnError::new(m),
        _ => ServerFnError::new(fallback),
    }
}

/// Parse a UUID string, returning a `ServerFnError` on failure.
#[cfg(feature = "server")]
fn parse_uuid(s: &str) -> Result<uuid::Uuid, ServerFnError> {
    s.parse::<uuid::Uuid>()
        .map_err(|_| ServerFnError::new("invalid id"))
}

/// Returns the local owner's username (no auth required).
#[server]
pub async fn get_owner_username() -> Result<String, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let row = sqlx::query("SELECT username FROM actors WHERE is_local = 1 LIMIT 1")
        .fetch_one(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    use sqlx::Row as _;
    let username: String = row
        .try_get("username")
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(username)
}

#[server]
pub async fn login(
    username_or_email: String,
    password: String,
) -> Result<LoginResult, ServerFnError> {
    use crate::server::error::AppError;
    use crate::web::server::*;

    let _rd = request_data();
    let state = _rd.app_data();
    let token = crate::server::service::do_login(state, &username_or_email, &password)
        .await
        .map_err(|e| {
            let msg = match e {
                AppError::NotFound | AppError::Unauthorized => "Invalid credentials",
                _ => "Login failed. Please try again.",
            };
            ServerFnError::new(msg)
        })?;
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("login failed"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    Ok(LoginResult {
        token,
        username: actor.username,
    })
}

#[server]
pub async fn password_reset_init(contact: String) -> Result<RegisterInitResult, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    crate::server::service::do_password_reset_init(state, &contact)
        .await
        .map(|(otp_id, code)| RegisterInitResult {
            otp_id: otp_id.to_string(),
            code,
        })
        .map_err(|e| bad_request_or(e, "Failed to send reset code. Please try again."))
}

#[server]
pub async fn password_reset_verify(
    otp_id: String,
    code: String,
    new_password: String,
) -> Result<LoginResult, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let uid = parse_uuid(&otp_id)?;
    let token = crate::server::service::do_password_reset_verify(state, uid, &code, &new_password)
        .await
        .map_err(into_sfn_err)?;
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("reset failed"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    Ok(LoginResult {
        token,
        username: actor.username,
    })
}

#[server]
pub async fn register_init(
    username: String,
    email: Option<String>,
    phone: Option<String>,
) -> Result<RegisterInitResult, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    crate::server::service::do_register_init(
        state,
        &username,
        email.as_deref(),
        phone.as_deref(),
    )
    .await
    .map(|(otp_id, code)| RegisterInitResult {
        otp_id: otp_id.to_string(),
        code,
    })
    .map_err(|e| bad_request_or(e, "Registration failed. Please try again."))
}

#[server]
pub async fn otp_verify(
    otp_id: String,
    code: String,
    password: String,
    display_name: Option<String>,
) -> Result<OtpVerifyResult, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let uid = parse_uuid(&otp_id)?;
    let outcome =
        crate::server::service::do_otp_verify(state, uid, &code, &password, display_name.as_deref())
            .await
            .map_err(into_sfn_err)?;
    Ok(OtpVerifyResult {
        token: outcome.token,
        username: outcome.username,
        ap_id: outcome.ap_id,
    })
}

#[server]
pub async fn get_me(token: String) -> Result<MeResult, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    Ok(MeResult {
        username: actor.username,
        ap_id: actor.ap_id.to_string(),
        display_name: actor.display_name,
        bio: actor.summary,
        email: account.email,
        phone: account.phone,
        avatar_url: actor.avatar_url,
        public_profile: account.public_profile,
        theme: account.theme.clone(),
        also_known_as: actor.also_known_as.0,
        moved_to: actor.moved_to,
    })
}

#[server]
pub async fn get_actor_info(username: String) -> Result<ActorInfo, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    // Strip @domain suffix if present (e.g. "alice@localhost:8080" → "alice").
    let local_username = username
        .split_once('@')
        .map(|(u, _)| u)
        .unwrap_or(&username);
    let row = ActorQueries::find_local_by_username(&state.db, local_username)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    let public_profile = AccountQueries::find_by_actor_id(&state.db, row.id)
        .await
        .map(|a| a.public_profile)
        .unwrap_or(true);
    let followers_count = FollowQueries::count_followers(&state.db, row.id)
        .await
        .unwrap_or(0);
    let following_count = FollowQueries::count_following(&state.db, row.id)
        .await
        .unwrap_or(0);
    Ok(ActorInfo {
        username: row.username,
        domain: row.domain,
        ap_id: row.ap_id.to_string(),
        display_name: row.display_name,
        bio: row.summary,
        avatar_url: row.avatar_url,
        public_profile,
        followers_count,
        following_count,
    })
}

/// Fetch posts for an actor's profile page.
///
/// `token` is optional; when supplied it is used to determine whether the
/// viewer follows the profile actor so follower-only posts can be included.
#[server]
pub async fn get_actor_posts(
    username: String,
    token: Option<String>,
) -> Result<Vec<FeedItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();

    let local_username = username
        .split_once('@')
        .map(|(u, _)| u)
        .unwrap_or(&username);
    let profile_actor = ActorQueries::find_local_by_username(&state.db, local_username)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    // Check if this is a private profile.
    let profile_account = AccountQueries::find_by_actor_id(&state.db, profile_actor.id)
        .await
        .ok();
    let profile_is_private = profile_account
        .as_ref()
        .map(|a| !a.public_profile)
        .unwrap_or(false);

    // Determine whether the viewer can see this profile's posts at all, and whether
    // they can see follower-only exercises.
    //
    // For public profiles: everyone can see public posts; followers see follower posts.
    // For private profiles: only the owner and accepted followers can see any posts.
    // Pending follows do NOT grant access to a private profile.
    let mut viewer_actor_ap_id: Option<String> = None;
    let (viewer_is_authorized, viewer_can_see_followers) = if let Some(t) = token {
        match AccountQueries::find_by_token(&state.db, &t).await {
            Ok(account) => {
                if let Ok(va) = ActorQueries::find_by_id(&state.db, account.actor_id).await {
                    viewer_actor_ap_id = Some(va.ap_id.to_string());
                }
                if account.actor_id == profile_actor.id {
                    // Owner: always authorized, always sees follower posts.
                    (true, true)
                } else {
                    let accepted = FollowQueries::is_following_accepted(
                        &state.db,
                        account.actor_id,
                        profile_actor.id,
                    )
                    .await
                    .unwrap_or(false);
                    (accepted || !profile_is_private, accepted)
                }
            }
            Err(_) => (!profile_is_private, false),
        }
    } else {
        (!profile_is_private, false)
    };

    if !viewer_is_authorized {
        return Ok(vec![]);
    }

    let rows = ActivityQueries::get_actor_profile_posts(
        &state.db,
        profile_actor.id,
        viewer_can_see_followers,
        50,
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let actor_username = profile_actor.username.clone();
    let actor_ap_id = profile_actor.ap_id.to_string();
    let actor_avatar_url = profile_actor.avatar_url.clone();
    let scheme = state.config.instance.scheme();
    let domain = state.config.instance.domain.clone();

    // Batch-fetch attachments, like counts and viewer like status in one pass each.
    let ap_ids: Vec<String> = rows.iter().map(|r| r.object_ap_id.clone()).collect();
    let attachments =
        crate::db::queries::MediaAttachmentQueries::fetch_for_objects(&state.db, &ap_ids)
            .await
            .unwrap_or_default();
    let like_counts = crate::db::queries::LikeQueries::count_batch(&state.db, &ap_ids)
        .await
        .unwrap_or_default();
    let viewer_liked = if let Some(ref vap) = viewer_actor_ap_id {
        crate::db::queries::LikeQueries::viewer_liked_batch(&state.db, vap, &ap_ids)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };
    // Group by object_ap_id for O(n) lookup.
    let mut attachment_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for a in attachments {
        attachment_map
            .entry(a.object_ap_id)
            .or_default()
            .push(a.url);
    }

    Ok(rows
        .into_iter()
        .map(|r| {
            let image_urls = attachment_map.remove(&r.object_ap_id).unwrap_or_default();
            let like_count = like_counts.get(&r.object_ap_id).copied().unwrap_or(0);
            let viewer_has_liked = viewer_liked.contains(&r.object_ap_id);
            let route_url =
                if r.object_type == "Exercise" && !r.hidden_stats.contains(&"map".to_string()) {
                    let uuid = r.object_ap_id.rsplit('/').next().unwrap_or("");
                    Some(format!("{scheme}://{domain}/api/exercises/{uuid}/route"))
                } else {
                    None
                };
            FeedItem {
                id: r.object_ap_id.clone(),
                object_ap_id: r.object_ap_id.clone(),
                viewer_is_owner: viewer_actor_ap_id.as_deref() == Some(actor_ap_id.as_str()),
                actor_username: actor_username.clone(),
                actor_domain: profile_actor.domain.clone(),
                actor_is_local: profile_actor.is_local,
                actor_ap_id: actor_ap_id.clone(),
                actor_avatar_url: actor_avatar_url.clone(),
                activity_type: "Create".to_string(),
                object_type: r.object_type,
                content: r.content,
                published: r.published.to_rfc3339(),
                exercise_type: r.exercise_type,
                duration_s: r.duration_s.map(|v| v as i64),
                distance_m: r.distance_m,
                elevation_gain_m: r.elevation_gain_m,
                avg_heart_rate_bpm: r.avg_heart_rate_bpm,
                max_heart_rate_bpm: r.max_heart_rate_bpm,
                avg_power_w: r.avg_power_w,
                max_power_w: r.max_power_w,
                normalized_power_w: r.normalized_power_w,
                avg_cadence_rpm: r.avg_cadence_rpm,
                avg_pace_s_per_km: r.avg_pace_s_per_km,
                device: r.device,
                title: r.title,
                image_urls,
                like_count,
                viewer_has_liked,
                reply_count: 0,
                in_reply_to: None,
                route_url,
                hidden_stats: r.hidden_stats.0,
                via_club_handle: None,
                via_club_display: None,
            }
        })
        .collect())
}

/// Map a `FeedRow` (raw DB row from either timeline query) to a `FeedItem`.
///
/// `viewer_ap_id`: `Some(ap_id)` for authenticated viewers; `None` for logged-out users.
/// Fields that require per-viewer state (`like_count`, `viewer_has_liked`, exercise stats)
/// are left at their zero/empty defaults — callers must fill them via batch enrichment.
#[cfg(feature = "server")]
fn feed_row_to_item(a: crate::db::models::FeedRow, viewer_ap_id: Option<&str>) -> FeedItem {
    let obj = a.ap_json.get("object");
    let object_ap_id = obj
        .and_then(|o| o.get("id"))
        .and_then(|id| id.as_str())
        .unwrap_or(&a.activity_ap_id)
        .to_owned();
    let object_type = obj
        .and_then(|o| o.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("Note")
        .to_owned();
    let content = obj
        .and_then(|o| o.get("content"))
        .and_then(|c| c.as_str())
        .map(str::to_owned);
    let exercise_type = obj
        .and_then(|o| o.get("activityType"))
        .and_then(|t| t.as_str())
        .map(str::to_owned);
    let duration_s = obj.and_then(|o| o.get("duration")).and_then(|d| d.as_i64());
    let distance_m = obj.and_then(|o| o.get("distance")).and_then(|d| d.as_f64());
    let elevation_gain_m = obj
        .and_then(|o| o.get("elevationGain"))
        .and_then(|d| d.as_f64());
    let avg_heart_rate_bpm = obj
        .and_then(|o| o.get("avgHeartRate"))
        .and_then(|d| d.as_i64())
        .map(|v| v as i32);
    let max_heart_rate_bpm = obj
        .and_then(|o| o.get("maxHeartRate"))
        .and_then(|d| d.as_i64())
        .map(|v| v as i32);
    let avg_power_w = obj.and_then(|o| o.get("avgPower")).and_then(|d| d.as_f64());
    let max_power_w = obj.and_then(|o| o.get("maxPower")).and_then(|d| d.as_f64());
    let normalized_power_w = obj
        .and_then(|o| o.get("normalizedPower"))
        .and_then(|d| d.as_f64());
    let avg_cadence_rpm = obj
        .and_then(|o| o.get("avgCadence"))
        .and_then(|d| d.as_f64());
    let avg_pace_s_per_km = obj.and_then(|o| o.get("avgPace")).and_then(|d| d.as_f64());
    let device = obj
        .and_then(|o| o.get("device"))
        .and_then(|d| d.as_str())
        .map(str::to_owned);
    let title = obj
        .and_then(|o| o.get("name"))
        .and_then(|t| t.as_str())
        .map(str::to_owned);
    let image_urls = obj
        .and_then(|o| o.get("attachment"))
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("url").and_then(|u| u.as_str()).map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let in_reply_to = obj
        .and_then(|o| o.get("inReplyTo"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let route_url = obj
        .and_then(|o| o.get("routeUrl"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    FeedItem {
        id: a.activity_ap_id,
        object_ap_id,
        viewer_is_owner: viewer_ap_id.is_some_and(|vap| vap == a.actor_ap_id),
        actor_username: a.actor_username,
        actor_domain: a.actor_domain,
        actor_is_local: a.actor_is_local,
        actor_ap_id: a.actor_ap_id,
        actor_avatar_url: a.actor_avatar_url,
        activity_type: a.activity_type,
        object_type,
        content,
        published: a.published.to_rfc3339(),
        exercise_type,
        duration_s,
        distance_m,
        elevation_gain_m,
        avg_heart_rate_bpm,
        max_heart_rate_bpm,
        avg_power_w,
        max_power_w,
        normalized_power_w,
        avg_cadence_rpm,
        avg_pace_s_per_km,
        device,
        title,
        image_urls,
        like_count: 0,
        viewer_has_liked: false,
        reply_count: 0,
        in_reply_to,
        route_url,
        hidden_stats: vec![],
        via_club_handle: None,
        via_club_display: None,
    }
}

/// Batch-enrich exercise stats from the DB into a `FeedItem` slice.
///
/// Replaces per-item lookups (N+1) with a single batch query.
/// Also fixes `hidden_stats` — the field is authoritative in `exercises`, not `ap_json`.
#[cfg(feature = "server")]
async fn enrich_exercise_stats(
    pool: &sqlx::SqlitePool,
    items: &mut [FeedItem],
) -> Result<(), crate::db::error::DbError> {
    use crate::db::queries::ExerciseQueries;
    let exercise_ap_ids: Vec<String> = items
        .iter()
        .filter(|i| i.object_type == "Exercise")
        .map(|i| i.object_ap_id.clone())
        .collect();
    if exercise_ap_ids.is_empty() {
        return Ok(());
    }
    let rows = ExerciseQueries::find_batch_by_ap_ids(pool, &exercise_ap_ids).await?;
    let ex_map: std::collections::HashMap<String, _> = rows
        .into_iter()
        .map(|r| (r.object_ap_id.clone(), r))
        .collect();
    for item in items.iter_mut() {
        if let Some(row) = ex_map.get(&item.object_ap_id) {
            // Only populate stats when the DB has real data. Remote exercises
            // received via v0.2 federation are stored with zero placeholders
            // until their statsUrl is fetched; leave those as None so the UI
            // renders a blank row rather than "0 km / 0:00".
            if row.duration_s > 0 || row.distance_m > 0.0 {
                item.exercise_type = Some(row.activity_type.clone());
                item.duration_s = Some(row.duration_s as i64);
                item.distance_m = Some(row.distance_m);
                item.elevation_gain_m = row.elevation_gain_m;
                item.avg_pace_s_per_km = row.avg_pace_s_per_km;
                item.avg_heart_rate_bpm = row.avg_heart_rate_bpm;
                item.max_heart_rate_bpm = row.max_heart_rate_bpm;
                item.avg_cadence_rpm = row.avg_cadence_rpm;
                item.avg_power_w = row.avg_power_w;
                item.max_power_w = row.max_power_w;
                item.normalized_power_w = row.normalized_power_w;
                item.device = row.device.clone();
            }
            // hidden_stats is always authoritative from the DB — propagate regardless
            // of whether there are numeric stats. Respects the author's visibility prefs.
            item.hidden_stats = row.hidden_stats.0.clone();
        }
    }
    Ok(())
}

#[server]
pub async fn get_feed(token: String) -> Result<Vec<FeedItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let activities = ActivityQueries::get_home_timeline(&state.db, actor.id, 20)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let viewer_ap_id = actor.ap_id.to_string();

    let mut items: Vec<FeedItem> = activities
        .into_iter()
        .map(|a| feed_row_to_item(a, Some(&viewer_ap_id)))
        .collect();

    // Enrich with like counts and viewer like status in batch.
    let object_ap_ids: Vec<String> = items.iter().map(|i| i.object_ap_id.clone()).collect();
    let like_counts = LikeQueries::count_batch(&state.db, &object_ap_ids)
        .await
        .unwrap_or_default();
    let viewer_liked = LikeQueries::viewer_liked_batch(&state.db, &viewer_ap_id, &object_ap_ids)
        .await
        .unwrap_or_default();
    for item in &mut items {
        item.like_count = like_counts.get(&item.object_ap_id).copied().unwrap_or(0);
        item.viewer_has_liked = viewer_liked.contains(&item.object_ap_id);
    }

    // Enrich exercise stats from the DB (batch — fixes the N+1 and populates hidden_stats).
    enrich_exercise_stats(&state.db, &mut items)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server]
pub async fn get_public_feed() -> Result<Vec<FeedItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();

    let activities = ActivityQueries::get_local_public_timeline(&state.db, 30)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut items: Vec<FeedItem> = activities
        .into_iter()
        .map(|a| feed_row_to_item(a, None))
        .collect();

    // Like counts — no viewer-specific state for logged-out users.
    let object_ap_ids: Vec<String> = items.iter().map(|i| i.object_ap_id.clone()).collect();
    let like_counts = LikeQueries::count_batch(&state.db, &object_ap_ids)
        .await
        .unwrap_or_default();
    for item in &mut items {
        item.like_count = like_counts.get(&item.object_ap_id).copied().unwrap_or(0);
    }

    // Exercise stats from DB — batch lookup, respects hidden_stats privacy prefs.
    enrich_exercise_stats(&state.db, &mut items)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server]
pub async fn get_club_feed(
    token: String,
    club_ap_id: String,
) -> Result<Vec<FeedItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;

    let activities = ActivityQueries::get_club_feed(&state.db, &club_ap_id, 30)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut items: Vec<FeedItem> = activities
        .into_iter()
        .map(|a| feed_row_to_item(a, None))
        .collect();

    let object_ap_ids: Vec<String> = items.iter().map(|i| i.object_ap_id.clone()).collect();
    let like_counts = LikeQueries::count_batch(&state.db, &object_ap_ids)
        .await
        .unwrap_or_default();
    for item in &mut items {
        item.like_count = like_counts.get(&item.object_ap_id).copied().unwrap_or(0);
    }

    enrich_exercise_stats(&state.db, &mut items)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

#[server]
pub async fn get_directory() -> Result<Vec<DirectoryItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let rows = ActorQueries::list_directory(&state.db)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|a| DirectoryItem {
            username: a.username,
            domain: a.domain,
            ap_id: a.ap_id.to_string(),
            display_name: a.display_name,
            bio: a.summary,
            avatar_url: a.avatar_url,
        })
        .collect())
}

#[server]
pub async fn get_theme(token: String) -> Result<String, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    Ok(account.theme)
}

#[server]
pub async fn set_theme(token: String, theme: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    if theme != "dark" && theme != "light" && theme != "system" {
        return Err(ServerFnError::new("invalid theme"));
    }
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    AccountQueries::update_theme(&state.db, account.id, &theme)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn set_privacy_settings(
    token: String,
    public_profile: bool,
) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    AccountQueries::update_privacy_settings(&state.db, account.id, public_profile)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Keep actors.manually_approves_followers in sync.
    // private profile (public_profile=false) → manually approves followers.
    ActorQueries::set_manually_approves_followers(&state.db, account.actor_id, !public_profile)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // When switching back to public, auto-accept all pending follow requests.
    if public_profile {
        crate::db::queries::FollowQueries::accept_all_pending(&state.db, account.actor_id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    Ok(())
}

#[server]
pub async fn delete_account(token: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let rd = request_data();
    let state = rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    crate::server::service::social::do_delete_account(&rd, &actor)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Shared implementation for [`follow_actor`] and [`follow_person`].
///
/// Resolves `handle_or_url` (a `@user@domain` WebFinger handle or a bare AP URL)
/// to an ActivityPub ID and sends a Follow activity.
/// `person_only` is forwarded to `do_follow` to restrict the target actor type.
#[cfg(feature = "server")]
async fn follow_with_options(
    token: String,
    handle_or_url: String,
    person_only: bool,
) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;

    // Resolve @user@domain handle via WebFinger if needed.
    let ap_id = if handle_or_url.starts_with('@') {
        let handle = handle_or_url.trim_start_matches('@');
        let parts: Vec<&str> = handle.splitn(2, '@').collect();
        if parts.len() != 2 {
            return Err(ServerFnError::new(format!("invalid handle: @{handle}")));
        }
        let (user, domain) = (parts[0], parts[1]);
        let scheme = if domain.starts_with("localhost") {
            "http"
        } else {
            "https"
        };
        let url =
            format!("{scheme}://{domain}/.well-known/webfinger?resource=acct:{user}@{domain}");

        let resp = state
            .http
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ServerFnError::new(format!("WebFinger request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(ServerFnError::new(format!(
                "WebFinger returned {}",
                resp.status()
            )));
        }

        let jrd: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ServerFnError::new(format!("WebFinger parse error: {e}")))?;

        jrd["links"]
            .as_array()
            .and_then(|links| {
                links.iter().find(|l| {
                    l["rel"].as_str() == Some("self")
                        && l["type"]
                            .as_str()
                            .is_some_and(|t| t.contains("activity+json"))
                })
            })
            .and_then(|l| l["href"].as_str())
            .map(str::to_owned)
            .ok_or_else(|| ServerFnError::new("WebFinger: no ActivityPub self link found"))?
    } else {
        let h = handle_or_url;
        if h.starts_with("http://") || h.starts_with("https://") {
            h
        } else {
            format!("https://{h}")
        }
    };

    let data = request_data();
    crate::server::service::do_follow(&data, account.actor_id, &ap_id, person_only)
        .await
        .map_err(into_sfn_err)
}

#[server]
pub async fn follow_actor(token: String, handle_or_url: String) -> Result<(), ServerFnError> {
    follow_with_options(token, handle_or_url, false).await
}

#[server]
pub async fn follow_person(token: String, handle_or_url: String) -> Result<(), ServerFnError> {
    follow_with_options(token, handle_or_url, true).await
}

#[server]
pub async fn unfollow_actor(token: String, ap_id: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let account = AccountQueries::find_by_token(&_rd.app_data().db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let data = request_data();
    crate::server::service::do_unfollow(&data, account.actor_id, &ap_id)
        .await
        .map_err(into_sfn_err)
}

#[server]
/// Returns the caller's follow status toward `target_ap_id`:
/// - `None`        → not following
/// - `Some(false)` → follow sent, awaiting acceptance
/// - `Some(true)`  → follow accepted
pub async fn check_following(
    token: String,
    target_ap_id: String,
) -> Result<Option<bool>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let target = match ActorQueries::find_by_ap_id(&state.db, &target_ap_id).await {
        Ok(row) => row,
        Err(_) => return Ok(None),
    };
    FollowQueries::following_status(&state.db, account.actor_id, target.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_followers(token: String) -> Result<Vec<FollowerItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let rows = FollowQueries::list_followers_detail(&state.db, actor.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| FollowerItem {
            ap_id: r.ap_id,
            username: r.username,
            domain: r.domain,
            is_local: r.is_local,
            display_name: r.display_name,
            avatar_url: r.avatar_url,
            accepted: r.accepted,
            follow_ap_id: r.follow_ap_id,
        })
        .collect())
}

#[server]
pub async fn get_following(token: String) -> Result<Vec<FollowingItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let rows = FollowQueries::list_following_detail(&state.db, actor.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| FollowingItem {
            ap_id: r.ap_id,
            username: r.username,
            domain: r.domain,
            is_local: r.is_local,
            display_name: r.display_name,
            avatar_url: r.avatar_url,
            accepted: r.accepted,
        })
        .collect())
}

#[server]
pub async fn list_joined_clubs(token: String) -> Result<Vec<FollowingItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let rows = FollowQueries::list_joined_clubs(&state.db, actor.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| FollowingItem {
            ap_id: r.ap_id,
            username: r.username,
            domain: r.domain,
            is_local: r.is_local,
            display_name: r.display_name,
            avatar_url: r.avatar_url,
            accepted: r.accepted,
        })
        .collect())
}

#[server]
pub async fn update_profile(
    token: String,
    display_name: Option<String>,
    bio: Option<String>,
) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    ActorQueries::update_profile(
        &state.db,
        account.actor_id,
        display_name.as_deref(),
        bio.as_deref(),
    )
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Upload a compressed avatar image and update the actor's avatar URL.
#[server]
pub async fn upload_avatar_fn(
    token: String,
    image_bytes: Vec<u8>,
) -> Result<String, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let storage = state.config.storage.as_ref().ok_or_else(|| {
        ServerFnError::new("Avatar upload not supported on this instance")
    })?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    let key = format!("avatars/{}.jpg", actor.id);
    let url = crate::server::service::storage::upload_bytes(storage, &key, &image_bytes, "image/jpeg")
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    ActorQueries::update_avatar_url(&state.db, actor.id, &url)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(url)
}

/// Upload an exercise file (GPX or FIT) from the compose UI.
///
/// `file_bytes` is the raw file content (not base64 — server functions transfer
/// binary via the Dioxus server function protocol).
/// `file_name` is used to detect file type (.gpx vs .fit).
#[server]
pub async fn upload_exercise_fn(
    token: String,
    file_bytes: Vec<u8>,
    file_name: String,
    meta: UploadExerciseMeta,
) -> Result<UploadExerciseResult, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let file_type = if file_name.to_lowercase().ends_with(".fit") {
        "fit"
    } else {
        "gpx"
    }
    .to_string();

    if meta.image_urls.len() > 8 {
        return Err(ServerFnError::new("too many images: maximum 8 per post"));
    }

    if !meta.image_urls.is_empty() && state.config.storage.is_none() {
        return Err(ServerFnError::new(
            "Image uploads are not configured on this server",
        ));
    }

    let result = crate::server::service::do_upload_exercise(
        state,
        &actor,
        crate::server::service::ExerciseUploadRequest {
            activity_type: meta.activity_type,
            file_bytes,
            file_type,
            file_name,
            visibility: meta.visibility,
            title: meta.title,
            description: meta.description,
            image_urls: meta.image_urls,
            hidden_stats: meta.hidden_stats,
        },
    )
    .await
    .map_err(into_sfn_err)?;

    Ok(UploadExerciseResult {
        id: result.id.to_string(),
        ap_id: result.ap_id,
    })
}

#[server]
pub async fn get_pending_followers(token: String) -> Result<Vec<FollowerItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let rows = FollowQueries::list_pending_followers(&state.db, actor.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| FollowerItem {
            ap_id: r.ap_id,
            username: r.username,
            domain: r.domain,
            is_local: r.is_local,
            display_name: r.display_name,
            avatar_url: r.avatar_url,
            accepted: r.accepted,
            follow_ap_id: r.follow_ap_id,
        })
        .collect())
}

#[server]
pub async fn accept_follow_request(
    token: String,
    follower_ap_id: String,
    follow_ap_id: String,
) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    use activitypub_federation::traits::Actor;
    let _rd = request_data();
    let data = request_data();
    let state = _rd.app_data();

    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let local_actor_row = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    // Dereference follower to get inbox URL
    let follower: activitypub_federation::fetch::object_id::ObjectId<DbActor> = follower_ap_id
        .parse()
        .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;
    let follower_actor = follower
        .dereference(&data)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Accept in DB.
    FollowQueries::accept_follower_pool(&state.db, local_actor_row.id, follower_actor.row.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // For local followers, the federation library won't deliver to the inbox,
    // so update the following record directly.
    if follower_actor.row.is_local {
        let local_db_actor = crate::web::server::DbActor {
            row: local_actor_row,
        };
        FollowQueries::accept_following(&state.db, follower_actor.row.id, local_db_actor.row.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        // Notify the local follower that their request was accepted.
        let _ = crate::db::queries::NotificationQueries::insert(
            &state.db,
            follower_actor.row.id,
            "follow_accepted",
            local_db_actor.row.id,
            None,
            None,
        )
        .await;
    } else {
        // Send Accept{Follow} AP activity to remote follower.
        let scheme = state.config.instance.scheme();
        let domain = &state.config.instance.domain;
        let accept_id: url::Url = format!("{scheme}://{domain}/accepts/{}", Uuid::now_v7())
            .parse()
            .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;
        let follow_id: url::Url = follow_ap_id
            .parse()
            .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;
        let local_ap_id: url::Url = local_actor_row.ap_id.0.clone();
        let follower_ap_obj_id: activitypub_federation::fetch::object_id::ObjectId<DbActor> =
            follower_actor.row.ap_id.0.clone().into();
        let follow_obj = Follow::new(
            follower_ap_obj_id,
            activitypub_federation::fetch::object_id::ObjectId::from(local_ap_id.clone()),
            follow_id,
        );
        let accept = Accept::new(
            activitypub_federation::fetch::object_id::ObjectId::from(local_ap_id),
            follow_obj,
            accept_id,
        );
        let local_db_actor = crate::web::server::DbActor {
            row: local_actor_row,
        };
        let inbox = follower_actor.inbox();
        tokio::spawn(async move {
            if let Err(e) = local_db_actor.send(accept, vec![inbox], &data).await {
                tracing::warn!(err=%e, "accept_follow_request: failed to deliver Accept");
            }
        });
    }

    Ok(())
}

#[server]
pub async fn reject_follow_request(
    token: String,
    follower_ap_id: String,
    follow_ap_id: String,
) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    use activitypub_federation::traits::Actor;
    let _rd = request_data();
    let data = request_data();
    let state = _rd.app_data();

    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let local_actor_row = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    // Dereference follower to get inbox URL.
    let follower: activitypub_federation::fetch::object_id::ObjectId<DbActor> = follower_ap_id
        .parse()
        .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;
    let follower_actor = follower
        .dereference(&data)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Delete from DB.
    FollowQueries::remove_follower(&state.db, local_actor_row.id, follower_actor.row.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // For local followers, the federation library won't deliver the Reject to
    // the local inbox, so clear the pending following record directly.
    if follower_actor.row.is_local {
        FollowQueries::remove_following(&state.db, follower_actor.row.id, local_actor_row.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        return Ok(());
    }

    // Send Reject{Follow} AP activity.
    let scheme = state.config.instance.scheme();
    let domain = &state.config.instance.domain;
    let reject_id: url::Url = format!("{scheme}://{domain}/rejects/{}", Uuid::now_v7())
        .parse()
        .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;

    let follow_id: url::Url = follow_ap_id
        .parse()
        .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;
    let local_ap_id: url::Url = local_actor_row.ap_id.0.clone();
    let follower_ap_obj_id: activitypub_federation::fetch::object_id::ObjectId<DbActor> =
        follower_actor.row.ap_id.0.clone().into();
    let follow_obj = Follow::new(
        follower_ap_obj_id,
        activitypub_federation::fetch::object_id::ObjectId::from(local_ap_id.clone()),
        follow_id,
    );
    let reject = Reject::new(
        activitypub_federation::fetch::object_id::ObjectId::from(local_ap_id),
        follow_obj,
        reject_id,
    );
    let local_db_actor = crate::web::server::DbActor {
        row: local_actor_row,
    };
    let inbox = follower_actor.inbox();
    tokio::spawn(async move {
        if let Err(e) = local_db_actor.send(reject, vec![inbox], &data).await {
            tracing::warn!(err=%e, "reject_follow_request: failed to deliver Reject");
        }
    });

    Ok(())
}

#[server]
pub async fn kick_follower(token: String, follower_ap_id: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    use activitypub_federation::traits::Actor;
    let _rd = request_data();
    let data = request_data();
    let state = _rd.app_data();

    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let local_actor_row = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let follower_row = ActorQueries::find_by_ap_id(&state.db, &follower_ap_id)
        .await
        .map_err(|_| ServerFnError::new("follower not found"))?;

    // Fetch the original Follow activity ID for the Reject object.
    let stored_follow_ap_id =
        FollowQueries::get_follow_ap_id(&state.db, local_actor_row.id, follower_row.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Remove the follower relationship.
    FollowQueries::remove_follower(&state.db, local_actor_row.id, follower_row.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if follower_row.is_local {
        // Local follower: directly remove their following record — no HTTP delivery needed.
        FollowQueries::remove_following(&state.db, follower_row.id, local_actor_row.id)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    } else {
        // Remote follower: send Reject(Follow) so their server removes the following record.
        let scheme = state.config.instance.scheme();
        let domain = &state.config.instance.domain;
        let reject_id: url::Url = format!("{scheme}://{domain}/rejects/{}", Uuid::now_v7())
            .parse()
            .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;

        // Use the stored follow AP ID; synthesize one if it was never recorded.
        let follow_id: url::Url = match stored_follow_ap_id {
            Some(ref id) => id
                .parse()
                .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?,
            None => format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
                .parse()
                .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?,
        };

        let local_ap_id: url::Url = local_actor_row.ap_id.0.clone();
        let follower_obj_id: activitypub_federation::fetch::object_id::ObjectId<DbActor> =
            follower_row.ap_id.0.clone().into();
        let follow_obj = Follow::new(
            follower_obj_id,
            activitypub_federation::fetch::object_id::ObjectId::from(local_ap_id.clone()),
            follow_id,
        );
        let reject = Reject::new(
            activitypub_federation::fetch::object_id::ObjectId::from(local_ap_id),
            follow_obj,
            reject_id,
        );

        let follower_inbox = DbActor { row: follower_row }.inbox();
        let local_db_actor = DbActor {
            row: local_actor_row,
        };
        tokio::spawn(async move {
            if let Err(e) = local_db_actor
                .send(reject, vec![follower_inbox], &data)
                .await
            {
                tracing::warn!(err=%e, "kick_follower: failed to deliver Reject");
            }
        });
    }

    Ok(())
}

#[server]
pub async fn like_object(token: String, object_ap_id: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let data = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    crate::server::service::do_like(&data, account.actor_id, &object_ap_id)
        .await
        .map_err(into_sfn_err)
}

#[server]
pub async fn unlike_object(token: String, object_ap_id: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let data = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    crate::server::service::do_unlike(&data, account.actor_id, &object_ap_id)
        .await
        .map_err(into_sfn_err)
}

#[server]
pub async fn delete_object(token: String, object_ap_id: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let data = request_data();
    let state = _rd.app_data();

    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    // Verify ownership.
    let obj = ObjectQueries::find_by_ap_id(&state.db, &object_ap_id)
        .await
        .map_err(|_| ServerFnError::new("post not found"))?;
    if obj.attributed_to != actor.ap_id.to_string() {
        return Err(ServerFnError::new("forbidden"));
    }

    ObjectQueries::delete_by_ap_id(&state.db, &object_ap_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Federate Delete to followers (non-fatal).
    let scheme = state.config.instance.scheme();
    let domain = &state.config.instance.domain;
    let delete_id: url::Url = format!("{scheme}://{domain}/activities/{}", Uuid::now_v7())
        .parse()
        .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;
    let object_url: url::Url = object_ap_id
        .parse()
        .map_err(|e: url::ParseError| ServerFnError::new(e.to_string()))?;
    let actor_url: url::Url = actor.ap_id.0.clone();

    let delete = Delete {
        kind: DeleteType::Delete,
        id: delete_id,
        actor: actor_url,
        object: DeleteObject::Url(object_url),
    };

    let inbox_urls = FollowQueries::list_follower_inbox_urls(&state.db, actor.id)
        .await
        .unwrap_or_default();

    if !inbox_urls.is_empty() {
        let db_actor = DbActor { row: actor };
        let inboxes: Vec<url::Url> = inbox_urls
            .into_iter()
            .filter_map(|u| u.parse().ok())
            .collect();
        tokio::spawn(async move {
            if let Err(e) = db_actor.send(delete, inboxes, &data).await {
                tracing::warn!(err=%e, "delete_object: federation send failed");
            }
        });
    }

    Ok(())
}

/// Return the following and follower lists for a local actor by username.
///
/// Visibility rules:
/// - Public profile: always returns the lists.
/// - Private profile: returns lists only if the viewer (identified by `token`)
///   is the owner or an accepted follower. Otherwise returns `visible: false`.
#[server]
pub async fn get_actor_connections(
    username: String,
    token: Option<String>,
) -> Result<ConnectionsResult, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();

    let local_username = username
        .split_once('@')
        .map(|(u, _)| u)
        .unwrap_or(&username);
    let actor_row = ActorQueries::find_local_by_username(&state.db, local_username)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let is_public = AccountQueries::find_by_actor_id(&state.db, actor_row.id)
        .await
        .map(|a| a.public_profile)
        .unwrap_or(true);

    let visible = if is_public {
        true
    } else if let Some(t) = token {
        match AccountQueries::find_by_token(&state.db, &t).await {
            Ok(viewer_account) => {
                viewer_account.actor_id == actor_row.id
                    || FollowQueries::is_following_accepted(
                        &state.db,
                        viewer_account.actor_id,
                        actor_row.id,
                    )
                    .await
                    .unwrap_or(false)
            }
            Err(_) => false,
        }
    } else {
        false
    };

    if !visible {
        return Ok(ConnectionsResult {
            visible: false,
            following: vec![],
            followers: vec![],
        });
    }

    let following = FollowQueries::list_following_detail(&state.db, actor_row.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let followers = FollowQueries::list_followers_detail(&state.db, actor_row.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(ConnectionsResult {
        visible: true,
        following: following
            .into_iter()
            .map(|r| crate::web::ConnectionItem {
                ap_id: r.ap_id,
                username: r.username,
                domain: r.domain,
                is_local: r.is_local,
                display_name: r.display_name,
            })
            .collect(),
        followers: followers
            .into_iter()
            .map(|r| crate::web::ConnectionItem {
                ap_id: r.ap_id,
                username: r.username,
                domain: r.domain,
                is_local: r.is_local,
                display_name: r.display_name,
            })
            .collect(),
    })
}

#[server]
pub async fn get_unread_notification_count(token: String) -> Result<i64, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    crate::db::queries::NotificationQueries::count_unread(&state.db, account.actor_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_notifications(
    token: String,
) -> Result<Vec<crate::web::NotificationItem>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let rows = crate::db::queries::NotificationQueries::list(&state.db, account.actor_id, 50)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| crate::web::NotificationItem {
            id: r.id.to_string(),
            kind: r.kind,
            from_ap_id: r.from_ap_id,
            from_username: r.from_username,
            from_display_name: r.from_display_name,
            from_avatar_url: r.from_avatar_url,
            object_ap_id: r.object_ap_id,
            object_title: r.object_title,
            is_read: r.is_read,
            created_at: r.created_at.to_rfc3339(),
        })
        .collect())
}

#[server]
pub async fn mark_all_notifications_read(token: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    crate::db::queries::NotificationQueries::mark_all_read(&state.db, account.actor_id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn dismiss_notification(
    token: String,
    notification_id: String,
) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let nid = parse_uuid(&notification_id)?;
    crate::db::queries::NotificationQueries::dismiss(&state.db, account.actor_id, nid)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

/// Fetch a parent `FeedItem` plus its flat, chronological list of local replies.
#[server]
pub async fn get_thread(
    object_ap_id: String,
    token: Option<String>,
) -> Result<(FeedItem, Vec<ThreadItem>), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();

    let viewer_actor_ap_id: Option<String> = if let Some(ref t) = token {
        match AccountQueries::find_by_token(&state.db, t).await {
            Ok(acc) => match ActorQueries::find_by_id(&state.db, acc.actor_id).await {
                Ok(a) => Some(a.ap_id.to_string()),
                Err(_) => None,
            },
            Err(_) => None,
        }
    } else {
        None
    };

    // Fetch the parent object — accept either a full AP ID (starts with "http")
    // or a bare UUID (last segment of any AP ID, works for Notes and Exercises).
    let parent = if object_ap_id.starts_with("http") {
        ObjectQueries::find_by_ap_id(&state.db, &object_ap_id).await
    } else {
        ObjectQueries::find_by_uuid(&state.db, &object_ap_id).await
    }
    .map_err(|_| ServerFnError::new("post not found"))?;
    let object_ap_id = parent.ap_id.to_string();

    // Resolve parent actor info.
    let parent_actor = ActorQueries::find_by_ap_id(&state.db, &parent.attributed_to)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let parent_ap_ids = vec![object_ap_id.clone()];
    let parent_like_counts = LikeQueries::count_batch(&state.db, &parent_ap_ids)
        .await
        .unwrap_or_default();
    let parent_viewer_liked = if let Some(ref vap) = viewer_actor_ap_id {
        LikeQueries::viewer_liked_batch(&state.db, vap, &parent_ap_ids)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    let image_urls = MediaAttachmentQueries::fetch_for_objects(&state.db, &parent_ap_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.object_ap_id == object_ap_id)
        .map(|a| a.url)
        .collect::<Vec<_>>();

    // For Exercise objects, load all stats and hidden_stats from the DB.
    // The vocabulary v0.2 wire format no longer carries inline metrics, so
    // ap_json parsing is unreliable for objects created after the vocabulary
    // upgrade. The exercises table is always authoritative.
    let (
        exercise_type,
        duration_s,
        distance_m,
        elevation_gain_m,
        avg_heart_rate_bpm,
        max_heart_rate_bpm,
        avg_power_w,
        max_power_w,
        normalized_power_w,
        avg_cadence_rpm,
        avg_pace_s_per_km,
        device,
        title,
        parent_hidden_stats,
    ) = if parent.object_type == "Exercise" {
        match ExerciseQueries::find_by_ap_id(&state.db, &object_ap_id).await {
            Ok(row) => (
                Some(row.activity_type.clone()),
                if row.duration_s > 0 {
                    Some(row.duration_s as i64)
                } else {
                    None
                },
                if row.distance_m > 0.0 {
                    Some(row.distance_m)
                } else {
                    None
                },
                row.elevation_gain_m,
                row.avg_heart_rate_bpm,
                row.max_heart_rate_bpm,
                row.avg_power_w,
                row.max_power_w,
                row.normalized_power_w,
                row.avg_cadence_rpm,
                row.avg_pace_s_per_km,
                row.device,
                row.title,
                row.hidden_stats.0,
            ),
            Err(_) => (
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                vec![],
            ),
        }
    } else {
        (
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            vec![],
        )
    };

    let parent_item = FeedItem {
        id: object_ap_id.clone(),
        object_ap_id: object_ap_id.clone(),
        viewer_is_owner: viewer_actor_ap_id
            .as_ref()
            .is_some_and(|v| *v == parent_actor.ap_id.to_string()),
        actor_username: parent_actor.username.clone(),
        actor_domain: parent_actor.domain.clone(),
        actor_is_local: parent_actor.is_local,
        actor_ap_id: parent_actor.ap_id.to_string(),
        actor_avatar_url: parent_actor.avatar_url.clone(),
        activity_type: "Create".to_string(),
        object_type: parent.object_type.clone(),
        content: parent.content.clone(),
        published: parent.published.map(|p| p.to_rfc3339()).unwrap_or_default(),
        exercise_type,
        duration_s,
        distance_m,
        elevation_gain_m,
        avg_heart_rate_bpm,
        max_heart_rate_bpm,
        avg_power_w,
        max_power_w,
        normalized_power_w,
        avg_cadence_rpm,
        avg_pace_s_per_km,
        device,
        title,
        image_urls,
        like_count: parent_like_counts.get(&object_ap_id).copied().unwrap_or(0),
        viewer_has_liked: parent_viewer_liked.contains(&object_ap_id),
        reply_count: parent.reply_count as i64,
        in_reply_to: parent.in_reply_to.clone(),
        route_url: parent
            .ap_json
            .get("routeUrl")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        hidden_stats: parent_hidden_stats,
        via_club_handle: None,
        via_club_display: None,
    };

    // Fetch replies.
    let replies = ObjectQueries::find_replies(&state.db, &object_ap_id)
        .await
        .unwrap_or_default();

    let reply_ap_ids: Vec<String> = replies.iter().map(|r| r.ap_id.to_string()).collect();
    let reply_like_counts = LikeQueries::count_batch(&state.db, &reply_ap_ids)
        .await
        .unwrap_or_default();
    let reply_viewer_liked = if let Some(ref vap) = viewer_actor_ap_id {
        LikeQueries::viewer_liked_batch(&state.db, vap, &reply_ap_ids)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashSet::new()
    };

    let mut thread_items = Vec::new();
    for reply in replies {
        let author = ActorQueries::find_by_ap_id(&state.db, &reply.attributed_to)
            .await
            .ok();
        let ap_id = reply.ap_id.to_string();
        let like_count = reply_like_counts.get(&ap_id).copied().unwrap_or(0);
        let viewer_has_liked = reply_viewer_liked.contains(&ap_id);
        let viewer_is_owner = viewer_actor_ap_id
            .as_ref()
            .is_some_and(|v| *v == reply.attributed_to);
        thread_items.push(ThreadItem {
            ap_id,
            author_username: author
                .as_ref()
                .map(|a| a.username.clone())
                .unwrap_or_default(),
            author_avatar_url: author.and_then(|a| a.avatar_url),
            content: reply.content,
            published: reply.published.map(|p| p.to_rfc3339()).unwrap_or_default(),
            like_count,
            viewer_has_liked,
            viewer_is_owner,
        });
    }

    Ok((parent_item, thread_items))
}

/// Post a reply to `in_reply_to_ap_id`. Returns the new `ThreadItem`.
#[server]
pub async fn create_reply(
    token: String,
    content: String,
    in_reply_to_ap_id: String,
) -> Result<ThreadItem, ServerFnError> {
    use crate::web::server::*;
    if content.trim().is_empty() {
        return Err(ServerFnError::new("content must not be empty"));
    }
    if content.len() > 10_000 {
        return Err(ServerFnError::new("content exceeds 10 000 character limit"));
    }

    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;

    let base = actor.ap_id.to_string();
    let note_id = format!("{base}/notes/{}", Uuid::now_v7());
    let activity_id = format!("{base}/activities/{}", Uuid::now_v7());
    let published = Utc::now();

    let note_json = serde_json::json!({
        "@context":    "https://www.w3.org/ns/activitystreams",
        "type":        "Note",
        "id":          &note_id,
        "attributedTo": &base,
        "content":     &content,
        "sensitive":   false,
        "published":   published.to_rfc3339(),
        "inReplyTo":   &in_reply_to_ap_id,
        "to":          ["https://www.w3.org/ns/activitystreams#Public"],
        "cc":          [format!("{base}/followers")],
    });

    let activity_json = serde_json::json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type":     "Create",
        "id":       &activity_id,
        "actor":    &base,
        "published": published.to_rfc3339(),
        "to":       ["https://www.w3.org/ns/activitystreams#Public"],
        "cc":       [format!("{base}/followers")],
        "object":   note_json.clone(),
    });

    // Atomic insert: object + activity + outbox.
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    sqlx::query(
        r#"INSERT INTO objects
           (ap_id, object_type, attributed_to, actor_id, content, sensitive,
            in_reply_to, published, url, ap_json, visibility)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(note_id.clone())
    .bind("Note")
    .bind(base.clone())
    .bind(actor.id)
    .bind(content.clone())
    .bind(false)
    .bind(in_reply_to_ap_id.clone())
    .bind(published)
    .bind(note_id.clone())
    .bind(note_json.clone())
    .bind("public")
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let activity_row = sqlx::query(
        r#"INSERT INTO activities (ap_id, activity_type, actor_id, object_ap_id, ap_json)
           VALUES (?, ?, ?, ?, ?)
           RETURNING id"#,
    )
    .bind(activity_id.clone())
    .bind("Create")
    .bind(actor.id)
    .bind(note_id.clone())
    .bind(activity_json)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    let activity_uuid: uuid::Uuid = {
        use sqlx::Row as _;
        activity_row
            .try_get("id")
            .map_err(|e| ServerFnError::new(e.to_string()))?
    };

    sqlx::query(
        r#"INSERT INTO outbox_items (owner_id, activity_id) VALUES (?, ?)
           ON CONFLICT (owner_id, activity_id) DO NOTHING"#,
    )
    .bind(actor.id)
    .bind(activity_uuid)
    .execute(&mut *tx)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Notify the local owner of the parent post (non-fatal, skip on error).
    {
        let db = state.db.clone();
        let replier_id = actor.id;
        let parent_ap_id = in_reply_to_ap_id.clone();
        tokio::spawn(async move {
            let parent = match ObjectQueries::find_by_ap_id(&db, &parent_ap_id).await {
                Ok(o) => o,
                Err(_) => return,
            };
            let owner_id = match parent.actor_id {
                Some(id) => id,
                None => return, // parent is remote — skip
            };
            if owner_id == replier_id {
                return; // no self-notification
            }
            let owner_is_local = AccountQueries::is_actor_local(&db, owner_id)
                .await
                .unwrap_or(false);
            if !owner_is_local {
                return;
            }
            let _ = crate::db::queries::NotificationQueries::insert(
                &db,
                owner_id,
                "reply",
                replier_id,
                Some(&parent_ap_id), // store parent so clicking notification opens the parent post
                None,
            )
            .await;
        });
    }

    // Deliver the reply to the remote parent post's author (non-fatal).
    {
        let db = state.db.clone();
        let activity_id = activity_uuid;
        let parent_ap_id = in_reply_to_ap_id.clone();
        tokio::spawn(async move {
            // Look up the parent's author actor.
            let parent = match ObjectQueries::find_by_ap_id(&db, &parent_ap_id).await {
                Ok(o) => o,
                Err(_) => return,
            };
            let author = match ActorQueries::find_by_ap_id(&db, &parent.attributed_to).await {
                Ok(a) => a,
                Err(_) => return,
            };
            // Skip if the author is local — notification already handled above.
            if author.is_local {
                return;
            }
            let inbox = author
                .shared_inbox_url
                .as_deref()
                .unwrap_or(&author.inbox_url)
                .to_owned();
            let _ =
                crate::db::queries::DeliveryQueries::insert_deliveries(&db, activity_id, &[inbox])
                    .await;
        });
    }

    Ok(ThreadItem {
        ap_id: note_id,
        author_username: actor.username,
        author_avatar_url: actor.avatar_url,
        content: Some(content),
        published: published.to_rfc3339(),
        like_count: 0,
        viewer_has_liked: false,
        viewer_is_owner: true,
    })
}

/// Fetch GeoJSON coordinates for an exercise route.
///
/// `route_url` is the full GeoJSON endpoint URL
/// (e.g. `http://localhost:8080/api/exercises/{uuid}/route`).
///
/// For **local** exercises (UUID found in this instance's DB) the query runs
/// against the local database with full visibility / auth enforcement.
///
/// For **remote** exercises (UUID not in local DB) the function makes a
/// server-side HTTP GET to `route_url`.  Only public routes are accessible
/// this way; follower/private remote routes are not fetched.
///
/// Returns `Some(Vec<(lat, lon)>)` on success, `None` when no route is
/// stored, silently returns `Ok(None)` on auth / not-found failures so the
/// card degrades gracefully.
#[server]
pub async fn get_exercise_route_fn(
    route_url: String,
    token: Option<String>,
) -> Result<Option<Vec<(f64, f64, Option<f64>)>>, ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();

    // Extract base58 ID from the route URL: .../api/exercises/{b58}/route
    let exercise_b58 = route_url
        .trim_end_matches("/route")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_owned();

    let id = match bs58::decode(&exercise_b58).into_vec() {
        Ok(bytes) => match uuid::Uuid::from_slice(&bytes) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };

    fn parse_geojson(route: serde_json::Value) -> Option<Vec<(f64, f64, Option<f64>)>> {
        route["coordinates"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let lon = c.get(0)?.as_f64()?;
                    let lat = c.get(1)?.as_f64()?;
                    let ele = c.get(2).and_then(|v| v.as_f64());
                    Some((lat, lon, ele))
                })
                .collect()
        })
    }

    match ExerciseQueries::find_with_route(&state.db, id).await {
        Ok(row) => {
            match row.visibility.as_str() {
                "public" => {}
                "followers" => {
                    let tok = match token {
                        Some(t) => t,
                        None => return Ok(None),
                    };
                    let account = match AccountQueries::find_by_token(&state.db, &tok).await {
                        Ok(a) => a,
                        Err(_) => return Ok(None),
                    };
                    let actor = match ActorQueries::find_by_id(&state.db, account.actor_id).await {
                        Ok(a) => a,
                        Err(_) => return Ok(None),
                    };
                    if actor.id != row.actor_id {
                        let follows =
                            FollowQueries::is_following(&state.db, actor.id, row.actor_id)
                                .await
                                .unwrap_or(false);
                        if !follows {
                            return Ok(None);
                        }
                    }
                }
                _ => {
                    let tok = match token {
                        Some(t) => t,
                        None => return Ok(None),
                    };
                    let account = match AccountQueries::find_by_token(&state.db, &tok).await {
                        Ok(a) => a,
                        Err(_) => return Ok(None),
                    };
                    let actor = match ActorQueries::find_by_id(&state.db, account.actor_id).await {
                        Ok(a) => a,
                        Err(_) => return Ok(None),
                    };
                    if actor.id != row.actor_id {
                        return Ok(None);
                    }
                }
            }
            return Ok(row.route.and_then(|v| parse_geojson(v.0)));
        }
        Err(_) => {
            // Not a local exercise — fall through to remote HTTP fetch.
        }
    }

    // Only public routes are accessible without cross-instance auth.
    // Rewrite https://localhost → http://localhost in debug builds.
    #[cfg(debug_assertions)]
    let fetch_url = if route_url.starts_with("https://localhost") {
        route_url.replacen("https://", "http://", 1)
    } else {
        route_url.clone()
    };
    #[cfg(not(debug_assertions))]
    let fetch_url = route_url.clone();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let resp = client.get(&fetch_url).send().await;
    let geojson: serde_json::Value = match resp {
        Ok(r) if r.status().is_success() => match r.json().await {
            Ok(j) => j,
            Err(_) => return Ok(None),
        },
        _ => return Ok(None),
    };

    Ok(parse_geojson(geojson))
}

#[server]
pub async fn update_post(
    token: String,
    object_ap_id: String,
    content: Option<String>,
    title: Option<String>,
    hidden_stats: Vec<String>,
    removed_image_urls: Vec<String>,
) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let data = request_data();
    let state = _rd.app_data();

    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;

    crate::server::service::do_update_post(
        &data,
        account.actor_id,
        &object_ap_id,
        crate::server::service::UpdatePostRequest {
            content,
            title,
            hidden_stats,
            removed_image_urls,
        },
    )
    .await
    .map_err(into_sfn_err)
}

#[server]
pub async fn add_alias(token: String, alias: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let data = request_data();
    let state = data.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    crate::server::service::do_add_alias(&data, &actor, &alias)
        .await
        .map_err(into_sfn_err)
}

#[server]
pub async fn remove_alias(token: String, alias: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let _rd = request_data();
    let state = _rd.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    crate::server::service::do_remove_alias(&state.db, account.actor_id, &alias)
        .await
        .map_err(into_sfn_err)
}

#[server]
pub async fn move_account(token: String, target: String) -> Result<(), ServerFnError> {
    use crate::web::server::*;
    let data = request_data();
    let state = data.app_data();
    let account = AccountQueries::find_by_token(&state.db, &token)
        .await
        .map_err(|_| ServerFnError::new("invalid token"))?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id)
        .await
        .map_err(|_| ServerFnError::new("actor not found"))?;
    crate::server::service::do_move_account(&data, &actor, &target)
        .await
        .map_err(into_sfn_err)
}
