use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

/// Row from the `followers` table.
#[derive(Debug, Clone, FromRow)]
pub struct FollowerRow {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub follower_id: Uuid,
    pub accepted: bool,
    pub created_at: DateTime<Utc>,
}

/// Row from the `following` table.
#[derive(Debug, Clone, FromRow)]
pub struct FollowingRow {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub accepted: bool,
    pub created_at: DateTime<Utc>,
}

/// Joined `following` + actor info (for display in the UI).
#[derive(Debug, Clone, FromRow)]
pub struct FollowingDetailRow {
    pub ap_id: String,
    pub username: String,
    pub domain: String,
    pub is_local: bool,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub accepted: bool,
}

/// Joined `followers` + actor info (for display in the UI).
#[derive(Debug, Clone, FromRow)]
pub struct FollowerDetailRow {
    pub ap_id: String,
    pub username: String,
    pub domain: String,
    pub is_local: bool,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub accepted: bool,
    /// Original AP `Follow` activity id URL — present for post-migration
    /// follows only.
    pub follow_ap_id: Option<String>,
}
