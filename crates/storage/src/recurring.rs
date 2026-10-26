//! Repeating things (round 15; SPEC §2.6): bills, bins, renewals, certificates, patching.
//! The rules (next due, warn days, the states) live in `shiftbook_core::recurring`; this
//! keeps the records. **Mark done** adds a Done entry on the day (0 minutes, so it shows as
//! a tick and adds no hours) or marks the day's entry for it Done; last done comes from
//! those entries. Owner-scoped, bound parameters only, soft-deleted.

use shiftbook_core::dates::{format_date, format_timestamp, parse_date};
use shiftbook_core::ids::{FrequencyId, OwnerId, RecurringTaskId, TimeEntryId, TimesheetCodeId};
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::{EntryTarget, RecurringTask, TimeEntry, STATUS_DONE};
use shiftbook_core::validate::validate_recurring_task;
use sqlx::sqlite::{SqliteConnection, SqliteRow};
use sqlx::Row;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::error::StorageError;
use crate::repo::{now_text, parse_opt_ts, parse_ts, parse_uuid, u32_from, Result};
use crate::work::{code_exists, entry_from_row, insert_entry};
use crate::SqliteStore;

/// The editable parts of a repeating thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurringInput {
    /// Name, e.g. "Bins out".
    pub name: String,
    /// Kind (recurring type list value), e.g. "Bills".
    pub recurring_type: Option<String>,
    /// How often it comes round.
    pub frequency_id: Option<FrequencyId>,
    /// When it expires (things that expire), or `None` (things that repeat).
    pub expiration_date: Option<Date>,
    /// The "last done" date the schedule counts from (see
    /// `shiftbook_core::recurring::last_done_for_next`).
    pub last_done_before_tracker: Option<Date>,
    /// Days ahead to warn (`None` = 30).
    pub warn_days_before: Option<u32>,
    /// False for routine things that only collect time.
    pub alerts_enabled: bool,
    /// Notes.
    pub notes: String,
    /// The timesheet code its time gets (`None` = its kind's code, then the usual rules).
    pub timesheet_code_id: Option<TimesheetCodeId>,
}

/// What Mark done did, so Undo can take it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkedDone {
    /// The Done entry.
    pub entry_id: TimeEntryId,
    /// True when Mark done made the entry (Undo removes it), false when it marked the day's
    /// existing entry Done (Undo clears the status).
    pub created: bool,
}

macro_rules! recurring_columns {
    () => {
        "id, owner_id, name, type, frequency_id, expiration_date, warn_days_before, \
         last_done_before_tracker, alerts_enabled, active, notes, default_timesheet_code_id, \
         created_at, updated_at, deleted_at"
    };
}

macro_rules! entry_columns {
    () => {
        "id, owner_id, date, minutes, target_type, target_id, category, description, status, \
         priority, next_step, timesheet_code_id, legacy_ticket_text, source, created_at, \
         updated_at, deleted_at"
    };
}

fn opt_date(text: Option<String>) -> Result<Option<Date>> {
    text.as_deref()
        .map(|t| parse_date(t).map_err(|_| StorageError::Corrupt("date")))
        .transpose()
}

fn opt_uuid(text: Option<String>) -> Result<Option<Uuid>> {
    text.as_deref().map(parse_uuid).transpose()
}

