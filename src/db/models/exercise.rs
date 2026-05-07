use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::db::types::{JsonValue, JsonVec};

/// DB row mapping to the `exercises` table joined with `actors.ap_id` and
/// `objects.ap_id`. Neither `actor_ap_id` nor `object_ap_id` is a column on
/// `exercises` itself — both come from JOINs.
#[derive(Debug, Clone, FromRow)]
pub struct ExerciseRow {
    pub id: Uuid,
    pub actor_id: Uuid,
    /// The AP ID (URL) of the owning actor, joined from `actors.ap_id`.
    pub actor_ap_id: String,
    /// The AP ID (URL) of the exercise object, joined from `objects.ap_id`.
    pub object_ap_id: String,
    pub object_id: Uuid,
    pub activity_type: String,
    pub started_at: DateTime<Utc>,
    pub duration_s: i32,
    pub distance_m: f64,
    pub elevation_gain_m: Option<f64>,
    pub avg_pace_s_per_km: Option<f64>,
    pub avg_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub avg_cadence_rpm: Option<f64>,
    pub avg_power_w: Option<f64>,
    pub max_power_w: Option<f64>,
    pub normalized_power_w: Option<f64>,
    pub title: Option<String>,
    pub file_type: String,
    pub device: Option<String>,
    pub gpx_url: Option<String>,
    pub visibility: String,
    pub hidden_stats: JsonVec<String>,
    pub created_at: DateTime<Utc>,
}

/// DB row for the route endpoint — includes the `route` JSON column.
#[derive(Debug, Clone, FromRow)]
pub struct ExerciseRouteRow {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub visibility: String,
    pub route: Option<JsonValue>,
}
