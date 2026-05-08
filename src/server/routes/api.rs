//! `/api/v1/*` — write API for the single local owner.
//!
//! No registration flow (owner is seeded via `seed-owner` CLI).
//! Password reset: owner's contact is from config or stored account contact.

use activitypub_federation::config::Data;
use axum::{
    Json,
    extract::{Multipart, Query},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use uuid::Uuid;

use crate::{
    db::queries::{AccountQueries, ActorQueries, FollowQueries},
    server::{auth::AuthenticatedUser, error::AppError, state::AppState},
};

#[derive(Deserialize)]
pub struct TokenRequest {
    pub login: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub username: String,
}

pub async fn token(
    data: Data<AppState>,
    Json(req): Json<TokenRequest>,
) -> Result<impl IntoResponse, AppError> {
    let state = data.app_data();
    let tok = crate::server::service::do_login(state, &req.login, &req.password).await?;
    let account = AccountQueries::find_by_token(&state.db, &tok).await?;
    let actor = ActorQueries::find_by_id(&state.db, account.actor_id).await?;
    Ok(Json(TokenResponse {
        token: tok,
        username: actor.username,
    }))
}

pub async fn get_me(AuthenticatedUser { actor, account }: AuthenticatedUser) -> impl IntoResponse {
    Json(json!({
        "username":     actor.username,
        "ap_id":        actor.ap_id.to_string(),
        "display_name": actor.display_name,
        "bio":          actor.summary,
        "email":        account.email,
        "phone":        account.phone,
        "created_at":   actor.created_at,
    }))
}

#[derive(Deserialize)]
pub struct UpdateMeRequest {
    pub display_name: Option<String>,
    pub bio: Option<String>,
}

pub async fn update_me(
    data: Data<AppState>,
    AuthenticatedUser { actor, .. }: AuthenticatedUser,
    Json(req): Json<UpdateMeRequest>,
) -> Result<impl IntoResponse, AppError> {
    ActorQueries::update_profile(
        &data.db,
        actor.id,
        req.display_name.as_deref(),
        req.bio.as_deref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_me(
    data: Data<AppState>,
    AuthenticatedUser { actor, .. }: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    ActorQueries::delete(&data.db, actor.id).await?;
    info!(username = actor.username, "account deleted");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct PrivacySettingsRequest {
    pub public_profile: bool,
}

pub async fn update_privacy(
    data: Data<AppState>,
    AuthenticatedUser { actor, account }: AuthenticatedUser,
    Json(req): Json<PrivacySettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    AccountQueries::update_privacy_settings(&data.db, account.id, req.public_profile).await?;
    ActorQueries::set_manually_approves_followers(&data.db, actor.id, !req.public_profile).await?;
    if req.public_profile {
        FollowQueries::accept_all_pending(&data.db, actor.id).await?;
    }
    info!(
        username = actor.username,
        public_profile = req.public_profile,
        "privacy settings updated"
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_account(
    data: Data<AppState>,
    axum::extract::Path(username): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let username = username.trim_start_matches('@');
    let actor = ActorQueries::find_local_by_username(&data.db, username).await?;
    Ok(Json(json!({
        "username": actor.username,
        "ap_id":    actor.ap_id.to_string(),
    })))
}

#[derive(Deserialize)]
pub struct PasswordResetInitRequest {
    /// Owner contact (email or phone). If omitted, uses owner config contact.
    pub contact: Option<String>,
}

pub async fn password_reset_init(
    data: Data<AppState>,
    Json(req): Json<PasswordResetInitRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Resolve contact: request body OR owner config contact.
    let contact = req
        .contact
        .unwrap_or_else(|| data.app_data().config.owner.contact.clone());

    let (otp_id, dev_code) =
        crate::server::service::do_password_reset_init(data.app_data(), &contact).await?;

    let mut resp = json!({ "otp_id": otp_id });
    if cfg!(debug_assertions) {
        if let Some(code) = dev_code {
            resp["code"] = json!(code);
        }
    }

    Ok((StatusCode::ACCEPTED, Json(resp)))
}

#[derive(Deserialize)]
pub struct PasswordResetVerifyRequest {
    pub otp_id: Uuid,
    pub code: String,
    pub new_password: String,
}

#[derive(Serialize)]
pub struct PasswordResetVerifyResponse {
    pub token: String,
}

pub async fn password_reset_verify(
    data: Data<AppState>,
    Json(req): Json<PasswordResetVerifyRequest>,
) -> Result<impl IntoResponse, AppError> {
    let token = crate::server::service::do_password_reset_verify(
        data.app_data(),
        req.otp_id,
        &req.code,
        &req.new_password,
    )
    .await?;
    Ok(Json(PasswordResetVerifyResponse { token }))
}

pub async fn list_my_followers(
    data: Data<AppState>,
    AuthenticatedUser { actor, .. }: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let rows = crate::db::queries::FollowQueries::list_followers(&data.db, actor.id, 200).await?;
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "ap_id":        r.ap_id,
                "username":     r.username,
                "domain":       r.domain,
                "is_local":     r.is_local,
                "display_name": r.display_name,
                "avatar_url":   r.avatar_url,
                "accepted":     r.accepted,
                "follow_ap_id": r.follow_ap_id,
            })
        })
        .collect();
    Ok(Json(items))
}

pub async fn list_my_following(
    data: Data<AppState>,
    AuthenticatedUser { actor, .. }: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    let rows = crate::db::queries::FollowQueries::list_following(&data.db, actor.id, 200).await?;
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "ap_id":        r.ap_id,
                "username":     r.username,
                "domain":       r.domain,
                "is_local":     r.is_local,
                "display_name": r.display_name,
                "avatar_url":   r.avatar_url,
                "accepted":     r.accepted,
            })
        })
        .collect();
    Ok(Json(items))
}

