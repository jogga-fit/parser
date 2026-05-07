use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::db::{error::DbError, models::MediaAttachmentRow};

pub struct MediaAttachmentQueries;

impl MediaAttachmentQueries {
    /// Insert a media attachment row.
    #[must_use = "Result must be checked"]
    pub async fn insert(
        conn: &mut SqliteConnection,
        object_ap_id: &str,
        url: &str,
        position: i16,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO media_attachments (id, object_ap_id, url, media_type, position)
               VALUES (?, ?, ?, 'image/jpeg', ?)"#,
            id,
            object_ap_id,
            url,
            position,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// List all attachments for an object, ordered by position.
    #[must_use = "Result must be checked"]
    pub async fn list_for_object(
        pool: &SqlitePool,
        object_ap_id: &str,
    ) -> Result<Vec<MediaAttachmentRow>, DbError> {
        sqlx::query_as!(
            MediaAttachmentRow,
            r#"SELECT id AS "id: Uuid",
                      object_ap_id, url, media_type, caption,
                      position AS "position: i32",
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>"
               FROM media_attachments
               WHERE object_ap_id = ?
               ORDER BY position ASC"#,
            object_ap_id,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// Delete specific attachment URLs from an object.
    #[must_use = "Result must be checked"]
    pub async fn delete_urls(
        pool: &SqlitePool,
        object_ap_id: &str,
        urls: &[String],
    ) -> Result<(), DbError> {
        for url in urls {
            sqlx::query!(
                "DELETE FROM media_attachments WHERE object_ap_id = ? AND url = ?",
                object_ap_id,
                url,
            )
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    /// Fetch all attachments for the given set of object AP IDs.
    ///
    /// SQLite has no `= ANY($1)` — iterates per object with individual queries.
    #[must_use = "Result must be checked"]
    pub async fn fetch_for_objects(
        pool: &SqlitePool,
        object_ap_ids: &[String],
    ) -> Result<Vec<MediaAttachmentRow>, DbError> {
        let mut out = Vec::new();
        for ap_id in object_ap_ids {
            let mut rows = sqlx::query_as!(
                MediaAttachmentRow,
                r#"SELECT id AS "id: Uuid",
                          object_ap_id, url, media_type, caption,
                          position AS "position: i32",
                          created_at AS "created_at: chrono::DateTime<chrono::Utc>"
                   FROM media_attachments
                   WHERE object_ap_id = ?
                   ORDER BY position ASC"#,
                ap_id,
            )
            .fetch_all(pool)
            .await?;
            out.append(&mut rows);
        }
        Ok(out)
    }
}
