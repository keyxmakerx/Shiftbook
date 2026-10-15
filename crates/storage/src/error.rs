//! Errors from the database and repositories. Messages are plain words; the
//! underlying sqlx error is kept as the `source` for debugging but never shown.

use std::io;

use shiftbook_core::CoreError;
use thiserror::Error;

/// Something went wrong reading or writing the database.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Wrong key, or the file is not a Shiftbook database.
    #[error("The data file couldn't be opened.")]
    CannotOpen,
    /// A database operation failed.
    #[error("The data file couldn't be read or saved.")]
    Database(#[source] sqlx::Error),
    /// The schema couldn't be brought up to date.
    #[error("The data file couldn't be updated for this version of Shiftbook.")]
    Migrate(#[source] sqlx::migrate::MigrateError),
    /// The data folder couldn't be created or secured.
    #[error("The data folder couldn't be prepared.")]
    Io(#[source] io::Error),
    /// The input broke a rule from core validation.
    #[error("{0}")]
    Invalid(#[from] CoreError),
    /// "Done" and "Blocked" drive the alert rules and can't be renamed or hidden.
    #[error("\"{0}\" is used by the alerts, so it can't be renamed or hidden.")]
    Protected(String),
    /// The value is already in the list (ignoring case).
    #[error("\"{0}\" is already in this list.")]
    Duplicate(String),
    /// The tray's roll-up entry gets its time and tickets from the tray.
    #[error("This entry's time and tickets come from the Done tickets tray.")]
    TrayManaged,
    /// The record doesn't exist (or belongs to someone else).
    #[error("That item no longer exists.")]
    NotFound,
    /// A stored value couldn't be read back.
    #[error("Some saved data couldn't be read ({0}).")]
    Corrupt(&'static str),
}

impl From<sqlx::Error> for StorageError {
    fn from(e: sqlx::Error) -> Self {
        StorageError::Database(e)
    }
}

impl From<sqlx::migrate::MigrateError> for StorageError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        StorageError::Migrate(e)
    }
}
