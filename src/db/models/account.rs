use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// DB row mapping to the `local_accounts` table.
///
/// Sovereign-mode differences from the multi-user version:
///   * `is_admin` is dropped — a single owner is implicitly an admin.
///   * `show_in_directory` is dropped — there is no instance directory.
#[derive(Debug, Clone, FromRow)]
pub struct LocalAccount {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub password_hash: String,
    pub api_token: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub public_profile: bool,
    pub theme: String,
    pub created_at: DateTime<Utc>,
}
