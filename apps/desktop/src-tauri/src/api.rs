//! Data commands for the alpha. Each one is thin: parse the arguments, call
//! `shiftbook-storage` (which applies the `shiftbook-core` rules) and return a
//! small camelCase view. Errors cross IPC only as [`CommandError`] codes.
//!
//! Every command maps 1:1 to a `DataApi` method in `web/src/lib/api/`.

use serde::{Deserialize, Serialize};
use shiftbook_core::capture::{parse_capture, Capture, CaptureProject};
use shiftbook_core::dates::{add_days, format_date, format_timestamp, parse_date};
use shiftbook_core::fields::{build_link, AppliesTo, FieldKind};
use shiftbook_core::hours::format_hours;
use shiftbook_core::ids::{
    CustomFieldId, NextUpItemId, OwnerId, ProjectId, RecurringTaskId, TicketId, TimeEntryId,
    TimesheetCodeId,
};
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::{
    CustomField, EntryTarget, ListName, Project, RecurringTask, TimesheetCode,
};
use shiftbook_core::nextup::{bring_up_on, next_work_day};
use shiftbook_core::prefs::Prefs;
use shiftbook_core::resume::{
    resume_due, resume_start, ResumeStart, ResumeTrigger, LOOK_BACK_DAYS,
};
use shiftbook_core::ribbon::{build_ribbon, mark_gaps, ribbon_range, RibbonWeek, RIBBON_WEEKS};
use shiftbook_core::samework::should_ask;
use shiftbook_core::search::rank;
use shiftbook_core::sheet::{build_week_sheet, EntryWithTickets, GroupBy, SheetContext, WeekSheet};
use shiftbook_core::timer::{TimerOffer, MAX_QUIET_STRETCHES};
use shiftbook_core::timesheet::{CodeDefaults, ResolvedCode};
use shiftbook_core::week::{week_of, WeekStart};
use shiftbook_storage::extras::{CodeInput, FieldInput};
use shiftbook_storage::repo::ListOptionRepository;
use shiftbook_storage::work::{CarriedTicket, EntryInput, EntryRecord, ProjectInput, TrayDay};
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::Date;

pub(crate) use shiftbook_app::views::{
    carried_view, entry_view, inbox_view, links_for, tray_item_view, CarriedView, EntryView,
    InboxView, TrayItemView,
};

use crate::notes::{find_notes, NoteView};
use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

pub(crate) fn date_arg(text: &str) -> Res<Date> {
    parse_date(text).map_err(|_| CommandError::InvalidInput)
}

fn id_arg<T: std::str::FromStr>(text: &str) -> Res<T> {
    text.parse().map_err(|_| CommandError::InvalidInput)
}

fn opt_id<T: std::str::FromStr>(text: &Option<String>) -> Res<Option<T>> {
    text.as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(id_arg)
        .transpose()
}

/// The week start day from the user's preferences.
async fn week_start(store: &SqliteStore, owner: OwnerId) -> Res<WeekStart> {
    Ok(store.prefs(owner).await?.week_start)
}

// ---------- views ----------

/// A project as the UI sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    id: ProjectId,
    name: String,
    color: Option<String>,
    default_timesheet_code_id: Option<TimesheetCodeId>,
    archived: bool,
}

impl From<Project> for ProjectView {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            color: p.color,
            default_timesheet_code_id: p.default_timesheet_code_id,
            archived: p.archived,
        }
    }
}

/// A day's tray.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayDayView {
    date: String,
    items: Vec<TrayItemView>,
    /// Still-open tickets from earlier days that come back in this tray.
    carried: Vec<CarriedView>,
    entry_id: Option<TimeEntryId>,
    minutes: Minutes,
    hours: String,
}

/// A timesheet code.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeView {
    id: TimesheetCodeId,
    area: String,
    phase_label: String,
    code: String,
    sort_order: i32,
    hidden: bool,
}

impl From<TimesheetCode> for CodeView {
    fn from(c: TimesheetCode) -> Self {
        Self {
            id: c.id,
            area: c.area,
            phase_label: c.phase_label,
            code: c.code,
            sort_order: c.sort_order,
            hidden: c.hidden,
        }
    }
}

/// A category and the code it maps to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCodeView {
    category: String,
    code_id: Option<TimesheetCodeId>,
}

/// A custom field definition.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    id: CustomFieldId,
    applies_to: AppliesTo,
    name: String,
    kind: FieldKind,
    options: Vec<String>,
    link_template: Option<String>,
    ticket_prefix: Option<String>,
    hidden: bool,
}

impl From<CustomField> for FieldView {
    fn from(f: CustomField) -> Self {
        Self {
            id: f.id,
            applies_to: f.applies_to,
            name: f.name,
            kind: f.kind,
            options: f.options,
            link_template: f.link_template,
            ticket_prefix: f.ticket_prefix,
            hidden: f.hidden,
        }
    }
}

/// A custom value on a record (with its link, for link fields).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueView {
    field_id: CustomFieldId,
    value: String,
    link: Option<String>,
}

/// A crib sheet tick-off.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TickView {
    day: String,
    line_key: String,
    done: bool,
}

/// One thing on the "next up" list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextUpView {
    id: NextUpItemId,
    text: String,
    done: bool,
    /// For a thing brought up from a card: the day it comes up.
    from: Option<String>,
}

/// What `unlock` tells the UI.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockOutcome {
    /// Unlocked with the recovery key: ask for a new password now.
    pub must_set_password: bool,
}

// ---------- inputs ----------

/// The editable parts of an entry, from the drawer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryArgs {
    date: String,
    minutes: u32,
    description: String,
    project_id: Option<String>,
    /// The repeating thing it belongs to, when it has no project.
    #[serde(default)]
    recurring_id: Option<String>,
    category: Option<String>,
    notes: Option<String>,
    timesheet_code_id: Option<String>,
}

impl EntryArgs {
    fn into_input(self) -> Res<EntryInput> {
        Ok(EntryInput {
            date: date_arg(&self.date)?,
            minutes: Minutes(self.minutes),
            description: self.description,
            project: opt_id(&self.project_id)?,
            recurring: opt_id(&self.recurring_id)?,
            category: self.category,
            next_step: self.notes,
            timesheet_code_id: opt_id(&self.timesheet_code_id)?,
        })
    }
}

/// The editable parts of a project.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectArgs {
    name: String,
    color: Option<String>,
    default_timesheet_code_id: Option<String>,
    archived: bool,
}

