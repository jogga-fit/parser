use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::error::DbError;

pub struct LikeQueries;

impl LikeQueries {
    /// Upsert a Like (idempotent on `like_ap_id`).
    #[must_use = "Result must be checked"]
    pub async fn upsert(
        pool: &SqlitePool,
        actor_ap_id: &str,
        object_ap_id: &str,
        like_ap_id: &str,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO likes (id, actor_ap_id, object_ap_id, like_ap_id)
               VALUES (?, ?, ?, ?)
               ON CONFLICT (like_ap_id) DO NOTHING"#,
            id,
            actor_ap_id,
            object_ap_id,
            like_ap_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Delete a Like by actor + object.
    #[must_use = "Result must be checked"]
    pub async fn delete(
        pool: &SqlitePool,
        actor_ap_id: &str,
        object_ap_id: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "DELETE FROM likes WHERE actor_ap_id = ? AND object_ap_id = ?",
            actor_ap_id,
            object_ap_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Count likes for an object.
    #[must_use = "Result must be checked"]
    pub async fn count(pool: &SqlitePool, object_ap_id: &str) -> Result<i64, DbError> {
        let n = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!: i64" FROM likes WHERE object_ap_id = ?"#,
            object_ap_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(n)
    }

    /// Return `true` if `actor_ap_id` has liked `object_ap_id`.
    #[must_use = "Result must be checked"]
    pub async fn exists(
        pool: &SqlitePool,
        actor_ap_id: &str,
        object_ap_id: &str,
    ) -> Result<bool, DbError> {
        let row = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM likes WHERE actor_ap_id = ? AND object_ap_id = ?) AS "e!: i64""#,
            actor_ap_id,
            object_ap_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(row != 0)
    }

    /// Count likes for each of `object_ap_ids`, returning a map of ap_id → count.
    ///
    /// SQLite has no `= ANY($1)` — iterates with individual queries.
    #[must_use = "Result must be checked"]
    pub async fn count_batch(
        pool: &SqlitePool,
        object_ap_ids: &[String],
    ) -> Result<std::collections::HashMap<String, i64>, DbError> {
        use sqlx::Row as _;
        let mut map = std::collections::HashMap::new();
        for ap_id in object_ap_ids {
            let row = sqlx::query("SELECT COUNT(*) AS n FROM likes WHERE object_ap_id = ?")
                .bind(ap_id.as_str())
                .fetch_one(pool)
                .await?;
            let n: i64 = row.try_get("n")?;
            map.insert(ap_id.clone(), n);
        }
        Ok(map)
    }

    /// Return the set of `object_ap_ids` that `viewer_ap_id` has liked.
    ///
    /// SQLite has no `= ANY($1)` — iterates with individual queries.
    #[must_use = "Result must be checked"]
    pub async fn viewer_liked_batch(
        pool: &SqlitePool,
        viewer_ap_id: &str,
        object_ap_ids: &[String],
    ) -> Result<std::collections::HashSet<String>, DbError> {
        use sqlx::Row as _;
        let mut set = std::collections::HashSet::new();
        for ap_id in object_ap_ids {
            let row = sqlx::query(
                "SELECT EXISTS(SELECT 1 FROM likes WHERE actor_ap_id = ? AND object_ap_id = ?) AS e",
            )
            .bind(viewer_ap_id)
            .bind(ap_id.as_str())
            .fetch_one(pool)
            .await
            ?;
            let liked: i64 = row.try_get("e")?;
            if liked != 0 {
                set.insert(ap_id.clone());
            }
        }
        Ok(set)
    }
}
