use chrono::{DateTime, Utc};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::db::{error::DbError, models::ObjectRow, types::JsonValue};

pub struct ObjectQueries;

pub struct NewObject<'a> {
    pub ap_id: &'a str,
    pub object_type: &'a str,
    pub attributed_to: &'a str,
    pub actor_id: Option<Uuid>,
    pub content: Option<&'a str>,
    pub content_map: Option<serde_json::Value>,
    pub summary: Option<&'a str>,
    pub sensitive: bool,
    pub in_reply_to: Option<&'a str>,
    pub published: Option<DateTime<Utc>>,
    pub url: Option<&'a str>,
    pub ap_json: serde_json::Value,
    pub visibility: &'a str,
}

impl ObjectQueries {
    /// Insert a new object row using a bare connection (for use inside transactions).
    #[must_use = "Result must be checked"]
    pub async fn insert(pool: &SqlitePool, obj: &NewObject<'_>) -> Result<ObjectRow, DbError> {
        let id = Uuid::new_v4();
        let ap_json = JsonValue(obj.ap_json.clone());
        let content_map = obj.content_map.clone().map(JsonValue);

        sqlx::query!(
            r#"INSERT INTO objects
               (id, ap_id, object_type, attributed_to, actor_id, content,
                content_map, summary, sensitive, in_reply_to, published,
                url, ap_json, visibility)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (ap_id) DO NOTHING"#,
            id,
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
        .execute(pool)
        .await?;

        Self::find_by_ap_id(pool, obj.ap_id).await
    }

    /// Insert inside a transaction connection.
    #[must_use = "Result must be checked"]
    pub async fn insert_tx(
        conn: &mut SqliteConnection,
        obj: &NewObject<'_>,
    ) -> Result<ObjectRow, DbError> {
        let id = Uuid::new_v4();
        let ap_json = JsonValue(obj.ap_json.clone());
        let content_map = obj.content_map.clone().map(JsonValue);

        sqlx::query!(
            r#"INSERT INTO objects
               (id, ap_id, object_type, attributed_to, actor_id, content,
                content_map, summary, sensitive, in_reply_to, published,
                url, ap_json, visibility)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (ap_id) DO NOTHING"#,
            id,
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

        sqlx::query_as!(
            ObjectRow,
            r#"SELECT id              AS "id: Uuid",
                      ap_id,
                      object_type, attributed_to,
                      actor_id        AS "actor_id: Uuid",
                      content, content_map AS "content_map: JsonValue",
                      summary,
                      sensitive       AS "sensitive: bool",
                      in_reply_to, reply_count AS "reply_count: i32", published AS "published: DateTime<Utc>",
                      url, ap_json    AS "ap_json: JsonValue",
                      visibility,
                      created_at      AS "created_at: DateTime<Utc>",
                      updated_at      AS "updated_at: DateTime<Utc>"
               FROM objects WHERE id = ?"#,
            id,
        )
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find by AP ID string.
    #[must_use = "Result must be checked"]
    pub async fn find_by_ap_id(pool: &SqlitePool, ap_id: &str) -> Result<ObjectRow, DbError> {
        sqlx::query_as!(
            ObjectRow,
            r#"SELECT id              AS "id: Uuid",
                      ap_id,
                      object_type, attributed_to,
                      actor_id        AS "actor_id: Uuid",
                      content, content_map AS "content_map: JsonValue",
                      summary,
                      sensitive       AS "sensitive: bool",
                      in_reply_to, reply_count AS "reply_count: i32", published AS "published: DateTime<Utc>",
                      url, ap_json    AS "ap_json: JsonValue",
                      visibility,
                      created_at      AS "created_at: DateTime<Utc>",
                      updated_at      AS "updated_at: DateTime<Utc>"
               FROM objects WHERE ap_id = ?"#,
            ap_id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find by UUID.
    #[must_use = "Result must be checked"]
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<ObjectRow, DbError> {
        sqlx::query_as!(
            ObjectRow,
            r#"SELECT id              AS "id: Uuid",
                      ap_id,
                      object_type, attributed_to,
                      actor_id        AS "actor_id: Uuid",
                      content, content_map AS "content_map: JsonValue",
                      summary,
                      sensitive       AS "sensitive: bool",
                      in_reply_to, reply_count AS "reply_count: i32", published AS "published: DateTime<Utc>",
                      url, ap_json    AS "ap_json: JsonValue",
                      visibility,
                      created_at      AS "created_at: DateTime<Utc>",
                      updated_at      AS "updated_at: DateTime<Utc>"
               FROM objects WHERE id = ?"#,
            id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Delete by AP ID. Non-fatal: doesn't error if not found.
    #[must_use = "Result must be checked"]
    pub async fn delete_by_ap_id(pool: &SqlitePool, ap_id: &str) -> Result<(), DbError> {
        sqlx::query!("DELETE FROM objects WHERE ap_id = ?", ap_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update content/title/ap_json for an existing object.
    #[must_use = "Result must be checked"]
    pub async fn update_post(
        pool: &SqlitePool,
        ap_id: &str,
        content: Option<&str>,
        summary: Option<&str>,
        ap_json: serde_json::Value,
    ) -> Result<(), DbError> {
        let ap_json = JsonValue(ap_json);
        sqlx::query!(
            r#"UPDATE objects
               SET content = ?, summary = ?, ap_json = ?,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
               WHERE ap_id = ?"#,
            content,
            summary,
            ap_json,
            ap_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Find reply AP IDs for a parent object.
    #[must_use = "Result must be checked"]
    pub async fn find_reply_ap_ids(
        pool: &SqlitePool,
        parent_ap_id: &str,
    ) -> Result<Vec<String>, DbError> {
        let rows = sqlx::query_scalar!(
            "SELECT ap_id FROM objects WHERE in_reply_to = ? ORDER BY published ASC",
            parent_ap_id,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Find an object by UUID string (the last path segment of an AP ID URL).
    ///
    /// Accepts either a full AP ID URL (falls back to `find_by_ap_id`) or a bare
    /// UUID string and looks up by the `id` column.
    #[must_use = "Result must be checked"]
    pub async fn find_by_uuid(
        pool: &SqlitePool,
        uuid_or_ap_id: &str,
    ) -> Result<ObjectRow, DbError> {
        // If it looks like a full URL, delegate to find_by_ap_id.
        if uuid_or_ap_id.starts_with("http") {
            return Self::find_by_ap_id(pool, uuid_or_ap_id).await;
        }
        // Otherwise parse as UUID and look up by id column.
        let id: uuid::Uuid = uuid_or_ap_id.parse().map_err(|_| DbError::NotFound)?;
        Self::find_by_id(pool, id).await
    }

    /// Find all reply objects for a parent AP ID, ordered by published time.
    #[must_use = "Result must be checked"]
    pub async fn find_replies(
        pool: &SqlitePool,
        object_ap_id: &str,
    ) -> Result<Vec<ObjectRow>, DbError> {
        sqlx::query_as::<_, ObjectRow>(
            "SELECT id, ap_id, object_type, attributed_to, actor_id, content, content_map, summary, sensitive, in_reply_to, reply_count, published, url, ap_json, visibility, created_at, updated_at FROM objects WHERE in_reply_to = ? ORDER BY published ASC",
        )
        .bind(object_ap_id)
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }
}