/// The editable parts of a timesheet code.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeArgs {
    area: String,
    phase_label: String,
    code: String,
    sort_order: i32,
    hidden: bool,
}

/// The editable parts of a custom field.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldArgs {
    applies_to: AppliesTo,
    name: String,
    kind: FieldKind,
    options: String,
    link_template: String,
    ticket_prefix: String,
    hidden: bool,
}

// ---------- helpers ----------

fn tray_view(day: TrayDay, carried: Vec<CarriedTicket>, fields: &[CustomField]) -> TrayDayView {
    TrayDayView {
        date: format_date(day.date),
        items: day
            .items
            .into_iter()
            .map(|i| tray_item_view(i, fields))
            .collect(),
        carried: carried
            .into_iter()
            .map(|c| carried_view(c, fields))
            .collect(),
        entry_id: day.entry_id,
        hours: format_hours(day.minutes),
        minutes: day.minutes,
    }
}

pub(crate) async fn one_entry(
    store: &SqliteStore,
    owner: OwnerId,
    rec: EntryRecord,
) -> Res<EntryView> {
    Ok(entry_view(rec, &links_for(store, owner).await?))
}

/// Read a capture line against the live projects, then the repeating things.
fn capture_with(
    text: &str,
    today: Date,
    start: WeekStart,
    projects: &[Project],
    repeating: &[RecurringTask],
) -> Capture {
    let live: Vec<CaptureProject<'_>> = projects
        .iter()
        .filter(|p| !p.archived)
        .map(|p| CaptureProject {
            id: EntryTarget::Project(p.id),
            name: &p.name,
        })
        .chain(
            repeating
                .iter()
                .filter(|r| r.active)
                .map(|r| CaptureProject {
                    id: EntryTarget::Recurring(r.id),
                    name: &r.name,
                }),
        )
        .collect();
    parse_capture(text, today, start, &live)
}

// ---------- preferences ----------

/// Text size zooms the whole webview, like a browser's own zoom, so the layout
/// still fills the window. (CSS `zoom` on the page shrank `100vh` with it and
/// left a blank band at small sizes.) A failure only leaves the size as it was.
fn apply_text_size(webview: &tauri::Webview, prefs: &Prefs) {
    let _ = webview.set_zoom(f64::from(prefs.text_size) / 100.0);
}

/// Maps to `DataApi.loadPrefs()`.
#[tauri::command]
pub async fn load_prefs(
    webview: tauri::Webview,
    window: tauri::WebviewWindow,
    guard: State<'_, crate::protect::CaptureGuard>,
    s: State<'_, VaultSession>,
) -> Res<Prefs> {
    let (store, owner) = s.data().await?;
    let prefs = store.prefs(owner).await?;
    apply_text_size(&webview, &prefs);
    crate::protect::apply_setting(&window, &guard, prefs.hide_screenshots);
    Ok(prefs)
}

/// Maps to `DataApi.savePref()`. Returns all preferences after the change.
#[tauri::command]
pub async fn save_pref(
    webview: tauri::Webview,
    window: tauri::WebviewWindow,
    guard: State<'_, crate::protect::CaptureGuard>,
    s: State<'_, VaultSession>,
    key: String,
    value: String,
) -> Res<Prefs> {
    let (store, owner) = s.data().await?;
    store.set_pref(owner, &key, &value).await?;
    let prefs = store.prefs(owner).await?;
    apply_text_size(&webview, &prefs);
    crate::protect::apply_setting(&window, &guard, prefs.hide_screenshots);
    Ok(prefs)
}

// ---------- projects ----------

/// Maps to `DataApi.listProjects()`.
#[tauri::command]
pub async fn list_projects(s: State<'_, VaultSession>) -> Res<Vec<ProjectView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .list_projects(owner)
        .await?
        .into_iter()
        .map(ProjectView::from)
        .collect())
}

/// Maps to `DataApi.createProject()`.
#[tauri::command]
pub async fn create_project(
    s: State<'_, VaultSession>,
    name: String,
    color: Option<String>,
) -> Res<ProjectView> {
    let (store, owner) = s.data().await?;
    Ok(store
        .create_project(owner, &name, color.as_deref().filter(|c| !c.is_empty()))
        .await?
        .into())
}

/// Maps to `DataApi.updateProject()`.
#[tauri::command]
pub async fn update_project(
    s: State<'_, VaultSession>,
    id: String,
    input: ProjectArgs,
) -> Res<ProjectView> {
    let (store, owner) = s.data().await?;
    let input = ProjectInput {
        name: input.name,
        color: input.color.filter(|c| !c.is_empty()),
        default_timesheet_code_id: opt_id(&input.default_timesheet_code_id)?,
        archived: input.archived,
    };
    Ok(store
        .update_project(owner, id_arg(&id)?, &input)
        .await?
        .into())
}

// ---------- entries and tickets ----------

/// Maps to `DataApi.listEntries()`: live entries from `from` to `to` inclusive.
#[tauri::command]
pub async fn list_entries(
    s: State<'_, VaultSession>,
    from: String,
    to: String,
) -> Res<Vec<EntryView>> {
    let (store, owner) = s.data().await?;
    let links = links_for(&store, owner).await?;
    Ok(store
        .list_entries(owner, date_arg(&from)?, date_arg(&to)?)
        .await?
        .into_iter()
        .map(|r| entry_view(r, &links))
        .collect())
}

/// Maps to `DataApi.createEntry()`.
#[tauri::command]
pub async fn create_entry(
    s: State<'_, VaultSession>,
    input: EntryArgs,
    tickets: String,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store
        .create_entry(owner, &input.into_input()?, &tickets)
        .await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.updateEntry()`.
#[tauri::command]
pub async fn update_entry(
    s: State<'_, VaultSession>,
    id: String,
    input: EntryArgs,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store
        .update_entry(owner, id_arg(&id)?, &input.into_input()?)
        .await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.addEntryMinutes()`; `delta` may be negative.
#[tauri::command]
pub async fn add_entry_minutes(
    s: State<'_, VaultSession>,
    id: String,
    delta: i32,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store.add_entry_minutes(owner, id_arg(&id)?, delta).await?;
    one_entry(&store, owner, rec).await
}

/// What "+15m" on an entry did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedTimeView {
    /// The entry that got the time.
    entry: EntryView,
    /// It was made just now (a copy on the day), so Undo removes it.
    created: bool,
}

