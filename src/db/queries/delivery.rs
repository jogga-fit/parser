use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, SqlitePool};
use uuid::Uuid;

use crate::db::{error::DbError, models::DeliveryRow};

pub struct DeliveryQueries;

impl DeliveryQueries {
    /// Batch-insert pending delivery rows for the given inbox URLs.
    /// Uses `QueryBuilder` because the URL slice length is dynamic.
    /// Silently skips duplicates (`ON CONFLICT DO NOTHING`).
    #[must_use = "Result must be checked"]
    pub async fn insert_deliveries(
        pool: &SqlitePool,
        activity_id: Uuid,
        inbox_urls: &[String],
    ) -> Result<(), DbError> {
        if inbox_urls.is_empty() {
            return Ok(());
        }
        let mut qb: QueryBuilder<sqlx::Sqlite> =
            QueryBuilder::new("INSERT INTO outbox_deliveries (id, activity_id, inbox_url) ");
        qb.push_values(inbox_urls, |mut b, url| {
            b.push_bind(Uuid::new_v4())
                .push_bind(activity_id)
                .push_bind(url);
        });
        qb.push(" ON CONFLICT (activity_id, inbox_url) DO NOTHING");
        qb.build().execute(pool).await?;
        Ok(())
    }

    /// Claim up to `limit` deliveries that are due for (re)processing.
    ///
    /// Atomically increments `attempt_count`, sets `status = 'failed'` (so a
    /// crashed worker doesn't leave rows stuck in 'pending'), and returns the
    /// claimed rows. SQLite has no `FOR UPDATE SKIP LOCKED`; since this is a
    /// single-owner single-process server the simpler subquery is safe.
    #[must_use = "Result must be checked"]
    pub async fn claim_due_deliveries(
        pool: &SqlitePool,
        limit: i64,
    ) -> Result<Vec<DeliveryRow>, DbError> {
        sqlx::query_as!(
            DeliveryRow,
            r#"UPDATE outbox_deliveries
               SET attempt_count   = attempt_count + 1,
                   last_attempt_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                   status          = 'failed'
               WHERE id IN (
                   SELECT id FROM outbox_deliveries
                   WHERE  status IN ('pending', 'failed')
                     AND  next_retry_at <= strftime('%Y-%m-%dT%H:%M:%fZ','now')
                   ORDER BY next_retry_at
                   LIMIT ?
               )
               RETURNING id              AS "id: Uuid",
                         activity_id     AS "activity_id: Uuid",
                         inbox_url, status,
                         attempt_count   AS "attempt_count: i32",
                         last_attempt_at AS "last_attempt_at: DateTime<Utc>",
                         last_error,
                         next_retry_at   AS "next_retry_at: DateTime<Utc>",
                         created_at      AS "created_at: DateTime<Utc>""#,
            limit,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// Mark a delivery as successfully sent.
    #[must_use = "Result must be checked"]
    pub async fn mark_success(pool: &SqlitePool, id: Uuid) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE outbox_deliveries SET status = 'success' WHERE id = ?",
            id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Mark a delivery as failed.
    ///
    /// - `next_retry_at = Some(t)` → `status = 'failed'`, retry scheduled at `t`.
    /// - `next_retry_at = None`    → `status = 'permanent_fail'`, no further retries.
    #[must_use = "Result must be checked"]
    pub async fn mark_failed(
        pool: &SqlitePool,
        id: Uuid,
        error: &str,
        next_retry_at: Option<DateTime<Utc>>,
    ) -> Result<(), DbError> {
        if let Some(next) = next_retry_at {
            sqlx::query!(
                "UPDATE outbox_deliveries SET status = 'failed', last_error = ?, next_retry_at = ? WHERE id = ?",
                error,
                next,
                id,
            )
            .execute(pool)
            .await
            ?;
        } else {
            sqlx::query!(
                "UPDATE outbox_deliveries SET status = 'permanent_fail', last_error = ? WHERE id = ?",
                error,
                id,
            )
            .execute(pool)
            .await
            ?;
        }
        Ok(())
    }
}
