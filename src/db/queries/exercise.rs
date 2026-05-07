use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, SqlitePool};
use uuid::Uuid;

use crate::db::{
    error::DbError,
    models::{ExerciseRouteRow, ExerciseRow},
    queries::object::NewObject,
    types::{JsonValue, JsonVec},
};

pub struct ExerciseQueries;

pub struct NewExercise {
    pub id: Uuid,
    pub actor_id: Uuid,
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
    pub route: Option<serde_json::Value>,
    pub visibility: String,
    pub hidden_stats: Vec<String>,
}

impl ExerciseQueries {
    /// Insert an `objects` row and an `exercises` row using the provided
    /// connection (which may be a bare connection or part of a transaction
    /// managed by the caller). Returns the UUID of the new object row.
    #[must_use = "Result must be checked"]
    pub async fn insert_with_object(
        conn: &mut sqlx::SqliteConnection,
        obj: &NewObject<'_>,
        ex: &NewExercise,
    ) -> Result<Uuid, DbError> {
        let obj_id = Uuid::new_v4();
        let ap_json = JsonValue(obj.ap_json.clone());
        let content_map = obj.content_map.clone().map(JsonValue);

        sqlx::query!(
            r#"INSERT INTO objects
               (id, ap_id, object_type, attributed_to, actor_id, content,
                content_map, summary, sensitive, in_reply_to, published,
                url, ap_json, visibility)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            obj_id,
            obj.ap_id,
            obj.object_type,
            obj.attributed_to,
            obj.actor_id,
            obj.content,
            content_map,
            obj.summary,
            obj.sensitive,
            obj.in_reply_to,
            obj.published,
            obj.url,
            ap_json,
            obj.visibility,
        )
        .execute(&mut *conn)
        .await?;

        let hidden = JsonVec(ex.hidden_stats.clone());
        let route = ex.route.clone().map(JsonValue);

        sqlx::query!(
            r#"INSERT INTO exercises
               (id, actor_id, object_id, activity_type, started_at, duration_s,
                distance_m, elevation_gain_m, avg_pace_s_per_km, avg_heart_rate_bpm,
                max_heart_rate_bpm, avg_cadence_rpm, avg_power_w, max_power_w,
                normalized_power_w, title, file_type, device, gpx_url, route,
                visibility, hidden_stats)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            ex.id,
            ex.actor_id,
            obj_id,
            ex.activity_type,
            ex.started_at,
            ex.duration_s,
            ex.distance_m,
            ex.elevation_gain_m,
            ex.avg_pace_s_per_km,
            ex.avg_heart_rate_bpm,
            ex.max_heart_rate_bpm,
            ex.avg_cadence_rpm,
            ex.avg_power_w,
            ex.max_power_w,
            ex.normalized_power_w,
            ex.title,
            ex.file_type,
            ex.device,
            ex.gpx_url,
            route,
            ex.visibility,
            hidden,
        )
        .execute(&mut *conn)
        .await?;

        Ok(obj_id)
    }

