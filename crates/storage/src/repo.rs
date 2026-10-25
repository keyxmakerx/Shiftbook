//! Repository traits and their SQLite implementation ([`SqliteStore`]).
//!
//! M1 covers only what setup and unlock need: the single local user, settings,
//! dropdown list options and frequencies. Every method takes the [`OwnerId`]
//! whose rows it may touch, so Phase 2 can pass the signed-in user instead.
//!
//! SQL uses `$n` placeholders and portable syntax so the same statements can
//! run on Postgres later. Values are always bound, never formatted into SQL.

use std::future::Future;

use shiftbook_core::dates::format_timestamp;
use shiftbook_core::ids::{FrequencyId, ListOptionId, OwnerId};
use shiftbook_core::model::{is_blocked, is_done, Frequency, ListName, ListOption, User};
use shiftbook_core::seed::{default_frequencies, default_list_options};
use shiftbook_core::validate::{
    validate_color, validate_frequency, validate_list_value, validate_required,
    MAX_LIST_VALUE_CHARS, MAX_NOTES_CHARS,
};
use shiftbook_core::CoreError;
use sqlx::sqlite::{SqliteConnection, SqlitePool, SqliteRow};
use sqlx::Row;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::StorageError;
use crate::items::link_items;

pub(crate) type Result<T> = std::result::Result<T, StorageError>;

/// Settings key recording which version of the default lists was seeded.
pub const SEED_VERSION_KEY: &str = "seed.version";
/// The current default-lists version.
pub const SEED_VERSION: &str = "1";
/// Longest settings key.
pub const MAX_SETTING_KEY_CHARS: usize = 100;

/// The people who own data. Phase 1 has exactly one local user.
pub trait UserRepository {
    /// The local user, if setup has created one.
    fn local_user(&self) -> impl Future<Output = Result<Option<User>>> + Send;
}

/// Per-owner key/value settings.
pub trait SettingsRepository {
    /// One setting, or `None` when unset.
    fn get_setting(
        &self,
        owner: OwnerId,
        key: &str,
    ) -> impl Future<Output = Result<Option<String>>> + Send;
    /// Create or replace a setting.
    fn set_setting(
        &self,
        owner: OwnerId,
        key: &str,
        value: &str,
    ) -> impl Future<Output = Result<()>> + Send;
    /// All live settings, sorted by key.
    fn list_settings(
        &self,
        owner: OwnerId,
    ) -> impl Future<Output = Result<Vec<(String, String)>>> + Send;
}

/// The user-editable dropdown lists (category, status, priority, recurring type).
pub trait ListOptionRepository {
    /// Options of one list in display order; hidden ones only when asked.
    fn list_options(
        &self,
        owner: OwnerId,
        list: ListName,
        include_hidden: bool,
    ) -> impl Future<Output = Result<Vec<ListOption>>> + Send;
    /// Add an option at the end of a list.
    fn add_list_option(
        &self,
        owner: OwnerId,
        list: ListName,
        value: &str,
        color: Option<&str>,
    ) -> impl Future<Output = Result<ListOption>> + Send;
    /// Rename an option and every record that uses the old text, in one transaction.
    /// "Done" and "Blocked" statuses are refused with [`StorageError::Protected`].
    fn rename_list_option(
        &self,
        owner: OwnerId,
        id: ListOptionId,
        new_value: &str,
    ) -> impl Future<Output = Result<ListOption>> + Send;
    /// Hide or show an option. Hiding "Done" or "Blocked" is refused.
    fn set_list_option_hidden(
        &self,
        owner: OwnerId,
        id: ListOptionId,
        hidden: bool,
    ) -> impl Future<Output = Result<()>> + Send;
    /// Set or clear an option's colour.
    fn set_list_option_color(
        &self,
        owner: OwnerId,
        id: ListOptionId,
        color: Option<&str>,
    ) -> impl Future<Output = Result<()>> + Send;
    /// Put the given options of one list in this order.
    fn reorder_list_options(
        &self,
        owner: OwnerId,
        list: ListName,
        ids: &[ListOptionId],
    ) -> impl Future<Output = Result<()>> + Send;
}

