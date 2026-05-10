use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use crate::db::error::DbError;

/// SQLite connection pool configuration.
///
/// `url` is a SQLite connection string — typically `sqlite:///path/to/db.sqlite3`
/// or `sqlite::memory:`.  The default `max_connections` of 5 is appropriate for a
/// single-user instance; SQLite serialises writes regardless of pool size, so the
/// limit primarily caps concurrent readers.
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 5,
        }
    }
}

/// Create a SQLite connection pool.
///
/// Foreign keys must be enabled per-connection on SQLite (the schema cannot
/// flip the pragma for a remote attacher), so this helper sets `PRAGMA
/// foreign_keys = ON` on every connection that joins the pool.
#[must_use = "Result must be checked"]
pub async fn create_pool(config: &DbConfig) -> Result<SqlitePool, DbError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(600)))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                use sqlx::Executor;
                conn.execute("PRAGMA foreign_keys = ON;").await?;
                conn.execute("PRAGMA journal_mode = WAL;").await?;
                conn.execute("PRAGMA busy_timeout = 5000;").await?;
                Ok(())
            })
        })
        .connect_with(
            config
                .url
                .parse::<SqliteConnectOptions>()
                .map_err(|e| DbError::Sqlx(sqlx::Error::Configuration(e.into())))?
                .create_if_missing(true),
        )
        .await?;
    Ok(pool)
}
