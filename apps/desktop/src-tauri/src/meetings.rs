//! Commands for meeting series and the bell (round 12). Thin: the rules live in
//! `shiftbook_core::meetings` and `shiftbook_core::bell`.

use serde::{Deserialize, Serialize};
use shiftbook_core::bell::{bell_items, timesheet_item, with_due, BellItem};
use shiftbook_core::canvas::CardKind;
use shiftbook_core::dates::parse_date;
use shiftbook_core::ids::{MeetingSeriesId, OwnerId, ProjectId};
use shiftbook_core::meetings::meeting_note_title;
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::MeetingSeries;
use shiftbook_core::notes::NoteKind;
use shiftbook_core::reminder::{last_work_day, timesheet_due, uncoded_entries, week_entered};
use shiftbook_core::week::week_of;
use shiftbook_storage::meetings::SeriesInput;
use shiftbook_storage::notes::NoteInput;
use shiftbook_storage::work::EntryInput;
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::OffsetDateTime;

use crate::api::{date_arg, one_entry, EntryView};
use crate::notes::{note_view, NoteView};
use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

fn id_arg(text: &str) -> Res<MeetingSeriesId> {
    text.parse().map_err(|_| CommandError::InvalidInput)
}

/// One meeting series.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SeriesView {
    id: MeetingSeriesId,
    name: String,
    #[cfg_attr(feature = "ts", ts(type = "boolean[]"))]
    days: [bool; 7],
    at: u32,
    remind_minutes: u32,
    length_minutes: u32,
    project_id: Option<ProjectId>,
}

fn series_view(m: MeetingSeries) -> SeriesView {
    SeriesView {
        id: m.id,
        name: m.name,
        days: m.days,
        at: m.at,
        remind_minutes: m.remind_minutes,
        length_minutes: m.length_minutes,
        project_id: m.project_id,
    }
}

/// What the UI sends to save a series.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesArgs {
    name: String,
    days: [bool; 7],
    at: u32,
    remind_minutes: u32,
    length_minutes: u32,
    project_id: Option<String>,
}

/// Maps to `DataApi.meetingSeries()`.
#[tauri::command]
pub async fn meeting_series(s: State<'_, VaultSession>) -> Res<Vec<SeriesView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .meeting_series(owner)
        .await?
        .into_iter()
        .map(series_view)
        .collect())
}

/// Maps to `DataApi.saveMeetingSeries()`: add one (`id` null) or change one.
#[tauri::command]
pub async fn save_meeting_series(
    s: State<'_, VaultSession>,
    id: Option<String>,
    input: SeriesArgs,
) -> Res<SeriesView> {
    let (store, owner) = s.data().await?;
    let input = SeriesInput {
        name: input.name,
        days: input.days,
        at: input.at,
        remind_minutes: input.remind_minutes,
        length_minutes: input.length_minutes,
        project_id: input
            .project_id
            .as_deref()
            .map(|p| p.parse().map_err(|_| CommandError::InvalidInput))
            .transpose()?,
    };
    let saved = match id.as_deref() {
        Some(id) => {
            store
                .update_meeting_series(owner, id_arg(id)?, &input)
                .await?
        }
        None => store.create_meeting_series(owner, &input).await?,
    };
    Ok(series_view(saved))
}

/// Maps to `DataApi.setMeetingSeriesDeleted()`: remove a series, or bring it back (Undo).
#[tauri::command]
pub async fn set_meeting_series_deleted(
    s: State<'_, VaultSession>,
    id: String,
    deleted: bool,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_meeting_series_deleted(owner, id_arg(&id)?, deleted)
        .await?)
}

