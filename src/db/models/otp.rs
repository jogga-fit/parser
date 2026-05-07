use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// DB row mapping to the `otp_requests` table.
#[derive(Debug, Clone, FromRow)]
pub struct OtpRequest {
    pub id: Uuid,
    pub contact: String,
    pub contact_type: String,
    pub purpose: String,
    pub username: Option<String>,
    pub code_hash: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
