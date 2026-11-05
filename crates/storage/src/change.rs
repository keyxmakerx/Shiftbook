//! MOPs on their change ticket (see `shiftbook_core::change`): attaching one to a ticket from
//! a template, a wiki page or blank, its status, change window and tick times. Owner-scoped,
//! bound parameters only, soft deletes.

use std::collections::HashMap;

use shiftbook_core::change::{join_tick_times, parse_tick_times, tick_times, MopStatus, TickTimes};
use shiftbook_core::dates::{format_date, parse_date};
use shiftbook_core::ids::{NoteId, OwnerId, ProjectId};
use shiftbook_core::model::Note;
use shiftbook_core::mop::{clean_values, fill_blanks, filled_title};
use shiftbook_core::notes::{join_tags, NoteKind};
use shiftbook_core::tray::new_tray_tickets;
use shiftbook_core::CoreError;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::error::StorageError;
use crate::items::item_for_number;
use crate::notes::NoteInput;
use crate::repo::{now_text, parse_opt_ts, parse_ts, parse_uuid, Result};
use crate::work::opt_uuid;
use crate::SqliteStore;

/// Latest start of a change window, minutes after midnight.
const LAST_MINUTE: u32 = 24 * 60 - 1;

/// What a new MOP starts from.
#[derive(Debug, Clone)]
pub enum MopFrom {
    /// A MOP template, with what's typed into its blanks (the ticket and title fill
    /// themselves when left out) and an optional title.
    Template {
        /// The template's note id.
        id: NoteId,
        /// Blank name and value pairs.
        values: Vec<(String, String)>,
        /// A title; blank uses the template's, filled in.
        title: String,
    },
    /// A wiki page, copied in so the page stays as it is.
    Page(NoteId),
    /// Nothing yet, with this title.
    Blank(String),
}

/// A MOP on a ticket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MopRun {
    /// The MOP's note id.
    pub mop: Uuid,
    /// Its title.
    pub title: String,
    /// The ticket it's on.
    pub ticket: String,
    /// The template it was made from, if any.
    pub template: Option<Uuid>,
    /// Where it stands.
    pub status: MopStatus,
    /// The change window's day and start (minutes after midnight), if set.
    pub window: Option<(Date, u32)>,
    /// When each ticked line was ticked.
    pub tick_times: TickTimes,
    /// When it first went Running.
    pub started_at: Option<OffsetDateTime>,
}

fn run_from_row(row: &SqliteRow) -> Result<MopRun> {
    let status: String = row.try_get("status")?;
    let on: Option<String> = row.try_get("window_on")?;
    let at: Option<i64> = row.try_get("window_at")?;
    let window = match (on, at) {
        (Some(on), Some(at)) => Some((
            parse_date(&on).map_err(|_| StorageError::Corrupt("date"))?,
            u32::try_from(at).unwrap_or(0).min(LAST_MINUTE),
        )),
        _ => None,
    };
    Ok(MopRun {
        mop: parse_uuid(row.try_get("id")?)?,
        title: row.try_get("title")?,
        ticket: row.try_get("number")?,
        template: opt_uuid(row.try_get("template_id")?)?,
        status: status
            .parse()
            .map_err(|_| StorageError::Corrupt("MOP status"))?,
        window,
        tick_times: parse_tick_times(&row.try_get::<String, _>("tick_times")?),
        started_at: parse_opt_ts(row.try_get("started_at")?)?,
    })
}

macro_rules! run_select {
    () => {
        "SELECT r.id, n.title, i.number, r.template_id, r.status, r.window_on, \
         r.window_at, r.tick_times, r.started_at FROM mop_runs r \
         JOIN notes n ON n.id = r.id JOIN items i ON i.id = r.item_id \
         WHERE r.owner_id = $1 AND r.deleted_at IS NULL AND n.deleted_at IS NULL"
    };
}

