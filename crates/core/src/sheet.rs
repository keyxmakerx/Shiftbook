//! The timesheet crib sheet: "what to enter" for one week.
//!
//! For each day the week's entries are grouped by their resolved timesheet code
//! (see [`crate::timesheet`]) or by project. Each group is one line to type into
//! the user's timesheet system: the code, the hours and a ready-made comment
//! such as `Firewall cleanup (CHG0012345); Ticket queue (INC1, INC2)`.
//!
//! Rules:
//! - Only live entries dated inside the week count.
//! - Within a line, entries with the same title (ignoring case and spaces) are
//!   merged into one comment item; their tickets are combined without repeats.
//!   An entry without a title shows as "Untitled".
//! - By code, Unassigned comes first, then codes in their timesheet order
//!   (sort order, then area, then code). By project, projects are in name order
//!   and "No project" is last.
//! - Warnings: unassigned minutes in the week; a weekday (Monday to Friday)
//!   before today with nothing logged; a day over 12 hours.
//! - The week's ticket count counts each ticket number once (ignoring case).
//! - "Copy whole week" is tab-separated: weekday, date, code, hours, comment.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::{Date, Weekday};

use crate::dates::{format_date, iso_date};
use crate::group::NO_PROJECT;
use crate::hours::format_hours;
use crate::ids::{ProjectId, TimeEntryId, TimesheetCodeId};
use crate::minutes::Minutes;
use crate::model::{EntryTarget, Project, TimeEntry, TimesheetCode};
use crate::timesheet::{CodeDefaults, ResolvedCode};
use crate::week::week_days;

/// A day with more than this many minutes gets a warning.
pub const LONG_DAY_MINUTES: u32 = 12 * 60;
/// Shown for an entry without a description.
pub const UNTITLED: &str = "Untitled";
/// Line key for entries without a timesheet code.
pub const UNASSIGNED_KEY: &str = "unassigned";
/// Line key for entries without a project.
pub const NO_PROJECT_KEY: &str = "none";

/// How the crib sheet groups each day's entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// One line per timesheet code (the default).
    #[default]
    Code,
    /// One line per project.
    Project,
}

/// How comments are written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentFormat {
    /// Put the tickets in brackets after each title.
    pub include_tickets: bool,
    /// Between items, e.g. `"; "`.
    pub separator: String,
}

impl Default for CommentFormat {
    fn default() -> Self {
        Self {
            include_tickets: true,
            separator: "; ".to_string(),
        }
    }
}

/// An entry with its ticket numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryWithTickets {
    /// The entry.
    pub entry: TimeEntry,
    /// Its tickets, in order.
    pub tickets: Vec<String>,
    /// True for a day's "Tickets" roll-up entry (made from the Tickets tray), which takes
    /// the tickets code when nothing else gives it one.
    pub rollup: bool,
}

impl EntryWithTickets {
    /// The entry's code.
    pub fn resolve(&self, defaults: &CodeDefaults) -> ResolvedCode {
        defaults.resolve_for(&self.entry, self.rollup)
    }
}

/// Everything besides the entries that the sheet needs.
#[derive(Debug, Clone, Copy)]
pub struct SheetContext<'a> {
    /// First day of the week.
    pub week_start: Date,
    /// The user's local date (for "nothing logged" warnings).
    pub today: Date,
    /// Grouping.
    pub group_by: GroupBy,
    /// Comment style.
    pub format: &'a CommentFormat,
    /// All timesheet codes (hidden ones too, for labels).
    pub codes: &'a [TimesheetCode],
    /// All projects (archived ones too, for names).
    pub projects: &'a [Project],
    /// Defaults used to resolve codes.
    pub defaults: &'a CodeDefaults,
}

/// One comment item: entries with the same title merged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetItem {
    /// The title.
    pub title: String,
    /// Tickets, without repeats.
    pub tickets: Vec<String>,
    /// The entries behind it.
    pub entry_ids: Vec<TimeEntryId>,
    /// Their minutes.
    pub minutes: Minutes,
}

