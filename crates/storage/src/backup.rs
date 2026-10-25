//! Encrypted backups (`.shiftbak`). See SECURITY.md "Backup file format".
//!
//! ```text
//! "SBAK" | u16 format version (BE) | u32 header length (BE) | header JSON | ciphertext
//! ciphertext = XChaCha20-Poly1305(DEK, nonce, gzip(payload JSON), AAD = every byte before it)
//! ```
//!
//! The header carries a copy of the vault (the DEK wrapped by the password and by
//! the recovery key, as they were when the backup was made), so a backup opens on
//! any computer with either secret. The DEK never changes, so on the computer
//! that made it a backup also opens with the key already unlocked in memory.
//!
//! The payload is every row of every table, so restoring replaces all data in one
//! transaction. Every way a file can be wrong (not a backup, damaged, edited,
//! wrong key) is checked before anything is changed.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, TypeInfo, ValueRef};
use thiserror::Error;
use time::macros::format_description;
use time::{Duration, OffsetDateTime, PrimitiveDateTime};
use zeroize::Zeroizing;

use crate::error::StorageError;
use crate::fsutil;
use crate::hex;
use crate::repo::SqliteStore;
use crate::vault::{Dek, SecretKind, VaultDir, VaultError};

/// File extension of a backup.
pub const BACKUP_EXTENSION: &str = "shiftbak";
/// Folder for automatic and safety backups, inside the data folder.
pub const BACKUP_DIR_NAME: &str = "backups";
/// How many automatic backups are kept (SPEC §2.10).
pub const KEEP_AUTOMATIC: usize = 14;
/// An automatic backup is made when the newest one is at least this old.
pub const AUTOMATIC_EVERY: Duration = Duration::hours(24);

const MAGIC: &[u8; 4] = b"SBAK";
const FORMAT_VERSION: u16 = 1;
const CIPHER: &str = "xchacha20poly1305";
const COMPRESSION: &str = "gzip";
const NONCE_BYTES: usize = 24;
const PREFIX_BYTES: usize = 4 + 2 + 4;
const MAX_HEADER_BYTES: u32 = 64 * 1024;
/// Largest backup file read, and largest payload unpacked (guards against a
/// crafted file that unpacks to something huge).
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_PAYLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const AUTO_PREFIX: &str = "auto-";
const BEFORE_RESTORE_PREFIX: &str = "before-restore-";
const STAMP: &[time::format_description::BorrowedFormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

/// Why a backup couldn't be made, opened or restored.
#[derive(Debug, Error)]
pub enum BackupError {
    /// Not a Shiftbook backup, or damaged or edited.
    #[error("This file isn't a Shiftbook backup, or it's damaged.")]
    Invalid,
    /// Made with a different key: the password or recovery key from then is needed.
    #[error("This backup needs the password or recovery key it was made with.")]
    NeedsSecret,
    /// The password or recovery key didn't open it.
    #[error("That password or recovery key doesn't open this backup.")]
    UnlockFailed,
    /// Made by a newer version of Shiftbook.
    #[error("This backup was made by a newer version of Shiftbook.")]
    TooNew,
    /// Reading or writing the database failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Reading or writing the file failed.
    #[error("The backup file couldn't be read or saved.")]
    Io(#[source] io::Error),
    /// The vault couldn't be read, or keys couldn't be prepared.
    #[error("The backup couldn't be prepared.")]
    Vault,
}

impl From<sqlx::Error> for BackupError {
    fn from(e: sqlx::Error) -> Self {
        BackupError::Storage(StorageError::Database(e))
    }
}

impl From<io::Error> for BackupError {
    fn from(e: io::Error) -> Self {
        BackupError::Io(e)
    }
}

/// The readable part of the file (also bound as AAD, so it can't be edited).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    app_version: String,
    created_at: String,
    schema_version: i64,
    cipher: String,
    compression: String,
    nonce: String,
    /// Tells "made with another key" apart from "damaged" without a secret.
    key_check: String,
    vault: Value,
}

/// One table: column names and rows of values in that order.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TableDump {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

/// The encrypted payload.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Contents {
    schema_version: i64,
    tables: BTreeMap<String, TableDump>,
}

