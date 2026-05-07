use chrono::{DateTime, Utc};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::db::{
    error::DbError,
    models::{ActivityRow, FeedRow, ProfilePostRow},
    types::{JsonValue, JsonVec},
};

pub struct ActivityQueries;

pub struct NewActivity {
    pub ap_id: String,
    pub activity_type: String,
    pub actor_id: Uuid,
    pub object_ap_id: String,
    pub target_ap_id: Option<String>,
    pub object_id: Option<Uuid>,
    pub ap_json: serde_json::Value,
}

impl ActivityQueries {
    /// Insert a new activity row (pool version — not transactional).
    #[must_use = "Result must be checked"]
    pub async fn insert(pool: &SqlitePool, act: &NewActivity) -> Result<ActivityRow, DbError> {
        let id = Uuid::new_v4();
        let ap_json = JsonValue(act.ap_json.clone());
        sqlx::query!(
            r#"INSERT INTO activities
               (id, ap_id, activity_type, actor_id, object_ap_id, target_ap_id, object_id, ap_json)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (ap_id) DO NOTHING"#,
            id,
            act.ap_id,
            act.activity_type,
            act.actor_id,
            act.object_ap_id,
            act.target_ap_id,
            act.object_id,
            ap_json,
        )
        .execute(pool)
        .await?;

        // Re-fetch (in case it was already there via ON CONFLICT DO NOTHING).
        Self::find_by_ap_id(pool, &act.ap_id).await
    }

