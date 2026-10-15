//! Project calculations and alert (SPEC §4).
//!
//! - minutes logged = sum of minutes of live entries targeting the project
//! - minutes left = estimate − logged (negative when over)
//! - percent used = logged / estimate
//! - alert, first match wins: Overdue → Over budget → Blocked → none
//!
//! An estimate of 0 or no estimate means "no budget": minutes left and
//! percent used are `None` and the project is never Over budget.

use serde::{Deserialize, Serialize};
use time::Date;

use crate::ids::ProjectId;
use crate::minutes::Minutes;
use crate::model::{is_blocked, is_done, EntryTarget, Project, TimeEntry};

/// A project's alert badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAlert {
    /// Due date passed and not Done.
    Overdue,
    /// More time logged than estimated.
    OverBudget,
    /// Status is Blocked.
    Blocked,
}

impl ProjectAlert {
    /// The text shown on the badge.
    pub const fn label(self) -> &'static str {
        match self {
            ProjectAlert::Overdue => "Overdue",
            ProjectAlert::OverBudget => "Over budget",
            ProjectAlert::Blocked => "Blocked",
        }
    }
}

/// Calculated fields shown for a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    /// Total minutes logged against the project.
    pub minutes_logged: Minutes,
    /// Estimate minus logged; negative when over. `None` without an estimate.
    pub minutes_left: Option<i64>,
    /// Logged as a whole percent of the estimate (rounded to nearest).
    /// `None` without an estimate.
    pub percent_used: Option<u32>,
    /// The alert badge, if any.
    pub alert: Option<ProjectAlert>,
}

/// Sum of minutes of live (not deleted) entries logged against `project`.
pub fn minutes_logged(project: ProjectId, entries: &[TimeEntry]) -> Minutes {
    entries
        .iter()
        .filter(|e| e.is_live() && e.target == EntryTarget::Project(project))
        .map(|e| e.minutes)
        .sum()
}

/// The estimate, treating 0 as "no estimate".
fn budget(estimate: Option<Minutes>) -> Option<Minutes> {
    estimate.filter(|m| !m.is_zero())
}

/// Estimate minus logged, or `None` without an estimate.
pub fn minutes_left(estimate: Option<Minutes>, logged: Minutes) -> Option<i64> {
    budget(estimate).map(|est| i64::from(est.get()) - i64::from(logged.get()))
}

/// Logged as a whole percent of the estimate (nearest, halves up), or `None`.
pub fn percent_used(estimate: Option<Minutes>, logged: Minutes) -> Option<u32> {
    budget(estimate).map(|est| {
        let est = u64::from(est.get());
        let pct = (u64::from(logged.get()) * 100 + est / 2) / est;
        u32::try_from(pct).unwrap_or(u32::MAX)
    })
}

/// The alert, first match wins: Overdue (due before today and not Done) →
/// Over budget (logged more than the estimate) → Blocked (status Blocked).
pub fn project_alert(
    due_date: Option<Date>,
    status: Option<&str>,
    estimate: Option<Minutes>,
    logged: Minutes,
    today: Date,
) -> Option<ProjectAlert> {
    if due_date.is_some_and(|due| due < today) && !is_done(status) {
        Some(ProjectAlert::Overdue)
    } else if budget(estimate).is_some_and(|est| logged > est) {
        Some(ProjectAlert::OverBudget)
    } else if is_blocked(status) {
        Some(ProjectAlert::Blocked)
    } else {
        None
    }
}

