use thiserror::Error;

/// Error type for the `db` crate.
///
/// `Conflict` and `Integrity` are raised by callers (or via [`DbError::from_sqlx`])
/// when a SQLite error code maps onto a uniqueness/foreign-key violation; the rest
/// is forwarded transparently from `sqlx`/`serde_json`.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("record not found")]
    NotFound,

    #[error("constraint violation: {0}")]
    Conflict(String),

    #[error("integrity violation: {0}")]
    Integrity(String),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

impl DbError {
    /// Convert a `sqlx::Error`, mapping SQLite uniqueness / FK errors onto
    /// the structured variants.  Falls through to `DbError::Sqlx` for anything
    /// else.
    ///
    /// SQLite reports `SQLITE_CONSTRAINT_*` as extended result codes 19xx;
    /// the three we care about are 2067 (UNIQUE), 1555 (PRIMARYKEY) and
    /// 787 (FOREIGNKEY).
    #[must_use]
    pub fn from_sqlx(e: sqlx::Error, conflict_msg: impl Into<String>) -> Self {
        if let sqlx::Error::Database(ref db_err) = e {
            // SQLite reports the extended code as a string in `code()`.
            match db_err.code().as_deref() {
                Some("2067") | Some("1555") | Some("19") => {
                    return DbError::Conflict(conflict_msg.into());
                }
                Some("787") => {
                    return DbError::Integrity(conflict_msg.into());
                }
                _ => {}
            }
            // Fall back to message inspection — sqlx reports human strings on
            // SQLite even when no SQLSTATE is present.
            let msg = db_err.message();
            if msg.contains("UNIQUE constraint failed") {
                return DbError::Conflict(conflict_msg.into());
            }
            if msg.contains("FOREIGN KEY constraint failed") {
                return DbError::Integrity(conflict_msg.into());
            }
        }
        DbError::Sqlx(e)
    }
}
