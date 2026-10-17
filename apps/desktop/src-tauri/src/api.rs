//! Data commands for the alpha. Each one is thin: parse the arguments, call
//! `shiftbook-storage` (which applies the `shiftbook-core` rules) and return a
//! small camelCase view. Errors cross IPC only as [`CommandError`] codes.
//!
//! Every command maps 1:1 to a `DataApi` method in `web/src/lib/api/`.

use serde::{Deserialize, Serialize};
use shiftbook_core::capture::{parse_capture, Capture, CaptureProject};
use shiftbook_core::dates::{add_days, format_date, format_timestamp, parse_date};
use shiftbook_core::fields::{build_link, ticket_matches_prefix, AppliesTo, FieldKind};
use shiftbook_core::hours::format_hours;
use shiftbook_core::ids::{
    CustomFieldId, InboxItemId, NextUpItemId, OwnerId, ProjectId, TicketId, TimeEntryId,
    TimesheetCodeId, TrayItemId,
};
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::{CustomField, EntryTarget, ListName, Project, TimesheetCode, TrayItem};
use shiftbook_core::prefs::Prefs;
use shiftbook_core::resume::{
    resume_due, resume_start, ResumeStart, ResumeTrigger, LOOK_BACK_DAYS,
};
use shiftbook_core::ribbon::{build_ribbon, mark_gaps, ribbon_range, RibbonWeek, RIBBON_WEEKS};
use shiftbook_core::search::rank;
use shiftbook_core::sheet::{build_week_sheet, EntryWithTickets, GroupBy, SheetContext, WeekSheet};
use shiftbook_core::timer::{TimerOffer, MAX_QUIET_STRETCHES};
use shiftbook_core::timesheet::CodeDefaults;
use shiftbook_core::week::{week_of, WeekStart};
use shiftbook_storage::extras::{CodeInput, FieldInput};
use shiftbook_storage::repo::ListOptionRepository;
use shiftbook_storage::work::{EntryInput, EntryRecord, ProjectInput, TrayDay};
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::Date;

use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

fn date_arg(text: &str) -> Res<Date> {
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

/// A link a ticket can be copied as (from a custom link field).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    field_name: String,
    url: String,
}

/// A ticket chip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketView {
    id: String,
    number: String,
    links: Vec<LinkView>,
}

/// A time entry with its tickets.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    id: TimeEntryId,
    date: String,
    minutes: Minutes,
    hours: String,
    description: String,
    project_id: Option<ProjectId>,
    category: Option<String>,
    notes: Option<String>,
    timesheet_code_id: Option<TimesheetCodeId>,
    tickets: Vec<TicketView>,
    is_tray: bool,
}

/// One tray ticket.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayItemView {
    id: TrayItemId,
    ticket: String,
    minutes: Minutes,
    links: Vec<LinkView>,
}

/// A day's tray.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayDayView {
    date: String,
    items: Vec<TrayItemView>,
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

/// An Inbox scrap.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxView {
    id: InboxItemId,
    text: String,
    created_at: String,
}

/// One thing on the "next up" list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextUpView {
    id: NextUpItemId,
    text: String,
    done: bool,
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

/// Link fields that can turn a ticket into a link (template and prefix set).
fn link_fields(fields: &[CustomField]) -> Vec<CustomField> {
    fields
        .iter()
        .filter(|f| !f.hidden && f.link_template.is_some() && f.ticket_prefix.is_some())
        .cloned()
        .collect()
}

fn ticket_links(fields: &[CustomField], ticket: &str) -> Vec<LinkView> {
    fields
        .iter()
        .filter_map(|f| {
            let prefix = f.ticket_prefix.as_deref()?;
            let template = f.link_template.as_deref()?;
            ticket_matches_prefix(prefix, ticket).then(|| LinkView {
                field_name: f.name.clone(),
                url: build_link(template, ticket),
            })
        })
        .collect()
}

fn entry_view(rec: EntryRecord, fields: &[CustomField]) -> EntryView {
    let project_id = match rec.entry.target {
        EntryTarget::Project(p) => Some(p),
        _ => None,
    };
    EntryView {
        id: rec.entry.id,
        date: format_date(rec.entry.date),
        minutes: rec.entry.minutes,
        hours: format_hours(rec.entry.minutes),
        description: rec.entry.description,
        project_id,
        category: rec.entry.category,
        notes: rec.entry.next_step,
        timesheet_code_id: rec.entry.timesheet_code_id,
        tickets: rec
            .tickets
            .into_iter()
            .map(|t| TicketView {
                id: t.id.to_string(),
                links: ticket_links(fields, &t.ticket_number),
                number: t.ticket_number,
            })
            .collect(),
        is_tray: rec.is_tray,
    }
}

fn tray_item_view(item: TrayItem, fields: &[CustomField]) -> TrayItemView {
    TrayItemView {
        id: item.id,
        links: ticket_links(fields, &item.ticket_number),
        ticket: item.ticket_number,
        minutes: item.minutes,
    }
}

fn tray_view(day: TrayDay, fields: &[CustomField]) -> TrayDayView {
    TrayDayView {
        date: format_date(day.date),
        items: day
            .items
            .into_iter()
            .map(|i| tray_item_view(i, fields))
            .collect(),
        entry_id: day.entry_id,
        hours: format_hours(day.minutes),
        minutes: day.minutes,
    }
}

async fn links_for(store: &SqliteStore, owner: OwnerId) -> Res<Vec<CustomField>> {
    Ok(link_fields(&store.list_custom_fields(owner).await?))
}

async fn one_entry(store: &SqliteStore, owner: OwnerId, rec: EntryRecord) -> Res<EntryView> {
    Ok(entry_view(rec, &links_for(store, owner).await?))
}