/// What a backup holds, shown before restoring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    /// When it was made (UTC, RFC 3339).
    pub created_at: String,
    /// Time entries (not deleted).
    pub entries: u64,
    /// Tickets on those entries.
    pub tickets: u64,
    /// Projects (not deleted).
    pub projects: u64,
}

/// A backup that has been checked and decrypted, ready to restore.
pub struct OpenedBackup {
    created_at: String,
    contents: Contents,
}

/// How to open a backup.
#[derive(Clone, Copy)]
pub enum BackupKey<'a> {
    /// The key already unlocked on this computer.
    Current(&'a Dek),
    /// The password or recovery key from when the backup was made.
    Secret(&'a SecretString, SecretKind),
}

fn table_names_sql() -> &'static str {
    "SELECT name FROM sqlite_master WHERE type = 'table' \
     AND name NOT LIKE '\\_%' ESCAPE '\\' AND name NOT LIKE 'sqlite%' ORDER BY name"
}

async fn schema_version(store: &SqliteStore) -> Result<i64, BackupError> {
    let v: Option<i64> = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(store.pool())
        .await?;
    Ok(v.unwrap_or(0))
}

async fn table_columns(
    conn: &mut sqlx::SqliteConnection,
    table: &str,
) -> Result<Vec<String>, BackupError> {
    Ok(
        sqlx::query_scalar("SELECT name FROM pragma_table_info($1) ORDER BY cid")
            .bind(table)
            .fetch_all(&mut *conn)
            .await?,
    )
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn cell(row: &SqliteRow, i: usize) -> Result<Value, BackupError> {
    let raw = row.try_get_raw(i)?;
    if raw.is_null() {
        return Ok(Value::Null);
    }
    let kind = raw.type_info().name().to_ascii_uppercase();
    Ok(match kind.as_str() {
        "INTEGER" => Value::from(row.try_get::<i64, _>(i)?),
        "REAL" => Value::from(row.try_get::<f64, _>(i)?),
        "TEXT" => Value::from(row.try_get::<String, _>(i)?),
        // No table stores blobs; refuse rather than lose data silently.
        _ => {
            return Err(BackupError::Storage(StorageError::Corrupt(
                "unexpected blob",
            )))
        }
    })
}

/// Read every row of every table, in one read transaction so the copy is consistent.
async fn dump(store: &SqliteStore) -> Result<Contents, BackupError> {
    let schema_version = schema_version(store).await?;
    let mut tx = store.pool().begin().await?;
    let names: Vec<String> = sqlx::query_scalar(table_names_sql())
        .fetch_all(&mut *tx)
        .await?;
    let mut tables = BTreeMap::new();
    for name in names {
        let columns = table_columns(&mut tx, &name).await?;
        let list = columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("SELECT {list} FROM {} ORDER BY rowid", quote_ident(&name));
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(&mut *tx)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in &rows {
            debug_assert_eq!(row.columns().len(), columns.len());
            out.push(
                (0..columns.len())
                    .map(|i| cell(row, i))
                    .collect::<Result<_, _>>()?,
            );
        }
        tables.insert(name, TableDump { columns, rows: out });
    }
    tx.commit().await?;
    Ok(Contents {
        schema_version,
        tables,
    })
}

fn random_nonce() -> Result<[u8; NONCE_BYTES], BackupError> {
    let mut n = [0u8; NONCE_BYTES];
    getrandom::fill(&mut n).map_err(|_| BackupError::Vault)?;
    Ok(n)
}

fn cipher_for(dek: &Dek) -> Result<XChaCha20Poly1305, BackupError> {
    XChaCha20Poly1305::new_from_slice(dek.expose()).map_err(|_| BackupError::Vault)
}

/// A fixed tag that identifies the key without revealing it: the Poly1305 tag of
/// an empty message under the key, a fixed all-zero nonce and a fixed label. The
/// fixed nonce is only ever used for this one empty message.
fn key_check(dek: &Dek) -> Result<String, BackupError> {
    let tag = cipher_for(dek)?
        .encrypt(
            &XNonce::from([0u8; NONCE_BYTES]),
            Payload {
                msg: b"",
                aad: b"shiftbook-backup-key-check-v1",
            },
        )
        .map_err(|_| BackupError::Vault)?;
    Ok(hex::encode(&tag))
}

/// Make a backup of all data as the bytes of a `.shiftbak` file.
pub async fn create_backup(
    store: &SqliteStore,
    vault_dir: &VaultDir,
    dek: &Dek,
    app_version: &str,
    now: OffsetDateTime,
) -> Result<Vec<u8>, BackupError> {
    let contents = dump(store).await?;
    let vault_bytes = vault_dir
        .vault_for_backup()
        .map_err(|_| BackupError::Vault)?;
    let vault: Value = serde_json::from_slice(&vault_bytes).map_err(|_| BackupError::Vault)?;
    let nonce = random_nonce()?;
    let header = Header {
        app_version: app_version.to_string(),
        created_at: shiftbook_core::dates::format_timestamp(now),
        schema_version: contents.schema_version,
        cipher: CIPHER.to_string(),
        compression: COMPRESSION.to_string(),
        nonce: hex::encode(&nonce),
        key_check: key_check(dek)?,
        vault,
    };
    let header_json = serde_json::to_vec(&header).map_err(|_| BackupError::Vault)?;
    let header_len = u32::try_from(header_json.len())
        .ok()
        .filter(|n| *n <= MAX_HEADER_BYTES)
        .ok_or(BackupError::Vault)?;

    let plain = Zeroizing::new(serde_json::to_vec(&contents).map_err(|_| BackupError::Vault)?);
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&plain)?;
    let packed = Zeroizing::new(gz.finish()?);

    let mut out = Vec::with_capacity(PREFIX_BYTES + header_json.len() + packed.len() + 16);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&header_len.to_be_bytes());
    out.extend_from_slice(&header_json);
    let sealed = cipher_for(dek)?
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &packed,
                aad: &out,
            },
        )
        .map_err(|_| BackupError::Vault)?;
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Split a file into (AAD, header, ciphertext) after checking its shape.
fn parse_frame(bytes: &[u8]) -> Result<(&[u8], Header, &[u8]), BackupError> {
    if bytes.len() < PREFIX_BYTES || &bytes[..4] != MAGIC {
        return Err(BackupError::Invalid);
    }
    let version = u16::from_be_bytes([bytes[4], bytes[5]]);
    if version > FORMAT_VERSION {
        return Err(BackupError::TooNew);
    }
    if version != FORMAT_VERSION {
        return Err(BackupError::Invalid);
    }
    let len = u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    if len > MAX_HEADER_BYTES {
        return Err(BackupError::Invalid);
    }
    let end = PREFIX_BYTES + len as usize;
    if bytes.len() <= end {
        return Err(BackupError::Invalid);
    }
    let header: Header =
        serde_json::from_slice(&bytes[PREFIX_BYTES..end]).map_err(|_| BackupError::Invalid)?;
    if header.cipher != CIPHER || header.compression != COMPRESSION {
        return Err(BackupError::Invalid);
    }
    Ok((&bytes[..end], header, &bytes[end..]))
}

