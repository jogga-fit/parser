use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

/// Joined notification row — used for display in the UI.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct NotificationDetailRow {
    pub id: Uuid,
    pub kind: String,
    pub from_ap_id: String,
    pub from_username: String,
    pub from_display_name: Option<String>,
    pub from_avatar_url: Option<String>,
    pub object_ap_id: Option<String>,
    pub object_title: Option<String>,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}
