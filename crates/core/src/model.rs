//! Domain records (SPEC §3). Plain data: no behaviour beyond small helpers.
//!
//! Every record carries `id`, `owner_id`, `created_at`, `updated_at` and
//! `deleted_at` (soft delete; `None` = live). Dates serialize as
//! `YYYY-MM-DD`, timestamps as RFC 3339 UTC, ids as UUID strings.
//! Category, status and priority are stored as their text value.

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};

use crate::dates::{iso_date, rfc3339};
use crate::fields::{AppliesTo, FieldKind};
use crate::ids::{
    CategoryCodeDefaultId, ChecklistItemId, CustomFieldId, CustomValueId, FrequencyId, InboxItemId,
    ListOptionId, NextUpItemId, NoteId, OwnerId, ProjectId, RecurringTaskId, TaskId, TicketId,
    TimeEntryId, TimesheetCodeId, TimesheetTickId, TimesheetWeekId, TrayItemId,
};
use crate::minutes::Minutes;
use crate::notes::NoteKind;

/// The status value that means "finished" in every rule.
pub const STATUS_DONE: &str = "Done";
/// The status value that marks a project as blocked.
pub const STATUS_BLOCKED: &str = "Blocked";

/// True when `status` is Done (ignoring case and surrounding spaces, like Excel's `=`).
pub fn is_done(status: Option<&str>) -> bool {
    status.is_some_and(|s| s.trim().eq_ignore_ascii_case(STATUS_DONE))
}

/// True when `status` is Blocked (ignoring case and surrounding spaces).
pub fn is_blocked(status: Option<&str>) -> bool {
    status.is_some_and(|s| s.trim().eq_ignore_ascii_case(STATUS_BLOCKED))
}

/// The person who owns records. Phase 1 has exactly one, created at setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// Id (also used as `owner_id` on every record they own).
    pub id: OwnerId,
    /// Name shown in the app.
    pub display_name: String,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// Which user-editable dropdown list an option belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListName {
    /// Kind of work (Troubleshooting, Meetings, ...).
    Category,
    /// Progress (Not started, In progress, Done, ...).
    Status,
    /// Importance (Low ... Urgent).
    Priority,
    /// Kind of recurring work (Certificate renewal, Patching, ...).
    RecurringType,
}

impl ListName {
    /// The text stored in `list_options.list_name`.
    pub const fn as_str(self) -> &'static str {
        match self {
            ListName::Category => "category",
            ListName::Status => "status",
            ListName::Priority => "priority",
            ListName::RecurringType => "recurring_type",
        }
    }
}

