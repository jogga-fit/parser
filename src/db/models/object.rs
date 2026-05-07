use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::db::types::JsonValue;

/// DB row mapping to the `objects` table.
#[derive(Debug, Clone, FromRow)]
pub struct ObjectRow {
    pub id: Uuid,
    pub ap_id: String,
    pub object_type: String,
    pub attributed_to: String,
    pub actor_id: Option<Uuid>,
    pub content: Option<String>,
    pub content_map: Option<JsonValue>,
    pub summary: Option<String>,
    pub sensitive: bool,
    pub in_reply_to: Option<String>,
    pub reply_count: i32,
    pub published: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub ap_json: JsonValue,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
