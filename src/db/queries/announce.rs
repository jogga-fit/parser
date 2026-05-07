use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use crate::db::error::DbError;

#[derive(Debug, Clone, FromRow)]
pub struct AnnounceRow {
    pub actor_ap_id: String,
    pub object_ap_id: String,
    pub announce_ap_id: String,
}

pub struct AnnounceQueries;

impl AnnounceQueries {
    /// Upsert an Announce row. Idempotent on `announce_ap_id`.
    /// Caller supplies a fresh UUID for the insert path.
    pub async fn upsert(
        pool: &SqlitePool,
        id: Uuid,
        actor_ap_id: &str,
        object_ap_id: &str,
        announce_ap_id: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            r#"INSERT INTO announces (id, actor_ap_id, object_ap_id, announce_ap_id)
               VALUES (?, ?, ?, ?)
               ON CONFLICT (announce_ap_id) DO NOTHING"#,
            id,
            actor_ap_id,
            object_ap_id,
            announce_ap_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// List announces by actor AP ID, newest first, with optional cursor.
    #[must_use = "Result must be checked"]
    pub async fn list_by_actor(
        pool: &SqlitePool,
        actor_ap_id: &str,
        limit: i64,
        before_ap_id: Option<&str>,
    ) -> Result<Vec<AnnounceRow>, DbError> {
        let rows = if let Some(cursor) = before_ap_id {
            sqlx::query_as!(
                AnnounceRow,
                r#"SELECT actor_ap_id, object_ap_id, announce_ap_id
                   FROM announces
                   WHERE actor_ap_id = ?
                     AND created_at < (SELECT created_at FROM announces WHERE announce_ap_id = ?)
                   ORDER BY created_at DESC
                   LIMIT ?"#,
                actor_ap_id,
                cursor,
                limit,
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as!(
                AnnounceRow,
                r#"SELECT actor_ap_id, object_ap_id, announce_ap_id
                   FROM announces
                   WHERE actor_ap_id = ?
                   ORDER BY created_at DESC
                   LIMIT ?"#,
                actor_ap_id,
                limit,
            )
            .fetch_all(pool)
            .await?
        };
        Ok(rows)
    }
}