/// Check and decrypt a backup. Slow when a secret is given (Argon2): run it off
/// the async runtime. Nothing is changed.
pub fn open_backup(
    bytes: &[u8],
    key: BackupKey<'_>,
    vault_dir: &VaultDir,
    current_schema: i64,
) -> Result<OpenedBackup, BackupError> {
    let (aad, header, sealed) = parse_frame(bytes)?;
    let nonce: [u8; NONCE_BYTES] = hex::decode_array(&header.nonce).ok_or(BackupError::Invalid)?;
    let derived;
    let dek = match key {
        BackupKey::Current(dek) => dek,
        BackupKey::Secret(secret, kind) => {
            let vault = serde_json::to_vec(&header.vault).map_err(|_| BackupError::Invalid)?;
            derived = vault_dir
                .unlock_copied_vault(&vault, secret, kind)
                .map_err(|e| match e {
                    VaultError::UnlockFailed => BackupError::UnlockFailed,
                    _ => BackupError::Vault,
                })?;
            &derived
        }
    };
    let same_key = key_check(dek)? == header.key_check;
    if !same_key {
        return Err(match key {
            BackupKey::Current(_) => BackupError::NeedsSecret,
            // The secret opened the copied vault, so its key is the backup's key.
            BackupKey::Secret(..) => BackupError::Invalid,
        });
    }
    let packed = Zeroizing::new(
        cipher_for(dek)?
            .decrypt(&XNonce::from(nonce), Payload { msg: sealed, aad })
            .map_err(|_| BackupError::Invalid)?,
    );
    // Authenticated from here on: anything wrong is a bug or a newer format.
    if header.schema_version > current_schema {
        return Err(BackupError::TooNew);
    }
    let mut plain = Zeroizing::new(Vec::new());
    GzDecoder::new(packed.as_slice())
        .take(MAX_PAYLOAD_BYTES)
        .read_to_end(&mut plain)
        .map_err(|_| BackupError::Invalid)?;
    let contents: Contents = serde_json::from_slice(&plain).map_err(|_| BackupError::Invalid)?;
    if contents.schema_version != header.schema_version {
        return Err(BackupError::Invalid);
    }
    for table in contents.tables.values() {
        if table.rows.iter().any(|r| r.len() != table.columns.len()) {
            return Err(BackupError::Invalid);
        }
    }
    Ok(OpenedBackup {
        created_at: header.created_at,
        contents,
    })
}