/// One line to type into the timesheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetLine {
    /// Stable key for tick-offs: a code id, a project id, `unassigned` or `none`.
    pub key: String,
    /// Small label above, e.g. `Operations › Phase`, `Project`, `No code yet`.
    pub area: String,
    /// The code or project name.
    pub label: String,
    /// Total minutes.
    pub minutes: Minutes,
    /// Total as decimal hours, e.g. `1.5`.
    pub hours: String,
    /// The ready-made comment.
    pub comment: String,
    /// The items in the comment.
    pub items: Vec<SheetItem>,
    /// True for the Unassigned line.
    pub unassigned: bool,
    /// The project, when grouping by project.
    pub project_id: Option<ProjectId>,
    /// Its entries that are a day's "Tickets" roll-up entry (from the Tickets tray).
    pub rollup_ids: Vec<TimeEntryId>,
}

/// One day of the week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetDay {
    /// The date.
    #[serde(with = "iso_date")]
    pub date: Date,
    /// Total minutes.
    pub minutes: Minutes,
    /// Total as decimal hours.
    pub hours: String,
    /// The lines, in order.
    pub lines: Vec<SheetLine>,
}

/// Minutes on one project in the week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEffort {
    /// The project (`None` = No project).
    pub project_id: Option<ProjectId>,
    /// Its name.
    pub name: String,
    /// Minutes.
    pub minutes: Minutes,
    /// As decimal hours.
    pub hours: String,
}

/// Something to check before submitting the timesheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SheetWarning {
    /// Minutes with no timesheet code.
    Unassigned {
        /// How many.
        minutes: Minutes,
        /// As decimal hours.
        hours: String,
    },
    /// A weekday before today with no entries.
    NothingLogged {
        /// The day.
        #[serde(with = "iso_date")]
        date: Date,
    },
    /// A day over 12 hours.
    LongDay {
        /// The day.
        #[serde(with = "iso_date")]
        date: Date,
        /// As decimal hours.
        hours: String,
    },
}

/// The whole crib sheet for one week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekSheet {
    /// First day.
    #[serde(with = "iso_date")]
    pub week_start: Date,
    /// How it is grouped.
    pub group_by: GroupBy,
    /// Seven days, in order.
    pub days: Vec<SheetDay>,
    /// Effort per project, largest first.
    pub effort: Vec<ProjectEffort>,
    /// Week total.
    pub minutes: Minutes,
    /// Week total as decimal hours.
    pub hours: String,
    /// Different tickets logged.
    pub ticket_count: usize,
    /// Entries logged.
    pub entry_count: usize,
    /// Things to check.
    pub warnings: Vec<SheetWarning>,
    /// The whole week, tab-separated, for pasting.
    pub tsv: String,
}

fn title_of(entry: &TimeEntry) -> String {
    let t = entry.description.trim();
    if t.is_empty() {
        UNTITLED.to_string()
    } else {
        t.to_string()
    }
}

fn push_unique(list: &mut Vec<String>, ticket: &str) {
    let ticket = ticket.trim();
    if !ticket.is_empty() && !list.iter().any(|t| t.eq_ignore_ascii_case(ticket)) {
        list.push(ticket.to_string());
    }
}

