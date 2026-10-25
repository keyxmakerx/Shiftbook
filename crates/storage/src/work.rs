//! Projects, time entries, their tickets, the Done tickets tray and Log work.
//!
//! All methods are owner-scoped and use bound parameters only. Records are
//! soft-deleted. Creation order is the UUID v7 id order.
//!
//! The tray's roll-up entry (`time_entries.source = 'tray'`, one per day) is kept
//! in step with the tray inside the same transaction as every tray change; its
//! minutes and tickets come from the tray (see `shiftbook_core::tray`), while
//! its title, project and notes stay editable.

use shiftbook_core::capture::Capture;
use shiftbook_core::dates::{format_date, format_timestamp, parse_date};
use shiftbook_core::error::CoreError;
use shiftbook_core::ids::{
    NoteId, OwnerId, ProjectId, TicketId, TimeEntryId, TimesheetCodeId, TrayItemId,
};
use shiftbook_core::minutes::{Minutes, MAX_ENTRY_MINUTES};
use shiftbook_core::model::{EntryTarget, Project, TargetType, Ticket, TimeEntry, TrayItem};
use shiftbook_core::notes::jot_to_entry;
use shiftbook_core::prefs::next_project_color;
use shiftbook_core::quicklog::{
    plan_add_time, plan_quick_log, AddTimePlan, DayEntry, PastUse, QuickLogPlan,
};
use shiftbook_core::tickets::add_tickets;
use shiftbook_core::tray::{
    carried_open, clean_tray_note, new_tray_tickets, plan_rollups, validate_tray_minutes,
    RollupAction, DEFAULT_TRAY_MINUTES, NEW_TICKET_DONE, TRAY_ENTRY_TITLE,
};
use shiftbook_core::validate::{
    clean_notes, validate_color, validate_project, validate_time_entry,
};
use sqlx::sqlite::{SqliteConnection, SqliteRow};
use sqlx::Row;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::error::StorageError;
use crate::items::{
    item_for_number, set_item_done, set_item_note, set_item_project, ticket_item, tray_row_item,
};
use crate::repo::{now_text, parse_opt_ts, parse_ts, parse_uuid, u32_from, Result};
use crate::SqliteStore;

/// `time_entries.source` for a day's tray roll-up.
pub const SOURCE_TRAY: &str = "tray";

/// An entry with its live tickets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRecord {
    /// The entry.
    pub entry: TimeEntry,
    /// Its tickets, in the order they were added.
    pub tickets: Vec<Ticket>,
    /// True for the day's tray roll-up entry.
    pub is_tray: bool,
}

/// The editable parts of an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInput {
    /// The day.
    pub date: Date,
    /// Minutes (ignored on update of a tray entry).
    pub minutes: Minutes,
    /// What I worked on.
    pub description: String,
    /// Project, or none.
    pub project: Option<ProjectId>,
    /// Category list value.
    pub category: Option<String>,
    /// Notes / next step.
    pub next_step: Option<String>,
    /// Timesheet code override.
    pub timesheet_code_id: Option<TimesheetCodeId>,
}

/// The editable parts of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInput {
    /// Name.
    pub name: String,
    /// Colour `#rrggbb`.
    pub color: Option<String>,
    /// Default timesheet code.
    pub default_timesheet_code_id: Option<TimesheetCodeId>,
    /// Archived.
    pub archived: bool,
}

/// One day's tray and its roll-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayDay {
    /// The day.
    pub date: Date,
    /// Tray tickets in the order they were pasted.
    pub items: Vec<TrayItem>,
    /// The plain "Tickets" roll-up entry, if any ticket has no project.
    pub entry_id: Option<TimeEntryId>,
    /// Total minutes of the day's roll-ups (the plain one and each project's).
    pub minutes: Minutes,
}

pub(crate) fn corrupt_date(text: &str) -> Result<Date> {
    parse_date(text).map_err(|_| StorageError::Corrupt("date"))
}

pub(crate) fn opt_uuid(text: Option<String>) -> Result<Option<Uuid>> {
    text.as_deref().map(parse_uuid).transpose()
}

macro_rules! project_columns {
    () => {
        "id, owner_id, name, description, requested_by, due_date, status, priority, \
         estimated_minutes, default_timesheet_code_id, color, archived, created_at, updated_at, \
         deleted_at"
    };
}

