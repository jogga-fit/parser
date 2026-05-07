use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::db::types::{JsonValue, JsonVec};

/// DB row mapping to the `activities` table.
#[derive(Debug, Clone, FromRow)]
pub struct ActivityRow {
    pub id: Uuid,
    pub ap_id: String,
    pub activity_type: String,
    pub actor_id: Uuid,
    pub object_ap_id: String,
    pub target_ap_id: Option<String>,
    pub object_id: Option<Uuid>,
    /// Stored as JSON-encoded TEXT in SQLite.
    pub ap_json: JsonValue,
    pub published: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Activity joined with actor info — used for home-timeline display.
#[derive(Debug, Clone, FromRow)]
pub struct FeedRow {
    pub activity_ap_id: String,
    pub activity_type: String,
    pub actor_username: String,
    pub actor_domain: String,
    pub actor_is_local: bool,
    pub actor_ap_id: String,
    pub actor_avatar_url: Option<String>,
    pub ap_json: JsonValue,
    pub published: DateTime<Utc>,
}

/// Activity + object joined row — used for actor-profile post listing.
#[derive(Debug, Clone, FromRow)]
pub struct ProfilePostRow {
    pub object_ap_id: String,
    pub object_type: String,
    pub content: Option<String>,
    pub published: DateTime<Utc>,
    /// Set only for Exercise objects.
    pub exercise_type: Option<String>,
    pub duration_s: Option<i32>,
    pub distance_m: Option<f64>,
    pub elevation_gain_m: Option<f64>,
    pub avg_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub avg_power_w: Option<f64>,
    pub max_power_w: Option<f64>,
    pub normalized_power_w: Option<f64>,
    pub avg_cadence_rpm: Option<f64>,
    pub avg_pace_s_per_km: Option<f64>,
    pub device: Option<String>,
    pub title: Option<String>,
    pub exercise_visibility: Option<String>,
    /// JSON-encoded TEXT, default `[]`.
    pub hidden_stats: JsonVec<String>,
}
