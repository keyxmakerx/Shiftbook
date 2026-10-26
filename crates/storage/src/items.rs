//! Items (round 15, step 1): one row per ticket number, which tray rows and entry tickets
//! point at. See `shiftbook_core::items` for the rules. Everything here runs inside the
//! caller's transaction; tray rows and entry tickets keep a copy of the item's done tick and
//! notepad, so the code that reads them is unchanged.
//!
//! A day's "Tickets" roll-up entry only mirrors the tray, so its tickets have no item.

use std::collections::HashMap;

use shiftbook_core::ids::{ItemId, OwnerId, ProjectId, RecurringTaskId};
use shiftbook_core::items::{reopens, seed_item, ItemKind, Occurrence};
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::EntryTarget;
use shiftbook_core::samework::{ticket_so_far, SoFar};
use sqlx::sqlite::SqliteConnection;
use sqlx::Row;
use time::Date;

use crate::repo::{now_text, parse_opt_ts, parse_ts, parse_uuid, Result};
use crate::work::{corrupt_date, opt_uuid};

/// What a row copies from its item when it's made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemState {
    /// The item.
    pub id: ItemId,
    /// Its notepad.
    pub note: Option<String>,
    /// The project it was last given.
    pub project: Option<ProjectId>,
    /// The repeating thing it was last filed under (round 15), when it has no project.
    pub recurring: Option<RecurringTaskId>,
}

/// The item for a ticket number (ignoring case), made if there is none yet.
pub(crate) async fn item_for_number(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    number: &str,
) -> Result<ItemState> {
    let row = sqlx::query(
        "SELECT id, note, project_id, recurring_id FROM items WHERE owner_id = $1 \
         AND lower(number) = lower($2) AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .bind(number)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = row {
        return Ok(ItemState {
            id: ItemId::from_uuid(parse_uuid(row.try_get("id")?)?),
            note: row.try_get("note")?,
            project: opt_uuid(row.try_get("project_id")?)?.map(ProjectId::from_uuid),
            recurring: opt_uuid(row.try_get("recurring_id")?)?.map(RecurringTaskId::from_uuid),
        });
    }
    let id = ItemId::new();
    insert_item(conn, owner, id, number, false, None, None).await?;
    Ok(ItemState {
        id,
        note: None,
        project: None,
        recurring: None,
    })
}

