use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// DB row mapping to the `media_attachments` table.
///
/// `position` is `i32` (SQLite has no SMALLINT — INTEGER spans 64 bits).
#[derive(Debug, Clone, FromRow)]
pub struct MediaAttachmentRow {
    pub id: Uuid,
    pub object_ap_id: String,
    pub url: String,
    pub media_type: String,
    pub caption: Option<String>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}