impl SqliteStore {
    /// Make a MOP and attach it to `ticket` (matched ignoring case).
    pub async fn attach_mop(&self, owner: OwnerId, ticket: &str, from: MopFrom) -> Result<Note> {
        let item: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, number, project_id FROM items WHERE owner_id = $1 \
             AND lower(number) = lower($2) AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(ticket.trim())
        .fetch_optional(&self.pool)
        .await?;
        let (item, number, project) = item.ok_or(StorageError::NotFound)?;
        let project = opt_uuid(project)?.map(ProjectId::from_uuid);
        let (input, template) = match from {
            MopFrom::Template { id, values, title } => {
                let tpl = self.get_note(owner, id).await?;
                if !(tpl.kind == NoteKind::Mop && tpl.template) {
                    return Err(CoreError::NotAChoice { value: tpl.title }.into());
                }
                let mut values = clean_values(&values)?;
                // The ticket and its title fill themselves unless typed.
                for (name, value) in [
                    ("ticket", number.clone()),
                    ("title", title.trim().to_string()),
                ] {
                    let typed = values
                        .iter()
                        .any(|(n, v)| n.eq_ignore_ascii_case(name) && !v.is_empty());
                    if !typed && !value.is_empty() {
                        values.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
                        values.push((name.to_string(), value));
                    }
                }
                let input = NoteInput {
                    kind: NoteKind::Mop,
                    title: filled_title("", &tpl.title, &values),
                    body: fill_blanks(&tpl.body, &values),
                    project: project.or(tpl.project_id),
                    tags: join_tags(&tpl.tags),
                    template: false,
                };
                (input, Some(*id.as_uuid()))
            }
            MopFrom::Page(id) => {
                let page = self.get_note(owner, id).await?;
                let input = NoteInput {
                    kind: NoteKind::Mop,
                    title: format!("{} · {}", number, page.title),
                    body: page.body,
                    project,
                    tags: String::new(),
                    template: false,
                };
                (input, None)
            }
            MopFrom::Blank(title) => {
                let title = if title.trim().is_empty() {
                    format!("MOP for {number}")
                } else {
                    title
                };
                let input = NoteInput {
                    kind: NoteKind::Mop,
                    title,
                    body: String::new(),
                    project,
                    tags: String::new(),
                    template: false,
                };
                (input, None)
            }
        };
        let note = self.create_note(owner, &input).await?;
        sqlx::query(
            "INSERT INTO mop_runs (id, owner_id, item_id, template_id, status, window_on, \
             window_at, tick_times, started_at, created_at, updated_at, deleted_at) \
             VALUES ($1, $2, $3, $4, 'draft', NULL, NULL, '', NULL, $5, $5, NULL)",
        )
        .bind(note.id.to_string())
        .bind(owner.to_string())
        .bind(item)
        .bind(template.map(|t| t.to_string()))
        .bind(now_text())
        .execute(&self.pool)
        .await?;
        Ok(note)
    }

