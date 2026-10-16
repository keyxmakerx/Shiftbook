//! Timesheet code resolution and totals (SPEC §2.8, §4).
//!
//! An entry's code, first match wins:
//! 1. the code chosen on the entry itself (override)
//! 2. the default code of its project or recurring task
//! 3. the default code of its category (category names match ignoring case
//!    and surrounding spaces)
//! 4. otherwise Unassigned
//!
//! Totals are minutes summed per (resolved code, date), live entries only.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use time::Date;

use crate::dates::iso_date;
use crate::ids::{ProjectId, RecurringTaskId, TimesheetCodeId};
use crate::minutes::Minutes;
use crate::model::{CategoryCodeDefault, EntryTarget, Project, RecurringTask, TimeEntry};

/// Where an entry's timesheet code came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeSource {
    /// Chosen on the entry itself.
    Entry,
    /// Default of its project or recurring task.
    Target,
    /// Default of its category.
    Category,
    /// No code found.
    Unassigned,
}

/// An entry's resolved code and where it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResolvedCode {
    /// The code, or `None` when Unassigned.
    pub code: Option<TimesheetCodeId>,
    /// Which rule supplied it.
    pub source: CodeSource,
}

/// First match wins: entry override → target default → category default → Unassigned.
pub fn resolve_code(
    entry_override: Option<TimesheetCodeId>,
    target_default: Option<TimesheetCodeId>,
    category_default: Option<TimesheetCodeId>,
) -> ResolvedCode {
    let (code, source) = match (entry_override, target_default, category_default) {
        (Some(c), _, _) => (Some(c), CodeSource::Entry),
        (None, Some(c), _) => (Some(c), CodeSource::Target),
        (None, None, Some(c)) => (Some(c), CodeSource::Category),
        (None, None, None) => (None, CodeSource::Unassigned),
    };
    ResolvedCode { code, source }
}

fn category_key(category: &str) -> String {
    category.trim().to_lowercase()
}

/// The defaults needed to resolve codes for many entries at once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeDefaults {
    projects: HashMap<ProjectId, TimesheetCodeId>,
    recurring: HashMap<RecurringTaskId, TimesheetCodeId>,
    categories: HashMap<String, TimesheetCodeId>,
}

impl CodeDefaults {
    /// Collect defaults from projects, recurring tasks and category mappings.
    /// Deleted records are ignored.
    pub fn new(
        projects: &[Project],
        recurring: &[RecurringTask],
        categories: &[CategoryCodeDefault],
    ) -> Self {
        Self {
            projects: projects
                .iter()
                .filter(|p| p.deleted_at.is_none())
                .filter_map(|p| p.default_timesheet_code_id.map(|c| (p.id, c)))
                .collect(),
            recurring: recurring
                .iter()
                .filter(|r| r.deleted_at.is_none())
                .filter_map(|r| r.default_timesheet_code_id.map(|c| (r.id, c)))
                .collect(),
            categories: categories
                .iter()
                .filter(|c| c.deleted_at.is_none())
                .map(|c| (category_key(&c.category), c.timesheet_code_id))
                .collect(),
        }
    }

    /// The default code of what the entry is logged against.
    pub fn target_default(&self, target: EntryTarget) -> Option<TimesheetCodeId> {
        match target {
            EntryTarget::Project(id) => self.projects.get(&id).copied(),
            EntryTarget::Recurring(id) => self.recurring.get(&id).copied(),
            EntryTarget::None => None,
        }
    }

    /// The default code of a category.
    pub fn category_default(&self, category: Option<&str>) -> Option<TimesheetCodeId> {
        category.and_then(|c| self.categories.get(&category_key(c)).copied())
    }

    /// Resolve one entry's code.
    pub fn resolve(&self, entry: &TimeEntry) -> ResolvedCode {
        resolve_code(
            entry.timesheet_code_id,
            self.target_default(entry.target),
            self.category_default(entry.category.as_deref()),
        )
    }
}

/// Minutes for one timesheet code on one date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimesheetCell {
    /// The code, or `None` for Unassigned.
    pub code: Option<TimesheetCodeId>,
    /// The date.
    #[serde(with = "iso_date")]
    pub date: Date,
    /// Total minutes.
    pub minutes: Minutes,
}

