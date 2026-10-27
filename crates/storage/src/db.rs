//! The encrypted database (SQLCipher through sqlx) and its migrations.
//!
//! Every connection runs, in this order (sqlx always puts `key` first):
//! `PRAGMA key = "x'<DEK hex>'"`, `PRAGMA cipher_compatibility = 4`,
//! `PRAGMA foreign_keys = ON`, `PRAGMA cipher_memory_security = ON`.
//! The raw 256-bit DEK is used directly, so SQLCipher's own PBKDF2 is skipped.
//! Never set `cipher_plaintext_header_size` or `temp_store = FILE`; statement
//! logging is off so the key pragma can't reach a log.
//!
//! Known limit: sqlx keeps the pragma text (with the hex key) in the connect
//! options for as long as the pool lives, as an ordinary `String` that is not
//! wiped. The options must never be printed with `{:?}`.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use sqlx::migrate::{Migration, MigrationType, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlSafeStr};

use crate::error::StorageError;
use crate::fsutil;
use crate::hex;
use crate::vault::Dek;
use crate::DATABASE_FILE_NAME;

/// Most connections the pool opens. SQLite allows one writer at a time anyway.
const MAX_CONNECTIONS: u32 = 4;

/// The schema migrations, embedded in the binary.
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (
        1,
        "initial schema",
        include_str!("../migrations/0001_initial_schema.sql"),
    ),
    (
        2,
        "alpha: colours, tray, custom fields, tick-offs, inbox",
        include_str!("../migrations/0002_alpha.sql"),
    ),
    (
        3,
        "next up list",
        include_str!("../migrations/0003_next_up.sql"),
    ),
    (4, "notes", include_str!("../migrations/0004_notes.sql")),
    (
        5,
        "saved responses",
        include_str!("../migrations/0005_saved_responses.sql"),
    ),
    (6, "canvas", include_str!("../migrations/0006_canvas.sql")),
    (
        7,
        "canvas areas and links",
        include_str!("../migrations/0007_canvas_areas_links.sql"),
    ),
    (
        8,
        "open tickets, ticket projects, bring up",
        include_str!("../migrations/0008_tickets_open_and_bring_up.sql"),
    ),
    (
        9,
        "meeting series",
        include_str!("../migrations/0009_meeting_series.sql"),
    ),
    (
        10,
        "canvas boxes",
        include_str!("../migrations/0010_canvas_boxes.sql"),
    ),
    (
        11,
        "stacks and drawers",
        include_str!("../migrations/0011_stacks_drawers.sql"),
    ),
    (
        12,
        "board spots",
        include_str!("../migrations/0012_board_spots.sql"),
    ),
    (13, "pins", include_str!("../migrations/0013_pins.sql")),
    (
        14,
        "ticket done",
        include_str!("../migrations/0014_ticket_done.sql"),
    ),
    (15, "items", include_str!("../migrations/0015_items.sql")),
    (16, "lists", include_str!("../migrations/0016_lists.sql")),
    (
        17,
        "meeting length, project and logged day",
        include_str!("../migrations/0017_meeting_log.sql"),
    ),
    (
        18,
        "repeating work",
        include_str!("../migrations/0018_repeating_work.sql"),
    ),
    (
        19,
        "MOP notebook: templates and ticks",
        include_str!("../migrations/0019_mop_notebook.sql"),
    ),
];

fn migrator() -> Migrator {
    Migrator::with_migrations(
        MIGRATIONS
            .iter()
            .map(|(version, description, sql)| {
                Migration::new(
                    *version,
                    Cow::Borrowed(*description),
                    MigrationType::Simple,
                    sql.into_sql_str(),
                    false,
                )
            })
            .collect(),
    )
}

/// Path of the database inside the data folder.
pub fn database_path(dir: &Path) -> PathBuf {
    dir.join(DATABASE_FILE_NAME)
}

/// Connect options for an encrypted database file. The key pragma value is
/// `"x'…'"` (a raw key, not a passphrase).
fn encrypted_options(path: &Path, dek: &Dek) -> SqliteConnectOptions {
    let key_hex = hex::encode_secret(dek.expose());
    let pragma_value = zeroize::Zeroizing::new(format!("\"x'{}'\"", key_hex.as_str()));
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .pragma("key", pragma_value.as_str().to_owned())
        .pragma("cipher_compatibility", "4")
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Delete)
        .pragma("cipher_memory_security", "ON")
        .disable_statement_logging()
}

/// Open (or create) the encrypted database at `path` with `dek`, check the key
/// works, then run migrations. A wrong key gives [`StorageError::CannotOpen`].
pub async fn open_encrypted(path: &Path, dek: &Dek) -> Result<SqlitePool, StorageError> {
    if let Some(dir) = path.parent() {
        fsutil::create_private_dir(dir).map_err(StorageError::Io)?;
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(encrypted_options(path, dek))
        .await
        .map_err(|_| StorageError::CannotOpen)?;
    // The first read of the file is where a wrong key shows up.
    if sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sqlite_master")
        .fetch_one(&pool)
        .await
        .is_err()
    {
        pool.close().await;
        return Err(StorageError::CannotOpen);
    }
    fsutil::make_file_private(path).map_err(StorageError::Io)?;
    migrator().run(&pool).await?;
    #[cfg(test)]
    crate::test_rt::track(&pool);
    Ok(pool)
}

/// Unit-test helper: options for opening a file with no key at all.
#[cfg(test)]
pub(crate) fn plain_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .disable_statement_logging()
}