impl OpenedBackup {
    fn live_rows(&self, table: &str) -> u64 {
        let Some(t) = self.contents.tables.get(table) else {
            return 0;
        };
        let deleted = t.columns.iter().position(|c| c == "deleted_at");
        t.rows
            .iter()
            .filter(|r| deleted.is_none_or(|i| r[i].is_null()))
            .count() as u64
    }

    /// What the backup holds.
    pub fn summary(&self) -> BackupSummary {
        BackupSummary {
            created_at: self.created_at.clone(),
            entries: self.live_rows("time_entries"),
            tickets: self.live_rows("tickets"),
            projects: self.live_rows("projects"),
        }
    }
}

/// The schema version of the open database (for [`open_backup`]).
pub async fn current_schema_version(store: &SqliteStore) -> Result<i64, BackupError> {
    schema_version(store).await
}

/// Replace all data with the backup's, in one transaction. Tables and columns
/// added since the backup was made keep their defaults; a table or column the
/// backup has but this version doesn't is refused.
pub async fn restore_backup(store: &SqliteStore, backup: &OpenedBackup) -> Result<(), BackupError> {
    let mut tx = store.pool().begin().await?;
    // Rows go in table by table; check references once, at commit.
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *tx)
        .await?;
    let names: Vec<String> = sqlx::query_scalar(table_names_sql())
        .fetch_all(&mut *tx)
        .await?;
    let mut current = BTreeMap::new();
    for name in &names {
        current.insert(name.clone(), table_columns(&mut tx, name).await?);
    }
    for (name, table) in &backup.contents.tables {
        let cols = current.get(name).ok_or(BackupError::TooNew)?;
        if table.columns.iter().any(|c| !cols.contains(c)) {
            return Err(BackupError::TooNew);
        }
    }
    for name in &names {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM {}",
            quote_ident(name)
        )))
        .execute(&mut *tx)
        .await?;
    }
    for (name, table) in &backup.contents.tables {
        if table.rows.is_empty() {
            continue;
        }
        let cols = table
            .columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let params = (1..=table.columns.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT INTO {} ({cols}) VALUES ({params})",
            quote_ident(name)
        );
        for row in &table.rows {
            let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
            for v in row {
                q = match v {
                    Value::Null => q.bind(None::<String>),
                    Value::String(s) => q.bind(s.as_str()),
                    Value::Number(n) => match (n.as_i64(), n.as_f64()) {
                        (Some(i), _) => q.bind(i),
                        (None, Some(f)) => q.bind(f),
                        _ => return Err(BackupError::Invalid),
                    },
                    _ => return Err(BackupError::Invalid),
                };
            }
            q.execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

// ---------- the backups folder ----------

/// What a file in the backups folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    /// Made automatically once a day.
    Automatic,
    /// The copy of the data made just before a restore.
    BeforeRestore,
}

/// A file in the backups folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    /// File name (also how the UI refers to it).
    pub name: String,
    /// Automatic or safety copy.
    pub kind: BackupKind,
    /// When it was made (UTC, RFC 3339), from the file name.
    pub created_at: String,
    /// Size in bytes.
    pub bytes: u64,
}