/// One option in a dropdown list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListOption {
    /// Id.
    pub id: ListOptionId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Which list.
    pub list_name: ListName,
    /// The text shown and stored on records.
    pub value: String,
    /// Position in the list (smaller first).
    pub sort_order: i32,
    /// Optional `#rrggbb` colour (used for statuses).
    pub color: Option<String>,
    /// Hidden from pickers but kept so history still shows it.
    pub hidden: bool,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// How often recurring work repeats: `months` then `days` are added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frequency {
    /// Id.
    pub id: FrequencyId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Label shown to the user, e.g. "Every 6 months".
    pub label: String,
    /// Whole months in the interval.
    pub months: u32,
    /// Extra days in the interval.
    pub days: u32,
    /// Position in the list.
    pub sort_order: i32,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted (hidden), if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A user-defined timesheet code: `Area > Phase > Code`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimesheetCode {
    /// Id.
    pub id: TimesheetCodeId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Top level, e.g. "Operations".
    pub area: String,
    /// Middle level label, e.g. "Phase".
    pub phase_label: String,
    /// The code itself, e.g. "1 - Support".
    pub code: String,
    /// Position in the timesheet grid.
    pub sort_order: i32,
    /// Hidden from pickers but kept for history.
    pub hidden: bool,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// Default timesheet code for a category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryCodeDefault {
    /// Id.
    pub id: CategoryCodeDefaultId,
    /// Owner.
    pub owner_id: OwnerId,
    /// The category list value.
    pub category: String,
    /// The code entries in this category get by default.
    pub timesheet_code_id: TimesheetCodeId,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A project with an optional due date and hour estimate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Id.
    pub id: ProjectId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Name (required); also its Group label.
    pub name: String,
    /// Longer description.
    pub description: String,
    /// Who asked for it.
    pub requested_by: Option<String>,
    /// When it should be finished.
    #[serde(with = "iso_date::option")]
    pub due_date: Option<Date>,
    /// Status list value.
    pub status: Option<String>,
    /// Priority list value.
    pub priority: Option<String>,
    /// Estimate in minutes (`None` = no estimate).
    pub estimated_minutes: Option<Minutes>,
    /// Timesheet code its entries get unless overridden.
    pub default_timesheet_code_id: Option<TimesheetCodeId>,
    /// Colour shown everywhere for this project, `#rrggbb` (`None` = neutral).
    pub color: Option<String>,
    /// Archived projects keep their history but leave the pickers.
    pub archived: bool,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// Recurring work (certificate renewals, patching, audits) or a routine bucket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurringTask {
    /// Id.
    pub id: RecurringTaskId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Name (required).
    pub name: String,
    /// Recurring type list value (blank groups as "Other").
    pub recurring_type: Option<String>,
    /// How often it repeats.
    pub frequency_id: Option<FrequencyId>,
    /// Expiration / due date, if known.
    #[serde(with = "iso_date::option")]
    pub expiration_date: Option<Date>,
    /// Days before the due date to warn (`None` = 30).
    pub warn_days_before: Option<u32>,
    /// Last done before the tracker existed.
    #[serde(with = "iso_date::option")]
    pub last_done_before_tracker: Option<Date>,
    /// False for routine buckets that only collect hours.
    pub alerts_enabled: bool,
    /// False when retired.
    pub active: bool,
    /// Free-text notes.
    pub notes: String,
    /// Timesheet code its entries get unless overridden.
    pub default_timesheet_code_id: Option<TimesheetCodeId>,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// What a time entry is logged against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum EntryTarget {
    /// A project.
    Project(ProjectId),
    /// A recurring task.
    Recurring(RecurringTaskId),
    /// Nothing ("No project").
    None,
}

/// The `target_type` column value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetType {
    /// Logged against a project.
    Project,
    /// Logged against a recurring task.
    Recurring,
    /// Not logged against anything.
    None,
}

impl TargetType {
    /// The text stored in `time_entries.target_type`.
    pub const fn as_str(self) -> &'static str {
        match self {
            TargetType::Project => "project",
            TargetType::Recurring => "recurring",
            TargetType::None => "none",
        }
    }
}

impl EntryTarget {
    /// The `target_type` column value.
    pub const fn target_type(&self) -> TargetType {
        match self {
            EntryTarget::Project(_) => TargetType::Project,
            EntryTarget::Recurring(_) => TargetType::Recurring,
            EntryTarget::None => TargetType::None,
        }
    }

    /// The `target_id` column value.
    pub fn target_id(&self) -> Option<uuid::Uuid> {
        match self {
            EntryTarget::Project(id) => Some(*id.as_uuid()),
            EntryTarget::Recurring(id) => Some(*id.as_uuid()),
            EntryTarget::None => None,
        }
    }

    /// Rebuild from the two columns. `None` if a project/recurring row has no id.
    pub fn from_columns(target_type: TargetType, target_id: Option<uuid::Uuid>) -> Option<Self> {
        match (target_type, target_id) {
            (TargetType::Project, Some(id)) => Some(EntryTarget::Project(id.into())),
            (TargetType::Recurring, Some(id)) => Some(EntryTarget::Recurring(id.into())),
            (TargetType::None, _) => Some(EntryTarget::None),
            _ => None,
        }
    }
}