/// Repeat frequencies (label + months + days).
pub trait FrequencyRepository {
    /// Live frequencies in display order.
    fn list_frequencies(
        &self,
        owner: OwnerId,
    ) -> impl Future<Output = Result<Vec<Frequency>>> + Send;
    /// Add a frequency at the end.
    fn add_frequency(
        &self,
        owner: OwnerId,
        label: &str,
        months: u32,
        days: u32,
    ) -> impl Future<Output = Result<Frequency>> + Send;
    /// Change a frequency's label or interval.
    fn update_frequency(
        &self,
        owner: OwnerId,
        id: FrequencyId,
        label: &str,
        months: u32,
        days: u32,
    ) -> impl Future<Output = Result<Frequency>> + Send;
    /// Hide (soft-delete) a frequency; records that use it keep it.
    fn hide_frequency(
        &self,
        owner: OwnerId,
        id: FrequencyId,
    ) -> impl Future<Output = Result<()>> + Send;
}

/// The SQLite (SQLCipher) implementation of every repository.
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pub(crate) pool: SqlitePool,
}

pub(crate) fn now_text() -> String {
    format_timestamp(OffsetDateTime::now_utc())
}

pub(crate) fn parse_ts(text: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|_| StorageError::Corrupt("timestamp"))
}

pub(crate) fn parse_opt_ts(text: Option<String>) -> Result<Option<OffsetDateTime>> {
    text.as_deref().map(parse_ts).transpose()
}

pub(crate) fn parse_uuid(text: &str) -> Result<Uuid> {
    Uuid::parse_str(text).map_err(|_| StorageError::Corrupt("id"))
}

fn parse_list_name(text: &str) -> Result<ListName> {
    match text {
        "category" => Ok(ListName::Category),
        "status" => Ok(ListName::Status),
        "priority" => Ok(ListName::Priority),
        "recurring_type" => Ok(ListName::RecurringType),
        _ => Err(StorageError::Corrupt("list name")),
    }
}

pub(crate) fn u32_from(value: i64, what: &'static str) -> Result<u32> {
    u32::try_from(value).map_err(|_| StorageError::Corrupt(what))
}

pub(crate) fn i32_from(value: i64) -> Result<i32> {
    i32::try_from(value).map_err(|_| StorageError::Corrupt("sort order"))
}