#[derive(Deserialize)]
pub struct FollowRequest {
    #[serde(alias = "ap_id")]
    pub target: String,
}

pub async fn follow(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Json(req): Json<FollowRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ap_id = if req.target.starts_with('@') {
        crate::server::service::resolve_handle(data.app_data(), &req.target).await?
    } else {
        req.target
    };
    crate::server::service::do_follow(&data, auth.actor.id, &ap_id, false).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
pub struct UnfollowRequest {
    #[serde(alias = "ap_id")]
    pub target: String,
}

pub async fn unfollow(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Json(req): Json<UnfollowRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ap_id = if req.target.starts_with('@') {
        crate::server::service::resolve_handle(data.app_data(), &req.target).await?
    } else {
        req.target
    };
    crate::server::service::do_unfollow(&data, auth.actor.id, &ap_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct MoveRequest {
    pub target: String,
}

pub async fn move_account(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Json(req): Json<MoveRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::server::service::do_move_account(&data, &auth.actor, &req.target).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
pub struct AliasRequest {
    pub also_known_as: String,
}

pub async fn add_alias(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Json(req): Json<AliasRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::server::service::do_add_alias(&data, &auth.actor, &req.also_known_as).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_alias(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Json(req): Json<AliasRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::server::service::do_remove_alias(&data.db, auth.actor.id, &req.also_known_as).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct UploadExerciseResponse {
    pub id: Uuid,
    pub ap_id: String,
}

pub async fn upload_exercise(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let mut activity_type: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::from("activity.gpx");
    let mut visibility: Option<String> = None;
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut hidden_stats: Vec<String> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        match field.name() {
            Some("activityType") => {
                activity_type = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(e.to_string()))?,
                );
            }
            Some("gpx") | Some("fit") | Some("file") => {
                let fname = field
                    .file_name()
                    .map(str::to_owned)
                    .unwrap_or_else(|| "activity.gpx".into());
                file_name = fname;
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::BadRequest(e.to_string()))?
                        .to_vec(),
                );
            }
            Some("visibility") => {
                visibility = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(e.to_string()))?,
                );
            }
            Some("title") => {
                title = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(e.to_string()))?,
                );
            }
            Some("description") => {
                description = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(e.to_string()))?,
                );
            }
            Some("hiddenStats") => {
                let v = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                if !v.is_empty() {
                    hidden_stats.push(v);
                }
            }
            _ => {}
        }
    }

    let activity_type =
        activity_type.ok_or_else(|| AppError::BadRequest("missing activityType field".into()))?;
    let file_bytes =
        file_bytes.ok_or_else(|| AppError::BadRequest("missing file field (gpx or fit)".into()))?;

    let file_type = if file_name.to_lowercase().ends_with(".fit") {
        "fit"
    } else {
        "gpx"
    }
    .to_string();

    let result = crate::server::service::do_upload_exercise(
        data.app_data(),
        &auth.actor,
        crate::server::service::ExerciseUploadRequest {
            activity_type,
            file_bytes,
            file_type,
            file_name,
            visibility: visibility.unwrap_or_else(|| "public".to_string()),
            title,
            description,
            image_urls: vec![],
            hidden_stats,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(UploadExerciseResponse {
            id: result.id,
            ap_id: result.ap_id,
        }),
    ))
}

#[derive(Deserialize)]
pub struct LikeRequest {
    pub object_ap_id: String,
}

pub async fn like(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Json(req): Json<LikeRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::server::service::do_like(&data, auth.actor.id, &req.object_ap_id).await?;
    Ok(StatusCode::ACCEPTED)
}

pub async fn unlike(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Json(req): Json<LikeRequest>,
) -> Result<impl IntoResponse, AppError> {
    crate::server::service::do_unlike(&data, auth.actor.id, &req.object_ap_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct PaginationParams {
    /// Maximum number of items to return. Defaults to 20, capped at 100.
    pub limit: Option<i64>,
}

pub async fn list_notifications(
    data: Data<AppState>,
    auth: AuthenticatedUser,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, AppError> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let notifications =
        crate::db::queries::NotificationQueries::list(&data.db, auth.actor.id, limit).await?;
    Ok(Json(json!({ "notifications": notifications })))
}

pub async fn mark_notifications_read(
    data: Data<AppState>,
    auth: AuthenticatedUser,
) -> Result<impl IntoResponse, AppError> {
    crate::db::queries::NotificationQueries::mark_all_read(&data.db, auth.actor.id).await?;
    Ok(StatusCode::NO_CONTENT)
}