/// The comment for a list of items, e.g. `Title (T1, T2); Title2`.
pub fn comment(items: &[SheetItem], format: &CommentFormat) -> String {
    items
        .iter()
        .map(|item| {
            if format.include_tickets && !item.tickets.is_empty() {
                format!("{} ({})", item.title, item.tickets.join(", "))
            } else {
                item.title.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(&format.separator)
}

fn merge_items(entries: &[&EntryWithTickets]) -> Vec<SheetItem> {
    let mut items: Vec<SheetItem> = Vec::new();
    for e in entries {
        let title = title_of(&e.entry);
        let key = title.to_lowercase();
        let index = match items.iter().position(|i| i.title.to_lowercase() == key) {
            Some(i) => i,
            None => {
                items.push(SheetItem {
                    title,
                    tickets: Vec::new(),
                    entry_ids: Vec::new(),
                    minutes: Minutes::ZERO,
                });
                items.len() - 1
            }
        };
        let item = &mut items[index];
        for t in &e.tickets {
            push_unique(&mut item.tickets, t);
        }
        item.entry_ids.push(e.entry.id);
        item.minutes = item.minutes + e.entry.minutes;
    }
    items
}

/// Sort position of a line: (group, order, area, label).
type LineOrder = (u8, i64, String, String);

fn line_for_code(
    ctx: &SheetContext<'_>,
    code: Option<TimesheetCodeId>,
) -> (String, String, String, LineOrder, bool) {
    match code {
        None => (
            UNASSIGNED_KEY.to_string(),
            "No code yet".to_string(),
            "Unassigned".to_string(),
            (0, 0, String::new(), String::new()),
            true,
        ),
        Some(id) => match ctx.codes.iter().find(|c| c.id == id) {
            Some(c) => {
                let area = match (c.area.trim(), c.phase_label.trim()) {
                    ("", "") => "Timesheet code".to_string(),
                    (a, "") => a.to_string(),
                    ("", p) => p.to_string(),
                    (a, p) => format!("{a} › {p}"),
                };
                let order = (
                    1,
                    i64::from(c.sort_order),
                    c.area.to_lowercase(),
                    c.code.to_lowercase(),
                );
                (
                    id.to_string(),
                    area,
                    c.code.trim().to_string(),
                    order,
                    false,
                )
            }
            None => (
                id.to_string(),
                "Timesheet code".to_string(),
                "Removed code".to_string(),
                (2, 0, String::new(), id.to_string()),
                false,
            ),
        },
    }
}

fn project_name(projects: &[Project], id: Option<ProjectId>) -> String {
    id.and_then(|id| projects.iter().find(|p| p.id == id))
        .map(|p| p.name.trim().to_string())
        .unwrap_or_else(|| NO_PROJECT.to_string())
}

fn project_of(entry: &TimeEntry) -> Option<ProjectId> {
    match entry.target {
        EntryTarget::Project(id) => Some(id),
        _ => None,
    }
}

fn line_for_project(
    ctx: &SheetContext<'_>,
    project: Option<ProjectId>,
) -> (String, String, String, LineOrder, bool) {
    let name = project_name(ctx.projects, project);
    match project {
        Some(id) => (
            id.to_string(),
            "Project".to_string(),
            name.clone(),
            (1, 0, name.to_lowercase(), id.to_string()),
            false,
        ),
        None => (
            NO_PROJECT_KEY.to_string(),
            "Project".to_string(),
            name,
            (2, 0, String::new(), String::new()),
            false,
        ),
    }
}

fn day_lines(ctx: &SheetContext<'_>, entries: &[&EntryWithTickets]) -> Vec<SheetLine> {
    struct Group<'e> {
        key: String,
        area: String,
        label: String,
        order: LineOrder,
        unassigned: bool,
        project_id: Option<ProjectId>,
        entries: Vec<&'e EntryWithTickets>,
    }
    let mut groups: Vec<Group<'_>> = Vec::new();
    for e in entries {
        let (key, area, label, order, unassigned, project_id) = match ctx.group_by {
            GroupBy::Code => {
                let (k, a, l, o, u) = line_for_code(ctx, e.resolve(ctx.defaults).code);
                (k, a, l, o, u, None)
            }
            GroupBy::Project => {
                let p = project_of(&e.entry);
                let (k, a, l, o, u) = line_for_project(ctx, p);
                (k, a, l, o, u, p)
            }
        };
        match groups.iter_mut().find(|g| g.key == key) {
            Some(g) => g.entries.push(e),
            None => groups.push(Group {
                key,
                area,
                label,
                order,
                unassigned,
                project_id,
                entries: vec![e],
            }),
        }
    }
    groups.sort_by(|a, b| a.order.cmp(&b.order));
    groups
        .into_iter()
        .map(|g| {
            let items = merge_items(&g.entries);
            let minutes: Minutes = g.entries.iter().map(|e| e.entry.minutes).sum();
            SheetLine {
                comment: comment(&items, ctx.format),
                key: g.key,
                area: g.area,
                label: g.label,
                hours: format_hours(minutes),
                minutes,
                items,
                unassigned: g.unassigned,
                project_id: g.project_id,
                rollup_ids: g
                    .entries
                    .iter()
                    .filter(|e| e.rollup)
                    .map(|e| e.entry.id)
                    .collect(),
            }
        })
        .collect()
}

/// Short English weekday name, e.g. `Mon`.
pub fn weekday_short(date: Date) -> &'static str {
    match date.weekday() {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

fn is_weekday(date: Date) -> bool {
    !matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday)
}

fn cell(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c == '\t' || c == '\n' || c == '\r' {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// One line as tab-separated text: weekday, date, code, hours, comment.
pub fn line_tsv(date: Date, line: &SheetLine) -> String {
    let code = if line.unassigned {
        line.label.clone()
    } else {
        format!("{} › {}", line.area, line.label)
    };
    [
        weekday_short(date).to_string(),
        format_date(date),
        cell(&code),
        line.hours.clone(),
        cell(&line.comment),
    ]
    .join("\t")
}

/// Build the crib sheet for the week starting `ctx.week_start`.
pub fn build_week_sheet(ctx: &SheetContext<'_>, entries: &[EntryWithTickets]) -> WeekSheet {
    let dates = week_days(ctx.week_start);
    let live: Vec<&EntryWithTickets> = entries
        .iter()
        .filter(|e| e.entry.is_live() && dates.contains(&e.entry.date))
        .collect();

    let mut days = Vec::with_capacity(7);
    let mut warnings = Vec::new();
    for date in dates {
        let todays: Vec<&EntryWithTickets> = live
            .iter()
            .copied()
            .filter(|e| e.entry.date == date)
            .collect();
        let minutes: Minutes = todays.iter().map(|e| e.entry.minutes).sum();
        if todays.is_empty() && is_weekday(date) && date < ctx.today {
            warnings.push(SheetWarning::NothingLogged { date });
        }
        if minutes.get() > LONG_DAY_MINUTES {
            warnings.push(SheetWarning::LongDay {
                date,
                hours: format_hours(minutes),
            });
        }
        days.push(SheetDay {
            date,
            minutes,
            hours: format_hours(minutes),
            lines: day_lines(ctx, &todays),
        });
    }

    let unassigned: Minutes = live
        .iter()
        .filter(|e| e.resolve(ctx.defaults).code.is_none())
        .map(|e| e.entry.minutes)
        .sum();
    if !unassigned.is_zero() {
        warnings.insert(
            0,
            SheetWarning::Unassigned {
                minutes: unassigned,
                hours: format_hours(unassigned),
            },
        );
    }

    let mut by_project: HashMap<Option<ProjectId>, Minutes> = HashMap::new();
    for e in &live {
        let slot = by_project.entry(project_of(&e.entry)).or_default();
        *slot = *slot + e.entry.minutes;
    }
    let mut effort: Vec<ProjectEffort> = by_project
        .into_iter()
        .filter(|(_, m)| !m.is_zero())
        .map(|(project_id, minutes)| ProjectEffort {
            project_id,
            name: project_name(ctx.projects, project_id),
            minutes,
            hours: format_hours(minutes),
        })
        .collect();
    effort.sort_by(|a, b| {
        b.minutes
            .cmp(&a.minutes)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let mut tickets: Vec<String> = Vec::new();
    for e in &live {
        for t in &e.tickets {
            push_unique(&mut tickets, t);
        }
    }
    let minutes: Minutes = live.iter().map(|e| e.entry.minutes).sum();
    let tsv = days
        .iter()
        .flat_map(|d| d.lines.iter().map(|l| line_tsv(d.date, l)))
        .collect::<Vec<_>>()
        .join("\n");

    WeekSheet {
        week_start: ctx.week_start,
        group_by: ctx.group_by,
        days,
        effort,
        minutes,
        hours: format_hours(minutes),
        ticket_count: tickets.len(),
        entry_count: live.len(),
        warnings,
        tsv,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{CategoryCodeDefaultId, OwnerId};
    use crate::model::CategoryCodeDefault;
    use time::macros::{date, datetime};

    const MON: Date = date!(2026 - 10 - 12);
    const THU: Date = date!(2026 - 10 - 15);

    fn code(area: &str, phase: &str, name: &str, order: i32) -> TimesheetCode {
        TimesheetCode {
            id: TimesheetCodeId::new(),
            owner_id: OwnerId::new(),
            area: area.into(),
            phase_label: phase.into(),
            code: name.into(),
            sort_order: order,
            hidden: false,
            created_at: datetime!(2026-10-01 09:00 UTC),
            updated_at: datetime!(2026-10-01 09:00 UTC),
            deleted_at: None,
        }
    }

    fn project(name: &str, code: Option<TimesheetCodeId>) -> Project {
        Project {
            id: ProjectId::new(),
            owner_id: OwnerId::new(),
            name: name.into(),
            description: String::new(),
            requested_by: None,
            due_date: None,
            status: None,
            priority: None,
            estimated_minutes: None,
            default_timesheet_code_id: code,
            color: None,
            archived: false,
            created_at: datetime!(2026-10-01 09:00 UTC),
            updated_at: datetime!(2026-10-01 09:00 UTC),
            deleted_at: None,
        }
    }

    fn entry(
        date: Date,
        minutes: u32,
        title: &str,
        p: Option<&Project>,
        tickets: &[&str],
    ) -> EntryWithTickets {
        EntryWithTickets {
            entry: TimeEntry {
                id: TimeEntryId::new(),
                owner_id: OwnerId::new(),
                date,
                minutes: Minutes(minutes),
                target: p.map_or(EntryTarget::None, |p| EntryTarget::Project(p.id)),
                category: None,
                description: title.into(),
                status: None,
                priority: None,
                next_step: None,
                timesheet_code_id: None,
                legacy_ticket_text: None,
                created_at: datetime!(2026-10-12 09:00 UTC),
                updated_at: datetime!(2026-10-12 09:00 UTC),
                deleted_at: None,
            },
            tickets: tickets.iter().map(|t| t.to_string()).collect(),
            rollup: false,
        }
    }

    struct Fixture {
        support: TimesheetCode,
        maint: TimesheetCode,
        net: Project,
        tickets: Project,
        admin: Project,
        codes: Vec<TimesheetCode>,
        projects: Vec<Project>,
        defaults: CodeDefaults,
    }

    fn fixture() -> Fixture {
        let support = code("Operations", "Phase", "1 - Support", 1);
        let maint = code("Operations", "", "2 - Maintenance", 2);
        let net = project("Network refresh", Some(maint.id));
        let tickets = project("Tickets", Some(support.id));
        let admin = project("Admin", None);
        let codes = vec![maint.clone(), support.clone()];
        let projects = vec![net.clone(), tickets.clone(), admin.clone()];
        let defaults = CodeDefaults::new(&projects, &[], &[]);
        Fixture {
            support,
            maint,
            net,
            tickets,
            admin,
            codes,
            projects,
            defaults,
        }
    }

    fn ctx<'a>(f: &'a Fixture, group_by: GroupBy, format: &'a CommentFormat) -> SheetContext<'a> {
        SheetContext {
            week_start: MON,
            today: THU,
            group_by,
            format,
            codes: &f.codes,
            projects: &f.projects,
            defaults: &f.defaults,
        }
    }

    #[test]
    fn groups_by_code_with_comments() {
        let f = fixture();
        let format = CommentFormat::default();
        let entries = vec![
            entry(THU, 90, "Firewall cleanup", Some(&f.net), &["CHG0012345"]),
            entry(THU, 60, "Ticket queue", Some(&f.tickets), &["INC1", "INC2"]),
            entry(
                THU,
                30,
                "ticket queue ",
                Some(&f.tickets),
                &["inc2", "INC3"],
            ),
            entry(THU, 45, "Expenses", Some(&f.admin), &[]),
            entry(THU, 15, "", None, &[]),
        ];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        let day = &sheet.days[3];
        assert_eq!(day.date, THU);
        assert_eq!(day.minutes, Minutes(240));
        assert_eq!(day.hours, "4");
        let keys: Vec<&str> = day.lines.iter().map(|l| l.label.as_str()).collect();
        assert_eq!(keys, ["Unassigned", "1 - Support", "2 - Maintenance"]);

        let unassigned = &day.lines[0];
        assert!(unassigned.unassigned);
        assert_eq!(unassigned.key, UNASSIGNED_KEY);
        assert_eq!(unassigned.area, "No code yet");
        assert_eq!(unassigned.comment, "Expenses; Untitled");
        assert_eq!(unassigned.hours, "1");

        let support = &day.lines[1];
        assert_eq!(support.key, f.support.id.to_string());
        assert_eq!(support.area, "Operations › Phase");
        assert_eq!(support.minutes, Minutes(90));
        assert_eq!(support.hours, "1.5");
        assert_eq!(support.items.len(), 1, "same title merged");
        assert_eq!(support.items[0].entry_ids.len(), 2);
        assert_eq!(support.comment, "Ticket queue (INC1, INC2, INC3)");

        let maint = &day.lines[2];
        assert_eq!(maint.key, f.maint.id.to_string());
        assert_eq!(maint.area, "Operations");
        assert_eq!(maint.comment, "Firewall cleanup (CHG0012345)");

        assert_eq!(sheet.minutes, Minutes(240));
        assert_eq!(sheet.ticket_count, 4);
        assert_eq!(sheet.entry_count, 5);
        assert_eq!(sheet.days.len(), 7);
    }

    #[test]
    fn comment_format_options() {
        let f = fixture();
        let format = CommentFormat {
            include_tickets: false,
            separator: " / ".into(),
        };
        let entries = vec![
            entry(THU, 60, "Queue", Some(&f.tickets), &["INC1"]),
            entry(THU, 60, "Call", Some(&f.tickets), &["INC2"]),
        ];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        assert_eq!(sheet.days[3].lines[0].comment, "Queue / Call");
    }

    #[test]
    fn groups_by_project() {
        let f = fixture();
        let format = CommentFormat::default();
        let entries = vec![
            entry(MON, 60, "Queue", Some(&f.tickets), &[]),
            entry(MON, 30, "Lunch talk", None, &[]),
            entry(MON, 120, "Expenses", Some(&f.admin), &[]),
            entry(MON, 45, "Switches", Some(&f.net), &[]),
        ];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Project, &format), &entries);
        let labels: Vec<&str> = sheet.days[0]
            .lines
            .iter()
            .map(|l| l.label.as_str())
            .collect();
        assert_eq!(
            labels,
            ["Admin", "Network refresh", "Tickets", "No project"]
        );
        assert_eq!(sheet.days[0].lines[0].project_id, Some(f.admin.id));
        assert_eq!(sheet.days[0].lines[3].key, NO_PROJECT_KEY);
        assert_eq!(sheet.group_by, GroupBy::Project);
    }

    #[test]
    fn effort_by_project_largest_first() {
        let f = fixture();
        let format = CommentFormat::default();
        let entries = vec![
            entry(MON, 60, "a", Some(&f.tickets), &[]),
            entry(date!(2026 - 10 - 13), 120, "b", Some(&f.net), &[]),
            entry(THU, 60, "c", Some(&f.tickets), &[]),
            entry(THU, 60, "d", None, &[]),
        ];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        let effort: Vec<(&str, u32)> = sheet
            .effort
            .iter()
            .map(|e| (e.name.as_str(), e.minutes.get()))
            .collect();
        assert_eq!(
            effort,
            [
                ("Network refresh", 120),
                ("Tickets", 120),
                ("No project", 60)
            ]
        );
        assert_eq!(sheet.hours, "5");
    }

    #[test]
    fn warnings() {
        let f = fixture();
        let format = CommentFormat::default();
        let entries = vec![
            // Monday: 13 h, all assigned.
            entry(MON, 780, "Outage", Some(&f.net), &[]),
            // Tuesday and Wednesday: nothing. Thursday is today, also nothing.
            // Saturday (before or after today) never warns.
            entry(date!(2026 - 10 - 16), 30, "Plan", Some(&f.admin), &[]),
        ];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        assert_eq!(
            sheet.warnings,
            vec![
                SheetWarning::Unassigned {
                    minutes: Minutes(30),
                    hours: "0.5".into()
                },
                SheetWarning::LongDay {
                    date: MON,
                    hours: "13".into()
                },
                SheetWarning::NothingLogged {
                    date: date!(2026 - 10 - 13)
                },
                SheetWarning::NothingLogged {
                    date: date!(2026 - 10 - 14)
                },
            ]
        );
        let exactly_twelve = vec![entry(MON, 720, "Long", Some(&f.net), &[])];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &exactly_twelve);
        assert!(!sheet
            .warnings
            .iter()
            .any(|w| matches!(w, SheetWarning::LongDay { .. })));
    }

    #[test]
    fn ignores_deleted_and_other_weeks() {
        let f = fixture();
        let format = CommentFormat::default();
        let mut gone = entry(THU, 60, "Gone", Some(&f.net), &["INC9"]);
        gone.entry.deleted_at = Some(datetime!(2026-10-15 10:00 UTC));
        let entries = vec![
            gone,
            entry(date!(2026 - 10 - 11), 60, "Last week", Some(&f.net), &[]),
            entry(date!(2026 - 10 - 19), 60, "Next week", Some(&f.net), &[]),
        ];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        assert_eq!(sheet.entry_count, 0);
        assert_eq!(sheet.minutes, Minutes::ZERO);
        assert_eq!(sheet.ticket_count, 0);
        assert!(sheet.days.iter().all(|d| d.lines.is_empty()));
        assert_eq!(sheet.tsv, "");
    }

    #[test]
    fn category_and_entry_codes_resolve() {
        let mut f = fixture();
        let mapping = CategoryCodeDefault {
            id: CategoryCodeDefaultId::new(),
            owner_id: OwnerId::new(),
            category: "Meetings".into(),
            timesheet_code_id: f.support.id,
            created_at: datetime!(2026-10-01 09:00 UTC),
            updated_at: datetime!(2026-10-01 09:00 UTC),
            deleted_at: None,
        };
        f.defaults = CodeDefaults::new(&f.projects, &[], &[mapping]);
        let format = CommentFormat::default();
        let mut meeting = entry(THU, 60, "Sync", None, &[]);
        meeting.entry.category = Some("meetings".into());
        let mut chosen = entry(THU, 30, "Override", Some(&f.tickets), &[]);
        chosen.entry.timesheet_code_id = Some(f.maint.id);
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &[meeting, chosen]);
        let labels: Vec<&str> = sheet.days[3]
            .lines
            .iter()
            .map(|l| l.label.as_str())
            .collect();
        assert_eq!(labels, ["1 - Support", "2 - Maintenance"]);
        assert!(sheet
            .warnings
            .iter()
            .all(|w| !matches!(w, SheetWarning::Unassigned { .. })));
    }

    #[test]
    fn tickets_rollup_takes_the_tickets_code() {
        let mut f = fixture();
        let format = CommentFormat::default();
        let mut rollup = entry(THU, 90, "Tickets", None, &["INC1", "INC2"]);
        rollup.rollup = true;
        let rollup_id = rollup.entry.id;
        let other = entry(THU, 30, "Misc", None, &[]);
        let entries = vec![rollup, other];
        // No tickets code yet: both are Unassigned, and the line says it holds tickets.
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        let line = &sheet.days[3].lines[0];
        assert!(line.unassigned);
        assert_eq!(line.rollup_ids, [rollup_id]);
        assert_eq!(line.minutes, Minutes(120));
        // With one, the roll-up moves to it and only "Misc" is left Unassigned.
        f.defaults = f.defaults.clone().with_tickets(Some(f.support.id));
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        let lines = &sheet.days[3].lines;
        // Unassigned sorts first.
        assert!(lines[0].unassigned && lines[0].rollup_ids.is_empty());
        assert_eq!(lines[1].label, "1 - Support");
        assert_eq!(lines[1].rollup_ids, [rollup_id]);
        assert!(sheet.warnings.iter().any(
            |w| matches!(w, SheetWarning::Unassigned { minutes, .. } if *minutes == Minutes(30))
        ));
    }

    #[test]
    fn removed_codes_still_show() {
        let f = fixture();
        let format = CommentFormat::default();
        let mut e = entry(THU, 60, "Old", None, &[]);
        e.entry.timesheet_code_id = Some(TimesheetCodeId::new());
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &[e]);
        assert_eq!(sheet.days[3].lines[0].label, "Removed code");
    }

    #[test]
    fn copy_week_is_tab_separated() {
        let f = fixture();
        let format = CommentFormat::default();
        let entries = vec![
            entry(MON, 90, "Fire\twall\nwork", Some(&f.net), &["CHG1"]),
            entry(THU, 30, "Misc", None, &[]),
        ];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        assert_eq!(
            sheet.tsv,
            "Mon\t2026-10-12\tOperations › 2 - Maintenance\t1.5\tFire wall work (CHG1)\n\
             Thu\t2026-10-15\tUnassigned\t0.5\tMisc"
        );
    }

    #[test]
    fn serializes_for_the_ui() {
        let f = fixture();
        let format = CommentFormat::default();
        let entries = vec![entry(MON, 60, "Queue", None, &[])];
        let sheet = build_week_sheet(&ctx(&f, GroupBy::Code, &format), &entries);
        let json = serde_json::to_value(&sheet).unwrap();
        assert_eq!(json["weekStart"], "2026-10-12");
        assert_eq!(json["groupBy"], "code");
        assert_eq!(json["days"][0]["lines"][0]["key"], "unassigned");
        assert_eq!(json["warnings"][0]["kind"], "unassigned");
        assert_eq!(json["warnings"][1]["kind"], "nothing_logged");
        assert_eq!(json["warnings"][1]["date"], "2026-10-13");
    }
}
