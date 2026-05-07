use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::db::{
    error::DbError,
    models::{ActorRow, actor::ApId},
    types::JsonVec,
};

pub struct ActorQueries;

pub struct NewActor<'a> {
    pub ap_id: &'a str,
    pub username: &'a str,
    pub domain: &'a str,
    pub actor_type: &'a str,
    pub display_name: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub public_key_pem: &'a str,
    pub private_key_pem: Option<&'a str>,
    pub inbox_url: &'a str,
    pub outbox_url: &'a str,
    pub followers_url: &'a str,
    pub following_url: &'a str,
    pub shared_inbox_url: Option<&'a str>,
    pub manually_approves_followers: bool,
    pub is_local: bool,
    pub ap_json: Option<serde_json::Value>,
    pub also_known_as: &'a [String],
    pub moved_to: Option<&'a str>,
}

impl ActorQueries {
    /// Find a local actor by username.
    #[must_use = "Result must be checked"]
    pub async fn find_local_by_username(
        pool: &SqlitePool,
        username: &str,
    ) -> Result<ActorRow, DbError> {
        sqlx::query_as!(
            ActorRow,
            r#"SELECT id              AS "id: Uuid",
                      ap_id           AS "ap_id: ApId",
                      username, domain, actor_type,
                      display_name, summary, public_key_pem, private_key_pem,
                      inbox_url, outbox_url, followers_url, following_url,
                      shared_inbox_url,
                      manually_approves_followers AS "manually_approves_followers: bool",
                      is_local                    AS "is_local: bool",
                      is_suspended                AS "is_suspended: bool",
                      avatar_url,
                      also_known_as               AS "also_known_as: JsonVec<String>",
                      moved_to,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                      updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
               FROM actors
               WHERE username = ? AND is_local = 1"#,
            username
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find any actor by their AP ID (URL).
    #[must_use = "Result must be checked"]
    pub async fn find_by_ap_id(pool: &SqlitePool, ap_id: &str) -> Result<ActorRow, DbError> {
        sqlx::query_as!(
            ActorRow,
            r#"SELECT id              AS "id: Uuid",
                      ap_id           AS "ap_id: ApId",
                      username, domain, actor_type,
                      display_name, summary, public_key_pem, private_key_pem,
                      inbox_url, outbox_url, followers_url, following_url,
                      shared_inbox_url,
                      manually_approves_followers AS "manually_approves_followers: bool",
                      is_local                    AS "is_local: bool",
                      is_suspended                AS "is_suspended: bool",
                      avatar_url,
                      also_known_as               AS "also_known_as: JsonVec<String>",
                      moved_to,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                      updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
               FROM actors WHERE ap_id = ?"#,
            ap_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Find by UUID.
    #[must_use = "Result must be checked"]
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<ActorRow, DbError> {
        sqlx::query_as!(
            ActorRow,
            r#"SELECT id              AS "id: Uuid",
                      ap_id           AS "ap_id: ApId",
                      username, domain, actor_type,
                      display_name, summary, public_key_pem, private_key_pem,
                      inbox_url, outbox_url, followers_url, following_url,
                      shared_inbox_url,
                      manually_approves_followers AS "manually_approves_followers: bool",
                      is_local                    AS "is_local: bool",
                      is_suspended                AS "is_suspended: bool",
                      avatar_url,
                      also_known_as               AS "also_known_as: JsonVec<String>",
                      moved_to,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                      updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
               FROM actors WHERE id = ?"#,
            id
        )
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Insert a new actor row using an existing connection (transaction-compatible).
    #[must_use = "Result must be checked"]
    pub async fn insert(
        conn: &mut SqliteConnection,
        actor: &NewActor<'_>,
    ) -> Result<ActorRow, DbError> {
        let id = Uuid::new_v4();
        let also_known_as = JsonVec(actor.also_known_as.to_vec());

        sqlx::query!(
            r#"INSERT INTO actors (
               id, ap_id, username, domain, actor_type, display_name, summary,
               public_key_pem, private_key_pem, inbox_url, outbox_url,
               followers_url, following_url, shared_inbox_url,
               manually_approves_followers, is_local, also_known_as, moved_to)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            id,
            actor.ap_id,
            actor.username,
            actor.domain,
            actor.actor_type,
            actor.display_name,
            actor.summary,
            actor.public_key_pem,
            actor.private_key_pem,
            actor.inbox_url,
            actor.outbox_url,
            actor.followers_url,
            actor.following_url,
            actor.shared_inbox_url,
            actor.manually_approves_followers,
            actor.is_local,
            also_known_as,
            actor.moved_to,
        )
        .execute(&mut *conn)
        .await
        .map_err(|e| DbError::from_sqlx(e, format!("actor '{}' already exists", actor.username)))?;

        sqlx::query_as!(
            ActorRow,
            r#"SELECT id              AS "id: Uuid",
                      ap_id           AS "ap_id: ApId",
                      username, domain, actor_type,
                      display_name, summary, public_key_pem, private_key_pem,
                      inbox_url, outbox_url, followers_url, following_url,
                      shared_inbox_url,
                      manually_approves_followers AS "manually_approves_followers: bool",
                      is_local                    AS "is_local: bool",
                      is_suspended                AS "is_suspended: bool",
                      avatar_url,
                      also_known_as               AS "also_known_as: JsonVec<String>",
                      moved_to,
                      created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                      updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
               FROM actors WHERE id = ?"#,
            id
        )
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(DbError::NotFound)
    }

    /// Upsert a remote actor (insert or update on ap_id conflict).
    #[must_use = "Result must be checked"]
    pub async fn upsert_remote(
        pool: &SqlitePool,
        actor: &NewActor<'_>,
    ) -> Result<ActorRow, DbError> {
        let id = Uuid::new_v4();
        let also_known_as = JsonVec(actor.also_known_as.to_vec());

        sqlx::query!(
            r#"INSERT INTO actors (
               id, ap_id, username, domain, actor_type, display_name, summary,
               public_key_pem, private_key_pem, inbox_url, outbox_url,
               followers_url, following_url, shared_inbox_url,
               manually_approves_followers, is_local, also_known_as, moved_to)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)
               ON CONFLICT (ap_id) DO UPDATE SET
                   username                    = excluded.username,
                   domain                      = excluded.domain,
                   actor_type                  = excluded.actor_type,
                   display_name                = excluded.display_name,
                   summary                     = excluded.summary,
                   public_key_pem              = excluded.public_key_pem,
                   inbox_url                   = excluded.inbox_url,
                   outbox_url                  = excluded.outbox_url,
                   followers_url               = excluded.followers_url,
                   following_url               = excluded.following_url,
                   shared_inbox_url            = excluded.shared_inbox_url,
                   manually_approves_followers = excluded.manually_approves_followers,
                   also_known_as               = excluded.also_known_as,
                   moved_to                    = excluded.moved_to,
                   updated_at                  = strftime('%Y-%m-%dT%H:%M:%fZ','now')"#,
            id,
            actor.ap_id,
            actor.username,
            actor.domain,
            actor.actor_type,
            actor.display_name,
            actor.summary,
            actor.public_key_pem,
            actor.private_key_pem,
            actor.inbox_url,
            actor.outbox_url,
            actor.followers_url,
            actor.following_url,
            actor.shared_inbox_url,
            actor.manually_approves_followers,
            also_known_as,
            actor.moved_to,
        )
        .execute(pool)
        .await?;

        Self::find_by_ap_id(pool, actor.ap_id).await
    }

    /// Delete an actor by UUID.
    #[must_use = "Result must be checked"]
    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<(), DbError> {
        sqlx::query!("DELETE FROM actors WHERE id = ?", id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Set `manually_approves_followers`.
    #[must_use = "Result must be checked"]
    pub async fn set_manually_approves_followers(
        pool: &SqlitePool,
        id: Uuid,
        value: bool,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE actors SET manually_approves_followers = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            value,
            id,
        )
        .execute(pool)
        .await
        ?;
        Ok(())
    }

    /// Set `moved_to` for account migration.
    #[must_use = "Result must be checked"]
    pub async fn set_moved_to(pool: &SqlitePool, id: Uuid, moved_to: &str) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE actors SET moved_to = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            moved_to,
            id,
        )
        .execute(pool)
        .await
        ?;
        Ok(())
    }

    /// Add an alsoKnownAs entry.
    #[must_use = "Result must be checked"]
    pub async fn add_alias(pool: &SqlitePool, id: Uuid, alias: &str) -> Result<(), DbError> {
        let actor = Self::find_by_id(pool, id).await?;
        let mut list = actor.also_known_as.0;
        if !list.contains(&alias.to_string()) {
            list.push(alias.to_string());
        }
        let j = JsonVec(list);
        sqlx::query!(
            "UPDATE actors SET also_known_as = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            j,
            id,
        )
        .execute(pool)
        .await
        ?;
        Ok(())
    }

    /// Remove an alsoKnownAs entry.
    #[must_use = "Result must be checked"]
    pub async fn remove_alias(pool: &SqlitePool, id: Uuid, alias: &str) -> Result<(), DbError> {
        let actor = Self::find_by_id(pool, id).await?;
        let list: Vec<String> = actor
            .also_known_as
            .0
            .into_iter()
            .filter(|a| a != alias)
            .collect();
        let j = JsonVec(list);
        sqlx::query!(
            "UPDATE actors SET also_known_as = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            j,
            id,
        )
        .execute(pool)
        .await
        ?;
        Ok(())
    }

    /// Update display name and bio for a local actor.
    #[must_use = "Result must be checked"]
    pub async fn update_profile(
        pool: &SqlitePool,
        actor_id: Uuid,
        display_name: Option<&str>,
        summary: Option<&str>,
    ) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE actors SET display_name = ?, summary = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND is_local = 1",
        )
        .bind(display_name)
        .bind(summary)
        .bind(actor_id)
        .execute(pool)
        .await
        ?;
        Ok(())
    }

    /// Update the avatar URL for a local actor.
    #[must_use = "Result must be checked"]
    pub async fn update_avatar_url(
        pool: &SqlitePool,
        actor_id: Uuid,
        url: &str,
    ) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE actors SET avatar_url = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND is_local = 1",
        )
        .bind(url)
        .bind(actor_id)
        .execute(pool)
        .await
        ?;
        Ok(())
    }

    /// Return all local actors who have opted into the directory.
    ///
    /// Jogga is a single-user instance with no public directory; always returns
    /// an empty list.
    #[must_use = "Result must be checked"]
    pub async fn list_directory(_pool: &SqlitePool) -> Result<Vec<ActorRow>, DbError> {
        Ok(vec![])
    }
}