/// The backups folder inside the data folder.
pub fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(BACKUP_DIR_NAME)
}

fn parse_name(name: &str) -> Option<(BackupKind, OffsetDateTime)> {
    let stem = name.strip_suffix(&format!(".{BACKUP_EXTENSION}"))?;
    let (kind, stamp) = match stem.strip_prefix(AUTO_PREFIX) {
        Some(rest) => (BackupKind::Automatic, rest),
        None => (
            BackupKind::BeforeRestore,
            stem.strip_prefix(BEFORE_RESTORE_PREFIX)?,
        ),
    };
    let at = PrimitiveDateTime::parse(stamp, STAMP).ok()?.assume_utc();
    Some((kind, at))
}

fn file_name(kind: BackupKind, at: OffsetDateTime) -> String {
    let prefix = match kind {
        BackupKind::Automatic => AUTO_PREFIX,
        BackupKind::BeforeRestore => BEFORE_RESTORE_PREFIX,
    };
    let stamp = at.format(STAMP).unwrap_or_default();
    format!("{prefix}{stamp}.{BACKUP_EXTENSION}")
}

/// The backups in `dir`, newest first. A missing folder is an empty list.
pub fn list_backups(dir: &Path) -> io::Result<Vec<BackupFile>> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some((kind, at)) = parse_name(&name) else {
            continue;
        };
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        out.push((
            at,
            BackupFile {
                name,
                kind,
                created_at: shiftbook_core::dates::format_timestamp(at),
                bytes: meta.len(),
            },
        ));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.name.cmp(&a.1.name)));
    Ok(out.into_iter().map(|(_, f)| f).collect())
}

/// The path of a backup in `dir` by the name [`list_backups`] gave, or `None`
/// for anything that isn't one of its plain backup names.
pub fn backup_path(dir: &Path, name: &str) -> Option<PathBuf> {
    parse_name(name)?;
    if name.contains(['/', '\\']) || name.starts_with('.') {
        return None;
    }
    Some(dir.join(name))
}

/// True when an automatic backup is due: none yet, or the newest is a day old.
pub fn automatic_due(dir: &Path, now: OffsetDateTime) -> io::Result<bool> {
    let newest = list_backups(dir)?
        .into_iter()
        .filter(|f| f.kind == BackupKind::Automatic)
        .find_map(|f| parse_name(&f.name).map(|(_, at)| at));
    Ok(newest.is_none_or(|at| now - at >= AUTOMATIC_EVERY))
}

/// Save backup bytes in the backups folder (private, written atomically) and,
/// for automatic backups, delete all but the newest [`KEEP_AUTOMATIC`].
pub fn save_in_folder(
    dir: &Path,
    kind: BackupKind,
    at: OffsetDateTime,
    bytes: &[u8],
) -> io::Result<String> {
    fsutil::create_private_dir(dir)?;
    let name = file_name(kind, at);
    fsutil::write_atomic(&dir.join(&name), bytes)?;
    if kind == BackupKind::Automatic {
        for old in list_backups(dir)?
            .into_iter()
            .filter(|f| f.kind == BackupKind::Automatic)
            .skip(KEEP_AUTOMATIC)
        {
            fs::remove_file(dir.join(old.name))?;
        }
    }
    Ok(name)
}

/// Save backup bytes at a path the user chose (private, written atomically).
pub fn save_at(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fsutil::write_atomic(path, bytes)
}

