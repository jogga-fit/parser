use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::{error::DbError, models::OtpRequest};

pub struct OtpQueries;

impl OtpQueries {
    /// Insert a new OTP request row.
    #[must_use = "Result must be checked"]
    pub async fn insert(
        pool: &SqlitePool,
        contact: &str,
        contact_type: &str,
        purpose: &str,
        username: Option<&str>,
        code_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<OtpRequest, DbError> {
        let id = Uuid::new_v4();
        sqlx::query_as!(
            OtpRequest,
            r#"INSERT INTO otp_requests
                   (id, contact, contact_type, purpose, username, code_hash, expires_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               RETURNING id        AS "id: Uuid",
                         contact, contact_type, purpose, username, code_hash,
                         expires_at AS "expires_at: DateTime<Utc>",
                         used_at    AS "used_at: DateTime<Utc>",
                         created_at AS "created_at: DateTime<Utc>""#,
            id,
            contact,
            contact_type,
            purpose,
            username,
            code_hash,
            expires_at,
        )
        .fetch_one(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// Find an active (non-expired, non-used) OTP by UUID.
    #[must_use = "Result must be checked"]
    pub async fn find_active(pool: &SqlitePool, id: Uuid) -> Result<OtpRequest, DbError> {
        sqlx::query_as!(
            OtpRequest,
            r#"SELECT id        AS "id: Uuid",
                      contact, contact_type, purpose, username, code_hash,
                      expires_at AS "expires_at: DateTime<Utc>",
                      used_at    AS "used_at: DateTime<Utc>",
                      created_at AS "created_at: DateTime<Utc>"
               FROM otp_requests
               WHERE id = ?
                 AND used_at IS NULL
                 AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')"#,
            id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Mark an OTP as used (consumed).
    #[must_use = "Result must be checked"]
    pub async fn mark_used(pool: &SqlitePool, id: Uuid) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE otp_requests SET used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
