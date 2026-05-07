use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// Row from the `outbox_deliveries` table.
#[derive(Debug, Clone, FromRow)]
pub struct DeliveryRow {
    pub id: Uuid,
    pub activity_id: Uuid,
    pub inbox_url: String,
    pub status: String,
    pub attempt_count: i32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub next_retry_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