fn project_from_row(row: &SqliteRow) -> Result<Project> {
    let estimate: Option<i64> = row.try_get("estimated_minutes")?;
    let due: Option<String> = row.try_get("due_date")?;
    Ok(Project {
        id: ProjectId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        requested_by: row.try_get("requested_by")?,
        due_date: due.as_deref().map(corrupt_date).transpose()?,
        status: row.try_get("status")?,
        priority: row.try_get("priority")?,
        estimated_minutes: estimate
            .map(|m| u32_from(m, "estimate").map(Minutes))
            .transpose()?,
        default_timesheet_code_id: opt_uuid(row.try_get("default_timesheet_code_id")?)?
            .map(TimesheetCodeId::from_uuid),
        color: row.try_get("color")?,
        archived: row.try_get("archived")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

macro_rules! entry_columns {
    () => {
        "id, owner_id, date, minutes, target_type, target_id, category, description, status, \
         priority, next_step, timesheet_code_id, legacy_ticket_text, source, created_at, \
         updated_at, deleted_at"
    };
}

fn parse_target_type(text: &str) -> Result<TargetType> {
    match text {
        "project" => Ok(TargetType::Project),
        "recurring" => Ok(TargetType::Recurring),
        "none" => Ok(TargetType::None),
        _ => Err(StorageError::Corrupt("target type")),
    }
}

pub(crate) fn entry_from_row(row: &SqliteRow) -> Result<(TimeEntry, bool)> {
    let target_type = parse_target_type(row.try_get("target_type")?)?;
    let target_id = opt_uuid(row.try_get("target_id")?)?;
    let source: Option<String> = row.try_get("source")?;
    let entry = TimeEntry {
        id: TimeEntryId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        date: corrupt_date(row.try_get("date")?)?,
        minutes: Minutes(u32_from(row.try_get("minutes")?, "minutes")?),
        target: EntryTarget::from_columns(target_type, target_id)
            .ok_or(StorageError::Corrupt("target"))?,
        category: row.try_get("category")?,
        description: row.try_get("description")?,
        status: row.try_get("status")?,
        priority: row.try_get("priority")?,
        next_step: row.try_get("next_step")?,
        timesheet_code_id: opt_uuid(row.try_get("timesheet_code_id")?)?
            .map(TimesheetCodeId::from_uuid),
        legacy_ticket_text: row.try_get("legacy_ticket_text")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    };
    Ok((entry, source.as_deref() == Some(SOURCE_TRAY)))
}

fn ticket_from_row(row: &SqliteRow) -> Result<Ticket> {
    Ok(Ticket {
        id: TicketId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        time_entry_id: TimeEntryId::from_uuid(parse_uuid(row.try_get("time_entry_id")?)?),
        ticket_number: row.try_get("ticket_number")?,
        note: row.try_get("note")?,
        done: row.try_get("done")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

macro_rules! tray_columns {
    () => {
        "id, owner_id, date, ticket_number, minutes, note, done, project_id, created_at, \
         updated_at, deleted_at"
    };
}

fn tray_from_row(row: &SqliteRow) -> Result<TrayItem> {
    Ok(TrayItem {
        id: TrayItemId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        date: corrupt_date(row.try_get("date")?)?,
        ticket_number: row.try_get("ticket_number")?,
        minutes: Minutes(u32_from(row.try_get("minutes")?, "minutes")?),
        note: row.try_get("note")?,
        done: row.try_get("done")?,
        project_id: opt_uuid(row.try_get("project_id")?)?.map(ProjectId::from_uuid),
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

pub(crate) async fn project_exists(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: ProjectId,
) -> Result<()> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM projects WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
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

pub(crate) async fn code_exists(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TimesheetCodeId,
) -> Result<()> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM timesheet_codes WHERE id = $1 AND owner_id = $2 \
         AND deleted_at IS NULL",
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

async fn fetch_project(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: ProjectId,
) -> Result<Project> {
    let row = sqlx::query(concat!(
        "SELECT ",
        project_columns!(),
        " FROM projects WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    project_from_row(&row)
}

async fn ensure_unique_project(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    name: &str,
    except: Option<ProjectId>,
) -> Result<()> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT id, name FROM projects WHERE owner_id = $1 AND deleted_at IS NULL")
            .bind(owner.to_string())
            .fetch_all(&mut *conn)
            .await?;
    let wanted = name.trim().to_lowercase();
    let except = except.map(|id| id.to_string());
    if rows
        .iter()
        .any(|(id, n)| Some(id) != except.as_ref() && n.trim().to_lowercase() == wanted)
    {
        return Err(StorageError::Duplicate(name.trim().to_string()));
    }
    Ok(())
}

async fn fetch_entry(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TimeEntryId,
) -> Result<(TimeEntry, bool)> {
    let row = sqlx::query(concat!(
        "SELECT ",
        entry_columns!(),
        " FROM time_entries WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    entry_from_row(&row)
}

async fn entry_tickets(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TimeEntryId,
) -> Result<Vec<Ticket>> {
    let rows = sqlx::query(
        "SELECT id, owner_id, time_entry_id, ticket_number, note, done, created_at, updated_at, \
         deleted_at FROM tickets WHERE owner_id = $1 AND time_entry_id = $2 \
         AND deleted_at IS NULL ORDER BY id",
    )
    .bind(owner.to_string())
    .bind(id.to_string())
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(ticket_from_row).collect()
}

async fn entry_record(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TimeEntryId,
) -> Result<EntryRecord> {
    let (entry, is_tray) = fetch_entry(conn, owner, id).await?;
    let tickets = entry_tickets(conn, owner, id).await?;
    Ok(EntryRecord {
        entry,
        tickets,
        is_tray,
    })
}

/// Add tickets to an entry. Each joins its item (round 15): it starts still open
/// (`NEW_TICKET_DONE`), which reopens the item from the entry's day, and shows the item's
/// notepad. A tray roll-up's tickets only mirror the tray (their state lives there), so they
/// have no item.
async fn insert_tickets(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    entry: TimeEntryId,
    tickets: &[String],
) -> Result<()> {
    let now = now_text();
    let (date, rollup): (String, Option<String>) =
        sqlx::query_as("SELECT date, source FROM time_entries WHERE id = $1 AND owner_id = $2")
            .bind(entry.to_string())
            .bind(owner.to_string())
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(StorageError::NotFound)?;
    let date = corrupt_date(&date)?;
    for t in tickets {
        let item = match rollup {
            Some(_) => None,
            None => Some(item_for_number(conn, owner, t).await?),
        };
        sqlx::query(
            "INSERT INTO tickets (id, owner_id, time_entry_id, ticket_number, note, done, \
             item_id, created_at, updated_at, deleted_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, NULL)",
        )
        .bind(TicketId::new().to_string())
        .bind(owner.to_string())
        .bind(entry.to_string())
        .bind(t)
        .bind(item.as_ref().and_then(|i| i.note.clone()))
        .bind(item.is_none() || NEW_TICKET_DONE)
        .bind(item.as_ref().map(|i| i.id.to_string()))
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        if let Some(item) = item {
            set_item_done(conn, owner, item.id, NEW_TICKET_DONE, date).await?;
        }
    }
    Ok(())
}

fn clean_opt(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn checked_entry(owner: OwnerId, input: &EntryInput) -> Result<TimeEntry> {
    let now = OffsetDateTime::now_utc();
    let entry = TimeEntry {
        id: TimeEntryId::new(),
        owner_id: owner,
        date: input.date,
        minutes: input.minutes,
        target: input
            .project
            .map_or(EntryTarget::None, EntryTarget::Project),
        category: clean_opt(&input.category),
        description: input.description.trim().to_string(),
        status: None,
        priority: None,
        next_step: input
            .next_step
            .as_deref()
            .map(str::trim_end)
            .filter(|v| !v.trim().is_empty())
            .map(str::to_string),
        timesheet_code_id: input.timesheet_code_id,
        legacy_ticket_text: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    validate_time_entry(&entry)?;
    Ok(entry)
}

async fn check_links(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    input: &EntryInput,
) -> Result<()> {
    if let Some(p) = input.project {
        project_exists(conn, owner, p).await?;
    }
    if let Some(c) = input.timesheet_code_id {
        code_exists(conn, owner, c).await?;
    }
    Ok(())
}

/// Insert an entry. `source` marks a tray roll-up, and `tray_project` which project's
/// tray tickets it rolls up (`None` for the plain "Tickets" entry and for user entries).
pub(crate) async fn insert_entry(
    conn: &mut SqliteConnection,
    entry: &TimeEntry,
    source: Option<&str>,
    tray_project: Option<ProjectId>,
) -> Result<()> {
    sqlx::query(concat!(
        "INSERT INTO time_entries (",
        entry_columns!(),
        ", tray_project_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NULL, NULL, $9, $10, NULL, \
         $11, $12, $12, NULL, $13)"
    ))
    .bind(entry.id.to_string())
    .bind(entry.owner_id.to_string())
    .bind(format_date(entry.date))
    .bind(i64::from(entry.minutes.get()))
    .bind(entry.target.target_type().as_str())
    .bind(entry.target.target_id().map(|u| u.to_string()))
    .bind(entry.category.as_deref())
    .bind(&entry.description)
    .bind(entry.next_step.as_deref())
    .bind(entry.timesheet_code_id.map(|c| c.to_string()))
    .bind(source)
    .bind(format_timestamp(entry.created_at))
    .bind(tray_project.map(|p| p.to_string()))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn set_minutes(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TimeEntryId,
    minutes: Minutes,
) -> Result<()> {
    if minutes.get() > MAX_ENTRY_MINUTES {
        return Err(CoreError::HoursTooLarge.into());
    }
    sqlx::query(
        "UPDATE time_entries SET minutes = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
    )
    .bind(i64::from(minutes.get()))
    .bind(now_text())
    .bind(id.to_string())
    .bind(owner.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn create_entry_in(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    input: &EntryInput,
    tickets: &str,
) -> Result<TimeEntryId> {
    let entry = checked_entry(owner, input)?;
    check_links(conn, owner, input).await?;
    let tickets = add_tickets::<&str>(&[], tickets)?;
    insert_entry(conn, &entry, None, None).await?;
    insert_tickets(conn, owner, entry.id, &tickets).await?;
    Ok(entry.id)
}

async fn tray_items(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    date: Date,
) -> Result<Vec<TrayItem>> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        tray_columns!(),
        " FROM ticket_tray WHERE owner_id = $1 AND date = $2 AND deleted_at IS NULL ORDER BY id"
    ))
    .bind(owner.to_string())
    .bind(format_date(date))
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(tray_from_row).collect()
}

/// The day's live roll-up entries, by the project they roll up (`None` = plain "Tickets").
async fn tray_entries(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    date: Date,
) -> Result<Vec<(Option<ProjectId>, TimeEntryId)>> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, tray_project_id FROM time_entries WHERE owner_id = $1 AND date = $2 \
         AND source = $3 AND deleted_at IS NULL ORDER BY id",
    )
    .bind(owner.to_string())
    .bind(format_date(date))
    .bind(SOURCE_TRAY)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|(id, project)| {
            Ok((
                opt_uuid(project)?.map(ProjectId::from_uuid),
                TimeEntryId::from_uuid(parse_uuid(&id)?),
            ))
        })
        .collect()
}

/// The day's plain "Tickets" roll-up entry, if there is one.
async fn tray_entry_id(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    date: Date,
) -> Result<Option<TimeEntryId>> {
    Ok(tray_entries(conn, owner, date)
        .await?
        .into_iter()
        .find(|(p, _)| p.is_none())
        .map(|(_, id)| id))
}

/// The project a roll-up entry rolls up, if it is a project's roll-up.
async fn tray_group(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TimeEntryId,
) -> Result<Option<ProjectId>> {
    let project: Option<Option<String>> = sqlx::query_scalar(
        "SELECT tray_project_id FROM time_entries WHERE id = $1 AND owner_id = $2",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?;
    Ok(opt_uuid(project.flatten())?.map(ProjectId::from_uuid))
}

/// Make the tickets on `entry` exactly `wanted` (ignoring case), keeping the
/// ones already there.
async fn replace_tickets(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    entry: TimeEntryId,
    wanted: &[String],
) -> Result<()> {
    let current = entry_tickets(conn, owner, entry).await?;
    let now = now_text();
    for t in &current {
        if !wanted
            .iter()
            .any(|w| w.eq_ignore_ascii_case(&t.ticket_number))
        {
            sqlx::query(
                "UPDATE tickets SET deleted_at = $1, updated_at = $1 WHERE id = $2 AND owner_id = $3",
            )
            .bind(&now)
            .bind(t.id.to_string())
            .bind(owner.to_string())
            .execute(&mut *conn)
            .await?;
        }
    }
    let missing: Vec<String> = wanted
        .iter()
        .filter(|w| {
            !current
                .iter()
                .any(|t| t.ticket_number.eq_ignore_ascii_case(w))
        })
        .cloned()
        .collect();
    insert_tickets(conn, owner, entry, &missing).await
}

/// Bring the day's roll-up entries (one per project, plus the plain one) in line with
/// its tray.
async fn sync_tray(conn: &mut SqliteConnection, owner: OwnerId, date: Date) -> Result<()> {
    let items = tray_items(conn, owner, date).await?;
    let triples: Vec<(String, Minutes, Option<ProjectId>)> = items
        .iter()
        .map(|i| (i.ticket_number.clone(), i.minutes, i.project_id))
        .collect();
    let existing = tray_entries(conn, owner, date).await?;
    for (group, action) in plan_rollups(&triples, &existing) {
        apply_rollup(conn, owner, date, group, action).await?;
    }
    Ok(())
}

async fn apply_rollup(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    date: Date,
    group: Option<ProjectId>,
    action: RollupAction,
) -> Result<()> {
    match action {
        RollupAction::Create(rollup) => {
            let now = OffsetDateTime::now_utc();
            let entry = TimeEntry {
                id: TimeEntryId::new(),
                owner_id: owner,
                date,
                minutes: rollup.minutes,
                target: group.map_or(EntryTarget::None, EntryTarget::Project),
                category: None,
                description: TRAY_ENTRY_TITLE.to_string(),
                status: None,
                priority: None,
                next_step: None,
                timesheet_code_id: None,
                legacy_ticket_text: None,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            };
            insert_entry(conn, &entry, Some(SOURCE_TRAY), group).await?;
            insert_tickets(conn, owner, entry.id, &rollup.tickets).await
        }
        RollupAction::Update { entry, rollup } => {
            set_minutes(conn, owner, entry, rollup.minutes).await?;
            replace_tickets(conn, owner, entry, &rollup.tickets).await
        }
        RollupAction::Remove(entry) => {
            sqlx::query(
                "UPDATE time_entries SET deleted_at = $1, updated_at = $1 \
                 WHERE id = $2 AND owner_id = $3",
            )
            .bind(now_text())
            .bind(entry.to_string())
            .bind(owner.to_string())
            .execute(&mut *conn)
            .await?;
            Ok(())
        }
        RollupAction::Nothing => Ok(()),
    }
}

async fn tray_day_in(conn: &mut SqliteConnection, owner: OwnerId, date: Date) -> Result<TrayDay> {
    let items = tray_items(conn, owner, date).await?;
    let entry_id = tray_entry_id(conn, owner, date).await?;
    let mut minutes = Minutes::ZERO;
    for (_, id) in tray_entries(conn, owner, date).await? {
        minutes = minutes + fetch_entry(conn, owner, id).await?.0.minutes;
    }
    Ok(TrayDay {
        date,
        items,
        entry_id,
        minutes,
    })
}

async fn live_tray_item(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TrayItemId,
) -> Result<TrayItem> {
    let row = sqlx::query(concat!(
        "SELECT ",
        tray_columns!(),
        " FROM ticket_tray WHERE owner_id = $1 AND id = $2 AND deleted_at IS NULL"
    ))
    .bind(owner.to_string())
    .bind(id.to_string())
    .fetch_optional(&mut *conn)
    .await?;
    tray_from_row(&row.ok_or(StorageError::NotFound)?)
}

async fn tray_item_date(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TrayItemId,
    live: bool,
) -> Result<Date> {
    let date: Option<String> = sqlx::query_scalar(
        "SELECT date FROM ticket_tray WHERE id = $1 AND owner_id = $2 \
         AND ((deleted_at IS NULL) = $3)",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .bind(live)
    .fetch_optional(&mut *conn)
    .await?;
    corrupt_date(&date.ok_or(StorageError::NotFound)?)
}

/// Add one ticket to a day's tray as part of its item (round 15): the item's notepad and
/// project come along. `done` false keeps it open (reopening the item from `date`); true
/// finishes the item everywhere.
async fn insert_tray_row(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    date: Date,
    ticket: &str,
    done: bool,
) -> Result<()> {
    let item = item_for_number(conn, owner, ticket).await?;
    let now = now_text();
    sqlx::query(concat!(
        "INSERT INTO ticket_tray (",
        tray_columns!(),
        ", item_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, NULL, $10)"
    ))
    .bind(TrayItemId::new().to_string())
    .bind(owner.to_string())
    .bind(format_date(date))
    .bind(ticket)
    .bind(i64::from(DEFAULT_TRAY_MINUTES))
    .bind(item.note)
    .bind(done)
    .bind(item.project.map(|p| p.to_string()))
    .bind(&now)
    .bind(item.id.to_string())
    .execute(&mut *conn)
    .await?;
    set_item_done(conn, owner, item.id, done, date).await
}

impl SqliteStore {
    // ---------- projects ----------

    /// Live projects, active first, then by name.
    pub async fn list_projects(&self, owner: OwnerId) -> Result<Vec<Project>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            project_columns!(),
            " FROM projects WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY archived, lower(name), id"
        ))
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(project_from_row).collect()
    }

    /// Add a project. Without a colour it gets the next unused palette colour.
    pub async fn create_project(
        &self,
        owner: OwnerId,
        name: &str,
        color: Option<&str>,
    ) -> Result<Project> {
        let color = color.map(validate_color).transpose()?;
        let mut tx = self.pool.begin().await?;
        ensure_unique_project(&mut tx, owner, name, None).await?;
        let used: Vec<Option<String>> = sqlx::query_scalar(
            "SELECT color FROM projects WHERE owner_id = $1 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .fetch_all(&mut *tx)
        .await?;
        let now = parse_ts(&now_text())?;
        let project = Project {
            id: ProjectId::new(),
            owner_id: owner,
            name: name.trim().to_string(),
            description: String::new(),
            requested_by: None,
            due_date: None,
            status: None,
            priority: None,
            estimated_minutes: None,
            default_timesheet_code_id: None,
            color: Some(color.unwrap_or_else(|| next_project_color(&used))),
            archived: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        validate_project(&project)?;
        sqlx::query(concat!(
            "INSERT INTO projects (",
            project_columns!(),
            ") VALUES ($1, $2, $3, '', NULL, NULL, NULL, NULL, NULL, NULL, $4, FALSE, $5, $5, NULL)"
        ))
        .bind(project.id.to_string())
        .bind(owner.to_string())
        .bind(&project.name)
        .bind(project.color.as_deref())
        .bind(format_timestamp(now))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(project)
    }

    /// Change a project's name, colour, default code or archived flag.
    pub async fn update_project(
        &self,
        owner: OwnerId,
        id: ProjectId,
        input: &ProjectInput,
    ) -> Result<Project> {
        let color = input.color.as_deref().map(validate_color).transpose()?;
        let mut tx = self.pool.begin().await?;
        let mut project = fetch_project(&mut tx, owner, id).await?;
        project.name = input.name.trim().to_string();
        validate_project(&project)?;
        ensure_unique_project(&mut tx, owner, &project.name, Some(id)).await?;
        if let Some(code) = input.default_timesheet_code_id {
            code_exists(&mut tx, owner, code).await?;
        }
        sqlx::query(
            "UPDATE projects SET name = $1, color = $2, default_timesheet_code_id = $3, \
             archived = $4, updated_at = $5 WHERE id = $6 AND owner_id = $7",
        )
        .bind(&project.name)
        .bind(color)
        .bind(input.default_timesheet_code_id.map(|c| c.to_string()))
        .bind(input.archived)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let updated = fetch_project(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(updated)
    }

    // ---------- entries ----------

    /// One live entry with its tickets.
    pub async fn get_entry(&self, owner: OwnerId, id: TimeEntryId) -> Result<EntryRecord> {
        let mut conn = self.pool.acquire().await?;
        entry_record(&mut conn, owner, id).await
    }

    /// Live entries from `from` to `to` (inclusive), by date then creation.
    pub async fn list_entries(
        &self,
        owner: OwnerId,
        from: Date,
        to: Date,
    ) -> Result<Vec<EntryRecord>> {
        let mut conn = self.pool.acquire().await?;
        let rows = sqlx::query(concat!(
            "SELECT ",
            entry_columns!(),
            " FROM time_entries WHERE owner_id = $1 AND date >= $2 AND date <= $3 \
             AND deleted_at IS NULL ORDER BY date, id"
        ))
        .bind(owner.to_string())
        .bind(format_date(from))
        .bind(format_date(to))
        .fetch_all(&mut *conn)
        .await?;
        let mut records: Vec<EntryRecord> = rows
            .iter()
            .map(|r| {
                entry_from_row(r).map(|(entry, is_tray)| EntryRecord {
                    entry,
                    tickets: Vec::new(),
                    is_tray,
                })
            })
            .collect::<Result<_>>()?;
        let ticket_rows = sqlx::query(
            "SELECT t.id, t.owner_id, t.time_entry_id, t.ticket_number, t.note, t.done, \
             t.created_at, t.updated_at, t.deleted_at FROM tickets t \
             JOIN time_entries e ON e.id = t.time_entry_id \
             WHERE t.owner_id = $1 AND e.owner_id = $1 AND e.date >= $2 AND e.date <= $3 \
             AND e.deleted_at IS NULL AND t.deleted_at IS NULL ORDER BY t.id",
        )
        .bind(owner.to_string())
        .bind(format_date(from))
        .bind(format_date(to))
        .fetch_all(&mut *conn)
        .await?;
        for row in &ticket_rows {
            let ticket = ticket_from_row(row)?;
            if let Some(rec) = records
                .iter_mut()
                .find(|r| r.entry.id == ticket.time_entry_id)
            {
                rec.tickets.push(ticket);
            }
        }
        Ok(records)
    }

    /// Add an entry with tickets pasted as text (split like multi-paste).
    pub async fn create_entry(
        &self,
        owner: OwnerId,
        input: &EntryInput,
        tickets: &str,
    ) -> Result<EntryRecord> {
        let mut tx = self.pool.begin().await?;
        let id = create_entry_in(&mut tx, owner, input, tickets).await?;
        let rec = entry_record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Change an entry. A tray roll-up entry keeps its date and minutes (they
    /// come from the tray).
    pub async fn update_entry(
        &self,
        owner: OwnerId,
        id: TimeEntryId,
        input: &EntryInput,
    ) -> Result<EntryRecord> {
        let mut tx = self.pool.begin().await?;
        let (current, is_tray) = fetch_entry(&mut tx, owner, id).await?;
        let mut input = input.clone();
        if is_tray {
            input.minutes = current.minutes;
            input.date = current.date;
            if let Some(project) = tray_group(&mut tx, owner, id).await? {
                input.project = Some(project);
            }
        }
        let mut checked = checked_entry(owner, &input)?;
        if input.project.is_none() && matches!(current.target, EntryTarget::Recurring(_)) {
            // A repeating thing's entry has no project to pick; keep what it belongs to.
            checked.target = current.target;
        }
        check_links(&mut tx, owner, &input).await?;
        sqlx::query(
            "UPDATE time_entries SET date = $1, minutes = $2, target_type = $3, target_id = $4, \
             category = $5, description = $6, next_step = $7, timesheet_code_id = $8, \
             updated_at = $9 WHERE id = $10 AND owner_id = $11",
        )
        .bind(format_date(checked.date))
        .bind(i64::from(checked.minutes.get()))
        .bind(checked.target.target_type().as_str())
        .bind(checked.target.target_id().map(|u| u.to_string()))
        .bind(checked.category.as_deref())
        .bind(&checked.description)
        .bind(checked.next_step.as_deref())
        .bind(checked.timesheet_code_id.map(|c| c.to_string()))
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = entry_record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Log a jot (any note) as an entry on `date` (round 14, see
    /// `shiftbook_core::notes::jot_to_entry`): its title says what was done, the rest becomes
    /// the entry's notes, ticket numbers in it become tickets, and it keeps the note's project.
    /// The note moves to the archive in the same step.
    pub async fn log_note(
        &self,
        owner: OwnerId,
        id: NoteId,
        date: Date,
        minutes: u32,
    ) -> Result<EntryRecord> {
        let note = self.get_note(owner, id).await?;
        let made = jot_to_entry(&note.title, &note.body);
        let input = EntryInput {
            date,
            minutes: Minutes(minutes),
            description: made.description,
            project: note.project_id,
            category: None,
            next_step: made.notes,
            timesheet_code_id: None,
        };
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        let entry = create_entry_in(&mut tx, owner, &input, &made.tickets.join("\n")).await?;
        sqlx::query(
            "UPDATE notes SET archived_at = $1, updated_at = $1 \
             WHERE id = $2 AND owner_id = $3 AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = entry_record(&mut tx, owner, entry).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Add `minutes` of work on an entry, landing on `day` (see
    /// `shiftbook_core::quicklog::plan_add_time`): the entry itself when it's from `day`, else
    /// `day`'s entry with the same title and project, else a copy of it on `day`. Returns the
    /// entry that got the time and whether it was made just now.
    pub async fn add_time_on(
        &self,
        owner: OwnerId,
        id: TimeEntryId,
        day: Date,
        minutes: u32,
    ) -> Result<(EntryRecord, bool)> {
        let rec = self.get_entry(owner, id).await?;
        if rec.is_tray {
            return Err(StorageError::TrayManaged);
        }
        let on_day = self.list_entries(owner, day, day).await?;
        let listed: Vec<(TimeEntryId, &str, EntryTarget, bool)> = on_day
            .iter()
            .map(|r| {
                (
                    r.entry.id,
                    r.entry.description.as_str(),
                    r.entry.target,
                    r.is_tray,
                )
            })
            .collect();
        let e = &rec.entry;
        let plan = plan_add_time((e.id, e.date, &e.description, e.target), day, &listed);
        let delta = i32::try_from(minutes).map_err(|_| CoreError::HoursTooLarge)?;
        match plan {
            AddTimePlan::AddTo(to) => Ok((self.add_entry_minutes(owner, to, delta).await?, false)),
            AddTimePlan::Copy => {
                let input = EntryInput {
                    date: day,
                    minutes: Minutes(minutes),
                    description: e.description.clone(),
                    project: match e.target {
                        EntryTarget::Project(p) => Some(p),
                        _ => None,
                    },
                    category: e.category.clone(),
                    next_step: None,
                    timesheet_code_id: e.timesheet_code_id,
                };
                let tickets: Vec<&str> = rec
                    .tickets
                    .iter()
                    .map(|t| t.ticket_number.as_str())
                    .collect();
                Ok((
                    self.create_entry(owner, &input, &tickets.join("\n"))
                        .await?,
                    true,
                ))
            }
        }
    }

    /// Add (or with a negative number, take away) minutes. Never below zero;
    /// over 24 hours is refused. Not allowed on the tray roll-up.
    pub async fn add_entry_minutes(
        &self,
        owner: OwnerId,
        id: TimeEntryId,
        delta: i32,
    ) -> Result<EntryRecord> {
        let mut tx = self.pool.begin().await?;
        let (entry, is_tray) = fetch_entry(&mut tx, owner, id).await?;
        if is_tray {
            return Err(StorageError::TrayManaged);
        }
        let next = (i64::from(entry.minutes.get()) + i64::from(delta)).max(0);
        let next = u32::try_from(next).map_err(|_| CoreError::HoursTooLarge)?;
        set_minutes(&mut tx, owner, id, Minutes(next)).await?;
        let rec = entry_record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Save an entry's notepad (its notes; blank clears them). Works on tray roll-ups too.
    pub async fn set_entry_notes(
        &self,
        owner: OwnerId,
        id: TimeEntryId,
        notes: &str,
    ) -> Result<EntryRecord> {
        let notes = clean_notes(notes)?;
        let mut tx = self.pool.begin().await?;
        fetch_entry(&mut tx, owner, id).await?;
        sqlx::query(
            "UPDATE time_entries SET next_step = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(notes)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = entry_record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Soft-delete an entry. Deleting a tray roll-up also takes its tickets out of that
    /// day's tray: all of the plain entry's, or one project's (restoring brings both back).
    pub async fn delete_entry(&self, owner: OwnerId, id: TimeEntryId) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let (entry, is_tray) = fetch_entry(&mut tx, owner, id).await?;
        let now = now_text();
        sqlx::query(
            "UPDATE time_entries SET deleted_at = $1, updated_at = $1 WHERE id = $2 AND owner_id = $3",
        )
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        if is_tray {
            let group = tray_group(&mut tx, owner, id).await?;
            sqlx::query(
                "UPDATE ticket_tray SET deleted_at = $1, updated_at = $1 \
                 WHERE owner_id = $2 AND date = $3 AND deleted_at IS NULL \
                 AND COALESCE(project_id, '') = $4",
            )
            .bind(&now)
            .bind(owner.to_string())
            .bind(format_date(entry.date))
            .bind(group.map(|p| p.to_string()).unwrap_or_default())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Undo a delete.
    pub async fn restore_entry(&self, owner: OwnerId, id: TimeEntryId) -> Result<EntryRecord> {
        let mut tx = self.pool.begin().await?;
        let row: Option<(String, Option<String>, String)> = sqlx::query_as(
            "SELECT date, source, deleted_at FROM time_entries \
             WHERE id = $1 AND owner_id = $2 AND deleted_at IS NOT NULL",
        )
        .bind(id.to_string())
        .bind(owner.to_string())
        .fetch_optional(&mut *tx)
        .await?;
        let (date, source, deleted_at) = row.ok_or(StorageError::NotFound)?;
        let is_tray = source.as_deref() == Some(SOURCE_TRAY);
        let group = tray_group(&mut tx, owner, id).await?;
        if is_tray
            && tray_entries(&mut tx, owner, corrupt_date(&date)?)
                .await?
                .iter()
                .any(|(g, _)| *g == group)
        {
            return Err(StorageError::Duplicate(TRAY_ENTRY_TITLE.to_string()));
        }
        let now = now_text();
        sqlx::query(
            "UPDATE time_entries SET deleted_at = NULL, updated_at = $1 WHERE id = $2 AND owner_id = $3",
        )
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        if is_tray {
            sqlx::query(
                "UPDATE ticket_tray SET deleted_at = NULL, updated_at = $1 \
                 WHERE owner_id = $2 AND date = $3 AND deleted_at = $4 \
                 AND COALESCE(project_id, '') = $5",
            )
            .bind(&now)
            .bind(owner.to_string())
            .bind(&date)
            .bind(&deleted_at)
            .bind(group.map(|p| p.to_string()).unwrap_or_default())
            .execute(&mut *tx)
            .await?;
            sync_tray(&mut tx, owner, corrupt_date(&date)?).await?;
        }
        let rec = entry_record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Paste tickets onto an entry (multi-paste; repeats skipped).
    pub async fn add_entry_tickets(
        &self,
        owner: OwnerId,
        id: TimeEntryId,
        text: &str,
    ) -> Result<EntryRecord> {
        let mut tx = self.pool.begin().await?;
        let (_, is_tray) = fetch_entry(&mut tx, owner, id).await?;
        if is_tray {
            return Err(StorageError::TrayManaged);
        }
        let existing: Vec<String> = entry_tickets(&mut tx, owner, id)
            .await?
            .into_iter()
            .map(|t| t.ticket_number)
            .collect();
        let added = add_tickets(&existing, text)?;
        insert_tickets(&mut tx, owner, id, &added).await?;
        let rec = entry_record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Tick an entry's ticket done, or mark it still open (round 14). Returns the entry.
    pub async fn set_ticket_done(
        &self,
        owner: OwnerId,
        ticket: TicketId,
        done: bool,
    ) -> Result<EntryRecord> {
        self.change_ticket(owner, ticket, Some(done), None).await
    }

    /// Write an entry ticket's notepad (blank clears it). Returns the entry.
    pub async fn set_ticket_note(
        &self,
        owner: OwnerId,
        ticket: TicketId,
        note: &str,
    ) -> Result<EntryRecord> {
        let note = clean_notes(note)?;
        self.change_ticket(owner, ticket, None, Some(note)).await
    }

    async fn change_ticket(
        &self,
        owner: OwnerId,
        ticket: TicketId,
        done: Option<bool>,
        note: Option<Option<String>>,
    ) -> Result<EntryRecord> {
        let mut tx = self.pool.begin().await?;
        let entry: Option<String> = sqlx::query_scalar(
            "SELECT time_entry_id FROM tickets WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
        )
        .bind(ticket.to_string())
        .bind(owner.to_string())
        .fetch_optional(&mut *tx)
        .await?;
        let entry = TimeEntryId::from_uuid(parse_uuid(&entry.ok_or(StorageError::NotFound)?)?);
        let (current, is_tray) = fetch_entry(&mut tx, owner, entry).await?;
        if is_tray {
            return Err(StorageError::TrayManaged);
        }
        // The ticket's item changes (round 15), so it changes everywhere the ticket appears.
        if let Some(of) = ticket_item(&mut tx, owner, &ticket.to_string()).await? {
            if let Some(done) = done {
                set_item_done(&mut tx, owner, of, done, current.date).await?;
            }
            if let Some(note) = note {
                set_item_note(&mut tx, owner, of, note.as_deref()).await?;
            }
            let rec = entry_record(&mut tx, owner, entry).await?;
            tx.commit().await?;
            return Ok(rec);
        }
        let now = now_text();
        if let Some(done) = done {
            sqlx::query(
                "UPDATE tickets SET done = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
            )
            .bind(done)
            .bind(&now)
            .bind(ticket.to_string())
            .bind(owner.to_string())
            .execute(&mut *tx)
            .await?;
        }
        if let Some(note) = note {
            sqlx::query(
                "UPDATE tickets SET note = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
            )
            .bind(note)
            .bind(&now)
            .bind(ticket.to_string())
            .bind(owner.to_string())
            .execute(&mut *tx)
            .await?;
        }
        let rec = entry_record(&mut tx, owner, entry).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Numbers of the tickets on entries (not the tray's) still open, oldest first.
    pub async fn open_entry_tickets(&self, owner: OwnerId) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar(
            "SELECT t.ticket_number FROM tickets t JOIN time_entries e ON e.id = t.time_entry_id \
             WHERE t.owner_id = $1 AND e.owner_id = $1 AND t.done = FALSE \
             AND t.deleted_at IS NULL AND e.deleted_at IS NULL AND e.source IS NULL \
             ORDER BY e.date, t.created_at",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?)
    }

    /// Remove one ticket from its entry (soft delete). Returns the entry.
    pub async fn remove_ticket(&self, owner: OwnerId, ticket: TicketId) -> Result<EntryRecord> {
        let mut tx = self.pool.begin().await?;
        let entry: Option<String> = sqlx::query_scalar(
            "SELECT time_entry_id FROM tickets WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
        )
        .bind(ticket.to_string())
        .bind(owner.to_string())
        .fetch_optional(&mut *tx)
        .await?;
        let entry = TimeEntryId::from_uuid(parse_uuid(&entry.ok_or(StorageError::NotFound)?)?);
        let (_, is_tray) = fetch_entry(&mut tx, owner, entry).await?;
        if is_tray {
            return Err(StorageError::TrayManaged);
        }
        sqlx::query(
            "UPDATE tickets SET deleted_at = $1, updated_at = $1 WHERE id = $2 AND owner_id = $3",
        )
        .bind(now_text())
        .bind(ticket.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = entry_record(&mut tx, owner, entry).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// The most recent earlier user entry carrying one of `tickets` (tray roll-ups excluded).
    async fn past_use(
        conn: &mut SqliteConnection,
        owner: OwnerId,
        tickets: &[String],
    ) -> Result<Option<PastUse>> {
        let mut best: Option<(String, String, PastUse)> = None;
        for t in tickets {
            let row = sqlx::query(
                "SELECT e.date, e.id, e.description, e.target_type, e.target_id \
                 FROM tickets t JOIN time_entries e ON e.id = t.time_entry_id \
                 WHERE t.owner_id = $1 AND e.owner_id = $1 AND lower(t.ticket_number) = lower($2) \
                 AND t.deleted_at IS NULL AND e.deleted_at IS NULL AND e.source IS NULL \
                 ORDER BY e.date DESC, e.id DESC LIMIT 1",
            )
            .bind(owner.to_string())
            .bind(t)
            .fetch_optional(&mut *conn)
            .await?;
            if let Some(row) = row {
                let date: String = row.try_get("date")?;
                let id: String = row.try_get("id")?;
                let target = EntryTarget::from_columns(
                    parse_target_type(row.try_get("target_type")?)?,
                    opt_uuid(row.try_get("target_id")?)?,
                )
                .ok_or(StorageError::Corrupt("target"))?;
                let found = PastUse {
                    description: row.try_get("description")?,
                    target,
                };
                let newer = best.as_ref().is_none_or(|(d, i, _)| (&date, &id) > (d, i));
                if newer {
                    best = Some((date, id, found));
                }
            }
        }
        Ok(best.map(|(_, _, p)| p))
    }

    /// Log work: add `minutes` on `date` following `shiftbook_core::quicklog`.
    /// The capture's own hours and day are ignored (the button and the selected
    /// day decide). Returns the entry that was created or added to.
    pub async fn quick_log(
        &self,
        owner: OwnerId,
        date: Date,
        capture: &Capture,
        minutes: Minutes,
    ) -> Result<EntryRecord> {
        let day: Vec<DayEntry> = self
            .list_entries(owner, date, date)
            .await?
            .into_iter()
            .map(|r| DayEntry {
                id: r.entry.id,
                description: r.entry.description,
                tickets: r.tickets.into_iter().map(|t| t.ticket_number).collect(),
                is_tray: r.is_tray,
            })
            .collect();
        let mut tx = self.pool.begin().await?;
        let past = Self::past_use(&mut tx, owner, &capture.tickets).await?;
        let id = match plan_quick_log(capture, minutes, &day, past.as_ref())? {
            QuickLogPlan::AddTo {
                entry,
                minutes,
                new_tickets,
            } => {
                let (current, _) = fetch_entry(&mut tx, owner, entry).await?;
                set_minutes(&mut tx, owner, entry, current.minutes + minutes).await?;
                let existing: Vec<String> = entry_tickets(&mut tx, owner, entry)
                    .await?
                    .into_iter()
                    .map(|t| t.ticket_number)
                    .collect();
                let added = add_tickets(&existing, &new_tickets.join("\n"))?;
                insert_tickets(&mut tx, owner, entry, &added).await?;
                entry
            }
            QuickLogPlan::Create {
                description,
                target,
                minutes,
                tickets,
            } => {
                let project = match target {
                    EntryTarget::Project(p) => Some(p),
                    _ => None,
                };
                let input = EntryInput {
                    date,
                    minutes,
                    description,
                    project,
                    category: None,
                    next_step: None,
                    timesheet_code_id: None,
                };
                create_entry_in(&mut tx, owner, &input, &tickets.join("\n")).await?
            }
        };
        let rec = entry_record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    // ---------- the Done tickets tray ----------

    /// One day's tray.
    pub async fn tray_day(&self, owner: OwnerId, date: Date) -> Result<TrayDay> {
        let mut conn = self.pool.acquire().await?;
        tray_day_in(&mut conn, owner, date).await
    }

    /// Paste tickets into a day's tray (multi-paste splits, repeats skipped);
    /// each gets 15 minutes and starts still open (`NEW_TICKET_DONE`). A ticket seen on an
    /// earlier day keeps its notepad and project. The roll-ups follow.
    pub async fn tray_add(&self, owner: OwnerId, date: Date, text: &str) -> Result<TrayDay> {
        let mut tx = self.pool.begin().await?;
        let existing: Vec<String> = tray_items(&mut tx, owner, date)
            .await?
            .into_iter()
            .map(|i| i.ticket_number)
            .collect();
        let added = new_tray_tickets(&existing, text)?;
        for t in &added {
            insert_tray_row(&mut tx, owner, date, t, NEW_TICKET_DONE).await?;
        }
        sync_tray(&mut tx, owner, date).await?;
        let day = tray_day_in(&mut tx, owner, date).await?;
        tx.commit().await?;
        Ok(day)
    }

    /// Live Done tickets from `from` to `to` (inclusive), by date.
    pub async fn tray_between(
        &self,
        owner: OwnerId,
        from: Date,
        to: Date,
    ) -> Result<Vec<TrayItem>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            tray_columns!(),
            " FROM ticket_tray WHERE owner_id = $1 AND date >= $2 AND date <= $3 \
             AND deleted_at IS NULL ORDER BY date, id"
        ))
        .bind(owner.to_string())
        .bind(format_date(from))
        .bind(format_date(to))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(tray_from_row).collect()
    }

    /// One live Done ticket, or None.
    pub async fn tray_item(&self, owner: OwnerId, id: TrayItemId) -> Result<Option<TrayItem>> {
        let row = sqlx::query(concat!(
            "SELECT ",
            tray_columns!(),
            " FROM ticket_tray WHERE owner_id = $1 AND id = $2 AND deleted_at IS NULL"
        ))
        .bind(owner.to_string())
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(tray_from_row).transpose()
    }

    /// Change one tray ticket's minutes (0 to 480).
    pub async fn tray_set_minutes(
        &self,
        owner: OwnerId,
        id: TrayItemId,
        minutes: u32,
    ) -> Result<TrayDay> {
        let minutes = validate_tray_minutes(minutes)?;
        let mut tx = self.pool.begin().await?;
        let date = tray_item_date(&mut tx, owner, id, true).await?;
        sqlx::query(
            "UPDATE ticket_tray SET minutes = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(i64::from(minutes.get()))
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        sync_tray(&mut tx, owner, date).await?;
        let day = tray_day_in(&mut tx, owner, date).await?;
        tx.commit().await?;
        Ok(day)
    }

    /// Set or clear a tray ticket's notepad (the same notepad on every day it's on).
    pub async fn tray_set_note(
        &self,
        owner: OwnerId,
        id: TrayItemId,
        note: &str,
    ) -> Result<TrayDay> {
        let note = clean_tray_note(note)?;
        let mut tx = self.pool.begin().await?;
        let date = tray_item_date(&mut tx, owner, id, true).await?;
        match tray_row_item(&mut tx, owner, &id.to_string()).await? {
            // One notepad per ticket (round 15): it shows wherever the ticket does.
            Some(of) => set_item_note(&mut tx, owner, of, note.as_deref()).await?,
            None => {
                sqlx::query(
                    "UPDATE ticket_tray SET note = $1, updated_at = $2 WHERE id = $3 \
                     AND owner_id = $4",
                )
                .bind(note)
                .bind(now_text())
                .bind(id.to_string())
                .bind(owner.to_string())
                .execute(&mut *tx)
                .await?;
            }
        }
        let day = tray_day_in(&mut tx, owner, date).await?;
        tx.commit().await?;
        Ok(day)
    }

    /// Tick a tray ticket done, or untick it to keep it open. It's its item that changes
    /// (round 15): ticking finishes the ticket everywhere it appears, unticking reopens it
    /// from this day on.
    pub async fn tray_set_done(
        &self,
        owner: OwnerId,
        id: TrayItemId,
        done: bool,
    ) -> Result<TrayDay> {
        let mut tx = self.pool.begin().await?;
        let item = live_tray_item(&mut tx, owner, id).await?;
        match tray_row_item(&mut tx, owner, &id.to_string()).await? {
            Some(of) => set_item_done(&mut tx, owner, of, done, item.date).await?,
            None => {
                sqlx::query(
                    "UPDATE ticket_tray SET done = $1, updated_at = $2 WHERE id = $3 \
                     AND owner_id = $4",
                )
                .bind(done)
                .bind(now_text())
                .bind(id.to_string())
                .bind(owner.to_string())
                .execute(&mut *tx)
                .await?;
            }
        }
        if done {
            // A ticket ticked done joins the Desk's "Done today" pile, so it gives up the spot
            // it had (unless it's filed in a drawer).
            sqlx::query(
                "UPDATE canvas_cards SET deleted_at = $1, updated_at = $1 \
                 WHERE owner_id = $2 AND record_kind = 'tray' AND record_id = $3 \
                 AND drawer IS NULL AND deleted_at IS NULL",
            )
            .bind(now_text())
            .bind(owner.to_string())
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        }
        let day = tray_day_in(&mut tx, owner, item.date).await?;
        tx.commit().await?;
        Ok(day)
    }

    /// Give a tray ticket a project (its minutes then count under it), or `None` to put
    /// it back in the day's plain "Tickets" entry.
    pub async fn tray_set_project(
        &self,
        owner: OwnerId,
        id: TrayItemId,
        project: Option<ProjectId>,
    ) -> Result<TrayDay> {
        let mut tx = self.pool.begin().await?;
        if let Some(p) = project {
            project_exists(&mut tx, owner, p).await?;
        }
        let date = tray_item_date(&mut tx, owner, id, true).await?;
        sqlx::query(
            "UPDATE ticket_tray SET project_id = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(project.map(|p| p.to_string()))
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        // The ticket's next days start with this project; days already logged keep theirs.
        if let Some(of) = tray_row_item(&mut tx, owner, &id.to_string()).await? {
            set_item_project(&mut tx, owner, of, project).await?;
        }
        sync_tray(&mut tx, owner, date).await?;
        let day = tray_day_in(&mut tx, owner, date).await?;
        tx.commit().await?;
        Ok(day)
    }

    /// Still-open tickets from before `date` that come back in its tray (latest row per
    /// ticket, oldest first), leaving out tickets already in that day's tray.
    pub async fn tray_carried(&self, owner: OwnerId, date: Date) -> Result<Vec<TrayItem>> {
        let mut conn = self.pool.acquire().await?;
        let rows = sqlx::query(concat!(
            "SELECT ",
            tray_columns!(),
            " FROM ticket_tray WHERE owner_id = $1 AND done = FALSE AND date < $2 \
             AND deleted_at IS NULL ORDER BY date, id"
        ))
        .bind(owner.to_string())
        .bind(format_date(date))
        .fetch_all(&mut *conn)
        .await?;
        let open: Vec<TrayItem> = rows.iter().map(tray_from_row).collect::<Result<_>>()?;
        let today: Vec<String> = tray_items(&mut conn, owner, date)
            .await?
            .into_iter()
            .map(|i| i.ticket_number)
            .collect();
        let pairs: Vec<(&str, Date)> = open
            .iter()
            .map(|i| (i.ticket_number.as_str(), i.date))
            .collect();
        Ok(carried_open(&pairs, &today, date)
            .into_iter()
            .map(|i| open[i].clone())
            .collect())
    }

    /// Carry a still-open ticket into `date`'s tray: a new open row with 15 minutes, its
    /// notepad and its project. Already there: nothing changes.
    pub async fn tray_carry(&self, owner: OwnerId, id: TrayItemId, date: Date) -> Result<TrayDay> {
        let mut tx = self.pool.begin().await?;
        let item = live_tray_item(&mut tx, owner, id).await?;
        let there = tray_items(&mut tx, owner, date)
            .await?
            .iter()
            .any(|i| i.ticket_number.eq_ignore_ascii_case(&item.ticket_number));
        if !there {
            insert_tray_row(&mut tx, owner, date, &item.ticket_number, false).await?;
            sync_tray(&mut tx, owner, date).await?;
        }
        let day = tray_day_in(&mut tx, owner, date).await?;
        tx.commit().await?;
        Ok(day)
    }

    /// Take a ticket out of the tray (soft delete).
    pub async fn tray_remove(&self, owner: OwnerId, id: TrayItemId) -> Result<TrayDay> {
        self.tray_set_deleted(owner, id, true).await
    }

    /// Undo [`Self::tray_remove`].
    pub async fn tray_restore(&self, owner: OwnerId, id: TrayItemId) -> Result<TrayDay> {
        self.tray_set_deleted(owner, id, false).await
    }

    async fn tray_set_deleted(
        &self,
        owner: OwnerId,
        id: TrayItemId,
        delete: bool,
    ) -> Result<TrayDay> {
        let mut tx = self.pool.begin().await?;
        let date = tray_item_date(&mut tx, owner, id, delete).await?;
        let now = now_text();
        sqlx::query(
            "UPDATE ticket_tray SET deleted_at = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(if delete { Some(now.as_str()) } else { None })
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref d) if d.is_unique_violation() => {
                StorageError::Duplicate(String::from("That ticket"))
            }
            other => StorageError::Database(other),
        })?;
        sync_tray(&mut tx, owner, date).await?;
        let day = tray_day_in(&mut tx, owner, date).await?;
        tx.commit().await?;
        Ok(day)
    }
}
