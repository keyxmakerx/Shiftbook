//! Timesheet code resolution and totals (SPEC §2.8, §4).
//!
//! An entry's code, first match wins:
//! 1. the code chosen on the entry itself (override)
//! 2. the default code of its project or recurring task
//! 3. for a recurring task, the default code of its kind ("Certificates"); kind names
//!    match ignoring case and surrounding spaces
//! 4. the default code of its category (category names match ignoring case
//!    and surrounding spaces)
//! 5. for a day's "Tickets" roll-up entry (made from the Tickets tray), the
//!    tickets code from Customize → Timesheet
//! 6. otherwise Unassigned
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum CodeSource {
    /// Chosen on the entry itself.
    Entry,
    /// Default of its project or recurring task.
    Target,
    /// Default of its recurring task's kind.
    Kind,
    /// Default of its category.
    Category,
    /// The tickets code (the day's "Tickets" roll-up entry only).
    Tickets,
    /// No code found.
    Unassigned,
}

/// An entry's resolved code and where it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ResolvedCode {
    /// The code, or `None` when Unassigned.
    pub code: Option<TimesheetCodeId>,
    /// Which rule supplied it.
    pub source: CodeSource,
}

/// First match wins: entry override → target default → kind default → category default →
/// tickets code → Unassigned. `tickets_default` is only passed for a "Tickets" roll-up entry.
pub fn resolve_code(
    entry_override: Option<TimesheetCodeId>,
    target_default: Option<TimesheetCodeId>,
    kind_default: Option<TimesheetCodeId>,
    category_default: Option<TimesheetCodeId>,
    tickets_default: Option<TimesheetCodeId>,
) -> ResolvedCode {
    let found = [
        (entry_override, CodeSource::Entry),
        (target_default, CodeSource::Target),
        (kind_default, CodeSource::Kind),
        (category_default, CodeSource::Category),
        (tickets_default, CodeSource::Tickets),
    ]
    .into_iter()
    .find_map(|(code, source)| code.map(|c| (c, source)));
    match found {
        Some((c, source)) => ResolvedCode {
            code: Some(c),
            source,
        },
        None => ResolvedCode {
            code: None,
            source: CodeSource::Unassigned,
        },
    }
}

fn category_key(category: &str) -> String {
    category.trim().to_lowercase()
}