fn recurring_from_row(row: &SqliteRow) -> Result<RecurringTask> {
    let warn: Option<i64> = row.try_get("warn_days_before")?;
    Ok(RecurringTask {
        id: RecurringTaskId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        name: row.try_get("name")?,
        recurring_type: row.try_get("type")?,
        frequency_id: opt_uuid(row.try_get("frequency_id")?)?.map(FrequencyId::from_uuid),
        expiration_date: opt_date(row.try_get("expiration_date")?)?,
        warn_days_before: warn.map(|w| u32_from(w, "warn days")).transpose()?,
        last_done_before_tracker: opt_date(row.try_get("last_done_before_tracker")?)?,
        alerts_enabled: row.try_get("alerts_enabled")?,
        active: row.try_get("active")?,
        notes: row.try_get("notes")?,
        default_timesheet_code_id: opt_uuid(row.try_get("default_timesheet_code_id")?)?
            .map(shiftbook_core::ids::TimesheetCodeId::from_uuid),
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

async fn frequency_exists(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: FrequencyId,
) -> Result<()> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM frequencies WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_one(&mut *conn)
    .await?;
    if n == 0 {
        Err(StorageError::NotFound)
    } else {
        Ok(())
    }
}

async fn fetch_recurring(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: RecurringTaskId,
) -> Result<RecurringTask> {
    let row = sqlx::query(concat!(
        "SELECT ",
        recurring_columns!(),
        " FROM recurring_tasks WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    recurring_from_row(&row)
}

/// A checked record from the input; `id` and timestamps are filled by the caller.
fn checked(owner: OwnerId, id: RecurringTaskId, input: &RecurringInput) -> Result<RecurringTask> {
    let now = OffsetDateTime::now_utc();
    let task = RecurringTask {
        id,
        owner_id: owner,
        name: input.name.trim().to_string(),
        recurring_type: input
            .recurring_type
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string),
        frequency_id: input.frequency_id,
        expiration_date: input.expiration_date,
        warn_days_before: input.warn_days_before,
        last_done_before_tracker: input.last_done_before_tracker,
        alerts_enabled: input.alerts_enabled,
        active: true,
        notes: input.notes.trim_end().to_string(),
        default_timesheet_code_id: input.timesheet_code_id,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    validate_recurring_task(&task)?;
    Ok(task)
}

/// Mark `task` done on `date` (see [`SqliteStore::mark_recurring_done`]), noting the expiry
/// date it was renewed to, if any.
async fn mark_done_in(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    task: &RecurringTask,
    date: Date,
    renewed_to: Option<Date>,
) -> Result<MarkedDone> {
    let id = task.id;
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT id FROM time_entries WHERE owner_id = $1 AND target_type = 'recurring' \
         AND target_id = $2 AND date = $3 AND deleted_at IS NULL ORDER BY id LIMIT 1",
    )
    .bind(owner.to_string())
    .bind(id.to_string())
    .bind(format_date(date))
    .fetch_optional(&mut *conn)
    .await?;
    let marked = match existing {
        Some(entry) => {
            sqlx::query(
                "UPDATE time_entries SET status = $1, updated_at = $2, \
                 renewed_to = COALESCE($5, renewed_to) WHERE id = $3 AND owner_id = $4",
            )
            .bind(STATUS_DONE)
            .bind(now_text())
            .bind(&entry)
            .bind(owner.to_string())
            .bind(renewed_to.map(format_date))
            .execute(&mut *conn)
            .await?;
            MarkedDone {
                entry_id: TimeEntryId::from_uuid(parse_uuid(&entry)?),
                created: false,
            }
        }
        None => {
            let now = OffsetDateTime::now_utc();
            let entry = TimeEntry {
                id: TimeEntryId::new(),
                owner_id: owner,
                date,
                minutes: Minutes(0),
                target: EntryTarget::Recurring(id),
                category: None,
                description: task.name.clone(),
                status: None,
                priority: None,
                next_step: None,
                timesheet_code_id: None,
                legacy_ticket_text: None,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            };
            insert_entry(&mut *conn, &entry, None, None).await?;
            sqlx::query(
                "UPDATE time_entries SET status = $1, renewed_to = $4 \
                 WHERE id = $2 AND owner_id = $3",
            )
            .bind(STATUS_DONE)
            .bind(entry.id.to_string())
            .bind(owner.to_string())
            .bind(renewed_to.map(format_date))
            .execute(&mut *conn)
            .await?;
            MarkedDone {
                entry_id: entry.id,
                created: true,
            }
        }
    };
    Ok(marked)
}

impl SqliteStore {
    /// Every live repeating thing, by name.
    pub async fn recurring_all(&self, owner: OwnerId) -> Result<Vec<RecurringTask>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            recurring_columns!(),
            " FROM recurring_tasks WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY lower(name)"
        ))
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(recurring_from_row).collect()
    }

    /// One live repeating thing.
    pub async fn recurring_one(
        &self,
        owner: OwnerId,
        id: RecurringTaskId,
    ) -> Result<RecurringTask> {
        let mut conn = self.pool.acquire().await?;
        fetch_recurring(&mut conn, owner, id).await
    }

    /// Live entries logged against any repeating thing (for last done and total time).
    pub async fn recurring_entries(&self, owner: OwnerId) -> Result<Vec<TimeEntry>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            entry_columns!(),
            " FROM time_entries WHERE owner_id = $1 AND target_type = 'recurring' \
             AND deleted_at IS NULL ORDER BY date"
        ))
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| entry_from_row(r).map(|(e, _)| e))
            .collect()
    }

    /// Add a repeating thing.
    pub async fn create_recurring(
        &self,
        owner: OwnerId,
        input: &RecurringInput,
    ) -> Result<RecurringTask> {
        let task = checked(owner, RecurringTaskId::new(), input)?;
        let mut tx = self.pool.begin().await?;
        if let Some(f) = task.frequency_id {
            frequency_exists(&mut tx, owner, f).await?;
        }
        if let Some(c) = task.default_timesheet_code_id {
            code_exists(&mut tx, owner, c).await?;
        }
        sqlx::query(concat!(
            "INSERT INTO recurring_tasks (",
            recurring_columns!(),
            ") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, TRUE, $10, $12, $11, $11, NULL)"
        ))
        .bind(task.id.to_string())
        .bind(owner.to_string())
        .bind(&task.name)
        .bind(task.recurring_type.as_deref())
        .bind(task.frequency_id.map(|f| f.to_string()))
        .bind(task.expiration_date.map(format_date))
        .bind(task.warn_days_before.map(i64::from))
        .bind(task.last_done_before_tracker.map(format_date))
        .bind(task.alerts_enabled)
        .bind(&task.notes)
        .bind(format_timestamp(task.created_at))
        .bind(task.default_timesheet_code_id.map(|c| c.to_string()))
        .execute(&mut *tx)
        .await?;
        let saved = fetch_recurring(&mut tx, owner, task.id).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Change a repeating thing.
    pub async fn update_recurring(
        &self,
        owner: OwnerId,
        id: RecurringTaskId,
        input: &RecurringInput,
    ) -> Result<RecurringTask> {
        let task = checked(owner, id, input)?;
        let mut tx = self.pool.begin().await?;
        fetch_recurring(&mut tx, owner, id).await?;
        if let Some(f) = task.frequency_id {
            frequency_exists(&mut tx, owner, f).await?;
        }
        if let Some(c) = task.default_timesheet_code_id {
            code_exists(&mut tx, owner, c).await?;
        }
        sqlx::query(
            "UPDATE recurring_tasks SET name = $1, type = $2, frequency_id = $3, \
             expiration_date = $4, warn_days_before = $5, last_done_before_tracker = $6, \
             alerts_enabled = $7, notes = $8, updated_at = $9, default_timesheet_code_id = $12 \
             WHERE id = $10 AND owner_id = $11",
        )
        .bind(&task.name)
        .bind(task.recurring_type.as_deref())
        .bind(task.frequency_id.map(|f| f.to_string()))
        .bind(task.expiration_date.map(format_date))
        .bind(task.warn_days_before.map(i64::from))
        .bind(task.last_done_before_tracker.map(format_date))
        .bind(task.alerts_enabled)
        .bind(&task.notes)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .bind(task.default_timesheet_code_id.map(|c| c.to_string()))
        .execute(&mut *tx)
        .await?;
        let saved = fetch_recurring(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Move a thing that expires to its new expiry date (after it was renewed).
    pub async fn set_recurring_expiry(
        &self,
        owner: OwnerId,
        id: RecurringTaskId,
        expires: Date,
    ) -> Result<RecurringTask> {
        let mut tx = self.pool.begin().await?;
        fetch_recurring(&mut tx, owner, id).await?;
        sqlx::query(
            "UPDATE recurring_tasks SET expiration_date = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4",
        )
        .bind(format_date(expires))
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let saved = fetch_recurring(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Retire a repeating thing (soft delete), or with `deleted = false` bring it back
    /// (Undo). Its entries stay.
    pub async fn set_recurring_deleted(
        &self,
        owner: OwnerId,
        id: RecurringTaskId,
        deleted: bool,
    ) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE recurring_tasks SET deleted_at = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4",
        )
        .bind(deleted.then_some(now.as_str()))
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            Err(StorageError::NotFound)
        } else {
            Ok(())
        }
    }

    /// Mark a repeating thing done on `date`: the day's entry for it becomes Done, or a
    /// Done entry of 0 minutes is added (a tick on the day).
    pub async fn mark_recurring_done(
        &self,
        owner: OwnerId,
        id: RecurringTaskId,
        date: Date,
    ) -> Result<MarkedDone> {
        let mut tx = self.pool.begin().await?;
        let task = fetch_recurring(&mut tx, owner, id).await?;
        let marked = mark_done_in(&mut tx, owner, &task, date, None).await?;
        tx.commit().await?;
        Ok(marked)
    }

    /// Renewed…: move a thing that expires to its new expiry date and mark it done on
    /// `date`, remembering the new date for its past renewals.
    pub async fn mark_recurring_renewed(
        &self,
        owner: OwnerId,
        id: RecurringTaskId,
        date: Date,
        expires: Date,
    ) -> Result<MarkedDone> {
        let mut tx = self.pool.begin().await?;
        let task = fetch_recurring(&mut tx, owner, id).await?;
        sqlx::query(
            "UPDATE recurring_tasks SET expiration_date = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4",
        )
        .bind(format_date(expires))
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let marked = mark_done_in(&mut tx, owner, &task, date, Some(expires)).await?;
        tx.commit().await?;
        Ok(marked)
    }

    /// Everything its past rounds are made from (see
    /// `shiftbook_core::recurring::past_times`): its live entries, each with the expiry
    /// date it renewed to, and the (date, ticket number) of tickets on those entries (the
    /// Tickets tray's roll-up entries carry their tickets too).
    pub async fn recurring_history(
        &self,
        owner: OwnerId,
        id: RecurringTaskId,
    ) -> Result<(Vec<(TimeEntry, Option<Date>)>, Vec<(Date, String)>)> {
        let mut conn = self.pool.acquire().await?;
        fetch_recurring(&mut conn, owner, id).await?;
        let rows = sqlx::query(concat!(
            "SELECT ",
            entry_columns!(),
            ", renewed_to FROM time_entries WHERE owner_id = $1 AND target_type = 'recurring' \
             AND target_id = $2 AND deleted_at IS NULL ORDER BY date, id"
        ))
        .bind(owner.to_string())
        .bind(id.to_string())
        .fetch_all(&mut *conn)
        .await?;
        let entries = rows
            .iter()
            .map(|r| {
                let (entry, _) = entry_from_row(r)?;
                let to: Option<String> = r.try_get("renewed_to")?;
                Ok((entry, to.as_deref().map(parse_date).transpose()?))
            })
            .collect::<Result<Vec<_>>>()?;
        let numbers: Vec<(String, String)> = sqlx::query_as(
            "SELECT e.date, t.ticket_number FROM tickets t \
             JOIN time_entries e ON e.id = t.time_entry_id \
             WHERE t.owner_id = $1 AND e.owner_id = $1 AND e.target_type = 'recurring' \
             AND e.target_id = $2 AND e.deleted_at IS NULL AND t.deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(id.to_string())
        .fetch_all(&mut *conn)
        .await?;
        let tickets = numbers
            .into_iter()
            .map(|(d, n)| Ok((parse_date(&d)?, n)))
            .collect::<Result<Vec<_>>>()?;
        Ok((entries, tickets))
    }

    /// Undo [`Self::mark_recurring_done`].
    pub async fn undo_recurring_done(&self, owner: OwnerId, marked: MarkedDone) -> Result<()> {
        let now = now_text();
        let sql = if marked.created {
            "UPDATE time_entries SET deleted_at = $1, updated_at = $1 \
             WHERE id = $2 AND owner_id = $3 AND target_type = 'recurring'"
        } else {
            "UPDATE time_entries SET status = NULL, renewed_to = NULL, updated_at = $1 \
             WHERE id = $2 AND owner_id = $3 AND target_type = 'recurring'"
        };
        let done = sqlx::query(sql)
            .bind(&now)
            .bind(marked.entry_id.to_string())
            .bind(owner.to_string())
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            Err(StorageError::NotFound)
        } else {
            Ok(())
        }
    }
}
