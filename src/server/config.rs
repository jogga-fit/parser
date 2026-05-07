//! Application configuration for jogga (single-owner ActivityPub server).
//!
//! ## Sources (lowest → highest priority)
//!
//! 1. Built-in defaults
//! 2. TOML file — path supplied via `--config <FILE>` CLI flag
//! 3. Environment variables — prefix `JOGGA`, separator `__`
//!    e.g. `JOGGA__DATABASE__URL`, `JOGGA__SERVER__PORT`
//!
//! ## Key differences from fedisport
//!
//! - `[owner]` section with `contact` field (email or E.164 phone) for OTP delivery
//! - No multi-user registration; `validate()` warns instead of panicking when
//!   no delivery channel is configured
//! - Config prefix: `JOGGA__` not `FEDISPORT__`

use std::{path::Path, sync::Arc};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config load error: {0}")]
    Load(#[from] config::ConfigError),
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: Arc<ServerConfig>,
    pub database: Arc<DatabaseConfig>,
    pub instance: Arc<InstanceConfig>,
    pub owner: Arc<OwnerConfig>,
    pub email: Option<Arc<EmailConfig>>,
    pub sms: Option<Arc<SmsConfig>>,
    pub storage: Option<Arc<StorageConfig>>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "defaults::server_host")]
    pub host: String,
    #[serde(default = "defaults::server_port")]
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "defaults::db_max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Deserialize)]
pub struct InstanceConfig {
    pub domain: String,
    #[serde(default = "defaults::instance_name")]
    pub name: String,
    #[serde(default = "defaults::instance_description")]
    pub description: String,
}

impl InstanceConfig {
    /// Returns the URL scheme for this instance.
    /// Returns `"http"` for localhost in debug builds, always `"https"` in release.
    pub fn scheme(&self) -> &'static str {
        #[cfg(debug_assertions)]
        if self.domain.starts_with("localhost") {
            return "http";
        }
        "https"
    }

    /// Constructs a full base URL: `scheme://domain`.
    pub fn base_url(&self) -> String {
        format!("{}://{}", self.scheme(), self.domain)
    }
}

/// Single-owner configuration.
#[derive(Debug, Deserialize)]
pub struct OwnerConfig {
    /// Contact email or E.164 phone number for OTP delivery (password reset).
    pub contact: String,
    /// Username for the owner actor (defaults to "owner").
    #[serde(default = "defaults::owner_username")]
    pub username: String,
}

/// SMTP configuration for email OTP delivery.
#[derive(Debug, Deserialize)]
pub struct EmailConfig {
    pub smtp_host: String,
    #[serde(default = "defaults::smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub smtp_username: String,
    #[serde(default)]
    pub smtp_password: String,
    #[serde(default = "defaults::smtp_from")]
    pub from_address: String,
}

/// Twilio configuration for SMS OTP delivery.
#[derive(Debug, Deserialize)]
pub struct SmsConfig {
    pub account_sid: String,
    #[serde(default)]
    pub auth_token: String,
    #[serde(default)]
    pub from_number: String,
}

/// Object storage configuration (optional — when absent, file uploads are skipped).
#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub endpoint: Option<String>,
    pub public_url: String,
    pub bucket: String,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

mod defaults {
    pub fn server_host() -> String {
        "0.0.0.0".into()
    }
    pub fn server_port() -> u16 {
        6060
    }
    pub fn db_max_connections() -> u32 {
        5
    }
    pub fn instance_name() -> String {
        "jogga".into()
    }
    pub fn instance_description() -> String {
        "A jogga ActivityPub server".into()
    }
    pub fn owner_username() -> String {
        "owner".into()
    }
    pub fn smtp_port() -> u16 {
        587
    }
    pub fn smtp_from() -> String {
        "noreply@localhost".into()
    }
}

#[derive(Deserialize)]
struct RawConfig {
    server: RawServerConfig,
    database: RawDatabaseConfig,
    instance: RawInstanceConfig,
    #[serde(default)]
    owner: RawOwnerConfig,
    #[serde(default)]
    email: Option<RawEmailConfig>,
    #[serde(default)]
    sms: Option<RawSmsConfig>,
    #[serde(default)]
    storage: Option<RawStorageConfig>,
}

#[derive(Deserialize)]
struct RawServerConfig {
    #[serde(default = "defaults::server_host")]
    host: String,
    #[serde(default = "defaults::server_port")]
    port: u16,
}