/// The defaults needed to resolve codes for many entries at once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct CodeDefaults {
    projects: HashMap<ProjectId, TimesheetCodeId>,
    recurring: HashMap<RecurringTaskId, TimesheetCodeId>,
    /// Each recurring task's kind, keyed like `kinds`.
    recurring_kinds: HashMap<RecurringTaskId, String>,
    kinds: HashMap<String, TimesheetCodeId>,
    categories: HashMap<String, TimesheetCodeId>,
    tickets: Option<TimesheetCodeId>,
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
            recurring_kinds: recurring
                .iter()
                .filter(|r| r.deleted_at.is_none())
                .filter_map(|r| r.recurring_type.as_deref().map(|k| (r.id, category_key(k))))
                .filter(|(_, k)| !k.is_empty())
                .collect(),
            kinds: HashMap::new(),
            categories: categories
                .iter()
                .filter(|c| c.deleted_at.is_none())
                .map(|c| (category_key(&c.category), c.timesheet_code_id))
                .collect(),
            tickets: None,
        }
    }

    /// Add the tickets code, used by a day's "Tickets" roll-up entry.
    pub fn with_tickets(mut self, tickets: Option<TimesheetCodeId>) -> Self {
        self.tickets = tickets;
        self
    }

    /// Add the kind codes (kind name → code), used by recurring tasks of that kind.
    pub fn with_kinds(mut self, kinds: &[(String, TimesheetCodeId)]) -> Self {
        self.kinds = kinds
            .iter()
            .map(|(kind, code)| (category_key(kind), *code))
            .collect();
        self
    }

    /// The tickets code, if one is set.
    pub fn tickets_default(&self) -> Option<TimesheetCodeId> {
        self.tickets
    }

    /// The default code of what the entry is logged against.
    pub fn target_default(&self, target: EntryTarget) -> Option<TimesheetCodeId> {
        match target {
            EntryTarget::Project(id) => self.projects.get(&id).copied(),
            EntryTarget::Recurring(id) => self.recurring.get(&id).copied(),
            EntryTarget::None => None,
        }
    }

    /// The default code of a recurring task's kind (`None` for anything else).
    pub fn kind_default(&self, target: EntryTarget) -> Option<TimesheetCodeId> {
        match target {
            EntryTarget::Recurring(id) => self
                .recurring_kinds
                .get(&id)
                .and_then(|k| self.kinds.get(k))
                .copied(),
            _ => None,
        }
    }

    /// The default code of a category.
    pub fn category_default(&self, category: Option<&str>) -> Option<TimesheetCodeId> {
        category.and_then(|c| self.categories.get(&category_key(c)).copied())
    }

    /// The code an entry gets when none is chosen on it: what "Use the usual code" means.
    pub fn usual(&self, target: EntryTarget, category: Option<&str>, rollup: bool) -> ResolvedCode {
        resolve_code(
            None,
            self.target_default(target),
            self.kind_default(target),
            self.category_default(category),
            if rollup { self.tickets } else { None },
        )
    }

    /// Resolve one ordinary entry's code.
    pub fn resolve(&self, entry: &TimeEntry) -> ResolvedCode {
        self.resolve_for(entry, false)
    }

    /// Resolve one entry's code; `rollup` is true for a day's "Tickets" roll-up entry,
    /// which falls back to the tickets code.
    pub fn resolve_for(&self, entry: &TimeEntry, rollup: bool) -> ResolvedCode {
        resolve_code(
            entry.timesheet_code_id,
            self.target_default(entry.target),
            self.kind_default(entry.target),
            self.category_default(entry.category.as_deref()),
            if rollup { self.tickets } else { None },
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
        let k = Some(TimesheetCodeId::new());
        let t = Some(TimesheetCodeId::new());
        use CodeSource::*;
        let cases = [
            (a, b, k, c, t, a, Entry),
            (a, None, None, None, None, a, Entry),
            (None, b, k, c, t, b, Target),
            (None, b, None, None, None, b, Target),
            (None, None, k, c, t, k, Kind),
            (None, None, None, c, t, c, Category),
            (None, None, None, None, t, t, Tickets),
            (None, None, None, None, None, None, Unassigned),
        ];
        for (entry_code, target, kind, category, tickets, code, source) in cases {
            assert_eq!(
                resolve_code(entry_code, target, kind, category, tickets),
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
    fn a_repeating_things_kind_has_a_code() {
        let certs = TimesheetCodeId::new();
        let own = TimesheetCodeId::new();
        let support = TimesheetCodeId::new();
        let mut plain = recurring(None);
        plain.recurring_type = Some("Certificates".into());
        let mut coded = recurring(Some(own));
        coded.recurring_type = Some("certificates".into());
        let mut other = recurring(None);
        other.recurring_type = Some("Audits".into());
        let defaults = CodeDefaults::new(
            &[],
            &[plain.clone(), coded.clone(), other.clone()],
            &[mapping("Support", support)],
        )
        .with_kinds(&[(" CERTIFICATES ".into(), certs)]);
        let d = date!(2026 - 10 - 15);
        let on = |r: &RecurringTask, cat: Option<&str>| {
            entry(d, 30, EntryTarget::Recurring(r.id), cat, None)
        };
        // The thing's own code wins, then its kind's, then the category's.
        assert_eq!(defaults.resolve(&on(&coded, None)).code, Some(own));
        assert_eq!(
            defaults.resolve(&on(&plain, Some("Support"))),
            ResolvedCode {
                code: Some(certs),
                source: CodeSource::Kind
            }
        );
        assert_eq!(
            defaults.resolve(&on(&other, Some("Support"))).source,
            CodeSource::Category
        );
        // A project never takes a kind's code.
        assert_eq!(defaults.kind_default(EntryTarget::None), None);
        assert_eq!(
            defaults
                .usual(EntryTarget::Recurring(plain.id), None, false)
                .source,
            CodeSource::Kind
        );
    }

    #[test]
    fn tickets_code_applies_to_the_rollup_entry_only() {
        let tickets = TimesheetCodeId::new();
        let support = TimesheetCodeId::new();
        let mine = TimesheetCodeId::new();
        let none = CodeDefaults::new(&[], &[], &[mapping("Support", support)]);
        let defaults = none.clone().with_tickets(Some(tickets));
        assert_eq!(defaults.tickets_default(), Some(tickets));
        let d = date!(2026 - 10 - 15);
        let rollup = entry(d, 90, EntryTarget::None, None, None);
        assert_eq!(
            defaults.resolve_for(&rollup, true),
            ResolvedCode {
                code: Some(tickets),
                source: CodeSource::Tickets
            }
        );
        // An ordinary entry with nothing set stays Unassigned.
        assert_eq!(defaults.resolve(&rollup).source, CodeSource::Unassigned);
        // No tickets code set: the roll-up is Unassigned, as before.
        assert_eq!(
            none.resolve_for(&rollup, true).source,
            CodeSource::Unassigned
        );
        // The usual code ignores what was chosen on the entry.
        assert_eq!(
            defaults.usual(EntryTarget::None, None, true).code,
            Some(tickets)
        );
        assert_eq!(
            defaults
                .usual(EntryTarget::None, Some("support"), false)
                .source,
            CodeSource::Category
        );
        // A code chosen on the roll-up entry wins.
        let picked = entry(d, 90, EntryTarget::None, None, Some(mine));
        assert_eq!(defaults.resolve_for(&picked, true).code, Some(mine));
        // So does a category, should the roll-up ever have one.
        let filed = entry(d, 90, EntryTarget::None, Some("Support"), None);
        assert_eq!(defaults.resolve_for(&filed, true).code, Some(support));
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