async fn insert_item(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: ItemId,
    number: &str,
    done: bool,
    note: Option<&str>,
    project: Option<ProjectId>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO items (id, owner_id, kind, number, title, done, note, project_id, \
         created_at, updated_at, deleted_at) VALUES ($1, $2, $3, $4, '', $5, $6, $7, $8, $8, NULL)",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .bind(ItemKind::Ticket.as_str())
    .bind(number)
    .bind(done)
    .bind(note)
    .bind(project.map(|p| p.to_string()))
    .bind(now_text())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// The item a tray row points at, if it has one.
pub(crate) async fn tray_row_item(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: &str,
) -> Result<Option<ItemId>> {
    let item: Option<Option<String>> =
        sqlx::query_scalar("SELECT item_id FROM ticket_tray WHERE id = $1 AND owner_id = $2")
            .bind(id)
            .bind(owner.to_string())
            .fetch_optional(&mut *conn)
            .await?;
    Ok(opt_uuid(item.flatten())?.map(ItemId::from_uuid))
}

/// The item an entry's ticket points at, if it has one.
pub(crate) async fn ticket_item(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: &str,
) -> Result<Option<ItemId>> {
    let item: Option<Option<String>> =
        sqlx::query_scalar("SELECT item_id FROM tickets WHERE id = $1 AND owner_id = $2")
            .bind(id)
            .bind(owner.to_string())
            .fetch_optional(&mut *conn)
            .await?;
    Ok(opt_uuid(item.flatten())?.map(ItemId::from_uuid))
}

/// Tick an item done (every occurrence finishes) or reopen it on `from` (occurrences from
/// that day on reopen; see `shiftbook_core::items::reopens`).
pub(crate) async fn set_item_done(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    item: ItemId,
    done: bool,
    from: Date,
) -> Result<()> {
    let now = now_text();
    sqlx::query("UPDATE items SET done = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4")
        .bind(done)
        .bind(&now)
        .bind(item.to_string())
        .bind(owner.to_string())
        .execute(&mut *conn)
        .await?;
    let tray: Vec<(String, String, bool)> = sqlx::query_as(
        "SELECT id, date, done FROM ticket_tray WHERE owner_id = $1 AND item_id = $2",
    )
    .bind(owner.to_string())
    .bind(item.to_string())
    .fetch_all(&mut *conn)
    .await?;
    for (id, date, was) in tray {
        if was != done && (done || reopens(corrupt_date(&date)?, from)) {
            sqlx::query(
                "UPDATE ticket_tray SET done = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
            )
            .bind(done)
            .bind(&now)
            .bind(&id)
            .bind(owner.to_string())
            .execute(&mut *conn)
            .await?;
        }
    }
    let tickets: Vec<(String, String, bool)> = sqlx::query_as(
        "SELECT t.id, e.date, t.done FROM tickets t JOIN time_entries e ON e.id = t.time_entry_id \
         WHERE t.owner_id = $1 AND e.owner_id = $1 AND t.item_id = $2",
    )
    .bind(owner.to_string())
    .bind(item.to_string())
    .fetch_all(&mut *conn)
    .await?;
    for (id, date, was) in tickets {
        if was != done && (done || reopens(corrupt_date(&date)?, from)) {
            sqlx::query(
                "UPDATE tickets SET done = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
            )
            .bind(done)
            .bind(&now)
            .bind(&id)
            .bind(owner.to_string())
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(())
}

/// Write an item's notepad, and its copy on every row that points at it.
pub(crate) async fn set_item_note(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    item: ItemId,
    note: Option<&str>,
) -> Result<()> {
    let now = now_text();
    for table in [
        "UPDATE items SET note = $1, updated_at = $2 WHERE owner_id = $3 AND id = $4",
        "UPDATE ticket_tray SET note = $1, updated_at = $2 WHERE owner_id = $3 AND item_id = $4",
        "UPDATE tickets SET note = $1, updated_at = $2 WHERE owner_id = $3 AND item_id = $4",
    ] {
        sqlx::query(table)
            .bind(note)
            .bind(&now)
            .bind(owner.to_string())
            .bind(item.to_string())
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Remember the project or repeating thing an item was last given: its next tray rows
/// start with it.
pub(crate) async fn set_item_home(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    item: ItemId,
    home: Option<EntryTarget>,
) -> Result<()> {
    let (project, recurring) = match home {
        Some(EntryTarget::Project(p)) => (Some(p.to_string()), None),
        Some(EntryTarget::Recurring(r)) => (None, Some(r.to_string())),
        _ => (None, None),
    };
    sqlx::query(
        "UPDATE items SET project_id = $1, recurring_id = $2, updated_at = $3 \
         WHERE id = $4 AND owner_id = $5",
    )
    .bind(project)
    .bind(recurring)
    .bind(now_text())
    .bind(item.to_string())
    .bind(owner.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// One row with no item yet.
struct Loose {
    tray: bool,
    id: String,
    number: String,
    date: Date,
    updated_at: time::OffsetDateTime,
    live: bool,
    done: bool,
    note: Option<String>,
    project: Option<ProjectId>,
}

/// Link every tray row and entry ticket that has no item yet (rows from before items, or
/// brought back by a restore). A number with no item gets one, seeded from its rows; when the
/// new item is done, its rows that were still open are finished too, and every row takes the
/// merged notepad. A number that already has an item just joins it. Safe to run again.
pub(crate) async fn link_items(conn: &mut SqliteConnection, owner: OwnerId) -> Result<()> {
    let rows = sqlx::query(
        "SELECT 1 AS tray, id, ticket_number, date, updated_at, deleted_at, done, note, \
         project_id FROM ticket_tray WHERE owner_id = $1 AND item_id IS NULL \
         UNION ALL \
         SELECT 0 AS tray, t.id, t.ticket_number, e.date, t.updated_at, \
         COALESCE(t.deleted_at, e.deleted_at) AS deleted_at, t.done, t.note, NULL AS project_id \
         FROM tickets t JOIN time_entries e ON e.id = t.time_entry_id \
         WHERE t.owner_id = $1 AND e.owner_id = $1 AND t.item_id IS NULL AND e.source IS NULL",
    )
    .bind(owner.to_string())
    .fetch_all(&mut *conn)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let mut groups: HashMap<String, Vec<Loose>> = HashMap::new();
    for row in &rows {
        let number: String = row.try_get("ticket_number")?;
        let tray: i64 = row.try_get("tray")?;
        let loose = Loose {
            tray: tray == 1,
            id: row.try_get("id")?,
            date: corrupt_date(row.try_get("date")?)?,
            updated_at: parse_ts(row.try_get("updated_at")?)?,
            live: parse_opt_ts(row.try_get("deleted_at")?)?.is_none(),
            done: row.try_get("done")?,
            note: row.try_get("note")?,
            project: opt_uuid(row.try_get("project_id")?)?.map(ProjectId::from_uuid),
            number,
        };
        groups
            .entry(loose.number.to_lowercase())
            .or_default()
            .push(loose);
    }
    let mut keys: Vec<String> = groups.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let group = &groups[&key];
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM items WHERE owner_id = $1 AND lower(number) = lower($2) \
             AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(&group[0].number)
        .fetch_optional(&mut *conn)
        .await?;
        let (item, seeded) = match existing {
            Some(id) => (ItemId::from_uuid(parse_uuid(&id)?), None),
            None => {
                let occurrences: Vec<Occurrence<'_>> = group
                    .iter()
                    .map(|r| Occurrence {
                        date: r.date,
                        updated_at: r.updated_at,
                        live: r.live,
                        done: r.done,
                        note: r.note.as_deref(),
                        project: r.tray.then_some(r.project),
                    })
                    .collect();
                let seed = seed_item(&occurrences);
                // The number as written on its newest row.
                let newest = group
                    .iter()
                    .max_by_key(|r| (r.date, r.updated_at))
                    .map_or(group[0].number.as_str(), |r| r.number.as_str());
                let id = ItemId::new();
                insert_item(
                    conn,
                    owner,
                    id,
                    newest,
                    seed.done,
                    seed.note.as_deref(),
                    seed.project,
                )
                .await?;
                (id, Some(seed))
            }
        };
        for r in group {
            let sql = if r.tray {
                "UPDATE ticket_tray SET item_id = $1 WHERE id = $2 AND owner_id = $3"
            } else {
                "UPDATE tickets SET item_id = $1 WHERE id = $2 AND owner_id = $3"
            };
            sqlx::query(sql)
                .bind(item.to_string())
                .bind(&r.id)
                .bind(owner.to_string())
                .execute(&mut *conn)
                .await?;
        }
        if let Some(seed) = seeded {
            if seed.done {
                let last = group.iter().map(|r| r.date).max().unwrap_or(Date::MIN);
                set_item_done(conn, owner, item, true, last).await?;
            }
            set_item_note(conn, owner, item, seed.note.as_deref()).await?;
        }
    }
    Ok(())
}

/// A ticket's item with its time so far (round 15, card r15-things): for a known ticket in
/// Log work and the tray, and for the Items view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    /// The item.
    pub id: ItemId,
    /// The ticket number as first written.
    pub number: String,
    /// The latest title of work it was on ("" when only in the tray).
    pub title: String,
    /// Ticked done.
    pub done: bool,
    /// Its notepad.
    pub note: Option<String>,
    /// The project it was last given.
    pub project: Option<ProjectId>,
    /// The repeating thing it was last filed under, when it has no project.
    pub recurring: Option<RecurringTaskId>,
    /// When it was worked and how long, if it's on anything.
    pub so_far: Option<SoFar>,
    /// The cards it's on, latest first: `tray:id` for tray rows, `entry:id` for entries.
    pub cards: Vec<String>,
}

impl crate::SqliteStore {
    /// Every live ticket item with its time so far; with `numbers`, only those tickets
    /// (ignoring case).
    pub async fn item_summaries(
        &self,
        owner: OwnerId,
        numbers: Option<&[String]>,
    ) -> Result<Vec<ItemSummary>> {
        let mut conn = self.pool.acquire().await?;
        let rows = sqlx::query(
            "SELECT id, number, done, note, project_id, recurring_id FROM items \
             WHERE owner_id = $1 AND kind = 'ticket' AND number IS NOT NULL \
             AND deleted_at IS NULL ORDER BY created_at, id",
        )
        .bind(owner.to_string())
        .fetch_all(&mut *conn)
        .await?;
        let wanted = |n: &str| {
            numbers.is_none_or(|list| list.iter().any(|w| w.trim().eq_ignore_ascii_case(n)))
        };
        let mut items: Vec<ItemSummary> = Vec::new();
        for row in &rows {
            let number: String = row.try_get("number")?;
            if !wanted(&number) {
                continue;
            }
            items.push(ItemSummary {
                id: ItemId::from_uuid(parse_uuid(row.try_get("id")?)?),
                number,
                title: String::new(),
                done: row.try_get("done")?,
                note: row.try_get("note")?,
                project: opt_uuid(row.try_get("project_id")?)?.map(ProjectId::from_uuid),
                recurring: opt_uuid(row.try_get("recurring_id")?)?.map(RecurringTaskId::from_uuid),
                so_far: None,
                cards: Vec::new(),
            });
        }
        if items.is_empty() {
            return Ok(items);
        }
        // (item, date, minutes, title, card): tray rows, then entries (not the roll-ups,
        // which only mirror the tray).
        let worked: Vec<(String, String, i64, String, String)> = sqlx::query_as(
            "SELECT item_id, date, minutes, '', 'tray:' || id FROM ticket_tray \
             WHERE owner_id = $1 AND item_id IS NOT NULL AND deleted_at IS NULL \
             UNION ALL \
             SELECT t.item_id, e.date, e.minutes, e.description, 'entry:' || e.id FROM tickets t \
             JOIN time_entries e ON e.id = t.time_entry_id \
             WHERE t.owner_id = $1 AND e.owner_id = $1 AND t.item_id IS NOT NULL \
             AND t.deleted_at IS NULL AND e.deleted_at IS NULL \
             AND COALESCE(e.source, '') <> 'tray'",
        )
        .bind(owner.to_string())
        .fetch_all(&mut *conn)
        .await?;
        // Per item: (day, minutes) worked, the latest (day, title), and (day, card).
        type Worked = (
            Vec<(Date, Minutes)>,
            Option<(Date, String)>,
            Vec<(Date, String)>,
        );
        let mut by_item: HashMap<String, Worked> = HashMap::new();
        for (item, date, minutes, title, card) in worked {
            let date = corrupt_date(&date)?;
            let slot = by_item.entry(item).or_default();
            slot.0
                .push((date, Minutes(u32::try_from(minutes).unwrap_or(0))));
            slot.2.push((date, card));
            if !title.trim().is_empty() && slot.1.as_ref().is_none_or(|(d, _)| date >= *d) {
                slot.1 = Some((date, title));
            }
        }
        for item in &mut items {
            if let Some((rows, title, cards)) = by_item.get_mut(&item.id.to_string()) {
                item.so_far = ticket_so_far(rows);
                cards.sort_by_key(|c| std::cmp::Reverse(c.0));
                item.cards = cards.iter().map(|(_, c)| c.clone()).collect();
                item.title = title.as_ref().map(|(_, t)| t.clone()).unwrap_or_default();
            }
        }
        Ok(items)
    }

    /// The Items view's tick: finish a ticket everywhere, or reopen it from `from` on.
    pub async fn item_set_done(
        &self,
        owner: OwnerId,
        item: ItemId,
        done: bool,
        from: Date,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM items WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
        )
        .bind(item.to_string())
        .bind(owner.to_string())
        .fetch_one(&mut *tx)
        .await?;
        if n == 0 {
            return Err(crate::StorageError::NotFound);
        }
        set_item_done(&mut tx, owner, item, done, from).await?;
        tx.commit().await?;
        Ok(())
    }
}