/// Read a backup file, refusing anything too large to be one.
pub fn read_backup_file(path: &Path) -> Result<Vec<u8>, BackupError> {
    let meta = fs::metadata(path)?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(BackupError::Invalid);
    }
    Ok(fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{database_path, open_encrypted};
    use crate::fsutil::test_dir::TestDir;
    use crate::repo::SettingsRepository;
    use crate::test_rt::block_on;
    use time::macros::datetime;

    const PW: &str = "first password here";

    struct Setup {
        _dir: TestDir,
        vault_dir: VaultDir,
        store: SqliteStore,
        vault: crate::Vault,
        key: String,
    }

    async fn setup(label: &str) -> Setup {
        let dir = TestDir::new(label);
        let vault_dir = VaultDir::new_for_tests(dir.path());
        let (vault, key) = vault_dir.create(&SecretString::from(PW)).unwrap();
        let pool = open_encrypted(&database_path(dir.path()), vault.dek())
            .await
            .unwrap();
        let store = SqliteStore::new(pool);
        Setup {
            _dir: dir,
            vault_dir,
            store,
            vault,
            key: secrecy::ExposeSecret::expose_secret(&key.display()).to_string(),
        }
    }

    async fn marker(store: &SqliteStore) -> Option<String> {
        let user = store.initialize_local_owner("Me").await.unwrap();
        store.get_setting(user.id, "marker").await.unwrap()
    }

    async fn set_marker(store: &SqliteStore, value: &str) {
        let user = store.initialize_local_owner("Me").await.unwrap();
        store.set_setting(user.id, "marker", value).await.unwrap();
    }

    #[test]
    fn round_trip_on_the_same_computer() {
        block_on(async {
            let s = setup("bak-same").await;
            set_marker(&s.store, "before").await;
            let bytes = create_backup(
                &s.store,
                &s.vault_dir,
                s.vault.dek(),
                "0.1.0",
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
            assert_eq!(&bytes[..4], b"SBAK");
            set_marker(&s.store, "after").await;
            let schema = current_schema_version(&s.store).await.unwrap();
            let opened = open_backup(
                &bytes,
                BackupKey::Current(s.vault.dek()),
                &s.vault_dir,
                schema,
            )
            .unwrap();
            restore_backup(&s.store, &opened).await.unwrap();
            assert_eq!(marker(&s.store).await.as_deref(), Some("before"));
        });
    }

    #[test]
    fn opens_elsewhere_with_password_or_recovery_key_only() {
        block_on(async {
            let a = setup("bak-a").await;
            set_marker(&a.store, "from a").await;
            let bytes = create_backup(
                &a.store,
                &a.vault_dir,
                a.vault.dek(),
                "0.1.0",
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
            let b = setup("bak-b").await;
            set_marker(&b.store, "from b").await;
            let schema = current_schema_version(&b.store).await.unwrap();
            assert!(matches!(
                open_backup(
                    &bytes,
                    BackupKey::Current(b.vault.dek()),
                    &b.vault_dir,
                    schema
                ),
                Err(BackupError::NeedsSecret)
            ));
            let wrong = SecretString::from("not the password at all");
            assert!(matches!(
                open_backup(
                    &bytes,
                    BackupKey::Secret(&wrong, SecretKind::Password),
                    &b.vault_dir,
                    schema
                ),
                Err(BackupError::UnlockFailed)
            ));
            let key = SecretString::from(a.key.as_str());
            assert!(open_backup(
                &bytes,
                BackupKey::Secret(&key, SecretKind::RecoveryKey),
                &b.vault_dir,
                schema
            )
            .is_ok());
            let pw = SecretString::from(PW);
            let opened = open_backup(
                &bytes,
                BackupKey::Secret(&pw, SecretKind::Password),
                &b.vault_dir,
                schema,
            )
            .unwrap();
            assert_eq!(opened.summary().entries, 0);
            restore_backup(&b.store, &opened).await.unwrap();
            assert_eq!(marker(&b.store).await.as_deref(), Some("from a"));
        });
    }

    #[test]
    fn entries_tickets_and_projects_come_back() {
        use crate::work::EntryInput;
        use shiftbook_core::minutes::Minutes;
        use time::macros::date;
        block_on(async {
            let a = setup("bak-entries-a").await;
            let me = a.store.initialize_local_owner("Me").await.unwrap().id;
            let p = a.store.create_project(me, "Network", None).await.unwrap();
            let entry = EntryInput {
                date: date!(2026 - 10 - 15),
                minutes: Minutes(90),
                description: "Firewall cleanup".into(),
                project: None,
                category: None,
                next_step: None,
                timesheet_code_id: None,
            };
            a.store
                .create_entry(me, &entry, "CHG1, INC2")
                .await
                .unwrap();
            let bytes = create_backup(
                &a.store,
                &a.vault_dir,
                a.vault.dek(),
                "0.1.0",
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();

            let b = setup("bak-entries-b").await;
            b.store.initialize_local_owner("Me").await.unwrap();
            let schema = current_schema_version(&b.store).await.unwrap();
            let pw = SecretString::from(PW);
            let opened = open_backup(
                &bytes,
                BackupKey::Secret(&pw, SecretKind::Password),
                &b.vault_dir,
                schema,
            )
            .unwrap();
            let summary = opened.summary();
            assert_eq!(
                (summary.entries, summary.tickets, summary.projects),
                (1, 2, 1)
            );
            restore_backup(&b.store, &opened).await.unwrap();
            let owner = b.store.initialize_local_owner("Me").await.unwrap().id;
            assert_eq!(owner, me, "the backup's owner replaces the new one");
            let entries = b
                .store
                .list_entries(owner, date!(2026 - 10 - 01), date!(2026 - 10 - 31))
                .await
                .unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].tickets.len(), 2);
            assert_eq!(b.store.list_projects(owner).await.unwrap()[0].id, p.id);
        });
    }

    /// A backup written by an earlier version, checked in so a change that stops old
    /// backups restoring fails here. Made with tiny test Argon2 parameters and the test
    /// password; it holds only the made-up records from [`write_fixture_data`].
    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/schema-14.shiftbak");

    async fn write_fixture_data(store: &SqliteStore) {
        use crate::work::EntryInput;
        use shiftbook_core::minutes::Minutes;
        use time::macros::date;
        let me = store.initialize_local_owner("Me").await.unwrap().id;
        let shed = store.create_project(me, "Garden shed", None).await.unwrap();
        let entry = EntryInput {
            date: date!(2026 - 10 - 20),
            minutes: Minutes(45),
            description: "Fixed the printer".into(),
            project: Some(shed.id),
            category: None,
            next_step: Some("Order more toner".into()),
            timesheet_code_id: None,
        };
        store.create_entry(me, &entry, "INC0001234").await.unwrap();
        store
            .tray_add(me, date!(2026 - 10 - 20), "RITM0005678")
            .await
            .unwrap();
        set_marker(store, "fixture").await;
    }

    /// Writes the fixture again (after a deliberate format change):
    /// `cargo test -p shiftbook-storage write_backup_fixture -- --ignored`.
    #[test]
    #[ignore = "writes tests/fixtures; run by hand"]
    fn write_backup_fixture() {
        block_on(async {
            let s = setup("bak-fixture-write").await;
            write_fixture_data(&s.store).await;
            let created = OffsetDateTime::parse(
                "2026-10-25T12:00:00Z",
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap();
            let bytes = create_backup(&s.store, &s.vault_dir, s.vault.dek(), "0.4.0", created)
                .await
                .unwrap();
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/schema-14.shiftbak");
            std::fs::write(path, bytes).unwrap();
        });
    }

    #[test]
    fn a_backup_from_an_earlier_version_still_restores() {
        use time::macros::date;
        block_on(async {
            let s = setup("bak-fixture").await;
            s.store.initialize_local_owner("Me").await.unwrap();
            let schema = current_schema_version(&s.store).await.unwrap();
            let pw = SecretString::from(PW);
            let opened = open_backup(
                FIXTURE,
                BackupKey::Secret(&pw, SecretKind::Password),
                &s.vault_dir,
                schema,
            )
            .unwrap();
            let summary = opened.summary();
            assert_eq!(
                (summary.entries, summary.tickets, summary.projects),
                (2, 2, 1),
                "the entry and the tray's roll-up entry, a ticket on each"
            );
            restore_backup(&s.store, &opened).await.unwrap();
            let owner = s.store.initialize_local_owner("Me").await.unwrap().id;
            assert_eq!(marker(&s.store).await.as_deref(), Some("fixture"));
            let entries = s
                .store
                .list_entries(owner, date!(2026 - 10 - 20), date!(2026 - 10 - 20))
                .await
                .unwrap();
            let printer = entries
                .iter()
                .find(|e| e.entry.description == "Fixed the printer")
                .expect("the entry came back");
            assert_eq!(printer.tickets[0].ticket_number, "INC0001234");
            let tray = s
                .store
                .tray_day(owner, date!(2026 - 10 - 20))
                .await
                .unwrap();
            assert_eq!(tray.items[0].ticket_number, "RITM0005678");
            assert_eq!(
                s.store.list_projects(owner).await.unwrap()[0].name,
                "Garden shed"
            );
        });
    }

    #[test]
    fn any_edited_byte_is_refused() {
        block_on(async {
            let s = setup("bak-tamper").await;
            set_marker(&s.store, "x").await;
            let bytes = create_backup(
                &s.store,
                &s.vault_dir,
                s.vault.dek(),
                "0.1.0",
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
            let schema = current_schema_version(&s.store).await.unwrap();
            let dek = s.vault.dek();
            // The header, the format version and the ciphertext are all covered.
            for at in [5, 20, bytes.len() / 2, bytes.len() - 1] {
                let mut edited = bytes.clone();
                edited[at] ^= 0x01;
                assert!(
                    open_backup(&edited, BackupKey::Current(dek), &s.vault_dir, schema).is_err(),
                    "byte {at}"
                );
            }
            assert!(matches!(
                open_backup(
                    b"not a backup",
                    BackupKey::Current(dek),
                    &s.vault_dir,
                    schema
                ),
                Err(BackupError::Invalid)
            ));
            assert!(matches!(
                open_backup(
                    &bytes[..bytes.len() - 10],
                    BackupKey::Current(dek),
                    &s.vault_dir,
                    schema
                ),
                Err(BackupError::Invalid)
            ));
            // A newer schema is refused after the file checks out.
            assert!(matches!(
                open_backup(&bytes, BackupKey::Current(dek), &s.vault_dir, schema - 1),
                Err(BackupError::TooNew)
            ));
        });
    }

    #[test]
    fn data_is_not_readable_in_the_file() {
        block_on(async {
            let s = setup("bak-secret").await;
            set_marker(&s.store, "very-recognisable-marker").await;
            let bytes = create_backup(
                &s.store,
                &s.vault_dir,
                s.vault.dek(),
                "0.1.0",
                OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
            let text = String::from_utf8_lossy(&bytes);
            assert!(!text.contains("very-recognisable-marker"));
            assert!(!text.contains("marker"));
        });
    }

    #[test]
    fn folder_keeps_fourteen_automatic_backups() {
        let dir = TestDir::new("bak-folder");
        let folder = backups_dir(dir.path());
        let start = datetime!(2026-10-01 08:00 UTC);
        assert!(automatic_due(&folder, start).unwrap());
        for day in 0..20 {
            save_in_folder(
                &folder,
                BackupKind::Automatic,
                start + Duration::days(day),
                b"x",
            )
            .unwrap();
        }
        save_in_folder(&folder, BackupKind::BeforeRestore, start, b"y").unwrap();
        let list = list_backups(&folder).unwrap();
        let autos: Vec<_> = list
            .iter()
            .filter(|f| f.kind == BackupKind::Automatic)
            .collect();
        assert_eq!(autos.len(), KEEP_AUTOMATIC);
        assert_eq!(autos[0].name, "auto-20261020-080000.shiftbak");
        assert_eq!(autos[0].created_at, "2026-10-20T08:00:00Z");
        assert!(list.iter().any(|f| f.kind == BackupKind::BeforeRestore));
        let newest = start + Duration::days(19);
        assert!(!automatic_due(&folder, newest + Duration::hours(23)).unwrap());
        assert!(automatic_due(&folder, newest + Duration::hours(24)).unwrap());
    }

    #[test]
    fn only_plain_backup_names_resolve() {
        let dir = Path::new("/data/backups");
        assert!(backup_path(dir, "auto-20261020-080000.shiftbak").is_some());
        assert!(backup_path(dir, "before-restore-20261020-080000.shiftbak").is_some());
        for bad in [
            "../vault.json",
            "auto-20261020-080000.shiftbak/../../x",
            "auto-x.shiftbak",
            "vault.json",
            "",
        ] {
            assert!(backup_path(dir, bad).is_none(), "{bad}");
        }
    }
}
