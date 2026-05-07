use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::{error::DbError, models::NotificationDetailRow};

pub struct NotificationQueries;

impl NotificationQueries {
    /// Insert a notification row (non-fatal — caller should log errors).
    #[must_use = "Result must be checked"]
    pub async fn insert(
        pool: &SqlitePool,
        actor_id: Uuid,
        kind: &str,
        from_actor_id: Uuid,
        object_ap_id: Option<&str>,
        object_title: Option<&str>,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO notifications
                   (id, actor_id, kind, from_actor_id, object_ap_id, object_title)
               VALUES (?, ?, ?, ?, ?, ?)
               ON CONFLICT DO NOTHING"#,
            id,
            actor_id,
            kind,
            from_actor_id,
            object_ap_id,
            object_title,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Delete a like notification (used when unlike is received).
    #[must_use = "Result must be checked"]
    pub async fn delete_like(
        pool: &SqlitePool,
        actor_id: Uuid,
        from_actor_id: Uuid,
        object_ap_id: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "DELETE FROM notifications WHERE actor_id = ? AND from_actor_id = ? AND object_ap_id = ? AND kind = 'like'",
            actor_id,
            from_actor_id,
            object_ap_id,
        )
        .execute(pool)
        .await
        ?;
        Ok(())
    }

    /// List notifications for `actor_id`, newest first.
    #[must_use = "Result must be checked"]
    pub async fn list(
        pool: &SqlitePool,
        actor_id: Uuid,
        limit: i64,
    ) -> Result<Vec<NotificationDetailRow>, DbError> {
        sqlx::query_as!(
            NotificationDetailRow,
            r#"SELECT n.id       AS "id: Uuid",
                      n.kind,
                      a.ap_id   AS from_ap_id,
                      a.username AS from_username,
                      a.display_name AS from_display_name,
                      a.avatar_url   AS from_avatar_url,
                      n.object_ap_id, n.object_title,
                      n.is_read AS "is_read: bool",
                      n.created_at AS "created_at: chrono::DateTime<chrono::Utc>"
               FROM notifications n
               JOIN actors a ON a.id = n.from_actor_id
               WHERE n.actor_id = ?
               ORDER BY n.created_at DESC
               LIMIT ?"#,
            actor_id,
            limit,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// Mark all notifications as read for `actor_id`.
    #[must_use = "Result must be checked"]
    pub async fn mark_all_read(pool: &SqlitePool, actor_id: Uuid) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE notifications SET is_read = 1 WHERE actor_id = ?",
            actor_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Count unread notifications for `actor_id`.
    #[must_use = "Result must be checked"]
    pub async fn count_unread(pool: &SqlitePool, actor_id: Uuid) -> Result<i64, DbError> {
        use sqlx::Row as _;
        let row = sqlx::query(
            "SELECT COUNT(*) AS n FROM notifications WHERE actor_id = ? AND is_read = 0",
        )
        .bind(actor_id)
        .fetch_one(pool)
        .await?;
        let n: i64 = row.try_get("n")?;
        Ok(n)
    }

    /// Mark a single notification as read (dismiss) for `actor_id`.
    #[must_use = "Result must be checked"]
    pub async fn dismiss(
        pool: &SqlitePool,
        actor_id: Uuid,
        notification_id: Uuid,
    ) -> Result<(), DbError> {
        sqlx::query("UPDATE notifications SET is_read = 1 WHERE actor_id = ? AND id = ?")
            .bind(actor_id)
            .bind(notification_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
