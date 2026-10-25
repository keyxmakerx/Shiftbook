//! Items (round 15, step 1): one record per ticket number.
//!
//! A ticket can be in the Tickets tray on many days and on many entries. Each of those rows
//! is an **occurrence** of one item. The item holds what is true of the ticket itself: whether
//! it's done, its one notepad and the project it was last given. Occurrences keep a copy of
//! the item's done tick and notepad, so every screen that reads them agrees.
//!
//! - Ticking it done finishes every occurrence.
//! - Reopening it on a day reopens the occurrences from that day on ([`reopens`]); earlier
//!   days keep what they were.
//! - Writing the notepad anywhere writes it everywhere.
//! - Pasting a ticket again finds its item: the new occurrence starts open, with the item's
//!   notepad and project.
//! - Data from before items: the newest occurrence decides done and project, and differing
//!   notepads are merged so none is lost ([`seed_item`]).

use time::{Date, OffsetDateTime};

use crate::dates::format_date;
use crate::error::CoreError;
use crate::ids::ProjectId;

/// What an item is. Only tickets exist so far; tasks and chores join in a later step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// Something with a number from another system, e.g. INC0045678.
    Ticket,
    /// A to-do of the owner's own.
    Task,
    /// Recurring work.
    Chore,
}

impl ItemKind {
    /// The stored name.
    pub const fn as_str(self) -> &'static str {
        match self {
            ItemKind::Ticket => "ticket",
            ItemKind::Task => "task",
            ItemKind::Chore => "chore",
        }
    }

    /// Read a stored name.
    pub fn parse(text: &str) -> Result<Self, CoreError> {
        match text {
            "ticket" => Ok(ItemKind::Ticket),
            "task" => Ok(ItemKind::Task),
            "chore" => Ok(ItemKind::Chore),
            other => Err(CoreError::NotAChoice {
                value: other.to_string(),
            }),
        }
    }
}

/// One row a ticket appears on, as far as making its item goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence<'a> {
    /// The day it's on (the tray's day, or the entry's).
    pub date: Date,
    /// When the row last changed.
    pub updated_at: OffsetDateTime,
    /// False for a removed row.
    pub live: bool,
    /// Its done tick.
    pub done: bool,
    /// Its notepad.
    pub note: Option<&'a str>,
    /// The project it rolls up under: `Some` for a tray row (which may have none), `None`
    /// for an entry's ticket, whose project is the entry's.
    pub project: Option<Option<ProjectId>>,
}

/// What a new item starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSeed {
    /// Done when its newest occurrence is.
    pub done: bool,
    /// Every different notepad, newest first.
    pub note: Option<String>,
    /// The project of its newest tray row.
    pub project: Option<ProjectId>,
}