/// Sum live entries' minutes per (resolved code, date). The caller picks the
/// entries (e.g. one week). Unassigned cells come first, then by code id and date.
pub fn timesheet_totals(entries: &[TimeEntry], defaults: &CodeDefaults) -> Vec<TimesheetCell> {
    let mut totals: BTreeMap<(Option<TimesheetCodeId>, Date), Minutes> = BTreeMap::new();
    for entry in entries.iter().filter(|e| e.is_live()) {
        let code = defaults.resolve(entry).code;
        let cell = totals.entry((code, entry.date)).or_default();
        *cell = *cell + entry.minutes;
    }
    totals
        .into_iter()
        .map(|((code, date), minutes)| TimesheetCell {
            code,
            date,
            minutes,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{CategoryCodeDefaultId, OwnerId, TimeEntryId};
    use time::macros::{date, datetime};

    fn entry(
        date: Date,
        minutes: u32,
        target: EntryTarget,
        category: Option<&str>,
        code: Option<TimesheetCodeId>,
    ) -> TimeEntry {
        TimeEntry {
            id: TimeEntryId::new(),
            owner_id: OwnerId::new(),
            date,
            minutes: Minutes(minutes),
            target,
            category: category.map(String::from),
            description: String::new(),
            status: None,
            priority: None,
            next_step: None,
            timesheet_code_id: code,
            legacy_ticket_text: None,
            created_at: datetime!(2026-10-15 09:00 UTC),
            updated_at: datetime!(2026-10-15 09:00 UTC),
            deleted_at: None,
        }
    }

    fn project(code: Option<TimesheetCodeId>) -> Project {
        Project {
            id: ProjectId::new(),
            owner_id: OwnerId::new(),
            name: "Website".into(),
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

    fn recurring(code: Option<TimesheetCodeId>) -> RecurringTask {
        RecurringTask {
            id: RecurringTaskId::new(),
            owner_id: OwnerId::new(),
            name: "Patching".into(),
            recurring_type: None,
            frequency_id: None,
            expiration_date: None,
            warn_days_before: None,
            last_done_before_tracker: None,
            alerts_enabled: true,
            active: true,
            notes: String::new(),
            default_timesheet_code_id: code,
            created_at: datetime!(2026-10-01 09:00 UTC),
            updated_at: datetime!(2026-10-01 09:00 UTC),
            deleted_at: None,
        }
    }

    fn mapping(category: &str, code: TimesheetCodeId) -> CategoryCodeDefault {
        CategoryCodeDefault {
            id: CategoryCodeDefaultId::new(),
            owner_id: OwnerId::new(),
            category: category.into(),
            timesheet_code_id: code,
            created_at: datetime!(2026-10-01 09:00 UTC),
            updated_at: datetime!(2026-10-01 09:00 UTC),
            deleted_at: None,
        }
    }

    #[test]
    fn resolve_order() {
        let a = Some(TimesheetCodeId::new());
        let b = Some(TimesheetCodeId::new());
        let c = Some(TimesheetCodeId::new());
        use CodeSource::*;
        let cases = [
            (a, b, c, a, Entry),
            (a, None, None, a, Entry),
            (None, b, c, b, Target),
            (None, b, None, b, Target),
            (None, None, c, c, Category),
            (None, None, None, None, Unassigned),
        ];
        for (entry_code, target, category, code, source) in cases {
            assert_eq!(
                resolve_code(entry_code, target, category),
                ResolvedCode { code, source }
            );
        }
    }

    #[test]
    fn resolve_from_records() {
        let support = TimesheetCodeId::new();
        let projects_code = TimesheetCodeId::new();
        let patch_code = TimesheetCodeId::new();
        let override_code = TimesheetCodeId::new();
        let p_with = project(Some(projects_code));
        let p_without = project(None);
        let r_with = recurring(Some(patch_code));
        let mut p_deleted = project(Some(projects_code));
        p_deleted.deleted_at = Some(datetime!(2026-10-02 09:00 UTC));
        let mut old_mapping = mapping("Meetings", support);
        old_mapping.deleted_at = Some(datetime!(2026-10-02 09:00 UTC));
        let defaults = CodeDefaults::new(
            &[p_with.clone(), p_without.clone(), p_deleted.clone()],
            std::slice::from_ref(&r_with),
            &[mapping("Support / Tickets", support), old_mapping],
        );
        let d = date!(2026 - 10 - 15);
        let cases = [
            (
                entry(
                    d,
                    60,
                    EntryTarget::Project(p_with.id),
                    Some("Support / Tickets"),
                    Some(override_code),
                ),
                Some(override_code),
                CodeSource::Entry,
            ),
            (
                entry(
                    d,
                    60,
                    EntryTarget::Project(p_with.id),
                    Some("Support / Tickets"),
                    None,
                ),
                Some(projects_code),
                CodeSource::Target,
            ),
            (
                entry(d, 60, EntryTarget::Recurring(r_with.id), None, None),
                Some(patch_code),
                CodeSource::Target,
            ),
            (
                entry(
                    d,
                    60,
                    EntryTarget::Project(p_without.id),
                    Some(" support / TICKETS "),
                    None,
                ),
                Some(support),
                CodeSource::Category,
            ),
            (
                entry(d, 60, EntryTarget::None, Some("Support / Tickets"), None),
                Some(support),
                CodeSource::Category,
            ),
            (
                entry(
                    d,
                    60,
                    EntryTarget::Project(p_deleted.id),
                    Some("Meetings"),
                    None,
                ),
                None,
                CodeSource::Unassigned,
            ),
            (
                entry(d, 60, EntryTarget::None, None, None),
                None,
                CodeSource::Unassigned,
            ),
        ];
        for (i, (e, code, source)) in cases.into_iter().enumerate() {
            assert_eq!(
                defaults.resolve(&e),
                ResolvedCode { code, source },
                "case {i}"
            );
        }
    }

    #[test]
    fn totals_group_by_code_and_date() {
        let support = TimesheetCodeId::new();
        let projects_code = TimesheetCodeId::new();
        let p = project(Some(projects_code));
        let defaults = CodeDefaults::new(
            std::slice::from_ref(&p),
            &[],
            &[mapping("Support", support)],
        );
        let mon = date!(2026 - 10 - 12);
        let tue = date!(2026 - 10 - 13);
        let mut deleted = entry(mon, 999, EntryTarget::None, Some("Support"), None);
        deleted.deleted_at = Some(datetime!(2026-10-13 09:00 UTC));
        let entries = vec![
            entry(mon, 60, EntryTarget::None, Some("Support"), None),
            entry(mon, 30, EntryTarget::None, Some("support"), None),
            entry(mon, 120, EntryTarget::Project(p.id), Some("Support"), None),
            entry(tue, 45, EntryTarget::None, Some("Support"), None),
            entry(tue, 15, EntryTarget::None, None, None),
            entry(tue, 10, EntryTarget::None, Some("Unmapped"), None),
            entry(
                mon,
                20,
                EntryTarget::None,
                Some("Support"),
                Some(projects_code),
            ),
            deleted,
        ];
        let totals = timesheet_totals(&entries, &defaults);
        let get = |code: Option<TimesheetCodeId>, date: Date| {
            totals
                .iter()
                .find(|c| c.code == code && c.date == date)
                .map(|c| c.minutes)
        };
        assert_eq!(get(Some(support), mon), Some(Minutes(90)));
        assert_eq!(get(Some(support), tue), Some(Minutes(45)));
        assert_eq!(get(Some(projects_code), mon), Some(Minutes(140)));
        assert_eq!(get(None, tue), Some(Minutes(25)));
        assert_eq!(get(None, mon), None);
        assert_eq!(totals.len(), 4);
        assert_eq!(totals[0].code, None, "Unassigned first");
        let sum: Minutes = totals.iter().map(|c| c.minutes).sum();
        assert_eq!(sum, Minutes(60 + 30 + 120 + 45 + 15 + 10 + 20));
    }

    #[test]
    fn totals_of_nothing_is_empty() {
        assert!(timesheet_totals(&[], &CodeDefaults::default()).is_empty());
    }
}