#[derive(Deserialize)]
struct RawDatabaseConfig {
    url: String,
    #[serde(default = "defaults::db_max_connections")]
    max_connections: u32,
}

#[derive(Deserialize)]
struct RawInstanceConfig {
    domain: String,
    #[serde(default = "defaults::instance_name")]
    name: String,
    #[serde(default = "defaults::instance_description")]
    description: String,
}

#[derive(Deserialize, Default)]
struct RawOwnerConfig {
    #[serde(default)]
    contact: String,
    #[serde(default = "defaults::owner_username")]
    username: String,
}

#[derive(Deserialize)]
struct RawEmailConfig {
    smtp_host: String,
    #[serde(default = "defaults::smtp_port")]
    smtp_port: u16,
    #[serde(default)]
    smtp_username: String,
    #[serde(default)]
    smtp_password: String,
    #[serde(default = "defaults::smtp_from")]
    from_address: String,
}

#[derive(Deserialize)]
struct RawSmsConfig {
    account_sid: String,
    #[serde(default)]
    auth_token: String,
    #[serde(default)]
    from_number: String,
}

#[derive(Deserialize)]
struct RawStorageConfig {
    endpoint: Option<String>,
    public_url: String,
    bucket: String,
    region: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
}

impl AppConfig {
    /// Load configuration from optional TOML file and/or environment variables.
    pub fn load(file_path: Option<&Path>) -> Result<Self, ConfigError> {
        let mut builder = config::Config::builder();

        if let Some(path) = file_path {
            builder = builder.add_source(config::File::from(path).required(true));
        }

        builder = builder.add_source(
            config::Environment::with_prefix("JOGGA")
                .prefix_separator("__")
                .separator("__")
                .try_parsing(true)
                .ignore_empty(true),
        );

        let raw: RawConfig = builder.build()?.try_deserialize()?;
        let cfg = Self::from_raw(raw);
        cfg.validate();
        Ok(cfg)
    }

    fn from_raw(r: RawConfig) -> Self {
        Self {
            server: Arc::new(ServerConfig {
                host: r.server.host,
                port: r.server.port,
            }),
            database: Arc::new(DatabaseConfig {
                url: r.database.url,
                max_connections: r.database.max_connections,
            }),
            instance: Arc::new(InstanceConfig {
                domain: r.instance.domain,
                name: r.instance.name,
                description: r.instance.description,
            }),
            owner: Arc::new(OwnerConfig {
                contact: r.owner.contact,
                username: r.owner.username,
            }),
            email: r.email.map(|e| {
                Arc::new(EmailConfig {
                    smtp_host: e.smtp_host,
                    smtp_port: e.smtp_port,
                    smtp_username: e.smtp_username,
                    smtp_password: e.smtp_password,
                    from_address: e.from_address,
                })
            }),
            sms: r.sms.map(|s| {
                Arc::new(SmsConfig {
                    account_sid: s.account_sid,
                    auth_token: s.auth_token,
                    from_number: s.from_number,
                })
            }),
            storage: r.storage.map(|s| {
                Arc::new(StorageConfig {
                    endpoint: s.endpoint,
                    public_url: s.public_url,
                    bucket: s.bucket,
                    region: s.region,
                    access_key_id: s.access_key_id,
                    secret_access_key: s.secret_access_key,
                })
            }),
        }
    }

    fn validate(&self) {
        let has_delivery = self.email.is_some() || self.sms.is_some();
        let owner_has_contact = !self.owner.contact.is_empty();

        // In release mode, warn (not panic) when no delivery channel is configured.
        // jogga is single-owner: delivery is optional but recommended.
        if !has_delivery && owner_has_contact {
            tracing::warn!(
                "No email or SMS delivery channel configured. \
                 Password-reset OTPs cannot be delivered. \
                 Add [email] or [sms] to your config."
            );
        }

    }

    /// Construct a minimal config for integration tests.
    pub fn for_test(database_url: &str, domain: &str) -> Self {
        Self {
            server: Arc::new(ServerConfig {
                host: "127.0.0.1".into(),
                port: 0,
            }),
            database: Arc::new(DatabaseConfig {
                url: database_url.to_string(),
                max_connections: 5,
            }),
            instance: Arc::new(InstanceConfig {
                domain: domain.to_string(),
                name: "Test Instance".into(),
                description: "Integration test instance".into(),
            }),
            owner: Arc::new(OwnerConfig {
                contact: "test@example.com".into(),
                username: "owner".into(),
            }),
            email: None,
            sms: None,
            storage: None,
        }
    }
}
