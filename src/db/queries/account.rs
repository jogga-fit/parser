use sqlx::SqlitePool;
use uuid::Uuid;

use crate::db::{error::DbError, models::LocalAccount};

pub struct AccountQueries;

impl AccountQueries {
    /// Insert a new local account row.  Call after creating the actor.
    /// Caller is responsible for generating the `id` UUID (use `Uuid::new_v4()`).
    #[must_use = "Result must be checked"]
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        actor_id: Uuid,
        password_hash: &str,
        api_token: &str,
        email: Option<&str>,
        phone: Option<&str>,
        email_verified: bool,
        phone_verified: bool,
    ) -> Result<LocalAccount, DbError> {
        sqlx::query_as!(
            LocalAccount,
            r#"INSERT INTO local_accounts
                   (id, actor_id, password_hash, api_token, email, phone,
                    email_verified, phone_verified)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)
               RETURNING id        AS "id: Uuid",
                         actor_id  AS "actor_id: Uuid",
                         password_hash, api_token,
                         email, phone,
                         email_verified AS "email_verified: bool",
                         phone_verified AS "phone_verified: bool",
                         public_profile AS "public_profile: bool",
                         theme,
                         created_at AS "created_at: chrono::DateTime<chrono::Utc>""#,
            id,
            actor_id,
            password_hash,
            api_token,
            email,
            phone,
            email_verified,
            phone_verified,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::from_sqlx(e, "account already exists"))
    }

    /// Look up an account by its bearer token.
    #[must_use = "Result must be checked"]
    pub async fn find_by_token(pool: &SqlitePool, token: &str) -> Result<LocalAccount, DbError> {
        sqlx::query_as!(
            LocalAccount,
            r#"SELECT id        AS "id: Uuid",
                      actor_id  AS "actor_id: Uuid",
                      password_hash, api_token,
                      email, phone,
                      email_verified AS "email_verified: bool",
                      phone_verified AS "phone_verified: bool",
                      public_profile AS "public_profile: bool",
                      theme,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>"
               FROM local_accounts
               WHERE api_token = ?"#,
            token,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Look up an account by its associated actor UUID.
    #[must_use = "Result must be checked"]
    pub async fn find_by_actor_id(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<LocalAccount, DbError> {
        sqlx::query_as!(
            LocalAccount,
            r#"SELECT id        AS "id: Uuid",
                      actor_id  AS "actor_id: Uuid",
                      password_hash, api_token,
                      email, phone,
                      email_verified AS "email_verified: bool",
                      phone_verified AS "phone_verified: bool",
                      public_profile AS "public_profile: bool",
                      theme,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>"
               FROM local_accounts
               WHERE actor_id = ?"#,
            actor_id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find an account by login string — matched against username (via actors JOIN),
    /// verified email, or verified phone, in that order.
    #[must_use = "Result must be checked"]
    pub async fn find_by_login(pool: &SqlitePool, login: &str) -> Result<LocalAccount, DbError> {
        sqlx::query_as!(
            LocalAccount,
            r#"SELECT la.id        AS "id: Uuid",
                      la.actor_id  AS "actor_id: Uuid",
                      la.password_hash, la.api_token,
                      la.email, la.phone,
                      la.email_verified AS "email_verified: bool",
                      la.phone_verified AS "phone_verified: bool",
                      la.public_profile AS "public_profile: bool",
                      la.theme,
                      la.created_at AS "created_at: chrono::DateTime<chrono::Utc>"
               FROM local_accounts la
               JOIN actors a ON a.id = la.actor_id
               WHERE (a.username = ?1 AND a.is_local = 1)
                  OR (la.email    = ?1 AND la.email_verified = 1)
                  OR (la.phone    = ?1 AND la.phone_verified = 1)
               LIMIT 1"#,
            login,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find an account by verified email address (password-reset path).
    #[must_use = "Result must be checked"]
    pub async fn find_by_email(pool: &SqlitePool, email: &str) -> Result<LocalAccount, DbError> {
        sqlx::query_as!(
            LocalAccount,
            r#"SELECT id        AS "id: Uuid",
                      actor_id  AS "actor_id: Uuid",
                      password_hash, api_token,
                      email, phone,
                      email_verified AS "email_verified: bool",
                      phone_verified AS "phone_verified: bool",
                      public_profile AS "public_profile: bool",
                      theme,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>"
               FROM local_accounts
               WHERE email = ? AND email_verified = 1"#,
            email,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find an account by verified phone number (password-reset path).
    #[must_use = "Result must be checked"]
    pub async fn find_by_phone(pool: &SqlitePool, phone: &str) -> Result<LocalAccount, DbError> {
        sqlx::query_as!(
            LocalAccount,
            r#"SELECT id        AS "id: Uuid",
                      actor_id  AS "actor_id: Uuid",
                      password_hash, api_token,
                      email, phone,
                      email_verified AS "email_verified: bool",
                      phone_verified AS "phone_verified: bool",
                      public_profile AS "public_profile: bool",
                      theme,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>"
               FROM local_accounts
               WHERE phone = ? AND phone_verified = 1"#,
            phone,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Replace the password hash and rotate the bearer token atomically.
    /// Rotating the token invalidates all existing sessions.
    #[must_use = "Result must be checked"]
    pub async fn update_password(
        pool: &SqlitePool,
        account_id: Uuid,
        new_hash: &str,
        new_token: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE local_accounts SET password_hash = ?2, api_token = ?3 WHERE id = ?1",
            account_id,
            new_hash,
            new_token,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update the public_profile flag.
    #[must_use = "Result must be checked"]
    pub async fn update_privacy_settings(
        pool: &SqlitePool,
        account_id: Uuid,
        public_profile: bool,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE local_accounts SET public_profile = ?2 WHERE id = ?1",
            account_id,
            public_profile,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update the theme preference (`'dark'` or `'light'`).
    #[must_use = "Result must be checked"]
    pub async fn update_theme(
        pool: &SqlitePool,
        account_id: Uuid,
        theme: &str,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE local_accounts SET theme = ?2 WHERE id = ?1",
            account_id,
            theme,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Returns `true` if `actor_id` belongs to a local (registered) account.
    #[must_use = "Result must be checked"]
    pub async fn is_actor_local(pool: &SqlitePool, actor_id: Uuid) -> Result<bool, DbError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM local_accounts WHERE actor_id = ?) AS "exists!: i64""#,
            actor_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(exists != 0)
    }

    /// Delete an account by actor UUID.
    /// Cascade on the FK removes the row automatically when the actor is deleted;
    /// this is provided for explicit use.
    #[must_use = "Result must be checked"]
    pub async fn delete_by_actor_id(pool: &SqlitePool, actor_id: Uuid) -> Result<(), DbError> {
        sqlx::query!("DELETE FROM local_accounts WHERE actor_id = ?", actor_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update whether the account is listed in the instance directory.
    ///
    /// Jogga is a single-user instance with no public directory; this is a no-op.
    #[must_use = "Result must be checked"]
    pub async fn update_directory_listing(
        _pool: &SqlitePool,
        _account_id: Uuid,
        _show: bool,
    ) -> Result<(), DbError> {
        Ok(())
    }
}