#[cfg(test)]
pub(crate) fn test_dek(byte: u8) -> Dek {
    Dek::from_bytes_for_tests([byte; 32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::test_dir::TestDir;
    use crate::test_rt::block_on;
    use sqlx::{Connection, Row};

    #[test]
    fn database_cannot_be_read_without_the_key() {
        let dir = TestDir::new("db-key");
        let path = database_path(dir.path());
        let dek = test_dek(7);
        block_on(async {
            let pool = open_encrypted(&path, &dek).await.unwrap();
            sqlx::query(
                "INSERT INTO users (id, display_name, created_at, updated_at) \
                 VALUES ($1, $2, $3, $3)",
            )
            .bind("0190f6a0-0000-7000-8000-000000000001")
            .bind("Secret Name")
            .bind("2026-10-15T00:00:00Z")
            .execute(&pool)
            .await
            .unwrap();
            pool.close().await;

            // The file does not start with the plain SQLite header.
            let bytes = std::fs::read(&path).unwrap();
            assert!(bytes.len() >= 4096);
            assert_ne!(&bytes[..16], b"SQLite format 3\0");
            // And the inserted text isn't in it anywhere.
            assert!(!bytes.windows(11).any(|w| w == b"Secret Name"));

            // No key: the first query fails.
            let mut conn = plain_options(&path).connect().await.unwrap();
            let res = sqlx::query("SELECT count(*) FROM sqlite_master")
                .fetch_one(&mut conn)
                .await;
            assert!(res.is_err(), "opened without a key");
            conn.close().await.ok();

            // Wrong key: refused by open_encrypted.
            assert!(matches!(
                open_encrypted(&path, &test_dek(8)).await,
                Err(StorageError::CannotOpen)
            ));

            // Right key: the data is there.
            let pool = open_encrypted(&path, &dek).await.unwrap();
            let name: String = sqlx::query_scalar("SELECT display_name FROM users")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(name, "Secret Name");
            pool.close().await;
        });
    }

    #[test]
    fn key_pragma_runs_before_anything_else() {
        // If any statement touched the file before `PRAGMA key`, SQLCipher would
        // treat the file as plain and a later key would be ignored or fail. A
        // brand-new file written through the pool must be encrypted from page 1,
        // and every pooled connection (not just the first) must carry the key.
        let dir = TestDir::new("db-order");
        let path = database_path(dir.path());
        let dek = test_dek(42);
        block_on(async {
            let pool = open_encrypted(&path, &dek).await.unwrap();
            let mut held = Vec::new();
            for _ in 0..MAX_CONNECTIONS {
                let mut conn = pool.acquire().await.unwrap();
                let n: i64 = sqlx::query_scalar("SELECT count(*) FROM settings")
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap();
                assert_eq!(n, 0);
                let row = sqlx::query("PRAGMA cipher_memory_security")
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap();
                let value: String = row.try_get(0).unwrap();
                assert_eq!(value, "1", "cipher_memory_security is on");
                let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap();
                assert_eq!(fk, 1);
                held.push(conn);
            }
            let version: String = sqlx::query_scalar("PRAGMA cipher_version")
                .fetch_one(&mut *held[0])
                .await
                .unwrap();
            assert!(version.starts_with('4'), "SQLCipher 4.x, got {version}");
            drop(held);
            pool.close().await;
        });
        let bytes = std::fs::read(&path).unwrap();
        assert_ne!(&bytes[..16], b"SQLite format 3\0");
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = TestDir::new("db-migrate");
        let path = database_path(dir.path());
        let dek = test_dek(1);
        block_on(async {
            let pool = open_encrypted(&path, &dek).await.unwrap();
            pool.close().await;
            let pool = open_encrypted(&path, &dek).await.unwrap();
            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT name FROM sqlite_master WHERE type = 'table' \
                 AND name NOT LIKE '\\_%' ESCAPE '\\' AND name NOT LIKE 'sqlite%' ORDER BY name",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(
                tables,
                [
                    "board_cards",
                    "canvas_areas",
                    "canvas_cards",
                    "canvas_folders",
                    "canvas_labels",
                    "canvas_links",
                    "canvas_stacks",
                    "category_code_defaults",
                    "custom_fields",
                    "custom_values",
                    "frequencies",
                    "inbox_items",
                    "items",
                    "kind_code_defaults",
                    "list_options",
                    "meeting_series",
                    "next_up_items",
                    "notes",
                    "pins",
                    "projects",
                    "recurring_tasks",
                    "saved_responses",
                    "settings",
                    "task_checklist_items",
                    "tasks",
                    "ticket_tray",
                    "tickets",
                    "time_entries",
                    "timesheet_codes",
                    "timesheet_ticks",
                    "timesheet_weeks",
                    "users"
                ]
            );
            // Every table has the common columns.
            for table in &tables {
                let cols: Vec<String> =
                    sqlx::query_scalar("SELECT name FROM pragma_table_info($1)")
                        .bind(table)
                        .fetch_all(&pool)
                        .await
                        .unwrap();
                for common in ["id", "created_at", "updated_at", "deleted_at"] {
                    assert!(cols.iter().any(|c| c == common), "{table}.{common}");
                }
                if table != "users" {
                    assert!(cols.iter().any(|c| c == "owner_id"), "{table}.owner_id");
                }
            }
            pool.close().await;
        });
    }

    #[cfg(unix)]
    #[test]
    fn database_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TestDir::new("db-perms");
        let path = database_path(dir.path());
        block_on(async {
            let pool = open_encrypted(&path, &test_dek(3)).await.unwrap();
            pool.close().await;
        });
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