fn capture_with(text: &str, today: Date, start: WeekStart, projects: &[Project]) -> Capture {
    let live: Vec<CaptureProject<'_>> = projects
        .iter()
        .filter(|p| !p.archived)
        .map(|p| CaptureProject {
            id: p.id,
            name: &p.name,
        })
        .collect();
    parse_capture(text, today, start, &live)
}

// ---------- preferences ----------

/// Maps to `DataApi.loadPrefs()`.
#[tauri::command]
pub async fn load_prefs(s: State<'_, VaultSession>) -> Res<Prefs> {
    let (store, owner) = s.data().await?;
    Ok(store.prefs(owner).await?)
}

/// Maps to `DataApi.savePref()`. Returns all preferences after the change.
#[tauri::command]
pub async fn save_pref(s: State<'_, VaultSession>, key: String, value: String) -> Res<Prefs> {
    let (store, owner) = s.data().await?;
    store.set_pref(owner, &key, &value).await?;
    Ok(store.prefs(owner).await?)
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

/// Maps to `DataApi.quickLog()`: Log work on `date` with the time button's minutes.
#[tauri::command]
pub async fn quick_log(
    s: State<'_, VaultSession>,
    date: String,
    text: String,
    minutes: u32,
) -> Res<EntryView> {
    let (store, owner) = s.data().await?;
    let date = date_arg(&date)?;
    let projects = store.list_projects(owner).await?;
    let capture = capture_with(&text, date, week_start(&store, owner).await?, &projects);
    let rec = store
        .quick_log(owner, date, &capture, Minutes(minutes))
        .await?;
    one_entry(&store, owner, rec).await
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
    let start = week_start(&store, owner).await?;
    Ok(capture_with(&text, date_arg(&today)?, start, &projects))
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
    let start = week_start(&store, owner).await?;
    let capture = capture_with(&text, date_arg(&today)?, start, &projects);
    let minutes = capture.minutes.ok_or(CommandError::InvalidInput)?;
    let rec = store
        .quick_log(owner, capture.date, &capture, minutes)
        .await?;
    one_entry(&store, owner, rec).await
}

// ---------- tray ----------

async fn tray_result(store: &SqliteStore, owner: OwnerId, day: TrayDay) -> Res<TrayDayView> {
    Ok(tray_view(day, &links_for(store, owner).await?))
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
    let prefs = store.prefs(owner).await?;
    let start = week_of(date_arg(&date)?, prefs.week_start);
    let today = date_arg(&today)?;
    let projects = store.list_projects(owner).await?;
    let codes = store.list_timesheet_codes(owner).await?;
    let mappings = store.list_category_codes(owner).await?;
    let defaults = CodeDefaults::new(&projects, &[], &mappings);
    let entries: Vec<EntryWithTickets> = store
        .list_entries(owner, start, shiftbook_core::dates::add_days(start, 6))
        .await?
        .into_iter()
        .map(|r| EntryWithTickets {
            tickets: r.tickets.into_iter().map(|t| t.ticket_number).collect(),
            entry: r.entry,
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

fn inbox_view(i: shiftbook_core::model::InboxItem) -> InboxView {
    InboxView {
        id: i.id,
        text: i.text,
        created_at: format_timestamp(i.created_at),
    }
}

/// How far back Find looks.
const SEARCH_DAYS: i64 = 2 * 366;
/// Most entries Find returns.
const SEARCH_ENTRIES: usize = 8;
/// Most Inbox scraps Find returns.
const SEARCH_SCRAPS: usize = 4;

/// What Find found.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchView {
    /// Entries, best first.
    entries: Vec<EntryView>,
    /// Inbox scraps, best first.
    inbox: Vec<InboxView>,
    /// Indexes into the `commands` that were passed in, best first.
    commands: Vec<usize>,
}

/// Maps to `DataApi.search()`: entries (title, notes, tickets, project) from the last
/// two years, Inbox scraps and the UI's command names that match `query`, ranked by
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
    let commands = rank(&query, commands.iter().cloned().enumerate(), commands.len());
    Ok(SearchView {
        entries,
        inbox,
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

/// Maps to `DataApi.tickNextUp()`.
#[tauri::command]
pub async fn tick_next_up(s: State<'_, VaultSession>, id: String, done: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.tick_next_up(owner, id_arg(&id)?, done).await?)
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
    let list = store.next_up(owner).await?;
    let records = store
        .list_entries(owner, add_days(today, -LOOK_BACK_DAYS), today)
        .await?;
    let pairs: Vec<_> = records
        .iter()
        .map(|r| (r.entry.clone(), r.is_tray))
        .collect();
    let view = match resume_start(&list, &pairs) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use shiftbook_core::ids::OwnerId;
    use time::OffsetDateTime;

    fn field(prefix: Option<&str>, template: Option<&str>, hidden: bool) -> CustomField {
        let now = OffsetDateTime::now_utc();
        CustomField {
            id: CustomFieldId::new(),
            owner_id: OwnerId::new(),
            applies_to: AppliesTo::Ticket,
            name: "Jira".into(),
            kind: FieldKind::Link,
            options: vec![],
            link_template: template.map(String::from),
            ticket_prefix: prefix.map(String::from),
            sort_order: 0,
            hidden,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    #[test]
    fn tickets_get_links_from_matching_fields() {
        let t = Some("https://example.atlassian.net/browse/{ticket}");
        let fields = link_fields(&[
            field(Some("PROJ-"), t, false),
            field(Some("PROJ-"), t, true),
            field(None, t, false),
            field(Some("OPS-"), None, false),
        ]);
        assert_eq!(fields.len(), 1);
        let links = ticket_links(&fields, "proj-12");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.atlassian.net/browse/proj-12");
        assert!(ticket_links(&fields, "INC0045678").is_empty());
    }

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
