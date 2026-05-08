use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::db::{
    error::DbError,
    models::{ActorRow, FollowerDetailRow, FollowingDetailRow},
};

pub struct FollowQueries;

impl FollowQueries {
    /// Return `true` if `actor_id` is following `target_id` (accepted or pending).
    #[must_use = "Result must be checked"]
    pub async fn is_following(
        pool: &SqlitePool,
        actor_id: Uuid,
        target_id: Uuid,
    ) -> Result<bool, DbError> {
        let row = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM following WHERE actor_id = ? AND target_id = ?) AS "e!: i64""#,
            actor_id,
            target_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(row != 0)
    }

    /// Return `true` if `actor_id` is following `target_id` and the follow is accepted.
    #[must_use = "Result must be checked"]
    pub async fn is_following_accepted(
        pool: &SqlitePool,
        actor_id: Uuid,
        target_id: Uuid,
    ) -> Result<bool, DbError> {
        let row = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM following WHERE actor_id = ? AND target_id = ? AND accepted = 1) AS "e!: i64""#,
            actor_id,
            target_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(row != 0)
    }

    /// Return `Some(accepted)` if a following row exists, else `None`.
    #[must_use = "Result must be checked"]
    pub async fn following_status(
        pool: &SqlitePool,
        actor_id: Uuid,
        target_id: Uuid,
    ) -> Result<Option<bool>, DbError> {
        let row = sqlx::query_scalar!(
            r#"SELECT accepted AS "accepted: bool" FROM following WHERE actor_id = ? AND target_id = ?"#,
            actor_id,
            target_id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Count accepted followers of `actor_id`.
    #[must_use = "Result must be checked"]
    pub async fn count_followers(pool: &SqlitePool, actor_id: Uuid) -> Result<i64, DbError> {
        let n = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!: i64" FROM followers WHERE actor_id = ? AND accepted = 1"#,
            actor_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(n)
    }

    /// Count accepted following of `actor_id`, excluding Group (club) actors.
    #[must_use = "Result must be checked"]
    pub async fn count_following(pool: &SqlitePool, actor_id: Uuid) -> Result<i64, DbError> {
        let n = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!: i64"
               FROM following f
               JOIN actors a ON a.id = f.target_id
               WHERE f.actor_id = ? AND f.accepted = 1 AND a.actor_type != 'Group'"#,
            actor_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(n)
    }

    /// Insert a pending following row for `actor_id` → `target_id`.
    #[must_use = "Result must be checked"]
    pub async fn add_following(
        pool: &SqlitePool,
        actor_id: Uuid,
        target_id: Uuid,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO following (id, actor_id, target_id, accepted)
               VALUES (?, ?, ?, 0)
               ON CONFLICT (actor_id, target_id) DO NOTHING"#,
            id,
            actor_id,
            target_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Mark a following row as accepted.
    #[must_use = "Result must be checked"]
    pub async fn accept_following(
        pool: &SqlitePool,
        actor_id: Uuid,
        target_id: Uuid,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE following SET accepted = 1 WHERE actor_id = ? AND target_id = ?",
            actor_id,
            target_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove a following row.
    #[must_use = "Result must be checked"]
    pub async fn remove_following(
        pool: &SqlitePool,
        actor_id: Uuid,
        target_id: Uuid,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "DELETE FROM following WHERE actor_id = ? AND target_id = ?",
            actor_id,
            target_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Insert a follower row (accepted or pending).
    #[must_use = "Result must be checked"]
    pub async fn add_follower(
        conn: &mut SqliteConnection,
        actor_id: Uuid,
        follower_id: Uuid,
        accepted: bool,
        follow_ap_id: Option<&str>,
    ) -> Result<(), DbError> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO followers (id, actor_id, follower_id, accepted, follow_ap_id)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT (actor_id, follower_id) DO UPDATE SET
                   accepted = excluded.accepted,
                   follow_ap_id = COALESCE(excluded.follow_ap_id, followers.follow_ap_id)"#,
            id,
            actor_id,
            follower_id,
            accepted,
            follow_ap_id,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Accept a pending follower.
    #[must_use = "Result must be checked"]
    pub async fn accept_follower(
        conn: &mut SqliteConnection,
        actor_id: Uuid,
        follower_id: Uuid,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE followers SET accepted = 1 WHERE actor_id = ? AND follower_id = ?",
            actor_id,
            follower_id,
        )
        .execute(&mut *conn)
        .await?;
        Ok(())
    }

    /// Remove a follower row.
    #[must_use = "Result must be checked"]
    pub async fn remove_follower(
        pool: &SqlitePool,
        actor_id: Uuid,
        follower_id: Uuid,
    ) -> Result<(), DbError> {
        sqlx::query!(
            "DELETE FROM followers WHERE actor_id = ? AND follower_id = ?",
            actor_id,
            follower_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Accept all pending follower rows for `actor_id` (used when switching to public profile).
    #[must_use = "Result must be checked"]
    pub async fn accept_all_pending(pool: &SqlitePool, actor_id: Uuid) -> Result<(), DbError> {
        sqlx::query!(
            "UPDATE followers SET accepted = 1 WHERE actor_id = ? AND accepted = 0",
            actor_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Retrieve the stored AP ID of an original Follow activity (if any).
    #[must_use = "Result must be checked"]
    pub async fn get_follow_ap_id(
        pool: &SqlitePool,
        actor_id: Uuid,
        follower_id: Uuid,
    ) -> Result<Option<String>, DbError> {
        let row = sqlx::query_scalar!(
            "SELECT follow_ap_id FROM followers WHERE actor_id = ? AND follower_id = ?",
            actor_id,
            follower_id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.flatten())
    }

    /// List all inbox URLs of accepted followers of `actor_id`.
    #[must_use = "Result must be checked"]
    pub async fn list_follower_inbox_urls(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<String>, DbError> {
        let rows = sqlx::query_scalar!(
            r#"SELECT COALESCE(a.shared_inbox_url, a.inbox_url) AS "url!: String"
               FROM followers f
               JOIN actors a ON a.id = f.follower_id
               WHERE f.actor_id = ? AND f.accepted = 1"#,
            actor_id,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// List inbox URLs of remote (non-local) accepted followers.
    #[must_use = "Result must be checked"]
    pub async fn list_remote_follower_inbox_urls(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<String>, DbError> {
        let rows = sqlx::query_scalar!(
            r#"SELECT COALESCE(a.shared_inbox_url, a.inbox_url) AS "url!: String"
               FROM followers f
               JOIN actors a ON a.id = f.follower_id
               WHERE f.actor_id = ? AND f.accepted = 1 AND a.is_local = 0"#,
            actor_id,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// List all local actors that follow `actor_id` (for Move redirect).
    #[must_use = "Result must be checked"]
    pub async fn list_local_followers_of(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<ActorRow>, DbError> {
        use crate::db::models::actor::ApId;
        use crate::db::types::JsonVec;
        sqlx::query_as!(
            ActorRow,
            r#"SELECT a.id AS "id: Uuid",
                      a.ap_id AS "ap_id: ApId",
                      a.username, a.domain, a.actor_type,
                      a.display_name, a.summary, a.public_key_pem, a.private_key_pem,
                      a.inbox_url, a.outbox_url, a.followers_url, a.following_url,
                      a.shared_inbox_url,
                      a.manually_approves_followers AS "manually_approves_followers: bool",
                      a.is_local AS "is_local: bool",
                      a.is_suspended AS "is_suspended: bool",
                      a.avatar_url,
                      a.also_known_as AS "also_known_as: JsonVec<String>",
                      a.moved_to,
                      a.created_at AS "created_at: chrono::DateTime<chrono::Utc>",
                      a.updated_at AS "updated_at: chrono::DateTime<chrono::Utc>"
               FROM followers f
               JOIN actors a ON a.id = f.follower_id
               WHERE f.actor_id = ? AND a.is_local = 1 AND f.accepted = 1"#,
            actor_id,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// List detailed following info (for display in UI).
    #[must_use = "Result must be checked"]
    pub async fn list_following(
        pool: &SqlitePool,
        actor_id: Uuid,
        limit: i64,
    ) -> Result<Vec<FollowingDetailRow>, DbError> {
        sqlx::query_as!(
            FollowingDetailRow,
            r#"SELECT a.ap_id, a.username, a.domain,
                      a.is_local AS "is_local: bool",
                      a.display_name, a.avatar_url,
                      f.accepted AS "accepted: bool"
               FROM following f
               JOIN actors a ON a.id = f.target_id
               WHERE f.actor_id = ?
               ORDER BY f.created_at DESC
               LIMIT ?"#,
            actor_id,
            limit,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// List detailed follower info (for display in UI).
    #[must_use = "Result must be checked"]
    pub async fn list_followers(
        pool: &SqlitePool,
        actor_id: Uuid,
        limit: i64,
    ) -> Result<Vec<FollowerDetailRow>, DbError> {
        sqlx::query_as!(
            FollowerDetailRow,
            r#"SELECT a.ap_id, a.username, a.domain,
                      a.is_local AS "is_local: bool",
                      a.display_name, a.avatar_url,
                      f.accepted AS "accepted: bool",
                      f.follow_ap_id
               FROM followers f
               JOIN actors a ON a.id = f.follower_id
               WHERE f.actor_id = ?
               ORDER BY f.created_at DESC
               LIMIT ?"#,
            actor_id,
            limit,
        )
        .fetch_all(pool)
        .await
        .map_err(DbError::Sqlx)
    }

    /// List pending (not yet accepted) follow requests for `actor_id`.
    #[must_use = "Result must be checked"]
    pub async fn list_pending_followers(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<FollowerDetailRow>, DbError> {
        use sqlx::Row as _;
        let rows = sqlx::query(
            r#"SELECT a.ap_id, a.username, a.domain,
                      a.is_local,
                      a.display_name, a.avatar_url,
                      f.accepted,
                      f.follow_ap_id
               FROM followers f
               JOIN actors a ON a.id = f.follower_id
               WHERE f.actor_id = ? AND f.accepted = 0
               ORDER BY f.created_at ASC"#,
        )
        .bind(actor_id)
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(|r| {
                Ok(FollowerDetailRow {
                    ap_id: r.try_get("ap_id")?,
                    username: r.try_get("username")?,
                    domain: r.try_get("domain")?,
                    is_local: r.try_get::<bool, _>("is_local")?,
                    display_name: r.try_get("display_name")?,
                    avatar_url: r.try_get("avatar_url")?,
                    accepted: r.try_get::<bool, _>("accepted")?,
                    follow_ap_id: r.try_get("follow_ap_id")?,
                })
            })
            .collect()
    }

    /// List who `actor_id` is following, joined with actor info for display.
    #[must_use = "Result must be checked"]
    pub async fn list_following_detail(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<FollowingDetailRow>, DbError> {
        use sqlx::Row as _;
        let rows = sqlx::query(
            r#"SELECT a.ap_id, a.username, a.domain,
                      a.is_local,
                      a.display_name, a.avatar_url,
                      f.accepted
               FROM following f
               JOIN actors a ON a.id = f.target_id
               WHERE f.actor_id = ? AND a.actor_type = 'Person'
               ORDER BY f.created_at DESC"#,
        )
        .bind(actor_id)
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(|r| {
                Ok(FollowingDetailRow {
                    ap_id: r.try_get("ap_id")?,
                    username: r.try_get("username")?,
                    domain: r.try_get("domain")?,
                    is_local: r.try_get::<bool, _>("is_local")?,
                    display_name: r.try_get("display_name")?,
                    avatar_url: r.try_get("avatar_url")?,
                    accepted: r.try_get::<bool, _>("accepted")?,
                })
            })
            .collect()
    }

    /// List accepted Person followers of `actor_id`, joined with actor info for display.
    #[must_use = "Result must be checked"]
    pub async fn list_followers_detail(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<FollowerDetailRow>, DbError> {
        use sqlx::Row as _;
        let rows = sqlx::query(
            r#"SELECT a.ap_id, a.username, a.domain,
                      a.is_local,
                      a.display_name, a.avatar_url,
                      f.accepted,
                      f.follow_ap_id
               FROM followers f
               JOIN actors a ON a.id = f.follower_id
               WHERE f.actor_id = ? AND f.accepted = 1 AND a.actor_type = 'Person'
               ORDER BY f.created_at DESC"#,
        )
        .bind(actor_id)
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(|r| {
                Ok(FollowerDetailRow {
                    ap_id: r.try_get("ap_id")?,
                    username: r.try_get("username")?,
                    domain: r.try_get("domain")?,
                    is_local: r.try_get::<bool, _>("is_local")?,
                    display_name: r.try_get("display_name")?,
                    avatar_url: r.try_get("avatar_url")?,
                    accepted: r.try_get::<bool, _>("accepted")?,
                    follow_ap_id: r.try_get("follow_ap_id")?,
                })
            })
            .collect()
    }

    /// List inbox URLs of all actors `actor_id` is following (any type, accepted only).
    #[must_use = "Result must be checked"]
    pub async fn list_following_inbox_urls(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<(String, String)>, DbError> {
        let rows = sqlx::query_as::<_, (String, String)>(
            r#"SELECT a.ap_id, COALESCE(a.shared_inbox_url, a.inbox_url)
               FROM following f
               JOIN actors a ON a.id = f.target_id
               WHERE f.actor_id = ? AND f.accepted = 1 AND a.is_local = 0"#,
        )
        .bind(actor_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// List Group-type actors the owner is following (their joined clubs).
    #[must_use = "Result must be checked"]
    pub async fn list_joined_clubs(
        pool: &SqlitePool,
        actor_id: Uuid,
    ) -> Result<Vec<FollowingDetailRow>, DbError> {
        use sqlx::Row as _;
        let rows = sqlx::query(
            r#"SELECT a.ap_id, a.username, a.domain,
                      a.is_local,
                      a.display_name, a.avatar_url,
                      f.accepted
               FROM following f
               JOIN actors a ON a.id = f.target_id
               WHERE f.actor_id = ? AND a.actor_type = 'Group'
               ORDER BY f.created_at DESC"#,
        )
        .bind(actor_id)
        .fetch_all(pool)
        .await?;
        rows.into_iter()
            .map(|r| {
                Ok(FollowingDetailRow {
                    ap_id: r.try_get("ap_id")?,
                    username: r.try_get("username")?,
                    domain: r.try_get("domain")?,
                    is_local: r.try_get::<bool, _>("is_local")?,
                    display_name: r.try_get("display_name")?,
                    avatar_url: r.try_get("avatar_url")?,
                    accepted: r.try_get::<bool, _>("accepted")?,
                })
            })
            .collect()
    }

    /// Accept a pending follower using a pool (acquires a connection internally).
    #[must_use = "Result must be checked"]
    pub async fn accept_follower_pool(
        pool: &SqlitePool,
        actor_id: Uuid,
        follower_id: Uuid,
    ) -> Result<(), DbError> {
        sqlx::query("UPDATE followers SET accepted = 1 WHERE actor_id = ? AND follower_id = ?")
            .bind(actor_id)
            .bind(follower_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
