//! jogga/db — SQLite-backed data layer for the single-user jogga server.
//!
//! Sibling of the multi-user `db` crate in the closed-source fedisport
//! workspace. Not interchangeable at compile time: this crate binds to
//! `sqlx::Sqlite`; the multi-user crate binds to `sqlx::Postgres`. Model and
//! query module names stay in sync by hand (and eventually via a shared
//! `core-models` crate).

pub mod error;
pub mod models;
pub mod pool;
pub mod queries;
pub mod types;

pub use error::DbError;
pub use models::{ActivityRow, ActorRow, DeliveryRow, ExerciseRow, LocalAccount, OtpRequest};
pub use pool::{DbConfig, create_pool};
pub use queries::{
    AccountQueries, ActivityQueries, ActorQueries, AnnounceQueries, DeliveryQueries,
    ExerciseQueries, FollowQueries, LikeQueries, MediaAttachmentQueries, NotificationQueries,
    ObjectQueries, OtpQueries,
};
pub use sqlx::SqlitePool;