/// Maps to `DataApi.logMeeting()`: "Log 30m" on a meeting that just finished. Adds an
/// entry named after the series on `today`, in its project, linked to the day's meeting
/// note when there is one, and marks today's meeting handled.
#[tauri::command]
pub async fn log_meeting(
    s: State<'_, VaultSession>,
    series_id: String,
    today: String,
    minutes: u32,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    let series = store
        .meeting_series(owner)
        .await?
        .into_iter()
        .find(|m| m.id.to_string() == series_id)
        .ok_or(CommandError::InvalidInput)?;
    let input = EntryInput {
        date: today,
        minutes: Minutes(minutes),
        description: series.name.clone(),
        project: series.project_id,
        recurring: None,
        category: None,
        next_step: None,
        timesheet_code_id: None,
    };
    let rec = store.create_entry(owner, &input, "").await?;
    let title = meeting_note_title(&series.name, today);
    let note = store
        .list_notes(owner, true)
        .await?
        .into_iter()
        .find(|n| n.kind == NoteKind::Meeting && n.title == title);
    if let Some(note) = note {
        store
            .set_link(
                owner,
                (CardKind::Entry, *rec.entry.id.as_uuid()),
                (CardKind::Note, *note.id.as_uuid()),
                true,
            )
            .await?;
    }
    store
        .set_meeting_handled(owner, series.id, Some(today))
        .await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.skipMeeting()`: "Skip" on a meeting that just finished (`skip`), or
/// offer it again (`!skip`, also the Undo of a log).
#[tauri::command]
pub async fn skip_meeting(
    s: State<'_, VaultSession>,
    series_id: String,
    today: String,
    skip: bool,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    Ok(store
        .set_meeting_handled(owner, id_arg(&series_id)?, skip.then_some(today))
        .await?)
}

/// Maps to `DataApi.meetingNote()`: the series' meeting note for `date`, made the first time
/// it is asked for.
#[tauri::command]
pub async fn meeting_note(
    s: State<'_, VaultSession>,
    series_id: String,
    date: String,
) -> Res<NoteView> {
    let (store, owner) = s.data().await?;
    let date = parse_date(&date).map_err(|_| CommandError::InvalidInput)?;
    let series = store
        .meeting_series(owner)
        .await?
        .into_iter()
        .find(|m| m.id.to_string() == series_id)
        .ok_or(CommandError::InvalidInput)?;
    let title = meeting_note_title(&series.name, date);
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let existing = store
        .list_notes(owner, true)
        .await?
        .into_iter()
        .find(|n| n.kind == NoteKind::Meeting && n.title == title);
    let note = match existing {
        Some(n) => n,
        None => {
            let input = NoteInput {
                kind: NoteKind::Meeting,
                title,
                body: String::new(),
                project: None,
                tags: String::new(),
            };
            store.create_note(owner, &input).await?
        }
    };
    Ok(note_view(note, OffsetDateTime::now_utc(), after))
}

/// Maps to `DataApi.bell()`: what's coming up at `now` (minutes after local midnight on
/// `today`).
#[tauri::command]
pub async fn bell(s: State<'_, VaultSession>, today: String, now: u32) -> Res<Vec<BellItem>> {
    let (store, owner) = s.data().await?;
    let today = parse_date(&today).map_err(|_| CommandError::InvalidInput)?;
    bell_for(&store, owner, today, now).await
}

/// Everything in the bell at `now` on `today`, repeating things included (round 15; also
/// counted for the phone's due reminder).
pub async fn bell_for(
    store: &SqliteStore,
    owner: OwnerId,
    today: time::Date,
    now: u32,
) -> Res<Vec<BellItem>> {
    let meetings = store.meeting_series(owner).await?;
    let next_up = store.next_up_all(owner).await?;
    let mut open: Vec<String> = store
        .tray_carried(owner, today)
        .await?
        .into_iter()
        .map(|c| c.item.ticket_number)
        .collect();
    open.extend(
        store
            .tray_day(owner, today)
            .await?
            .items
            .into_iter()
            .filter(|i| !i.done)
            .map(|i| i.ticket_number),
    );
    // Tickets on entries still open count too (round 14), once each.
    for t in store.open_entry_tickets(owner).await? {
        if !open.iter().any(|o| o.eq_ignore_ascii_case(&t)) {
            open.push(t);
        }
    }
    let chase: Vec<(String, String, time::Date)> = crate::pins::pinned(store, owner, today)
        .await?
        .into_iter()
        .filter_map(|(p, title)| {
            p.chase_on
                .map(|on| (title, format!("{}:{}", p.kind.as_str(), p.record_id), on))
        })
        .collect();
    let now = now.min(24 * 60 - 1);
    let mut items = bell_items(&meetings, &next_up, &chase, &open, today, now);
    if let Some(item) = timesheet_reminder(store, owner, today, now).await? {
        items.insert(0, item);
    }
    let due = crate::home::due_things(store, owner, today).await?;
    Ok(with_due(items, &due, today))
}

/// The timesheet reminder (round 15), when it's due: see `shiftbook_core::reminder`.
async fn timesheet_reminder(
    store: &SqliteStore,
    owner: OwnerId,
    today: time::Date,
    now: u32,
) -> Res<Option<BellItem>> {
    let prefs = store.prefs(owner).await?;
    if !prefs.use_mode.has_work() {
        return Ok(None);
    }
    let start = week_of(today, prefs.week_start);
    let last = last_work_day(start, |d| prefs.day_target(d) > 0);
    if !timesheet_due(today, now, last, false) {
        return Ok(None);
    }
    let sheet = crate::api::week_sheet_for(store, owner, today, today, None).await?;
    let ticks = store.list_ticks(owner, start).await?;
    let entered = week_entered(&sheet, |day, key| {
        ticks
            .iter()
            .any(|t| t.day == day && t.line_key == key && t.done)
    });
    Ok((!entered && sheet.entry_count > 0)
        .then(|| timesheet_item(start, &sheet.hours, uncoded_entries(&sheet))))
}