/// One time log entry: rough hours for a piece of work on a date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeEntry {
    /// Id.
    pub id: TimeEntryId,
    /// Owner.
    pub owner_id: OwnerId,
    /// The local calendar date the work happened.
    #[serde(with = "iso_date")]
    pub date: Date,
    /// How long, in minutes (0 to 1440).
    pub minutes: Minutes,
    /// What it was logged against.
    pub target: EntryTarget,
    /// Category list value.
    pub category: Option<String>,
    /// What I worked on.
    pub description: String,
    /// Status list value.
    pub status: Option<String>,
    /// Priority list value.
    pub priority: Option<String>,
    /// Next step.
    pub next_step: Option<String>,
    /// Timesheet code chosen on the entry itself (override).
    pub timesheet_code_id: Option<TimesheetCodeId>,
    /// Ticket text from an import that could not be split.
    pub legacy_ticket_text: Option<String>,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

impl TimeEntry {
    /// True unless soft-deleted.
    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }
}

/// Whether a timesheet week has been submitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimesheetWeek {
    /// Id.
    pub id: TimesheetWeekId,
    /// Owner.
    pub owner_id: OwnerId,
    /// First day of the week.
    #[serde(with = "iso_date")]
    pub week_start: Date,
    /// When marked as submitted.
    #[serde(with = "rfc3339::option")]
    pub submitted_at: Option<OffsetDateTime>,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A ticket number attached to a time entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    /// Id.
    pub id: TicketId,
    /// Owner.
    pub owner_id: OwnerId,
    /// The entry it belongs to.
    pub time_entry_id: TimeEntryId,
    /// The ticket number/text as pasted (trimmed).
    pub ticket_number: String,
    /// Optional note.
    pub note: Option<String>,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A task in the task manager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Id.
    pub id: TaskId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Title (required).
    pub title: String,
    /// Notes.
    pub notes: String,
    /// Linked project.
    pub project_id: Option<ProjectId>,
    /// Linked recurring task.
    pub recurring_task_id: Option<RecurringTaskId>,
    /// Status list value.
    pub status: Option<String>,
    /// Priority list value.
    pub priority: Option<String>,
    /// When it is due.
    #[serde(with = "iso_date::option")]
    pub due_date: Option<Date>,
    /// When it was completed.
    #[serde(with = "rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    /// Position within its list or board column.
    pub sort_order: i32,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A checklist item inside a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItem {
    /// Id.
    pub id: ChecklistItemId,
    /// Owner.
    pub owner_id: OwnerId,
    /// The task it belongs to.
    pub task_id: TaskId,
    /// The item text.
    pub text: String,
    /// Ticked off.
    pub done: bool,
    /// Position in the checklist.
    pub sort_order: i32,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A ticket in the "Done tickets" tray for one day (see [`crate::tray`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayItem {
    /// Id.
    pub id: TrayItemId,
    /// Owner.
    pub owner_id: OwnerId,
    /// The day it was finished.
    #[serde(with = "iso_date")]
    pub date: Date,
    /// The ticket number, upper-cased.
    pub ticket_number: String,
    /// Minutes it adds to the day's roll-up entry.
    pub minutes: Minutes,
    /// Optional note.
    pub note: Option<String>,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A custom field definition (see [`crate::fields`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    /// Id.
    pub id: CustomFieldId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Which records it appears on.
    pub applies_to: AppliesTo,
    /// Name shown as its label.
    pub name: String,
    /// What kind of value it holds.
    pub kind: FieldKind,
    /// Choices (choice fields only).
    pub options: Vec<String>,
    /// Link template with `{ticket}`, e.g. for Jira.
    pub link_template: Option<String>,
    /// Tickets starting with this get a "copy link" option, e.g. `PROJ-`.
    pub ticket_prefix: Option<String>,
    /// Position in forms.
    pub sort_order: i32,
    /// Hidden from forms but kept.
    pub hidden: bool,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A custom field's value on one record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomValue {
    /// Id.
    pub id: CustomValueId,
    /// Owner.
    pub owner_id: OwnerId,
    /// The field.
    pub field_id: CustomFieldId,
    /// The entry, ticket or project id it belongs to.
    pub record_id: uuid::Uuid,
    /// The value as text.
    pub value: String,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A crib sheet line ticked off as entered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimesheetTick {
    /// Id.
    pub id: TimesheetTickId,
    /// Owner.
    pub owner_id: OwnerId,
    /// First day of the week.
    #[serde(with = "iso_date")]
    pub week_start: Date,
    /// The day of the line.
    #[serde(with = "iso_date")]
    pub day: Date,
    /// The line's key (see [`crate::sheet::SheetLine::key`]).
    pub line_key: String,
    /// Ticked.
    pub done: bool,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A scrap saved to the Inbox from capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    /// Id.
    pub id: InboxItemId,
    /// Owner.
    pub owner_id: OwnerId,
    /// What was typed.
    pub text: String,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// One thing on the "next up" list (see `crate::nextup`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextUpItem {
    /// Id.
    pub id: NextUpItemId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Place in the list, from 0.
    pub position: u32,
    /// What to do.
    pub text: String,
    /// When ticked off (UTC), if it is.
    #[serde(with = "rfc3339::option")]
    pub done_at: Option<OffsetDateTime>,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

/// A note (see `crate::notes`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// Id.
    pub id: NoteId,
    /// Owner.
    pub owner_id: OwnerId,
    /// Jot, Daily, Meeting, Page or MOP.
    pub kind: NoteKind,
    /// Title (never blank; see `crate::notes::note_title`).
    pub title: String,
    /// What's written in it.
    pub body: String,
    /// The project it belongs to, if any.
    pub project_id: Option<ProjectId>,
    /// Tags, cleaned by `crate::notes::parse_tags`.
    pub tags: Vec<String>,
    /// Keep was pressed: auto-archive leaves it alone.
    pub keep: bool,
    /// When it went to the archive, if it has.
    #[serde(with = "rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
    /// When created (UTC).
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
    /// When last changed (UTC).
    #[serde(with = "rfc3339")]
    pub updated_at: OffsetDateTime,
    /// When soft-deleted, if ever.
    #[serde(with = "rfc3339::option")]
    pub deleted_at: Option<OffsetDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_and_blocked_ignore_case_and_spaces() {
        assert!(is_done(Some("Done")));
        assert!(is_done(Some(" done ")));
        assert!(is_done(Some("DONE")));
        assert!(!is_done(Some("Done-ish")));
        assert!(!is_done(Some("")));
        assert!(!is_done(None));
        assert!(is_blocked(Some("blocked")));
        assert!(!is_blocked(Some("Waiting")));
        assert!(!is_blocked(None));
    }

    #[test]
    fn entry_target_columns_round_trip() {
        let p = ProjectId::new();
        let r = RecurringTaskId::new();
        for target in [
            EntryTarget::Project(p),
            EntryTarget::Recurring(r),
            EntryTarget::None,
        ] {
            let back = EntryTarget::from_columns(target.target_type(), target.target_id());
            assert_eq!(back, Some(target));
        }
        assert_eq!(
            EntryTarget::from_columns(TargetType::Project, None),
            None,
            "a project entry needs an id"
        );
        assert_eq!(EntryTarget::Project(p).target_type().as_str(), "project");
        assert_eq!(TargetType::Recurring.as_str(), "recurring");
        assert_eq!(TargetType::None.as_str(), "none");
    }

    #[test]
    fn list_names() {
        assert_eq!(ListName::Category.as_str(), "category");
        assert_eq!(ListName::RecurringType.as_str(), "recurring_type");
    }
}