    /// Load an exercise by its UUID, joined with actors and objects.
    #[must_use = "Result must be checked"]
    pub async fn find_metadata_by_id(pool: &SqlitePool, id: Uuid) -> Result<ExerciseRow, DbError> {
        sqlx::query_as!(
            ExerciseRow,
            r#"SELECT e.id              AS "id: Uuid",
                      e.actor_id        AS "actor_id: Uuid",
                      a.ap_id           AS "actor_ap_id",
                      o.ap_id           AS "object_ap_id",
                      e.object_id       AS "object_id: Uuid",
                      e.activity_type, e.started_at AS "started_at: DateTime<Utc>",
                      e.duration_s AS "duration_s: i32", e.distance_m,
                      e.elevation_gain_m, e.avg_pace_s_per_km,
                      e.avg_heart_rate_bpm AS "avg_heart_rate_bpm: i32",
                      e.max_heart_rate_bpm AS "max_heart_rate_bpm: i32",
                      e.avg_cadence_rpm, e.avg_power_w,
                      e.max_power_w, e.normalized_power_w, e.title,
                      COALESCE(e.file_type, 'gpx') AS "file_type!: String",
                      e.device, e.gpx_url, e.visibility,
                      COALESCE(e.hidden_stats, '[]') AS "hidden_stats!: JsonVec<String>",
                      e.created_at AS "created_at: DateTime<Utc>"
               FROM exercises e
               JOIN actors  a ON a.id = e.actor_id
               JOIN objects o ON o.id = e.object_id
               WHERE e.id = ?"#,
            id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Load an exercise by its AP object ap_id (for inbox receive idempotency check).
    #[must_use = "Result must be checked"]
    pub async fn find_by_ap_id(pool: &SqlitePool, ap_id: &str) -> Result<ExerciseRow, DbError> {
        sqlx::query_as!(
            ExerciseRow,
            r#"SELECT e.id              AS "id: Uuid",
                      e.actor_id        AS "actor_id: Uuid",
                      a.ap_id           AS "actor_ap_id",
                      o.ap_id           AS "object_ap_id",
                      e.object_id       AS "object_id: Uuid",
                      e.activity_type, e.started_at AS "started_at: DateTime<Utc>",
                      e.duration_s AS "duration_s: i32", e.distance_m,
                      e.elevation_gain_m, e.avg_pace_s_per_km,
                      e.avg_heart_rate_bpm AS "avg_heart_rate_bpm: i32",
                      e.max_heart_rate_bpm AS "max_heart_rate_bpm: i32",
                      e.avg_cadence_rpm, e.avg_power_w,
                      e.max_power_w, e.normalized_power_w, e.title,
                      COALESCE(e.file_type, 'gpx') AS "file_type!: String",
                      e.device, e.gpx_url, e.visibility,
                      COALESCE(e.hidden_stats, '[]') AS "hidden_stats!: JsonVec<String>",
                      e.created_at AS "created_at: DateTime<Utc>"
               FROM exercises e
               JOIN actors  a ON a.id = e.actor_id
               JOIN objects o ON o.id = e.object_id
               WHERE o.ap_id = ?"#,
            ap_id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Load an exercise with its route column (for the GeoJSON endpoint).
    #[must_use = "Result must be checked"]
    pub async fn find_with_route(pool: &SqlitePool, id: Uuid) -> Result<ExerciseRouteRow, DbError> {
        sqlx::query_as!(
            ExerciseRouteRow,
            r#"SELECT id         AS "id: Uuid",
                      actor_id   AS "actor_id: Uuid",
                      visibility,
                      route      AS "route: JsonValue"
               FROM exercises WHERE id = ?"#,
            id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Update the editable fields of an exercise: title and hidden_stats.
    /// Looked up via object ap_id since that's what the UI works with.
    #[must_use = "Result must be checked"]
    pub async fn update_edit(
        pool: &SqlitePool,
        object_ap_id: &str,
        title: Option<&str>,
        hidden_stats: &[String],
    ) -> Result<(), DbError> {
        let hidden = JsonVec(hidden_stats.to_vec());
        sqlx::query!(
            r#"UPDATE exercises
               SET title = ?, hidden_stats = ?
               WHERE object_id = (SELECT id FROM objects WHERE ap_id = ?)"#,
            title,
            hidden,
            object_ap_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Batch-fetch exercises by their AP object ap_ids.
    /// Returns only rows that exist; the result may be shorter than `ap_ids`.
    /// Uses `QueryBuilder` to build a dynamic `IN (?, ?, ...)` clause since
    /// SQLite has no `ANY($1)` operator.
    #[must_use = "Result must be checked"]
    pub async fn find_batch_by_ap_ids(
        pool: &SqlitePool,
        ap_ids: &[String],
    ) -> Result<Vec<ExerciseRow>, DbError> {
        if ap_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut qb: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
            r#"SELECT e.id, e.actor_id, a.ap_id AS actor_ap_id, o.ap_id AS object_ap_id,
                      e.object_id, e.activity_type, e.started_at,
                      e.duration_s, e.distance_m,
                      e.elevation_gain_m, e.avg_pace_s_per_km,
                      e.avg_heart_rate_bpm, e.max_heart_rate_bpm,
                      e.avg_cadence_rpm, e.avg_power_w,
                      e.max_power_w, e.normalized_power_w, e.title,
                      COALESCE(e.file_type, 'gpx') AS file_type,
                      e.device, e.gpx_url, e.visibility,
                      COALESCE(e.hidden_stats, '[]') AS hidden_stats,
                      e.created_at
               FROM exercises e
               JOIN actors  a ON a.id = e.actor_id
               JOIN objects o ON o.id = e.object_id
               WHERE o.ap_id IN ("#,
        );
        let mut separated = qb.separated(", ");
        for id in ap_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let rows = qb.build_query_as::<ExerciseRow>().fetch_all(pool).await?;
        Ok(rows)
    }
}