/// All calculated fields for one project.
pub fn summarize_project(project: &Project, entries: &[TimeEntry], today: Date) -> ProjectSummary {
    let logged = minutes_logged(project.id, entries);
    ProjectSummary {
        minutes_logged: logged,
        minutes_left: minutes_left(project.estimated_minutes, logged),
        percent_used: percent_used(project.estimated_minutes, logged),
        alert: project_alert(
            project.due_date,
            project.status.as_deref(),
            project.estimated_minutes,
            logged,
            today,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{OwnerId, RecurringTaskId, TimeEntryId};
    use time::macros::{date, datetime};

    const TODAY: Date = date!(2026 - 10 - 15);

    fn entry(target: EntryTarget, minutes: u32) -> TimeEntry {
        TimeEntry {
            id: TimeEntryId::new(),
            owner_id: OwnerId::new(),
            date: TODAY,
            minutes: Minutes(minutes),
            target,
            category: None,
            description: String::new(),
            status: None,
            priority: None,
            next_step: None,
            timesheet_code_id: None,
            legacy_ticket_text: None,
            created_at: datetime!(2026-10-15 09:00 UTC),
            updated_at: datetime!(2026-10-15 09:00 UTC),
            deleted_at: None,
        }
    }

    fn project(due: Option<Date>, status: &str, estimate: Option<u32>) -> Project {
        Project {
            id: ProjectId::new(),
            owner_id: OwnerId::new(),
            name: "Website".into(),
            description: String::new(),
            requested_by: None,
            due_date: due,
            status: Some(status.into()),
            priority: None,
            estimated_minutes: estimate.map(Minutes),
            default_timesheet_code_id: None,
            created_at: datetime!(2026-10-01 09:00 UTC),
            updated_at: datetime!(2026-10-01 09:00 UTC),
            deleted_at: None,
        }
    }

    #[test]
    fn logged_counts_only_live_entries_for_this_project() {
        let p = ProjectId::new();
        let other = ProjectId::new();
        let mut deleted = entry(EntryTarget::Project(p), 500);
        deleted.deleted_at = Some(datetime!(2026-10-15 10:00 UTC));
        let entries = vec![
            entry(EntryTarget::Project(p), 90),
            entry(EntryTarget::Project(p), 30),
            entry(EntryTarget::Project(other), 60),
            entry(EntryTarget::Recurring(RecurringTaskId::new()), 60),
            entry(EntryTarget::None, 60),
            deleted,
        ];
        assert_eq!(minutes_logged(p, &entries), Minutes(120));
        assert_eq!(minutes_logged(ProjectId::new(), &entries), Minutes(0));
    }

    #[test]
    fn left_and_percent() {
        // (estimate, logged, expected minutes left, expected percent)
        type Case = (Option<u32>, u32, Option<i64>, Option<u32>);
        let cases: &[Case] = &[
            (Some(600), 0, Some(600), Some(0)),
            (Some(600), 150, Some(450), Some(25)),
            (Some(600), 600, Some(0), Some(100)),
            (Some(600), 900, Some(-300), Some(150)),
            (Some(180), 60, Some(120), Some(33)),
            (Some(180), 120, Some(60), Some(67)),
            (Some(200), 1, Some(199), Some(1)), // 0.5% rounds up
            (Some(201), 1, Some(200), Some(0)), // 0.497% rounds down
            (Some(0), 60, None, None),          // zero estimate = no budget
            (None, 60, None, None),
        ];
        for &(estimate, logged, left, pct) in cases {
            let est = estimate.map(Minutes);
            assert_eq!(
                minutes_left(est, Minutes(logged)),
                left,
                "{estimate:?}/{logged}"
            );
            assert_eq!(
                percent_used(est, Minutes(logged)),
                pct,
                "{estimate:?}/{logged}"
            );
        }
    }

    #[test]
    fn percent_handles_huge_values() {
        assert_eq!(
            percent_used(Some(Minutes(1)), Minutes(u32::MAX)),
            Some(u32::MAX)
        );
    }

    #[test]
    fn alert_priority_order() {
        let yesterday = date!(2026 - 10 - 14);
        let tomorrow = date!(2026 - 10 - 16);
        let est = Some(Minutes(60));
        let under = Minutes(30);
        let over = Minutes(61);
        use ProjectAlert::*;
        // (name, due date, status, estimate, logged, expected alert)
        type Case = (
            &'static str,
            Option<Date>,
            &'static str,
            Option<Minutes>,
            Minutes,
            Option<ProjectAlert>,
        );
        let cases: &[Case] = &[
            (
                "nothing wrong",
                Some(tomorrow),
                "In progress",
                est,
                under,
                None,
            ),
            ("no due date", None, "In progress", est, under, None),
            (
                "overdue",
                Some(yesterday),
                "In progress",
                est,
                under,
                Some(Overdue),
            ),
            (
                "due today is not overdue",
                Some(TODAY),
                "In progress",
                est,
                under,
                None,
            ),
            (
                "overdue beats over budget",
                Some(yesterday),
                "In progress",
                est,
                over,
                Some(Overdue),
            ),
            (
                "overdue beats blocked",
                Some(yesterday),
                "Blocked",
                est,
                over,
                Some(Overdue),
            ),
            (
                "done is never overdue",
                Some(yesterday),
                "Done",
                est,
                under,
                None,
            ),
            (
                "done but over budget",
                Some(yesterday),
                "Done",
                est,
                over,
                Some(OverBudget),
            ),
            (
                "over budget",
                Some(tomorrow),
                "In progress",
                est,
                over,
                Some(OverBudget),
            ),
            (
                "exactly on budget",
                Some(tomorrow),
                "In progress",
                est,
                Minutes(60),
                None,
            ),
            (
                "over budget beats blocked",
                Some(tomorrow),
                "Blocked",
                est,
                over,
                Some(OverBudget),
            ),
            (
                "blocked",
                Some(tomorrow),
                "Blocked",
                est,
                under,
                Some(Blocked),
            ),
            (
                "blocked any case",
                None,
                "blocked",
                None,
                under,
                Some(Blocked),
            ),
            (
                "no estimate never over budget",
                None,
                "In progress",
                None,
                over,
                None,
            ),
            (
                "zero estimate never over budget",
                None,
                "In progress",
                Some(Minutes(0)),
                over,
                None,
            ),
        ];
        for &(name, due, status, estimate, logged, expected) in cases {
            assert_eq!(
                project_alert(due, Some(status), estimate, logged, TODAY),
                expected,
                "{name}"
            );
        }
        assert_eq!(
            project_alert(Some(yesterday), None, None, Minutes(0), TODAY),
            Some(Overdue),
            "no status is not Done"
        );
    }

    #[test]
    fn summary_combines_everything() {
        let mut p = project(Some(date!(2026 - 10 - 30)), "In progress", Some(120));
        let entries = vec![
            entry(EntryTarget::Project(p.id), 90),
            entry(EntryTarget::Project(p.id), 60),
        ];
        let s = summarize_project(&p, &entries, TODAY);
        assert_eq!(
            s,
            ProjectSummary {
                minutes_logged: Minutes(150),
                minutes_left: Some(-30),
                percent_used: Some(125),
                alert: Some(ProjectAlert::OverBudget),
            }
        );
        p.estimated_minutes = None;
        let s = summarize_project(&p, &entries, TODAY);
        assert_eq!(s.alert, None);
        assert_eq!(s.percent_used, None);
    }

    #[test]
    fn labels() {
        assert_eq!(ProjectAlert::Overdue.label(), "Overdue");
        assert_eq!(ProjectAlert::OverBudget.label(), "Over budget");
        assert_eq!(ProjectAlert::Blocked.label(), "Blocked");
    }
}
