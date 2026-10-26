//! "Log work": paste a ticket or type a few words, then press 15m, 30m, 1h...
//!
//! The text is read with the capture parser (tickets, `#project`, words); the
//! time comes from the button. Then:
//!
//! 1. **Same ticket, same day:** if an entry on that day (not the tray's
//!    roll-up entry) already has one of the pasted tickets, and no new words were
//!    typed (or the words equal that entry's title, ignoring case), the time is
//!    added to that entry and any other pasted tickets are added to it.
//! 2. **Otherwise a new entry:** its title is the typed words, else the title
//!    of the most recent earlier entry with one of the tickets, else
//!    "Ticket work". Its project is the `#project`, else that earlier entry's
//!    project, else none.
//!
//! Nothing typed and no tickets is refused.

use time::Date;

use crate::capture::Capture;
use crate::error::CoreError;
use crate::ids::{ProjectId, TimeEntryId};
use crate::minutes::Minutes;
use crate::model::EntryTarget;

/// Title used when only tickets were pasted and none was seen before.
pub const DEFAULT_TICKET_TITLE: &str = "Ticket work";

/// An entry on the day being logged, with its tickets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayEntry {
    /// The entry.
    pub id: TimeEntryId,
    /// Its title.
    pub description: String,
    /// Its tickets.
    pub tickets: Vec<String>,
    /// True for the tray's roll-up entry, which is never reused.
    pub is_tray: bool,
}

/// The most recent earlier entry that has one of the pasted tickets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastUse {
    /// Its title.
    pub description: String,
    /// What it was logged against.
    pub target: EntryTarget,
}

/// What Log work should do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickLogPlan {
    /// Add time (and tickets) to an entry on that day.
    AddTo {
        /// The entry.
        entry: TimeEntryId,
        /// Minutes to add.
        minutes: Minutes,
        /// Pasted tickets it doesn't have yet.
        new_tickets: Vec<String>,
    },
    /// Make a new entry.
    Create {
        /// Title.
        description: String,
        /// Project (or none).
        target: EntryTarget,
        /// Minutes.
        minutes: Minutes,
        /// Tickets.
        tickets: Vec<String>,
    },
}

fn has_ticket(list: &[String], ticket: &str) -> bool {
    list.iter().any(|t| t.trim().eq_ignore_ascii_case(ticket))
}

/// Decide what Log work does with `capture` and `minutes` on a day.
pub fn plan_quick_log(
    capture: &Capture,
    minutes: Minutes,
    day_entries: &[DayEntry],
    past: Option<&PastUse>,
) -> Result<QuickLogPlan, CoreError> {
    let words = capture.description.trim();
    if words.is_empty() && capture.tickets.is_empty() {
        return Err(CoreError::NothingToLog);
    }
    if !capture.tickets.is_empty() {
        let same = day_entries.iter().find(|e| {
            !e.is_tray
                && capture.tickets.iter().any(|t| has_ticket(&e.tickets, t))
                && (words.is_empty() || e.description.trim().eq_ignore_ascii_case(words))
        });
        if let Some(entry) = same {
            return Ok(QuickLogPlan::AddTo {
                entry: entry.id,
                minutes,
                new_tickets: capture
                    .tickets
                    .iter()
                    .filter(|t| !has_ticket(&entry.tickets, t))
                    .cloned()
                    .collect(),
            });
        }
    }
    let description = if !words.is_empty() {
        words.to_string()
    } else if let Some(p) = past.filter(|p| !p.description.trim().is_empty()) {
        p.description.trim().to_string()
    } else {
        DEFAULT_TICKET_TITLE.to_string()
    };
    let target = match (capture.target(), past) {
        (EntryTarget::None, Some(p)) => p.target,
        (named, _) => named,
    };
    Ok(QuickLogPlan::Create {
        description,
        target,
        minutes,
        tickets: capture.tickets.clone(),
    })
}

/// The project of an entry target, if it is a project.
pub fn target_project(target: EntryTarget) -> Option<ProjectId> {
    match target {
        EntryTarget::Project(id) => Some(id),
        _ => None,
    }
}

/// Where "+15m" (or +30m, +1h) on an entry goes (round 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddTimePlan {
    /// Add the minutes to this entry.
    AddTo(TimeEntryId),
    /// Log a copy of the entry (same title, project, category, code and tickets) on the day.
    Copy,
}

/// Time added to an entry lands on `day` (today, or the day open on Day), never quietly on
/// an older day: the entry itself when it's from that day, else the day's entry with the same
/// title and project, else a copy. `on_day` lists the day's entries as (id, title, target,
/// is the tray roll-up).
pub fn plan_add_time(
    entry: (TimeEntryId, Date, &str, EntryTarget),
    day: Date,
    on_day: &[(TimeEntryId, &str, EntryTarget, bool)],
) -> AddTimePlan {
    let (id, date, title, target) = entry;
    if date == day {
        return AddTimePlan::AddTo(id);
    }
    on_day
        .iter()
        .find(|(_, t, tg, tray)| {
            !tray && *tg == target && t.trim().eq_ignore_ascii_case(title.trim())
        })
        .map_or(AddTimePlan::Copy, |(same, ..)| AddTimePlan::AddTo(*same))
}

