use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("not acceptable")]
    NotAcceptable,

    #[error("conflict: {0}")]
    Conflict(String),

    /// Resource has migrated. Body carries the new AP id (Mastodon-compatible
    /// 410 Gone for moved actors).
    #[error("gone: {0}")]
    Gone(String),

    /// Feature not available on this instance (e.g. email/SMS not configured).
    #[error("not available: {0}")]
    NotAvailable(String),

    #[error("internal error: {0}")]
    Internal(#[from] InternalError),
}

/// Structured internal errors — each variant identifies the subsystem that failed.
/// Never leaked to HTTP clients; always mapped to 500.
#[derive(Debug, Error)]
pub enum InternalError {
    #[error("url parse: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),

    #[error("otp delivery: {0}")]
    OtpDelivery(#[from] crate::server::notify::NotifyError),

    #[error("data integrity: {0}")]
    DataIntegrity(String),

    #[error("federation: {0}")]
    Federation(String),

    #[error("database: {0}")]
    Database(String),

    #[error("missing extension: {0}")]
    MissingExtension(&'static str),

    #[error("unexpected: {0}")]
    Unexpected(String),
}

/// Errors from cryptographic operations used in auth and registration.
/// Defined here (not in `auth.rs`) to avoid a circular import with `error.rs`.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// Covers both hash creation and verification failures from argon2.
    #[error("password hash error: {0}")]
    Hash(#[from] argon2::password_hash::Error),

    #[error("rsa key generation failed: {0}")]
    KeyGen(String),

    #[error("pem encoding failed: {0}")]
    PemEncode(String),
}

/// Allows `url::ParseError` to propagate directly via `?` in handlers.
impl From<url::ParseError> for AppError {
    fn from(e: url::ParseError) -> Self {
        AppError::Internal(InternalError::UrlParse(e))
    }
}

/// Allows `CryptoError` to propagate directly via `?` in handlers.
impl From<CryptoError> for AppError {
    fn from(e: CryptoError) -> Self {
        AppError::Internal(InternalError::Crypto(e))
    }
}

/// Allows `NotifyError` to propagate directly via `?` in handlers.
impl From<crate::server::notify::NotifyError> for AppError {
    fn from(e: crate::server::notify::NotifyError) -> Self {
        AppError::Internal(InternalError::OtpDelivery(e))
    }
}

impl From<activitypub_federation::error::Error> for AppError {
    fn from(e: activitypub_federation::error::Error) -> Self {
        AppError::Internal(InternalError::Federation(e.to_string()))
    }
}

impl From<crate::db::DbError> for AppError {
    fn from(e: crate::db::DbError) -> Self {
        match e {
            crate::db::DbError::NotFound => AppError::NotFound,
            crate::db::DbError::Conflict(msg) => AppError::BadRequest(msg),
            other => AppError::Internal(InternalError::Database(other.to_string())),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            AppError::NotAcceptable => (StatusCode::NOT_ACCEPTABLE, "not acceptable".to_string()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Gone(msg) => (StatusCode::GONE, msg.clone()),
            AppError::NotAvailable(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            AppError::Internal(e) => {
                error!(error = %e, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };
        (status, Json(json!({"error": message}))).into_response()
    }
}
