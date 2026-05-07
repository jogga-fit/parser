use activitypub_federation::config::FederationConfig;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{extract::FromRequestParts, http::request::Parts};
use http::header::AUTHORIZATION;
use rand::RngCore;

use crate::{
    db::{
        LocalAccount,
        models::ActorRow,
        queries::{AccountQueries, ActorQueries},
    },
    server::{
        error::{AppError, CryptoError, InternalError},
        state::AppState,
    },
};

/// Authenticated caller — actor row + account row extracted from Bearer token.
pub struct AuthenticatedUser {
    pub actor: ActorRow,
    pub account: LocalAccount,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, AppError> {
        let token = bearer_token(parts).ok_or(AppError::Unauthorized)?;

        let pool = parts
            .extensions
            .get::<FederationConfig<AppState>>()
            .ok_or(AppError::Internal(InternalError::MissingExtension(
                "federation config extension",
            )))?
            .to_request_data()
            .app_data()
            .db
            .clone();

        let account = AccountQueries::find_by_token(&pool, &token)
            .await
            .map_err(|_| AppError::Unauthorized)?;

        let actor = ActorQueries::find_by_id(&pool, account.actor_id)
            .await
            .map_err(|_| AppError::Unauthorized)?;

        if actor.is_suspended {
            return Err(AppError::Unauthorized);
        }

        Ok(AuthenticatedUser { actor, account })
    }
}

/// Extract the raw token from `Authorization: Bearer <token>`.
fn bearer_token(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Optional authenticated caller — present if a valid Bearer token is provided.
pub struct OptionalAuth(pub Option<AuthenticatedUser>);

impl<S: Send + Sync> FromRequestParts<S> for OptionalAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        if bearer_token(parts).is_none() {
            return Ok(OptionalAuth(None));
        }
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        Ok(OptionalAuth(Some(user)))
    }
}

/// Hash a plaintext password with argon2id.
pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

/// Return `true` if `password` matches the stored PHC hash string.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, CryptoError> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Generate a cryptographically random 6-digit numeric OTP code.
pub fn generate_otp() -> String {
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    let n = u32::from_be_bytes(bytes) % 900_000 + 100_000;
    n.to_string()
}

/// Hash a low-entropy OTP code with argon2id.
pub fn hash_otp(code: &str) -> Result<String, CryptoError> {
    hash_password(code)
}

/// Return `true` if `code` matches the stored argon2id hash.
pub fn verify_otp(code: &str, hash: &str) -> Result<bool, CryptoError> {
    verify_password(code, hash)
}

/// Generate a cryptographically random 32-byte bearer token as lowercase hex.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 hex digest of a token.
///
/// Tokens are generated as random hex strings and stored only as their SHA-256
/// hash in the DB (column `api_token`).  On lookup the caller hashes the raw
/// token and compares hashes — never storing the plaintext (C3).
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Validate a username against `^[a-zA-Z0-9_]{1,30}$`.
pub fn validate_username(username: &str) -> Result<(), AppError> {
    if username.is_empty() || username.len() > 30 {
        return Err(AppError::BadRequest(
            "username must be 1–30 characters".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AppError::BadRequest(
            "username may only contain letters, digits, and underscores".into(),
        ));
    }
    Ok(())
}

/// Validate a password — minimum 8, maximum 1024 characters.
pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 || password.len() > 1024 {
        return Err(AppError::BadRequest(
            "password must be 8–1024 characters".into(),
        ));
    }
    Ok(())
}

/// Validate an email address.
pub fn validate_email(email: &str) -> Result<(), AppError> {
    email
        .parse::<lettre::message::Mailbox>()
        .map(|_| ())
        .map_err(|e| AppError::BadRequest(format!("invalid email address: {e}")))
}

/// Validate a phone number — must be E.164 format: `+` followed by 7–15 digits.
pub fn validate_phone(phone: &str) -> Result<(), AppError> {
    let digits = phone.strip_prefix('+').ok_or_else(|| {
        AppError::BadRequest("phone must be in E.164 format, e.g. +15005550006".into())
    })?;
    if digits.len() < 7 || digits.len() > 15 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest(
            "phone must be in E.164 format, e.g. +15005550006".into(),
        ));
    }
    Ok(())
}
