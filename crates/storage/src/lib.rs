//! Shiftbook storage: repository traits and the SQL implementation.
//!
//! Empty in M0. From M1 this holds the sqlx/SQLCipher repositories and the
//! migrations in `migrations/`, written to run on both SQLite and Postgres.

/// File name of the local database inside the app data directory.
pub const DATABASE_FILE_NAME: &str = "shiftbook.db";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_file_name_matches_spec() {
        assert_eq!(DATABASE_FILE_NAME, "shiftbook.db");
    }
}
