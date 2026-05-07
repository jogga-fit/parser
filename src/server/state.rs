use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::db::{DbConfig, SqlitePool, create_pool};
use reqwest::Client;
use thiserror::Error;
use uuid::Uuid;

use crate::server::{config::AppConfig, notify::AppNotifier};

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("database: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("http client: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("notifier setup: {0}")]
    Notifier(#[from] crate::server::notify::NotifyError),
}

/// Shared application state injected via `FederationMiddleware`.
///
/// All fields are cheap to clone — `db` is a pool handle (Arc internally),
/// `http` is `reqwest::Client` (Arc internally), `config` is Arc-wrapped,
/// `notifier` is Arc.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub http: Client,
    pub config: Arc<AppConfig>,
    pub notifier: Arc<AppNotifier>,
    /// In-memory OTP wrong-attempt counter.
    /// Keyed by OTP row UUID; value is the number of wrong attempts.
    pub otp_attempts: Arc<Mutex<HashMap<Uuid, u8>>>,
}

impl AppState {
    pub async fn new(config: &AppConfig) -> Result<Self, StartupError> {
        let db_config = DbConfig {
            url: config.database.url.clone(),
            max_connections: config.database.max_connections,
        };
        let db = create_pool(&db_config).await?;

        // Run migrations.
        sqlx::migrate!()
            .run(&db)
            .await
            .map_err(crate::db::DbError::Migration)?;

        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .user_agent("jogga/0.1 (ActivityPub; +https://github.com/devdutt/jogga)")
            .build()?;

        let notifier = Arc::new(AppNotifier::new(config, http.clone())?);

        Ok(Self {
            db,
            http,
            config: Arc::new(config.clone()),
            notifier,
            otp_attempts: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}