/// Maps to `DataApi.addEntryTime()`: add work time on an entry, landing on `day` (today, or
/// the day open on Day): see `shiftbook_core::quicklog::plan_add_time`.
#[tauri::command]
pub async fn add_entry_time(
    s: State<'_, VaultSession>,
    id: String,
    day: String,
    minutes: u32,
) -> Res<AddedTimeView> {
    let (store, owner) = s.data().await?;
    let (rec, created) = store
        .add_time_on(owner, id_arg(&id)?, date_arg(&day)?, minutes)
        .await?;
    Ok(AddedTimeView {
        entry: one_entry(&store, owner, rec).await?,
        created,
    })
}

/// Maps to `DataApi.logNote()`: log a jot as an entry on `date` (its title says what was
/// done, the rest becomes notes, ticket numbers become tickets); the jot moves to the archive.
#[tauri::command]
pub async fn log_note(
    s: State<'_, VaultSession>,
    id: String,
    date: String,
    minutes: u32,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let note = id.parse().map_err(|_| CommandError::InvalidInput)?;
    let rec = store
        .log_note(owner, note, date_arg(&date)?, minutes)
        .await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.getEntry()`: one entry, for a window showing its notepad.
#[tauri::command]
pub async fn get_entry(s: State<'_, VaultSession>, id: String) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store.get_entry(owner, id_arg(&id)?).await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.trayItem()`: one tray ticket, for a window showing its notepad.
#[tauri::command]
pub async fn tray_item(s: State<'_, VaultSession>, id: String) -> Res<Option<TrayItemView>> {
    let (store, owner) = s.data().await?;
    let links = links_for(&store, owner).await?;
    Ok(store
        .tray_item(owner, id_arg(&id)?)
        .await?
        .map(|i| tray_item_view(i, &links)))
}

/// Maps to `DataApi.setEntryNotes()`: the entry's notepad, saved as you type.
#[tauri::command]
pub async fn set_entry_notes(
    s: State<'_, VaultSession>,
    id: String,
    notes: String,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store.set_entry_notes(owner, id_arg(&id)?, &notes).await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.deleteEntry()` (soft delete).
#[tauri::command]
pub async fn delete_entry(s: State<'_, VaultSession>, id: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.delete_entry(owner, id_arg(&id)?).await?)
}

/// Maps to `DataApi.restoreEntry()` (undo a delete).
#[tauri::command]
pub async fn restore_entry(s: State<'_, VaultSession>, id: String) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store.restore_entry(owner, id_arg(&id)?).await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.addEntryTickets()` (multi-paste).
#[tauri::command]
pub async fn add_entry_tickets(
    s: State<'_, VaultSession>,
    id: String,
    text: String,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store.add_entry_tickets(owner, id_arg(&id)?, &text).await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.removeTicket()`.
#[tauri::command]
pub async fn remove_ticket(s: State<'_, VaultSession>, id: String) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store.remove_ticket(owner, id_arg::<TicketId>(&id)?).await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.ticketSetDone()`: tick an entry's ticket done, or mark it still open.
#[tauri::command]
pub async fn ticket_set_done(s: State<'_, VaultSession>, id: String, done: bool) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store
        .set_ticket_done(owner, id_arg::<TicketId>(&id)?, done)
        .await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.ticketSetNote()`: write an entry ticket's notepad.
#[tauri::command]
pub async fn ticket_set_note(
    s: State<'_, VaultSession>,
    id: String,
    note: String,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let rec = store
        .set_ticket_note(owner, id_arg::<TicketId>(&id)?, &note)
        .await?;
    one_entry(&store, owner, rec).await
}

/// Maps to `DataApi.quickLog()`: Log work on `date` with the time button's minutes.
#[tauri::command]
pub async fn quick_log(
    s: State<'_, VaultSession>,
    date: String,
    text: String,
    minutes: u32,
    add_to: Option<bool>,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let date = date_arg(&date)?;
    let projects = store.list_projects(owner).await?;
    let repeating = store.recurring_all(owner).await?;
    let capture = capture_with(
        &text,
        date,
        week_start(&store, owner).await?,
        &projects,
        &repeating,
    );
    let rec = store
        .quick_log(owner, date, &capture, Minutes(minutes), add_to)
        .await?;
    one_entry(&store, owner, rec).await
}

/// Earlier work Log work asks about ("Add to …?", round 15).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SameWorkView {
    title: String,
    project_id: Option<ProjectId>,
    recurring_id: Option<RecurringTaskId>,
    minutes: u32,
    days: u32,
    open_tickets: Vec<String>,
}

/// Maps to `DataApi.sameWork()`: the earlier work a line's words are the title of, when Log
/// work should ask whether to add to it (`None` otherwise).
#[tauri::command]
pub async fn same_work(
    s: State<'_, VaultSession>,
    date: String,
    text: String,
) -> Res<Option<SameWorkView>> {
    let (store, owner) = s.data().await?;
    let date = date_arg(&date)?;
    let projects = store.list_projects(owner).await?;
    let repeating = store.recurring_all(owner).await?;
    let capture = capture_with(
        &text,
        date,
        week_start(&store, owner).await?,
        &projects,
        &repeating,
    );
    if capture.target() != EntryTarget::None {
        return Ok(None);
    }
    let same = store.same_work(owner, date, &capture.description).await?;
    if !should_ask(same.as_ref()) {
        return Ok(None);
    }
    Ok(same.map(|w| SameWorkView {
        title: w.title,
        project_id: match w.target {
            EntryTarget::Project(p) => Some(p),
            _ => None,
        },
        recurring_id: match w.target {
            EntryTarget::Recurring(r) => Some(r),
            _ => None,
        },
        minutes: w.minutes.get(),
        days: w.days,
        open_tickets: w.open_tickets,
    }))
}

/// A ticket item with its time so far (round 15): a known ticket, or a line of the Items view.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    id: String,
    number: String,
    title: String,
    done: bool,
    note: Option<String>,
    project_id: Option<ProjectId>,
    recurring_id: Option<RecurringTaskId>,
    first: Option<String>,
    last: Option<String>,
    minutes: u32,
    /// The cards it's on, latest first (`tray:id`, `entry:id`).
    cards: Vec<String>,
}

/// Maps to `DataApi.items()`: every ticket item, or with `numbers` only those (known tickets).
#[tauri::command]
pub async fn items(s: State<'_, VaultSession>, numbers: Option<Vec<String>>) -> Res<Vec<ItemView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .item_summaries(owner, numbers.as_deref())
        .await?
        .into_iter()
        .map(|i| ItemView {
            id: i.id.to_string(),
            number: i.number,
            title: i.title,
            done: i.done,
            note: i.note,
            project_id: i.project,
            recurring_id: i.recurring,
            first: i.so_far.map(|f| format_date(f.first)),
            last: i.so_far.map(|f| format_date(f.last)),
            minutes: i.so_far.map_or(0, |f| f.minutes.get()),
            cards: i.cards,
        })
        .collect())
}

/// Maps to `DataApi.itemSetDone()`: the Items view's tick (reopening counts from `today`).
#[tauri::command]
pub async fn item_set_done(
    s: State<'_, VaultSession>,
    id: String,
    done: bool,
    today: String,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .item_set_done(owner, id_arg(&id)?, done, date_arg(&today)?)
        .await?)
}

/// Maps to `DataApi.parseCapture()`: what a capture line means, for the live chips.
#[tauri::command]
pub async fn parse_capture_line(
    s: State<'_, VaultSession>,
    text: String,
    today: String,
) -> Res<Capture> {
    let (store, owner) = s.data().await?;
    let projects = store.list_projects(owner).await?;
    let repeating = store.recurring_all(owner).await?;
    let start = week_start(&store, owner).await?;
    Ok(capture_with(
        &text,
        date_arg(&today)?,
        start,
        &projects,
        &repeating,
    ))
}

/// Maps to `DataApi.captureLog()`: log a capture line (it must have hours).
#[tauri::command]
pub async fn capture_log(
    s: State<'_, VaultSession>,
    text: String,
    today: String,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let projects = store.list_projects(owner).await?;
    let repeating = store.recurring_all(owner).await?;
    let start = week_start(&store, owner).await?;
    let capture = capture_with(&text, date_arg(&today)?, start, &projects, &repeating);
    let minutes = capture.minutes.ok_or(CommandError::InvalidInput)?;
    let rec = store
        .quick_log(owner, capture.date, &capture, minutes, None)
        .await?;
    one_entry(&store, owner, rec).await
}

// ---------- tray ----------

async fn tray_result(store: &SqliteStore, owner: OwnerId, day: TrayDay) -> Res<TrayDayView> {
    let carried = store.tray_carried(owner, day.date).await?;
    Ok(tray_view(day, carried, &links_for(store, owner).await?))
}

/// Maps to `DataApi.trayDay()`.
#[tauri::command]
pub async fn tray_day(s: State<'_, VaultSession>, date: String) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store.tray_day(owner, date_arg(&date)?).await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.trayAdd()` (multi-paste).
#[tauri::command]
pub async fn tray_add(s: State<'_, VaultSession>, date: String, text: String) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store.tray_add(owner, date_arg(&date)?, &text).await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.traySetMinutes()`.
#[tauri::command]
pub async fn tray_set_minutes(
    s: State<'_, VaultSession>,
    id: String,
    minutes: u32,
) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store.tray_set_minutes(owner, id_arg(&id)?, minutes).await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.traySetNote()`: a few words about one tray ticket (blank clears them).
#[tauri::command]
pub async fn tray_set_note(
    s: State<'_, VaultSession>,
    id: String,
    note: String,
) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store.tray_set_note(owner, id_arg(&id)?, &note).await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.traySetDone()`: tick a ticket done, or untick it to keep it open.
#[tauri::command]
pub async fn tray_set_done(s: State<'_, VaultSession>, id: String, done: bool) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store.tray_set_done(owner, id_arg(&id)?, done).await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.traySetProject()`: the project a ticket's minutes count under
/// (`None` = the day's "Tickets" entry).
#[tauri::command]
pub async fn tray_set_project(
    s: State<'_, VaultSession>,
    id: String,
    project_id: Option<String>,
) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store
        .tray_set_project(owner, id_arg(&id)?, opt_id(&project_id)?)
        .await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.traySetRecurring()`: the repeating thing a ticket's minutes count
/// under (`None` = the day's "Tickets" entry).
#[tauri::command]
pub async fn tray_set_recurring(
    s: State<'_, VaultSession>,
    id: String,
    recurring_id: Option<String>,
) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store
        .tray_set_recurring(owner, id_arg(&id)?, opt_id(&recurring_id)?)
        .await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.trayCarry()`: work on a still-open ticket again on `date`.
#[tauri::command]
pub async fn tray_carry(s: State<'_, VaultSession>, id: String, date: String) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store
        .tray_carry(owner, id_arg(&id)?, date_arg(&date)?)
        .await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.trayRemove()`.
#[tauri::command]
pub async fn tray_remove(s: State<'_, VaultSession>, id: String) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store.tray_remove(owner, id_arg(&id)?).await?;
    tray_result(&store, owner, day).await
}

/// Maps to `DataApi.trayRestore()` (undo a remove).
#[tauri::command]
pub async fn tray_restore(s: State<'_, VaultSession>, id: String) -> Res<TrayDayView> {
    let (store, owner) = s.data().await?;
    let day = store.tray_restore(owner, id_arg(&id)?).await?;
    tray_result(&store, owner, day).await
}

// ---------- ribbon and timesheet ----------

/// Maps to `DataApi.ribbon()`: the last four weeks up to the week of `today`.
#[tauri::command]
pub async fn ribbon(s: State<'_, VaultSession>, today: String) -> Res<Vec<RibbonWeek>> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    let start = week_start(&store, owner).await?;
    let (from, to) = ribbon_range(today, start, RIBBON_WEEKS);
    let entries: Vec<_> = store
        .list_entries(owner, from, to)
        .await?
        .into_iter()
        .map(|r| r.entry)
        .collect();
    let mut weeks = build_ribbon(today, start, RIBBON_WEEKS, &entries);
    mark_gaps(&mut weeks, &store.prefs(owner).await?.day_targets);
    Ok(weeks)
}

/// Maps to `DataApi.weekSheet()`: the crib sheet for the week containing `date`.
#[tauri::command]
pub async fn week_sheet(
    s: State<'_, VaultSession>,
    date: String,
    today: String,
    group_by: Option<GroupBy>,
) -> Res<WeekSheet> {
    let (store, owner) = s.data().await?;
    week_sheet_for(&store, owner, date_arg(&date)?, date_arg(&today)?, group_by).await
}

/// Everything that gives an entry its code when none is chosen on it.
async fn code_defaults(
    store: &SqliteStore,
    owner: OwnerId,
    projects: &[Project],
    prefs: &Prefs,
) -> Res<CodeDefaults> {
    let mappings = store.list_category_codes(owner).await?;
    let repeating = store.recurring_all(owner).await?;
    let kinds = store.list_kind_codes(owner).await?;
    Ok(CodeDefaults::new(projects, &repeating, &mappings)
        .with_kinds(&kinds)
        .with_tickets(prefs.tickets_code))
}

/// Maps to `DataApi.usualCode()`: the code an entry gets when none is chosen on it, for
/// the project or repeating thing and category picked on the form (`id` is the entry being
/// edited, if any).
#[tauri::command]
pub async fn usual_code(
    s: State<'_, VaultSession>,
    id: Option<String>,
    project_id: Option<String>,
    recurring_id: Option<String>,
    category: Option<String>,
) -> Res<ResolvedCode> {
    let (store, owner) = s.data().await?;
    let rollup = match id {
        Some(id) => store.get_entry(owner, id_arg(&id)?).await?.is_tray,
        None => false,
    };
    let target = match (opt_id(&project_id)?, opt_id(&recurring_id)?) {
        (Some(p), _) => EntryTarget::Project(p),
        (None, Some(r)) => EntryTarget::Recurring(r),
        (None, None) => EntryTarget::None,
    };
    let prefs = store.prefs(owner).await?;
    let projects = store.list_projects(owner).await?;
    let defaults = code_defaults(&store, owner, &projects, &prefs).await?;
    Ok(defaults.usual(target, category.as_deref(), rollup))
}

/// Maps to `DataApi.setEntryCodes()`: choose the code for several entries at once (from
/// the Timesheet's "Pick a code"); no code goes back to the usual one.
#[tauri::command]
pub async fn set_entry_codes(
    s: State<'_, VaultSession>,
    ids: Vec<String>,
    code_id: Option<String>,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let ids = ids
        .iter()
        .map(|i| id_arg(i))
        .collect::<Res<Vec<TimeEntryId>>>()?;
    let code = opt_id(&code_id)?;
    Ok(store.set_entry_codes(owner, &ids, code).await?)
}

/// The crib sheet for the week holding `date` (also the timesheet reminder's).
pub(crate) async fn week_sheet_for(
    store: &SqliteStore,
    owner: OwnerId,
    date: Date,
    today: Date,
    group_by: Option<GroupBy>,
) -> Res<WeekSheet> {
    let prefs = store.prefs(owner).await?;
    let start = week_of(date, prefs.week_start);
    let projects = store.list_projects(owner).await?;
    let codes = store.list_timesheet_codes(owner).await?;
    let defaults = code_defaults(store, owner, &projects, &prefs).await?;
    let entries: Vec<EntryWithTickets> = store
        .list_entries(owner, start, shiftbook_core::dates::add_days(start, 6))
        .await?
        .into_iter()
        .map(|r| EntryWithTickets {
            tickets: r.tickets.into_iter().map(|t| t.ticket_number).collect(),
            entry: r.entry,
            rollup: r.is_tray,
        })
        .collect();
    let ctx = SheetContext {
        week_start: start,
        today,
        group_by: group_by.unwrap_or(prefs.group_by),
        format: &prefs.comment,
        codes: &codes,
        projects: &projects,
        defaults: &defaults,
    };
    Ok(build_week_sheet(&ctx, &entries))
}

/// Maps to `DataApi.listTicks()`.
#[tauri::command]
pub async fn list_ticks(s: State<'_, VaultSession>, week_start: String) -> Res<Vec<TickView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .list_ticks(owner, date_arg(&week_start)?)
        .await?
        .into_iter()
        .map(|t| TickView {
            day: format_date(t.day),
            line_key: t.line_key,
            done: t.done,
        })
        .collect())
}

/// Maps to `DataApi.setTick()`.
#[tauri::command]
pub async fn set_tick(
    s: State<'_, VaultSession>,
    week_start: String,
    day: String,
    line_key: String,
    done: bool,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_tick(
            owner,
            date_arg(&week_start)?,
            date_arg(&day)?,
            &line_key,
            done,
        )
        .await?)
}

// ---------- timesheet codes ----------

/// Maps to `DataApi.listTimesheetCodes()`.
#[tauri::command]
pub async fn list_timesheet_codes(s: State<'_, VaultSession>) -> Res<Vec<CodeView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .list_timesheet_codes(owner)
        .await?
        .into_iter()
        .map(CodeView::from)
        .collect())
}

/// Maps to `DataApi.saveTimesheetCode()`: add when `id` is missing, else update.
#[tauri::command]
pub async fn save_timesheet_code(
    s: State<'_, VaultSession>,
    id: Option<String>,
    input: CodeArgs,
) -> Res<CodeView> {
    let (store, owner) = s.data().await?;
    let input = CodeInput {
        area: input.area,
        phase_label: input.phase_label,
        code: input.code,
        sort_order: input.sort_order,
        hidden: input.hidden,
    };
    let saved = match opt_id::<TimesheetCodeId>(&id)? {
        Some(id) => store.update_timesheet_code(owner, id, &input).await?,
        None => store.add_timesheet_code(owner, &input).await?,
    };
    Ok(saved.into())
}

/// Maps to `DataApi.categoryCodes()`: every category with its code (if any).
#[tauri::command]
pub async fn category_codes(s: State<'_, VaultSession>) -> Res<Vec<CategoryCodeView>> {
    let (store, owner) = s.data().await?;
    let categories = store.list_options(owner, ListName::Category, false).await?;
    let maps = store.list_category_codes(owner).await?;
    let mut out: Vec<CategoryCodeView> = categories
        .into_iter()
        .map(|c| CategoryCodeView {
            code_id: maps
                .iter()
                .find(|m| m.category.trim().eq_ignore_ascii_case(c.value.trim()))
                .map(|m| m.timesheet_code_id),
            category: c.value,
        })
        .collect();
    for m in maps {
        if !out
            .iter()
            .any(|o| o.category.eq_ignore_ascii_case(&m.category))
        {
            out.push(CategoryCodeView {
                category: m.category,
                code_id: Some(m.timesheet_code_id),
            });
        }
    }
    Ok(out)
}

/// Maps to `DataApi.setCategoryCode()`; no `codeId` removes the mapping.
#[tauri::command]
pub async fn set_category_code(
    s: State<'_, VaultSession>,
    category: String,
    code_id: Option<String>,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_category_code(owner, &category, opt_id(&code_id)?)
        .await?)
}

/// One kind of repeating thing and its code, for Customize → Timesheet.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindCodeView {
    kind: String,
    code_id: Option<TimesheetCodeId>,
}

/// Maps to `DataApi.kindCodes()`: every kind of repeating thing with its code (if any).
#[tauri::command]
pub async fn kind_codes(s: State<'_, VaultSession>) -> Res<Vec<KindCodeView>> {
    let (store, owner) = s.data().await?;
    let kinds = store
        .list_options(owner, ListName::RecurringType, false)
        .await?;
    let maps = store.list_kind_codes(owner).await?;
    let find = |kind: &str| {
        maps.iter()
            .find(|(k, _)| k.trim().eq_ignore_ascii_case(kind.trim()))
            .map(|(_, c)| *c)
    };
    let mut out: Vec<KindCodeView> = kinds
        .into_iter()
        .map(|k| KindCodeView {
            code_id: find(&k.value),
            kind: k.value,
        })
        .collect();
    for (kind, code) in &maps {
        if !out.iter().any(|o| o.kind.eq_ignore_ascii_case(kind)) {
            out.push(KindCodeView {
                kind: kind.clone(),
                code_id: Some(*code),
            });
        }
    }
    Ok(out)
}

/// Maps to `DataApi.setKindCode()`; no `codeId` removes the mapping.
#[tauri::command]
pub async fn set_kind_code(
    s: State<'_, VaultSession>,
    kind: String,
    code_id: Option<String>,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.set_kind_code(owner, &kind, opt_id(&code_id)?).await?)
}

// ---------- custom fields ----------

/// Maps to `DataApi.listCustomFields()`.
#[tauri::command]
pub async fn list_custom_fields(s: State<'_, VaultSession>) -> Res<Vec<FieldView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .list_custom_fields(owner)
        .await?
        .into_iter()
        .map(FieldView::from)
        .collect())
}

/// Maps to `DataApi.saveCustomField()`: add when `id` is missing, else update.
#[tauri::command]
pub async fn save_custom_field(
    s: State<'_, VaultSession>,
    id: Option<String>,
    input: FieldArgs,
) -> Res<FieldView> {
    let (store, owner) = s.data().await?;
    let input = FieldInput {
        applies_to: input.applies_to,
        name: input.name,
        kind: input.kind,
        options: input.options,
        link_template: input.link_template,
        ticket_prefix: input.ticket_prefix,
        hidden: input.hidden,
    };
    let saved = match opt_id::<CustomFieldId>(&id)? {
        Some(id) => store.update_custom_field(owner, id, &input).await?,
        None => store.add_custom_field(owner, &input).await?,
    };
    Ok(saved.into())
}

/// Maps to `DataApi.deleteCustomField()`.
#[tauri::command]
pub async fn delete_custom_field(s: State<'_, VaultSession>, id: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.delete_custom_field(owner, id_arg(&id)?).await?)
}

/// Maps to `DataApi.listCustomValues()`: values on one record.
#[tauri::command]
pub async fn list_custom_values(
    s: State<'_, VaultSession>,
    record_id: String,
) -> Res<Vec<ValueView>> {
    let (store, owner) = s.data().await?;
    let record: uuid::Uuid = id_arg(&record_id)?;
    let fields = store.list_custom_fields(owner).await?;
    Ok(store
        .list_custom_values(owner, record)
        .await?
        .into_iter()
        .map(|v| {
            let link = fields
                .iter()
                .find(|f| f.id == v.field_id && f.kind == FieldKind::Link)
                .and_then(|f| f.link_template.as_deref())
                .map(|t| build_link(t, &v.value));
            ValueView {
                field_id: v.field_id,
                value: v.value,
                link,
            }
        })
        .collect())
}

/// Maps to `DataApi.setCustomValue()`; returns the stored value.
#[tauri::command]
pub async fn set_custom_value(
    s: State<'_, VaultSession>,
    field_id: String,
    record_id: String,
    value: String,
) -> Res<String> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_custom_value(owner, id_arg(&field_id)?, id_arg(&record_id)?, &value)
        .await?)
}

// ---------- inbox ----------

/// How far back Find looks.
const SEARCH_DAYS: i64 = 2 * 366;
/// Most entries Find returns.
const SEARCH_ENTRIES: usize = 8;
/// Most Inbox scraps Find returns.
const SEARCH_SCRAPS: usize = 4;
/// Most notes Find returns.
const SEARCH_NOTES: usize = 5;
/// Most tray tickets Find returns.
const SEARCH_TICKETS: usize = 6;

/// What Find found.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchView {
    /// Entries, best first.
    entries: Vec<EntryView>,
    /// Inbox scraps, best first.
    inbox: Vec<InboxView>,
    /// Notes (archived ones too), best first.
    notes: Vec<NoteView>,
    /// Tickets from the Tickets tray (done ones too), best first.
    tickets: Vec<TrayItemView>,
    /// Indexes into the `commands` that were passed in, best first.
    commands: Vec<usize>,
}

/// Maps to `DataApi.search()`: entries (title, notes, tickets, project) and tray tickets
/// (number, notepad, project) from the last two years, Inbox scraps, notes and the UI's command names that match `query`, ranked by
/// `shiftbook_core::search`.
#[tauri::command]
pub async fn search(
    s: State<'_, VaultSession>,
    query: String,
    today: String,
    commands: Vec<String>,
) -> Res<SearchView> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    let links = links_for(&store, owner).await?;
    let projects = store.list_projects(owner).await?;
    let mut records = store
        .list_entries(owner, add_days(today, -SEARCH_DAYS), today)
        .await?;
    records.reverse();
    let entries = rank(
        &query,
        records.into_iter().map(|r| {
            let project = match r.entry.target {
                EntryTarget::Project(id) => projects
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.as_str()),
                _ => None,
            };
            let text = [
                r.entry.description.as_str(),
                r.entry.next_step.as_deref().unwrap_or(""),
                project.unwrap_or(""),
            ]
            .into_iter()
            .chain(r.tickets.iter().map(|t| t.ticket_number.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
            (r, text)
        }),
        SEARCH_ENTRIES,
    )
    .into_iter()
    .map(|r| entry_view(r, &links))
    .collect();
    let inbox = rank(
        &query,
        store.list_inbox(owner).await?.into_iter().map(|i| {
            let text = i.text.clone();
            (i, text)
        }),
        SEARCH_SCRAPS,
    )
    .into_iter()
    .map(inbox_view)
    .collect();
    let notes = find_notes(&store, owner, &query, SEARCH_NOTES).await?;
    let mut tray = store
        .tray_between(owner, add_days(today, -SEARCH_DAYS), today)
        .await?;
    tray.sort_by_key(|t| std::cmp::Reverse(t.date));
    let tickets = rank(
        &query,
        tray.into_iter().map(|t| {
            let project = t
                .project_id
                .and_then(|id| projects.iter().find(|p| p.id == id))
                .map_or("", |p| p.name.as_str());
            let text = format!(
                "{} {} {project}",
                t.ticket_number,
                t.note.as_deref().unwrap_or("")
            );
            (t, text)
        }),
        SEARCH_TICKETS,
    )
    .into_iter()
    .map(|t| tray_item_view(t, &links))
    .collect();
    let commands = rank(&query, commands.iter().cloned().enumerate(), commands.len());
    Ok(SearchView {
        entries,
        inbox,
        notes,
        tickets,
        commands,
    })
}

/// Maps to `DataApi.listInbox()`.
#[tauri::command]
pub async fn list_inbox(s: State<'_, VaultSession>) -> Res<Vec<InboxView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .list_inbox(owner)
        .await?
        .into_iter()
        .map(inbox_view)
        .collect())
}

/// Maps to `DataApi.addInbox()`.
#[tauri::command]
pub async fn add_inbox(s: State<'_, VaultSession>, text: String) -> Res<InboxView> {
    let (store, owner) = s.data().await?;
    Ok(inbox_view(store.add_inbox(owner, &text).await?))
}

/// Maps to `DataApi.setInboxDeleted()` (remove, or undo a remove).
#[tauri::command]
pub async fn set_inbox_deleted(s: State<'_, VaultSession>, id: String, deleted: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_inbox_deleted(owner, id_arg(&id)?, deleted)
        .await?)
}

// ---------- next up ----------

fn next_up_views(items: Vec<shiftbook_core::model::NextUpItem>) -> Vec<NextUpView> {
    items
        .into_iter()
        .map(|i| NextUpView {
            id: i.id,
            text: i.text,
            done: i.done_at.is_some(),
            from: i.show_from.map(format_date),
        })
        .collect()
}

/// Maps to `DataApi.nextUp()`.
#[tauri::command]
pub async fn next_up(s: State<'_, VaultSession>) -> Res<Vec<NextUpView>> {
    let (store, owner) = s.data().await?;
    Ok(next_up_views(store.next_up(owner).await?))
}

/// Maps to `DataApi.setNextUp()`: replace the list (an empty list clears it).
#[tauri::command]
pub async fn set_next_up(s: State<'_, VaultSession>, items: Vec<String>) -> Res<Vec<NextUpView>> {
    let (store, owner) = s.data().await?;
    Ok(next_up_views(store.set_next_up(owner, &items).await?))
}

/// Maps to `DataApi.bringUpDays()`: the days "Bring up…" offers from `today`.
#[tauri::command]
pub async fn bring_up_days(s: State<'_, VaultSession>, today: String) -> Res<Vec<String>> {
    let today = date_arg(&today)?;
    let (store, owner) = s.data().await?;
    let prefs = store.prefs(owner).await?;
    Ok(
        shiftbook_core::nextup::bring_up_days(today, prefs.week_start, |d| prefs.day_target(d) > 0)
            .into_iter()
            .map(format_date)
            .collect(),
    )
}

/// Maps to `DataApi.bringUp()`: "Bring up…" on a card, titled `title`. It comes up on
/// `on` (moved to the next work day if that isn't one), or without `on` on the next work
/// day after `today` (a day with a target).
#[tauri::command]
pub async fn bring_up(
    s: State<'_, VaultSession>,
    today: String,
    kind: String,
    id: String,
    title: String,
    on: Option<String>,
) -> Res<NextUpView> {
    let today = date_arg(&today)?;
    let on = on.as_deref().map(date_arg).transpose()?;
    let about = (
        kind.parse().map_err(|_| CommandError::InvalidInput)?,
        uuid::Uuid::parse_str(&id).map_err(|_| CommandError::InvalidInput)?,
    );
    let (store, owner) = s.data().await?;
    let prefs = store.prefs(owner).await?;
    let is_work_day = |d: Date| prefs.day_target(d) > 0;
    let day = match on {
        // "Later today" (Follow up, round 15) stays today.
        Some(chosen) if chosen == today => today,
        Some(chosen) => {
            bring_up_on(today, chosen, is_work_day).ok_or(CommandError::InvalidInput)?
        }
        None => next_work_day(today, is_work_day),
    };
    let item = store.bring_up(owner, &title, day, Some(about)).await?;
    Ok(next_up_views(vec![item]).remove(0))
}

/// Maps to `DataApi.pastedTickets()`: the tickets in text pasted on the Desk, or `None`
/// when it isn't only tickets (then it becomes a jot).
#[tauri::command]
pub fn pasted_tickets(text: String) -> Option<Vec<String>> {
    shiftbook_core::capture::pasted_tickets(&text)
}

/// Maps to `DataApi.findDay()`: the day Find's text names ("fri", "last friday", "12 oct"),
/// or `None`.
#[tauri::command]
pub async fn find_day(
    s: State<'_, VaultSession>,
    text: String,
    today: String,
) -> Res<Option<String>> {
    let (store, owner) = s.data().await?;
    let start = week_start(&store, owner).await?;
    Ok(shiftbook_core::find::find_day(&text, date_arg(&today)?, start).map(format_date))
}

/// Maps to `DataApi.standup()`: "Copy for stand-up" from `today`. "Done" is the last day
/// before today with entries (within two weeks), "next" what you start with today: things
/// brought up for today, then the wrap-up list.
#[tauri::command]
pub async fn standup(s: State<'_, VaultSession>, today: String) -> Res<String> {
    use shiftbook_core::standup::{standup_text, DoneLine};
    let today = date_arg(&today)?;
    let (store, owner) = s.data().await?;
    let prefs = store.prefs(owner).await?;
    let earlier = store
        .list_entries(owner, add_days(today, -14), add_days(today, -1))
        .await?;
    let last_day = earlier.iter().map(|r| r.entry.date).max();
    let mut done = Vec::new();
    let mut closed = 0;
    if let Some(day) = last_day {
        done = earlier
            .iter()
            .filter(|r| r.entry.date == day && !r.is_tray)
            .map(|r| DoneLine {
                title: r.entry.description.clone(),
                tickets: r.tickets.iter().map(|t| t.ticket_number.clone()).collect(),
            })
            .collect();
        closed = store
            .tray_day(owner, day)
            .await?
            .items
            .iter()
            .filter(|i| i.done)
            .count();
    }
    let open: Vec<_> = store
        .next_up_all(owner)
        .await?
        .into_iter()
        .filter(|i| i.done_at.is_none() && i.show_from.is_none_or(|d| d <= today))
        .collect();
    let next: Vec<String> = open
        .iter()
        .filter(|i| i.show_from.is_some())
        .chain(open.iter().filter(|i| i.show_from.is_none()))
        .map(|i| i.text.clone())
        .collect();
    let words = &prefs.standup_words;
    Ok(standup_text((&words.0, &words.1), &done, closed, &next))
}

/// Maps to `DataApi.tickNextUp()`.
#[tauri::command]
pub async fn tick_next_up(s: State<'_, VaultSession>, id: String, done: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.tick_next_up(owner, id_arg(&id)?, done).await?)
}

// ---------- saved responses ----------

/// One saved response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseView {
    id: shiftbook_core::ids::SavedResponseId,
    name: String,
    text: String,
}

/// One saved response as typed in Settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseInput {
    name: String,
    text: String,
}

fn response_views(items: Vec<shiftbook_core::model::SavedResponse>) -> Vec<ResponseView> {
    items
        .into_iter()
        .map(|r| ResponseView {
            id: r.id,
            name: r.name,
            text: r.text,
        })
        .collect()
}

/// Maps to `DataApi.listResponses()`.
#[tauri::command]
pub async fn list_responses(s: State<'_, VaultSession>) -> Res<Vec<ResponseView>> {
    let (store, owner) = s.data().await?;
    Ok(response_views(store.responses(owner).await?))
}

/// Maps to `DataApi.saveResponses()`: replace the list (an empty list clears it).
#[tauri::command]
pub async fn save_responses(
    s: State<'_, VaultSession>,
    items: Vec<ResponseInput>,
) -> Res<Vec<ResponseView>> {
    let (store, owner) = s.data().await?;
    let drafts: Vec<shiftbook_core::responses::ResponseDraft> = items
        .into_iter()
        .map(|i| shiftbook_core::responses::ResponseDraft {
            name: i.name,
            text: i.text,
        })
        .collect();
    Ok(response_views(store.set_responses(owner, &drafts).await?))
}

// ---------- where was I? ----------

/// What the "Where was I?" card shows: the open wrap-up list (first item first), or
/// else the last entry logged.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeView {
    trigger: ResumeTrigger,
    list: Vec<NextUpView>,
    entry: Option<EntryView>,
    /// When that entry last changed (UTC, RFC 3339), for "last logged … ago".
    changed_at: Option<String>,
}

/// Maps to `DataApi.whereWasI()`. `trigger` is `unlocked`, `break` or `new_day`;
/// `away_minutes` is how long the quiet stretch lasted. `None` when the card shouldn't
/// show (a short break) or there's nothing to pick up.
#[tauri::command]
pub async fn where_was_i(
    s: State<'_, VaultSession>,
    today: String,
    trigger: String,
    away_minutes: i64,
) -> Res<Option<ResumeView>> {
    let today = date_arg(&today)?;
    let trigger = ResumeTrigger::parse(&trigger).ok_or(CommandError::InvalidInput)?;
    if !resume_due(trigger, away_minutes) {
        return Ok(None);
    }
    let (store, owner) = s.data().await?;
    let list = store.next_up_all(owner).await?;
    let records = store
        .list_entries(owner, add_days(today, -LOOK_BACK_DAYS), today)
        .await?;
    let pairs: Vec<_> = records
        .iter()
        .map(|r| (r.entry.clone(), r.is_tray))
        .collect();
    let view = match resume_start(&list, today, &pairs) {
        None => return Ok(None),
        Some(ResumeStart::List(open)) => ResumeView {
            trigger,
            list: next_up_views(open.into_iter().cloned().collect()),
            entry: None,
            changed_at: None,
        },
        Some(ResumeStart::Entry(e)) => {
            let id = e.id;
            let changed_at = format_timestamp(e.updated_at);
            let links = links_for(&store, owner).await?;
            let rec = records
                .into_iter()
                .find(|r| r.entry.id == id)
                .ok_or(CommandError::StorageError)?;
            ResumeView {
                trigger,
                list: Vec::new(),
                entry: Some(entry_view(rec, &links)),
                changed_at: Some(changed_at),
            }
        }
    };
    Ok(Some(view))
}

// ---------- timer ----------

/// Maps to `DataApi.timerOffer()`: what Stop offers to add for `seconds` counted, given
/// the quiet stretches (seconds) noticed while it ran. Needs no data, only the rule.
#[tauri::command]
pub fn timer_offer(seconds: u64, quiet: Vec<u64>) -> Res<TimerOffer> {
    if quiet.len() > MAX_QUIET_STRETCHES {
        return Err(CommandError::InvalidInput);
    }
    Ok(shiftbook_core::timer::timer_offer(seconds, &quiet))
}

/// Maps to `DataApi.timerAwayQuestion()`: after time in the background with the timer
/// running (phone), the minutes to ask about, or null when it's short enough to just count.
#[tauri::command]
pub fn timer_away_question(seconds: u64) -> Option<Minutes> {
    shiftbook_core::timer::away_question(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_checked() {
        assert_eq!(date_arg("2026-10-15").unwrap().day(), 15);
        assert_eq!(date_arg("15/10/2026"), Err(CommandError::InvalidInput));
        assert_eq!(id_arg::<ProjectId>("nope"), Err(CommandError::InvalidInput));
        assert_eq!(opt_id::<ProjectId>(&Some(String::new())), Ok(None));
        assert_eq!(opt_id::<ProjectId>(&None), Ok(None));
    }

    #[test]
    fn views_are_camel_case() {
        let json = serde_json::to_value(UnlockOutcome {
            must_set_password: true,
        })
        .unwrap();
        assert_eq!(json["mustSetPassword"], true);
        let json = serde_json::to_value(CategoryCodeView {
            category: "Meetings".into(),
            code_id: None,
        })
        .unwrap();
        assert!(json.get("codeId").is_some());
    }
}