/// The state an item takes from rows written before items existed. Removed rows only count
/// when there is nothing else.
pub fn seed_item(occurrences: &[Occurrence<'_>]) -> ItemSeed {
    let live: Vec<&Occurrence<'_>> = occurrences.iter().filter(|o| o.live).collect();
    let mut pool = if live.is_empty() {
        occurrences.iter().collect()
    } else {
        live
    };
    pool.sort_by_key(|o| std::cmp::Reverse((o.date, o.updated_at)));
    let notes: Vec<(Date, &str)> = pool
        .iter()
        .filter_map(|o| o.note.map(|n| (o.date, n)))
        .collect();
    ItemSeed {
        done: pool.first().is_some_and(|o| o.done),
        note: merge_notes(&notes),
        project: pool.iter().find_map(|o| o.project).flatten(),
    }
}

/// Merge notepads, newest first, into one. A note already contained in a newer one (the
/// tray copied notes forward, so most are) is left out; any other older note is kept under
/// an "Earlier note" line with its day.
pub fn merge_notes(newest_first: &[(Date, &str)]) -> Option<String> {
    let mut kept: Vec<(Date, &str)> = Vec::new();
    for &(date, note) in newest_first {
        let note = note.trim();
        if note.is_empty() || kept.iter().any(|(_, k)| k.contains(note)) {
            continue;
        }
        kept.push((date, note));
    }
    let (_, first) = kept.first()?;
    let mut out = (*first).to_string();
    for (date, note) in &kept[1..] {
        out.push_str(&format!(
            "\n\nEarlier note ({}):\n{note}",
            format_date(*date)
        ));
    }
    Some(out)
}

/// True when reopening an item on `from` reopens an occurrence on `date`: that day and later.
pub fn reopens(date: Date, from: Date) -> bool {
    date >= from
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    fn occ<'a>(date: Date, done: bool, note: Option<&'a str>) -> Occurrence<'a> {
        Occurrence {
            date,
            updated_at: datetime!(2026-10-20 12:00 UTC),
            live: true,
            done,
            note,
            project: None,
        }
    }

    #[test]
    fn kinds_round_trip() {
        for k in [ItemKind::Ticket, ItemKind::Task, ItemKind::Chore] {
            assert_eq!(ItemKind::parse(k.as_str()).unwrap(), k);
        }
        assert!(ItemKind::parse("project").is_err());
    }

    #[test]
    fn the_newest_occurrence_decides_done() {
        let rows = [
            occ(date!(2026 - 10 - 12), true, None),
            occ(date!(2026 - 10 - 14), false, None),
        ];
        assert!(!seed_item(&rows).done);
        let rows = [
            occ(date!(2026 - 10 - 12), false, None),
            occ(date!(2026 - 10 - 14), true, None),
        ];
        assert!(seed_item(&rows).done);
        assert!(!seed_item(&[]).done);
    }

    #[test]
    fn same_day_ties_go_to_the_latest_change() {
        let mut older = occ(date!(2026 - 10 - 14), true, None);
        older.updated_at = datetime!(2026-10-14 09:00 UTC);
        let mut newer = occ(date!(2026 - 10 - 14), false, None);
        newer.updated_at = datetime!(2026-10-14 17:00 UTC);
        assert!(!seed_item(&[older.clone(), newer.clone()]).done);
        assert!(!seed_item(&[newer, older]).done);
    }

    #[test]
    fn removed_rows_only_count_when_nothing_else_is_left() {
        let mut gone = occ(date!(2026 - 10 - 15), true, Some("removed"));
        gone.live = false;
        let kept = occ(date!(2026 - 10 - 12), false, Some("kept"));
        let seed = seed_item(&[gone.clone(), kept]);
        assert!(!seed.done);
        assert_eq!(seed.note.as_deref(), Some("kept"));
        let seed = seed_item(&[gone]);
        assert!(seed.done);
        assert_eq!(seed.note.as_deref(), Some("removed"));
    }

    #[test]
    fn project_comes_from_the_newest_tray_row() {
        let p = ProjectId::new();
        let q = ProjectId::new();
        let mut a = occ(date!(2026 - 10 - 12), false, None);
        a.project = Some(Some(p));
        let mut b = occ(date!(2026 - 10 - 13), false, None);
        b.project = Some(Some(q));
        let entry = occ(date!(2026 - 10 - 14), false, None);
        assert_eq!(seed_item(&[a.clone(), b, entry.clone()]).project, Some(q));
        // A newer tray row with no project wins over an older one with a project.
        let mut c = occ(date!(2026 - 10 - 15), false, None);
        c.project = Some(None);
        assert_eq!(seed_item(&[a, c, entry.clone()]).project, None);
        assert_eq!(seed_item(&[entry]).project, None);
    }

    #[test]
    fn notes_copied_forward_merge_into_the_newest() {
        let notes = [
            (
                date!(2026 - 10 - 14),
                "Rang the user.\nWaiting on the vendor.",
            ),
            (date!(2026 - 10 - 13), "Rang the user."),
            (date!(2026 - 10 - 12), "Rang the user."),
        ];
        assert_eq!(
            merge_notes(&notes).as_deref(),
            Some("Rang the user.\nWaiting on the vendor.")
        );
    }

    #[test]
    fn different_notes_are_all_kept_newest_first() {
        let notes = [
            (date!(2026 - 10 - 14), "New PSU ordered"),
            (date!(2026 - 10 - 13), "  "),
            (date!(2026 - 10 - 12), "Switch won't boot"),
        ];
        assert_eq!(
            merge_notes(&notes).as_deref(),
            Some("New PSU ordered\n\nEarlier note (2026-10-12):\nSwitch won't boot")
        );
        assert_eq!(merge_notes(&[]), None);
        assert_eq!(merge_notes(&[(date!(2026 - 10 - 12), " \n ")]), None);
    }

    #[test]
    fn reopening_reaches_that_day_and_later() {
        let from = date!(2026 - 10 - 14);
        assert!(reopens(date!(2026 - 10 - 14), from));
        assert!(reopens(date!(2026 - 10 - 15), from));
        assert!(!reopens(date!(2026 - 10 - 13), from));
    }
}