    /// Insert inside an active transaction.
    #[must_use = "Result must be checked"]
    pub async fn insert_tx(
        conn: &mut SqliteConnection,
        act: &NewActivity,
    ) -> Result<ActivityRow, DbError> {
        let id = Uuid::new_v4();
        let ap_json = JsonValue(act.ap_json.clone());
        sqlx::query!(
            r#"INSERT INTO activities
               (id, ap_id, activity_type, actor_id, object_ap_id, target_ap_id, object_id, ap_json)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (ap_id) DO NOTHING"#,
            id,
            act.ap_id,
            act.activity_type,
            act.actor_id,
            act.object_ap_id,
            act.target_ap_id,
            act.object_id,
            ap_json,
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query_as!(
            ActivityRow,
            r#"SELECT id          AS "id: Uuid",
                      ap_id,
                      activity_type,
                      actor_id     AS "actor_id: Uuid",
                      object_ap_id, target_ap_id,
                      object_id    AS "object_id: Uuid",
                      ap_json      AS "ap_json: JsonValue",
                      published    AS "published: DateTime<Utc>",
                      created_at   AS "created_at: DateTime<Utc>"
               FROM activities WHERE id = ?"#,
            id,
        )
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find an activity by its AP ID.
    #[must_use = "Result must be checked"]
    pub async fn find_by_ap_id(pool: &SqlitePool, ap_id: &str) -> Result<ActivityRow, DbError> {
        sqlx::query_as!(
            ActivityRow,
            r#"SELECT id          AS "id: Uuid",
                      ap_id,
                      activity_type,
                      actor_id     AS "actor_id: Uuid",
                      object_ap_id, target_ap_id,
                      object_id    AS "object_id: Uuid",
                      ap_json      AS "ap_json: JsonValue",
                      published    AS "published: DateTime<Utc>",
                      created_at   AS "created_at: DateTime<Utc>"
               FROM activities WHERE ap_id = ?"#,
            ap_id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find an activity by UUID (used in delivery worker).
    #[must_use = "Result must be checked"]
    pub async fn find_by_uuid(pool: &SqlitePool, id: Uuid) -> Result<ActivityRow, DbError> {
        sqlx::query_as!(
            ActivityRow,
            r#"SELECT id          AS "id: Uuid",
                      ap_id,
                      activity_type,
                      actor_id     AS "actor_id: Uuid",
                      object_ap_id, target_ap_id,
                      object_id    AS "object_id: Uuid",
                      ap_json      AS "ap_json: JsonValue",
                      published    AS "published: DateTime<Utc>",
                      created_at   AS "created_at: DateTime<Utc>"
               FROM activities WHERE id = ?"#,
            id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Add an activity to an actor's outbox.
    #[must_use = "Result must be checked"]
    pub async fn add_to_outbox(
        conn: &mut SqliteConnection,
        actor_id: Uuid,
        activity_id: Uuid,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO outbox_items (id, owner_id, activity_id)
               VALUES (?, ?, ?)
               ON CONFLICT DO NOTHING"#,
            id,
            actor_id,
            activity_id,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Count outbox entries for `actor_id`.
    #[must_use = "Result must be checked"]
    pub async fn count_outbox(pool: &SqlitePool, actor_id: Uuid) -> Result<i64, DbError> {
        let n = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!: i64" FROM outbox_items WHERE owner_id = ?"#,
            actor_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(n)
    }

    /// Get outbox activities for an actor, newest first, with optional time cursor.
    #[must_use = "Result must be checked"]
    pub async fn get_outbox(
        pool: &SqlitePool,
        actor_id: Uuid,
        limit: i64,
        before_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<ActivityRow>, DbError> {
        if let Some(before) = before_time {
            sqlx::query_as!(
                ActivityRow,
                r#"SELECT a.id          AS "id: Uuid",
                          a.ap_id,
                          a.activity_type,
                          a.actor_id     AS "actor_id: Uuid",
                          a.object_ap_id, a.target_ap_id,
                          a.object_id    AS "object_id: Uuid",
                          a.ap_json      AS "ap_json: JsonValue",
                          a.published    AS "published: DateTime<Utc>",
                          a.created_at   AS "created_at: DateTime<Utc>"
                   FROM activities a
                   JOIN outbox_items o ON o.activity_id = a.id
                   WHERE o.owner_id = ? AND a.published < ?
                   ORDER BY a.published DESC
                   LIMIT ?"#,
                actor_id,
                before,
                limit,
            )
            .fetch_all(pool)
            .await
            .map_err(DbError::Sqlx)
        } else {
            sqlx::query_as!(
                ActivityRow,
                r#"SELECT a.id          AS "id: Uuid",
                          a.ap_id,
                          a.activity_type,
                          a.actor_id     AS "actor_id: Uuid",
                          a.object_ap_id, a.target_ap_id,
                          a.object_id    AS "object_id: Uuid",
                          a.ap_json      AS "ap_json: JsonValue",
                          a.published    AS "published: DateTime<Utc>",
                          a.created_at   AS "created_at: DateTime<Utc>"
                   FROM activities a
                   JOIN outbox_items o ON o.activity_id = a.id
                   WHERE o.owner_id = ?
                   ORDER BY a.published DESC
                   LIMIT ?"#,
                actor_id,
                limit,
            )
            .fetch_all(pool)
            .await
            .map_err(DbError::Sqlx)
        }
    }

    /// Add an activity to an actor's inbox (fan-in from remote creates).
    #[must_use = "Result must be checked"]
    pub async fn add_to_inbox(
        pool: &SqlitePool,
        owner_id: Uuid,
        activity_id: Uuid,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO inbox_items (id, owner_id, activity_id)
               VALUES (?, ?, ?)
               ON CONFLICT (owner_id, activity_id) DO NOTHING"#,
            id,
            owner_id,
            activity_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Return UUIDs of all local actors that follow `followed_ap_id` (accepted follows).
    #[must_use = "Result must be checked"]
    pub async fn local_followers_of(
        pool: &SqlitePool,
        followed_ap_id: &str,
    ) -> Result<Vec<Uuid>, DbError> {
        let rows = sqlx::query!(
            r#"SELECT la.actor_id AS "actor_id: Uuid"
               FROM following f
               JOIN actors target ON target.id = f.target_id
               JOIN local_accounts la ON la.actor_id = f.actor_id
               WHERE target.ap_id = ? AND f.accepted = 1"#,
            followed_ap_id,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.actor_id).collect())
    }

    /// Get the timestamp of the activity at the cursor position (for pagination).
    #[must_use = "Result must be checked"]
    pub async fn outbox_cursor_time(
        pool: &SqlitePool,
        actor_id: Uuid,
        activity_id: Uuid,
    ) -> Result<DateTime<Utc>, DbError> {
        let ts = sqlx::query_scalar!(
            r#"SELECT a.published AS "ts: DateTime<Utc>"
               FROM activities a
               JOIN outbox_items o ON o.activity_id = a.id
               WHERE o.owner_id = ? AND a.id = ?"#,
            actor_id,
            activity_id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)?;
        Ok(ts)
    }

    /// Get home-timeline activities for `owner_id` — own outbox posts UNION
    /// inbox deliveries, newest first, excluding private objects.
    #[must_use = "Result must be checked"]
    pub async fn get_home_timeline(
        pool: &SqlitePool,
        owner_id: Uuid,
        limit: i64,
    ) -> Result<Vec<FeedRow>, DbError> {
        sqlx::query_as::<_, FeedRow>(
            r#"SELECT DISTINCT
                   a.ap_id              AS activity_ap_id,
                   a.activity_type,
                   act.username         AS actor_username,
                   act.domain           AS actor_domain,
                   act.is_local         AS actor_is_local,
                   act.ap_id            AS actor_ap_id,
                   act.avatar_url       AS actor_avatar_url,
                   a.ap_json            AS ap_json,
                   a.published          AS published
               FROM activities a
               JOIN actors     act ON act.id = a.actor_id
               LEFT JOIN objects o ON o.ap_id = a.object_ap_id
               WHERE a.id IN (
                   SELECT activity_id FROM outbox_items WHERE owner_id = ?
                   UNION
                   SELECT activity_id FROM inbox_items  WHERE owner_id = ?
               )
                 AND (o.id IS NULL OR o.visibility != 'private')
               ORDER BY a.published DESC
               LIMIT ?"#,
        )
        .bind(owner_id)
        .bind(owner_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// Fetch Create activities from the owner's outbox for the profile page.
    ///
    /// `include_followers` — when false only public/unlisted objects are returned.
    #[must_use = "Result must be checked"]
    pub async fn get_actor_profile_posts(
        pool: &SqlitePool,
        owner_id: Uuid,
        include_followers: bool,
        limit: i64,
    ) -> Result<Vec<ProfilePostRow>, DbError> {
        sqlx::query_as!(
            ProfilePostRow,
            r#"SELECT
                   o.ap_id              AS object_ap_id,
                   o.object_type,
                   o.content,
                   a.published          AS "published: DateTime<Utc>",
                   e.activity_type      AS exercise_type,
                   e.duration_s         AS "duration_s: i32",
                   e.distance_m,
                   e.elevation_gain_m,
                   e.avg_heart_rate_bpm AS "avg_heart_rate_bpm: i32",
                   e.max_heart_rate_bpm AS "max_heart_rate_bpm: i32",
                   e.avg_power_w,
                   e.max_power_w,
                   e.normalized_power_w,
                   e.avg_cadence_rpm,
                   e.avg_pace_s_per_km,
                   e.device,
                   e.title,
                   o.visibility         AS exercise_visibility,
                   COALESCE(e.hidden_stats, '[]') AS "hidden_stats!: JsonVec<String>"
               FROM outbox_items oi
               JOIN activities a   ON a.id   = oi.activity_id
               JOIN objects    o   ON o.ap_id = a.object_ap_id
               LEFT JOIN exercises e ON e.object_id = o.id
               WHERE oi.owner_id = ?
                 AND a.activity_type = 'Create'
                 AND (? OR o.visibility IN ('public', 'unlisted'))
               ORDER BY a.published DESC
               LIMIT ?"#,
            owner_id,
            include_followers,
            limit,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// Fetch public Create activities from local actors — used for the logged-out feed.
    #[must_use = "Result must be checked"]
    pub async fn get_local_public_timeline(
        pool: &SqlitePool,
        limit: i64,
    ) -> Result<Vec<FeedRow>, DbError> {
        sqlx::query_as!(
            FeedRow,
            r#"SELECT
                   a.ap_id              AS activity_ap_id,
                   a.activity_type,
                   act.username         AS actor_username,
                   act.domain           AS actor_domain,
                   act.is_local         AS "actor_is_local: bool",
                   act.ap_id            AS actor_ap_id,
                   act.avatar_url       AS actor_avatar_url,
                   a.ap_json            AS "ap_json: JsonValue",
                   a.published          AS "published: DateTime<Utc>"
               FROM outbox_items oi
               JOIN activities a   ON a.id   = oi.activity_id
               JOIN actors     act ON act.id = a.actor_id
               LEFT JOIN objects o ON o.ap_id = a.object_ap_id
               WHERE act.is_local = 1
                 AND a.activity_type = 'Create'
                 AND (o.id IS NULL OR o.visibility = 'public')
               ORDER BY a.published DESC
               LIMIT ?"#,
            limit,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }
}
