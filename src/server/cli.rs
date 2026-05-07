use clap::{Parser, Subcommand};

use crate::{db::SqlitePool, server::error::AppError, server::service};

#[derive(Parser)]
#[command(name = "jogga", about = "jogga ActivityPub server")]
pub struct Cli {
    /// Path to TOML config file (overrides env vars).
    #[arg(short = 'c', long = "config")]
    pub config: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Seed the single local owner account.
    ///
    /// Creates the actor + local_accounts row with an RSA keypair.
    /// Must be run once against a fresh DB before starting the server.
    SeedOwner {
        /// Password for the owner account.
        #[arg(long)]
        password: String,
    },
}

/// Arguments for seeding the owner account. Used by the CLI subcommand and
/// in integration tests via [`seed_owner`].
pub struct SeedOwnerArgs {
    pub username: String,
    pub email: String,
    pub password: String,
    pub domain: String,
}

/// Seed the single owner account with the supplied arguments.
///
/// This is a thin wrapper around [`service::seed_owner`] that maps the flat
/// [`SeedOwnerArgs`] struct to the positional parameters expected by the
/// service function. Exposed here so integration tests can import it from
/// `crate::server::cli` alongside the `SeedOwnerArgs` type.
pub async fn seed_owner(pool: &SqlitePool, args: SeedOwnerArgs) -> Result<(), AppError> {
    service::seed_owner(
        pool,
        &args.username,
        &args.password,
        &args.domain,
        "https",
        &args.email,
    )
    .await
}