fn user_from_row(row: &SqliteRow) -> Result<User> {
    Ok(User {
        id: OwnerId::from_uuid(parse_uuid(row.try_get("id")?)?),
        display_name: row.try_get("display_name")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

/// The `list_options` columns, as a literal so queries stay `&'static str`.
macro_rules! list_option_columns {
    () => {
        "id, owner_id, list_name, value, sort_order, color, hidden, created_at, updated_at, deleted_at"
    };
}

fn list_option_from_row(row: &SqliteRow) -> Result<ListOption> {
    Ok(ListOption {
        id: ListOptionId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        list_name: parse_list_name(row.try_get("list_name")?)?,
        value: row.try_get("value")?,
        sort_order: i32_from(row.try_get("sort_order")?)?,
        color: row.try_get("color")?,
        hidden: row.try_get("hidden")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn frequency_from_row(row: &SqliteRow) -> Result<Frequency> {
    Ok(Frequency {
        id: FrequencyId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        label: row.try_get("label")?,
        months: u32_from(row.try_get("months")?, "months")?,
        days: u32_from(row.try_get("days")?, "days")?,
        sort_order: i32_from(row.try_get("sort_order")?)?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

/// "Done" and "Blocked" statuses: matched like the core rules (ignoring case and
/// surrounding spaces), so a stored " done " is protected too.
fn is_protected(list: ListName, value: &str) -> bool {
    list == ListName::Status && (is_done(Some(value)) || is_blocked(Some(value)))
}

/// Statements that carry a list rename into the records that store the text.
/// Each takes `$1` new value, `$2` updated_at, `$3` owner, `$4` old value.
fn rename_statements(list: ListName) -> &'static [&'static str] {
    match list {
        ListName::Category => &[
            "UPDATE time_entries SET category = $1, updated_at = $2 \
             WHERE owner_id = $3 AND category = $4",
            "UPDATE category_code_defaults SET category = $1, updated_at = $2 \
             WHERE owner_id = $3 AND category = $4",
        ],
        ListName::Status => &[
            "UPDATE projects SET status = $1, updated_at = $2 WHERE owner_id = $3 AND status = $4",
            "UPDATE time_entries SET status = $1, updated_at = $2 \
             WHERE owner_id = $3 AND status = $4",
            "UPDATE tasks SET status = $1, updated_at = $2 WHERE owner_id = $3 AND status = $4",
        ],
        ListName::Priority => &[
            "UPDATE projects SET priority = $1, updated_at = $2 \
             WHERE owner_id = $3 AND priority = $4",
            "UPDATE time_entries SET priority = $1, updated_at = $2 \
             WHERE owner_id = $3 AND priority = $4",
            "UPDATE tasks SET priority = $1, updated_at = $2 \
             WHERE owner_id = $3 AND priority = $4",
        ],
        ListName::RecurringType => &["UPDATE recurring_tasks SET type = $1, updated_at = $2 \
             WHERE owner_id = $3 AND type = $4"],
    }
}

async fn fetch_list_option(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: ListOptionId,
) -> Result<ListOption> {
    let row = sqlx::query(concat!(
        "SELECT ",
        list_option_columns!(),
        " FROM list_options WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    list_option_from_row(&row)
}

/// Refuse `value` if another live option in the list has it (ignoring case).
async fn ensure_unique_option(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    list: ListName,
    value: &str,
    except: Option<ListOptionId>,
) -> Result<()> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, value FROM list_options \
         WHERE owner_id = $1 AND list_name = $2 AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .bind(list.as_str())
    .fetch_all(&mut *conn)
    .await?;
    let wanted = value.to_lowercase();
    let except = except.map(|id| id.to_string());
    if rows
        .iter()
        .any(|(id, v)| Some(id) != except.as_ref() && v.trim().to_lowercase() == wanted)
    {
        return Err(StorageError::Duplicate(value.to_string()));
    }
    Ok(())
}

async fn insert_list_option(conn: &mut SqliteConnection, o: &ListOption) -> Result<()> {
    sqlx::query(
        "INSERT INTO list_options \
         (id, owner_id, list_name, value, sort_order, color, hidden, created_at, updated_at, deleted_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(o.id.to_string())
    .bind(o.owner_id.to_string())
    .bind(o.list_name.as_str())
    .bind(&o.value)
    .bind(i64::from(o.sort_order))
    .bind(o.color.as_deref())
    .bind(o.hidden)
    .bind(format_timestamp(o.created_at))
    .bind(format_timestamp(o.updated_at))
    .bind(o.deleted_at.map(format_timestamp))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_frequency(conn: &mut SqliteConnection, f: &Frequency) -> Result<()> {
    sqlx::query(
        "INSERT INTO frequencies \
         (id, owner_id, label, months, days, sort_order, created_at, updated_at, deleted_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(f.id.to_string())
    .bind(f.owner_id.to_string())
    .bind(&f.label)
    .bind(i64::from(f.months))
    .bind(i64::from(f.days))
    .bind(i64::from(f.sort_order))
    .bind(format_timestamp(f.created_at))
    .bind(format_timestamp(f.updated_at))
    .bind(f.deleted_at.map(format_timestamp))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn upsert_setting(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    key: &str,
    value: &str,
) -> Result<()> {
    let now = now_text();
    sqlx::query(
        "INSERT INTO settings (id, owner_id, key, value, created_at, updated_at, deleted_at) \
         VALUES ($1, $2, $3, $4, $5, $5, NULL) \
         ON CONFLICT (owner_id, key) DO UPDATE \
         SET value = excluded.value, updated_at = excluded.updated_at, deleted_at = NULL",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(owner.to_string())
    .bind(key)
    .bind(value)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

fn check_setting(key: &str, value: &str) -> Result<()> {
    validate_required("Setting name", key, MAX_SETTING_KEY_CHARS)?;
    if key != key.trim() {
        return Err(CoreError::Required {
            field: "Setting name",
        }
        .into());
    }
    if value.chars().count() > MAX_NOTES_CHARS {
        return Err(CoreError::TooLong {
            field: "Setting",
            max: MAX_NOTES_CHARS,
        }
        .into());
    }
    Ok(())
}

impl SqliteStore {
    /// Wrap an open pool (see [`crate::db::open_encrypted`]).
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// The underlying pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// First-run (and every-unlock) initialisation, in one transaction: create the
    /// local user if there is none, then seed the default lists and frequencies
    /// once. Safe to call again; nothing is duplicated.
    pub async fn initialize_local_owner(&self, display_name: &str) -> Result<User> {
        let display_name = validate_required("Name", display_name, MAX_LIST_VALUE_CHARS)?;
        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query(
            "SELECT id, display_name, created_at, updated_at, deleted_at FROM users \
             WHERE deleted_at IS NULL ORDER BY created_at, id LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let user = match existing {
            Some(row) => user_from_row(&row)?,
            None => {
                let now = OffsetDateTime::now_utc();
                let user = User {
                    id: OwnerId::new(),
                    display_name,
                    created_at: now,
                    updated_at: now,
                    deleted_at: None,
                };
                sqlx::query(
                    "INSERT INTO users (id, display_name, created_at, updated_at, deleted_at) \
                     VALUES ($1, $2, $3, $3, NULL)",
                )
                .bind(user.id.to_string())
                .bind(&user.display_name)
                .bind(format_timestamp(now))
                .execute(&mut *tx)
                .await?;
                // Read the stored timestamp back so it matches what later reads return.
                User {
                    created_at: parse_ts(&format_timestamp(now))?,
                    updated_at: parse_ts(&format_timestamp(now))?,
                    ..user
                }
            }
        };
        let seeded: Option<String> = sqlx::query_scalar(
            "SELECT value FROM settings WHERE owner_id = $1 AND key = $2 AND deleted_at IS NULL",
        )
        .bind(user.id.to_string())
        .bind(SEED_VERSION_KEY)
        .fetch_optional(&mut *tx)
        .await?;
        if seeded.is_none() {
            let now = parse_ts(&now_text())?;
            for option in default_list_options(user.id, now) {
                insert_list_option(&mut tx, &option).await?;
            }
            for frequency in default_frequencies(user.id, now) {
                insert_frequency(&mut tx, &frequency).await?;
            }
            upsert_setting(&mut tx, user.id, SEED_VERSION_KEY, SEED_VERSION).await?;
        }
        // Tickets from before items (round 15), or brought back by a restore, join theirs.
        link_items(&mut tx, user.id).await?;
        tx.commit().await?;
        Ok(user)
    }
}

impl UserRepository for SqliteStore {
    async fn local_user(&self) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, display_name, created_at, updated_at, deleted_at FROM users \
             WHERE deleted_at IS NULL ORDER BY created_at, id LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(user_from_row).transpose()
    }
}

impl SettingsRepository for SqliteStore {
    async fn get_setting(&self, owner: OwnerId, key: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar(
            "SELECT value FROM settings WHERE owner_id = $1 AND key = $2 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(key)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn set_setting(&self, owner: OwnerId, key: &str, value: &str) -> Result<()> {
        check_setting(key, value)?;
        let mut conn = self.pool.acquire().await?;
        upsert_setting(&mut conn, owner, key, value).await
    }

    async fn list_settings(&self, owner: OwnerId) -> Result<Vec<(String, String)>> {
        Ok(sqlx::query_as(
            "SELECT key, value FROM settings WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY key",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?)
    }
}

impl ListOptionRepository for SqliteStore {
    async fn list_options(
        &self,
        owner: OwnerId,
        list: ListName,
        include_hidden: bool,
    ) -> Result<Vec<ListOption>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            list_option_columns!(),
            " FROM list_options WHERE owner_id = $1 AND list_name = $2 AND deleted_at IS NULL \
             AND (hidden = FALSE OR $3) ORDER BY sort_order, value"
        ))
        .bind(owner.to_string())
        .bind(list.as_str())
        .bind(include_hidden)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(list_option_from_row).collect()
    }

    async fn add_list_option(
        &self,
        owner: OwnerId,
        list: ListName,
        value: &str,
        color: Option<&str>,
    ) -> Result<ListOption> {
        let value = validate_list_value(value)?;
        let color = color.map(validate_color).transpose()?;
        let mut tx = self.pool.begin().await?;
        ensure_unique_option(&mut tx, owner, list, &value, None).await?;
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM list_options \
             WHERE owner_id = $1 AND list_name = $2",
        )
        .bind(owner.to_string())
        .bind(list.as_str())
        .fetch_one(&mut *tx)
        .await?;
        let now = parse_ts(&now_text())?;
        let option = ListOption {
            id: ListOptionId::new(),
            owner_id: owner,
            list_name: list,
            value,
            sort_order: i32_from(next)?,
            color,
            hidden: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        insert_list_option(&mut tx, &option).await?;
        tx.commit().await?;
        Ok(option)
    }

    async fn rename_list_option(
        &self,
        owner: OwnerId,
        id: ListOptionId,
        new_value: &str,
    ) -> Result<ListOption> {
        let new_value = validate_list_value(new_value)?;
        let mut tx = self.pool.begin().await?;
        let current = fetch_list_option(&mut tx, owner, id).await?;
        if is_protected(current.list_name, &current.value) {
            return Err(StorageError::Protected(current.value));
        }
        if current.value == new_value {
            return Ok(current);
        }
        ensure_unique_option(&mut tx, owner, current.list_name, &new_value, Some(id)).await?;
        let now = now_text();
        sqlx::query(
            "UPDATE list_options SET value = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(&new_value)
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        for statement in rename_statements(current.list_name) {
            sqlx::query(*statement)
                .bind(&new_value)
                .bind(&now)
                .bind(owner.to_string())
                .bind(&current.value)
                .execute(&mut *tx)
                .await?;
        }
        let renamed = fetch_list_option(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(renamed)
    }

    async fn set_list_option_hidden(
        &self,
        owner: OwnerId,
        id: ListOptionId,
        hidden: bool,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let current = fetch_list_option(&mut tx, owner, id).await?;
        if hidden && is_protected(current.list_name, &current.value) {
            return Err(StorageError::Protected(current.value));
        }
        sqlx::query(
            "UPDATE list_options SET hidden = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(hidden)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn set_list_option_color(
        &self,
        owner: OwnerId,
        id: ListOptionId,
        color: Option<&str>,
    ) -> Result<()> {
        let color = color.map(validate_color).transpose()?;
        let done = sqlx::query(
            "UPDATE list_options SET color = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(color)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    async fn reorder_list_options(
        &self,
        owner: OwnerId,
        list: ListName,
        ids: &[ListOptionId],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let now = now_text();
        for (position, id) in ids.iter().enumerate() {
            let position = i64::try_from(position).map_err(|_| StorageError::NotFound)?;
            let done = sqlx::query(
                "UPDATE list_options SET sort_order = $1, updated_at = $2 \
                 WHERE id = $3 AND owner_id = $4 AND list_name = $5 AND deleted_at IS NULL",
            )
            .bind(position)
            .bind(&now)
            .bind(id.to_string())
            .bind(owner.to_string())
            .bind(list.as_str())
            .execute(&mut *tx)
            .await?;
            if done.rows_affected() == 0 {
                return Err(StorageError::NotFound);
            }
        }
        tx.commit().await?;
        Ok(())
    }
}

async fn ensure_unique_frequency(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    label: &str,
    except: Option<FrequencyId>,
) -> Result<()> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, label FROM frequencies WHERE owner_id = $1 AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .fetch_all(&mut *conn)
    .await?;
    let wanted = label.to_lowercase();
    let except = except.map(|id| id.to_string());
    if rows
        .iter()
        .any(|(id, l)| Some(id) != except.as_ref() && l.trim().to_lowercase() == wanted)
    {
        return Err(StorageError::Duplicate(label.to_string()));
    }
    Ok(())
}

async fn fetch_frequency(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: FrequencyId,
) -> Result<Frequency> {
    let row = sqlx::query(
        "SELECT id, owner_id, label, months, days, sort_order, created_at, updated_at, deleted_at \
         FROM frequencies WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    frequency_from_row(&row)
}

impl FrequencyRepository for SqliteStore {
    async fn list_frequencies(&self, owner: OwnerId) -> Result<Vec<Frequency>> {
        let rows = sqlx::query(
            "SELECT id, owner_id, label, months, days, sort_order, created_at, updated_at, \
             deleted_at FROM frequencies WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY sort_order, label",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(frequency_from_row).collect()
    }

    async fn add_frequency(
        &self,
        owner: OwnerId,
        label: &str,
        months: u32,
        days: u32,
    ) -> Result<Frequency> {
        validate_frequency(label, months, days)?;
        let label = label.trim().to_string();
        let mut tx = self.pool.begin().await?;
        ensure_unique_frequency(&mut tx, owner, &label, None).await?;
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM frequencies WHERE owner_id = $1",
        )
        .bind(owner.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let now = parse_ts(&now_text())?;
        let frequency = Frequency {
            id: FrequencyId::new(),
            owner_id: owner,
            label,
            months,
            days,
            sort_order: i32_from(next)?,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        insert_frequency(&mut tx, &frequency).await?;
        tx.commit().await?;
        Ok(frequency)
    }

    async fn update_frequency(
        &self,
        owner: OwnerId,
        id: FrequencyId,
        label: &str,
        months: u32,
        days: u32,
    ) -> Result<Frequency> {
        validate_frequency(label, months, days)?;
        let label = label.trim().to_string();
        let mut tx = self.pool.begin().await?;
        fetch_frequency(&mut tx, owner, id).await?;
        ensure_unique_frequency(&mut tx, owner, &label, Some(id)).await?;
        sqlx::query(
            "UPDATE frequencies SET label = $1, months = $2, days = $3, updated_at = $4 \
             WHERE id = $5 AND owner_id = $6",
        )
        .bind(&label)
        .bind(i64::from(months))
        .bind(i64::from(days))
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let updated = fetch_frequency(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(updated)
    }

    async fn hide_frequency(&self, owner: OwnerId, id: FrequencyId) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE frequencies SET deleted_at = $1, updated_at = $1 \
             WHERE id = $2 AND owner_id = $3 AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }
}