    /// Put a MOP written on its own (in Notes, before its ticket existed) on `ticket`. The
    /// ticket is made if it isn't there yet. A MOP already on a ticket: refused.
    pub async fn link_mop(&self, owner: OwnerId, mop: NoteId, ticket: &str) -> Result<()> {
        let note = self.get_note(owner, mop).await?;
        if note.kind != NoteKind::Mop || note.template {
            return Err(CoreError::NotAChoice { value: note.title }.into());
        }
        if self.mop_run(owner, mop).await?.is_some() {
            return Err(CoreError::NotAChoice {
                value: "already on a ticket".into(),
            }
            .into());
        }
        let number = match new_tray_tickets::<&str>(&[], ticket)?.as_slice() {
            [one] => one.clone(),
            _ => {
                return Err(CoreError::NotAChoice {
                    value: ticket.chars().take(30).collect(),
                }
                .into())
            }
        };
        let mut tx = self.write_tx().await?;
        let item = item_for_number(&mut tx, owner, &number).await?;
        let now = now_text();
        sqlx::query(
            "INSERT INTO mop_runs (id, owner_id, item_id, template_id, status, window_on, \
             window_at, tick_times, started_at, created_at, updated_at, deleted_at) \
             VALUES ($1, $2, $3, NULL, 'draft', NULL, NULL, '', NULL, $4, $4, NULL) \
             ON CONFLICT (id) DO UPDATE SET item_id = $3, deleted_at = NULL, updated_at = $4",
        )
        .bind(mop.to_string())
        .bind(owner.to_string())
        .bind(item.id.to_string())
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// MOPs on tickets, newest first; with `ticket`, only that ticket's (ignoring case).
    pub async fn mop_runs(&self, owner: OwnerId, ticket: Option<&str>) -> Result<Vec<MopRun>> {
        let rows = sqlx::query(concat!(
            run_select!(),
            " AND ($2 IS NULL OR lower(i.number) = lower($2)) \
             ORDER BY r.created_at DESC, r.id DESC"
        ))
        .bind(owner.to_string())
        .bind(ticket.map(str::trim))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(run_from_row).collect()
    }

    /// One MOP's run (`None` for a note that isn't a MOP on a ticket).
    pub async fn mop_run(&self, owner: OwnerId, mop: NoteId) -> Result<Option<MopRun>> {
        let row = sqlx::query(concat!(run_select!(), " AND r.id = $2"))
            .bind(owner.to_string())
            .bind(mop.to_string())
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(run_from_row).transpose()
    }

    /// Set where a MOP stands. The first time it goes Running, its start is kept.
    pub async fn set_mop_status(
        &self,
        owner: OwnerId,
        mop: NoteId,
        status: MopStatus,
    ) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE mop_runs SET status = $1, updated_at = $2, \
             started_at = CASE WHEN $1 = 'running' AND started_at IS NULL THEN $2 ELSE started_at END \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(status.as_str())
        .bind(&now)
        .bind(mop.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Set (or clear, with `None`) a MOP's change window: its day and start.
    pub async fn set_mop_window(
        &self,
        owner: OwnerId,
        mop: NoteId,
        window: Option<(Date, u32)>,
    ) -> Result<()> {
        if window.is_some_and(|(_, at)| at > LAST_MINUTE) {
            return Err(CoreError::SettingInvalid {
                value: "time".into(),
            }
            .into());
        }
        let done = sqlx::query(
            "UPDATE mop_runs SET window_on = $1, window_at = $2, updated_at = $3 \
             WHERE id = $4 AND owner_id = $5 AND deleted_at IS NULL",
        )
        .bind(window.map(|(on, _)| format_date(on)))
        .bind(window.map(|(_, at)| i64::from(at)))
        .bind(now_text())
        .bind(mop.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Tick a MOP's lines off (like Checklist mode), keeping when each line was ticked.
    pub async fn tick_mop(&self, owner: OwnerId, mop: NoteId, ticks: &[String]) -> Result<Note> {
        let run = self
            .mop_run(owner, mop)
            .await?
            .ok_or(StorageError::NotFound)?;
        let note = self.set_note_ticks(owner, mop, ticks).await?;
        let now = parse_ts(&now_text())?;
        let times = tick_times(&run.tick_times, &note.ticks, now);
        sqlx::query(
            "UPDATE mop_runs SET tick_times = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(join_tick_times(&times))
        .bind(now_text())
        .bind(mop.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        Ok(note)
    }

    /// When each template was last used for a MOP.
    pub async fn templates_last_used(
        &self,
        owner: OwnerId,
    ) -> Result<HashMap<Uuid, OffsetDateTime>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT template_id, MAX(created_at) FROM mop_runs WHERE owner_id = $1 \
             AND template_id IS NOT NULL AND deleted_at IS NULL GROUP BY template_id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|(id, at)| Ok((parse_uuid(id)?, parse_ts(at)?)))
            .collect()
    }
}