#[cfg(test)]
mod tests {
    #[test]
    fn added_time_lands_on_the_day_worked() {
        use super::{plan_add_time, AddTimePlan};
        use crate::ids::{ProjectId, TimeEntryId};
        use crate::model::EntryTarget;
        use time::macros::date;
        let (old, today_one, other) = (TimeEntryId::new(), TimeEntryId::new(), TimeEntryId::new());
        let p = EntryTarget::Project(ProjectId::new());
        let (mon, tue) = (date!(2026 - 10 - 19), date!(2026 - 10 - 20));
        // From the day itself: add to it.
        assert_eq!(
            plan_add_time((old, tue, "Closet cleanup", p), tue, &[]),
            AddTimePlan::AddTo(old)
        );
        // From an older day: the day's matching entry, else a copy.
        let day = [
            (other, "Closet cleanup", EntryTarget::None, false),
            (today_one, " closet CLEANUP", p, false),
        ];
        assert_eq!(
            plan_add_time((old, mon, "Closet cleanup", p), tue, &day),
            AddTimePlan::AddTo(today_one)
        );
        assert_eq!(
            plan_add_time((old, mon, "Closet cleanup", p), tue, &day[..1]),
            AddTimePlan::Copy
        );
        // The tray roll-up is never added to.
        let tray = [(today_one, "Closet cleanup", p, true)];
        assert_eq!(
            plan_add_time((old, mon, "Closet cleanup", p), tue, &tray),
            AddTimePlan::Copy
        );
    }

    use super::*;
    use crate::capture::{parse_capture, CaptureProject};
    use crate::week::WeekStart;
    use time::macros::date;

    fn cap(text: &str, projects: &[CaptureProject<'_>]) -> Capture {
        parse_capture(text, date!(2026 - 10 - 15), WeekStart::default(), projects)
    }

    fn day(id: TimeEntryId, title: &str, tickets: &[&str], is_tray: bool) -> DayEntry {
        DayEntry {
            id,
            description: title.into(),
            tickets: tickets.iter().map(|t| t.to_string()).collect(),
            is_tray,
        }
    }

    #[test]
    fn refuses_nothing() {
        assert_eq!(
            plan_quick_log(&cap("  ", &[]), Minutes(15), &[], None),
            Err(CoreError::NothingToLog)
        );
    }

    #[test]
    fn same_ticket_same_day_adds_time() {
        let id = TimeEntryId::new();
        let entries = [
            day(TimeEntryId::new(), "Tickets", &["INC1111"], true),
            day(id, "Printer", &["INC1111", "INC2222"], false),
        ];
        let plan =
            plan_quick_log(&cap("inc1111 INC3333", &[]), Minutes(30), &entries, None).unwrap();
        assert_eq!(
            plan,
            QuickLogPlan::AddTo {
                entry: id,
                minutes: Minutes(30),
                new_tickets: vec!["INC3333".into()],
            }
        );
        // Same title typed: still the same entry.
        let plan =
            plan_quick_log(&cap("INC2222 printer", &[]), Minutes(15), &entries, None).unwrap();
        assert!(matches!(plan, QuickLogPlan::AddTo { entry, .. } if entry == id));
    }

    #[test]
    fn new_words_make_a_new_entry() {
        let entries = [day(TimeEntryId::new(), "Printer", &["INC1111"], false)];
        let plan = plan_quick_log(
            &cap("INC1111 follow-up call", &[]),
            Minutes(15),
            &entries,
            None,
        )
        .unwrap();
        assert_eq!(
            plan,
            QuickLogPlan::Create {
                description: "follow-up call".into(),
                target: EntryTarget::None,
                minutes: Minutes(15),
                tickets: vec!["INC1111".into()],
            }
        );
    }

    #[test]
    fn the_tray_entry_is_never_reused() {
        let entries = [day(TimeEntryId::new(), "Tickets", &["INC1111"], true)];
        let plan = plan_quick_log(&cap("INC1111", &[]), Minutes(15), &entries, None).unwrap();
        assert!(matches!(plan, QuickLogPlan::Create { .. }));
    }

    #[test]
    fn earlier_use_gives_title_and_project() {
        let p = ProjectId::new();
        let past = PastUse {
            description: "Switch upgrade".into(),
            target: EntryTarget::Project(p),
        };
        let plan = plan_quick_log(&cap("CHG0012345", &[]), Minutes(60), &[], Some(&past)).unwrap();
        assert_eq!(
            plan,
            QuickLogPlan::Create {
                description: "Switch upgrade".into(),
                target: EntryTarget::Project(p),
                minutes: Minutes(60),
                tickets: vec!["CHG0012345".into()],
            }
        );
        // A #project typed wins over the earlier project.
        let other = ProjectId::new();
        let list = [CaptureProject {
            id: EntryTarget::Project(other),
            name: "Admin",
        }];
        let plan = plan_quick_log(
            &cap("CHG0012345 #adm", &list),
            Minutes(60),
            &[],
            Some(&past),
        )
        .unwrap();
        assert!(
            matches!(plan, QuickLogPlan::Create { target, .. } if target == EntryTarget::Project(other))
        );
        assert_eq!(target_project(EntryTarget::Project(other)), Some(other));
        assert_eq!(target_project(EntryTarget::None), None);
    }

    #[test]
    fn only_tickets_never_seen() {
        let plan = plan_quick_log(&cap("INC0045678", &[]), Minutes(15), &[], None).unwrap();
        assert!(
            matches!(plan, QuickLogPlan::Create { ref description, .. } if description == DEFAULT_TICKET_TITLE)
        );
    }

    #[test]
    fn words_only() {
        let plan =
            plan_quick_log(&cap("printer on 2nd floor", &[]), Minutes(45), &[], None).unwrap();
        assert!(
            matches!(plan, QuickLogPlan::Create { ref description, ref tickets, .. }
            if description == "printer on 2nd floor" && tickets.is_empty())
        );
    }
}
