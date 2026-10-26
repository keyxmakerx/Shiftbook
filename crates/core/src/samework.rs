//! "Add to …?" (round 15, card r15-things): Log work recognises work you've done before.
//!
//! - **Same work:** typed words that are the title of earlier work (ignoring case and
//!   surrounding spaces, the tray's roll-ups left out) are that work. When it was already
//!   logged on the day, the new time joins it without asking; when it was only logged on
//!   earlier days, Log work asks once: "Add to it" files the time where that work went
//!   (its project or repeating thing), "No, it's new" logs it on its own.
//! - **A known ticket:** a pasted ticket Shiftbook has seen shows since when it's been worked,
//!   the time on it so far, its project and its notepad.

use time::Date;

use crate::minutes::Minutes;
use crate::model::{EntryTarget, TimeEntry};

/// How far back Log work looks for the same work.
pub const SAME_WORK_DAYS: i64 = 60;

/// One entry Log work compares against, with its tickets (number, done).
#[derive(Debug, Clone, Copy)]
pub struct WorkLine<'a> {
    /// The entry.
    pub entry: &'a TimeEntry,
    /// Its tickets and whether each is done.
    pub tickets: &'a [(String, bool)],
    /// True for a tray roll-up, which is never "the same work".
    pub rollup: bool,
}

/// Earlier work with the same title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SameWork {
    /// Its title, as last written.
    pub title: String,
    /// Where its time went most recently (a project or repeating thing), or none.
    pub target: EntryTarget,
    /// Time logged on it.
    pub minutes: Minutes,
    /// How many days it was worked.
    pub days: u32,
    /// True when it was already logged on the day being logged: no need to ask.
    pub on_day: bool,
    /// Its tickets still open, first seen first.
    pub open_tickets: Vec<String>,
}

/// The earlier work (on or before `day`) whose title is `words`, if any.
pub fn same_work(words: &str, day: Date, lines: &[WorkLine<'_>]) -> Option<SameWork> {
    let words = words.trim();
    if words.is_empty() {
        return None;
    }
    let mut mine: Vec<&WorkLine<'_>> = lines
        .iter()
        .filter(|l| !l.rollup && l.entry.is_live() && l.entry.date <= day)
        .filter(|l| l.entry.description.trim().eq_ignore_ascii_case(words))
        .collect();
    if mine.is_empty() {
        return None;
    }
    mine.sort_by_key(|l| (l.entry.date, l.entry.created_at));
    let latest = mine.last()?.entry;
    let mut dates: Vec<Date> = mine.iter().map(|l| l.entry.date).collect();
    dates.dedup();
    let mut open: Vec<String> = Vec::new();
    for l in &mine {
        for (number, done) in l.tickets {
            if !done && !open.iter().any(|o| o.eq_ignore_ascii_case(number)) {
                open.push(number.clone());
            }
        }
    }
    // A ticket ticked done anywhere since isn't open.
    open.retain(|n| {
        !mine
            .iter()
            .flat_map(|l| l.tickets.iter())
            .any(|(t, done)| *done && t.eq_ignore_ascii_case(n))
    });
    Some(SameWork {
        title: latest.description.trim().to_string(),
        target: mine
            .iter()
            .rev()
            .map(|l| l.entry.target)
            .find(|t| *t != EntryTarget::None)
            .unwrap_or(EntryTarget::None),
        minutes: mine.iter().map(|l| l.entry.minutes).sum(),
        days: dates.len() as u32,
        on_day: dates.contains(&day),
        open_tickets: open,
    })
}

/// Where the new time goes, for a line naming no project or repeating thing: the same
/// work's when it was logged on the day or the answer was "Add to it" (`Some(true)`).
pub fn same_work_target(same: Option<&SameWork>, add_to: Option<bool>) -> Option<EntryTarget> {
    let same = same?;
    (same.on_day || add_to == Some(true)).then_some(same.target)
}

/// True when Log work should ask "Add to …?" before logging.
pub fn should_ask(same: Option<&SameWork>) -> bool {
    same.is_some_and(|s| !s.on_day)
}

/// A ticket's time so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoFar {
    /// The first day it was worked.
    pub first: Date,
    /// The last day it was worked.
    pub last: Date,
    /// Minutes on it.
    pub minutes: Minutes,
}

/// A ticket's time so far, from (day, minutes) of each tray row and entry it's on. `None`
/// when it's on nothing.
pub fn ticket_so_far(rows: &[(Date, Minutes)]) -> Option<SoFar> {
    Some(SoFar {
        first: rows.iter().map(|(d, _)| *d).min()?,
        last: rows.iter().map(|(d, _)| *d).max()?,
        minutes: rows.iter().map(|(_, m)| *m).sum(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{OwnerId, ProjectId, TimeEntryId};
    use time::macros::{date, datetime};

    const TUE: Date = date!(2026 - 10 - 27);

    fn entry(date: Date, title: &str, minutes: u32, target: EntryTarget) -> TimeEntry {
        TimeEntry {
            id: TimeEntryId::new(),
            owner_id: OwnerId::new(),
            date,
            minutes: Minutes(minutes),
            target,
            category: None,
            description: title.into(),
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

    #[test]
    fn earlier_work_with_the_same_title() {
        let net = EntryTarget::Project(ProjectId::new());
        let mon = entry(
            date!(2026 - 10 - 26),
            "Switch replacement, floor 2",
            90,
            net,
        );
        let fri = entry(
            date!(2026 - 10 - 23),
            "switch replacement, floor 2 ",
            60,
            net,
        );
        let other = entry(
            date!(2026 - 10 - 26),
            "Printer queue",
            30,
            EntryTarget::None,
        );
        let rollup = entry(
            date!(2026 - 10 - 26),
            "Switch replacement, floor 2",
            15,
            net,
        );
        let open = [("INC0045678".to_string(), false)];
        let done = [("INC0045601".to_string(), true)];
        let lines = [
            WorkLine {
                entry: &mon,
                tickets: &open,
                rollup: false,
            },
            WorkLine {
                entry: &fri,
                tickets: &done,
                rollup: false,
            },
            WorkLine {
                entry: &other,
                tickets: &[],
                rollup: false,
            },
            WorkLine {
                entry: &rollup,
                tickets: &[],
                rollup: true,
            },
        ];
        let same = same_work(" SWITCH replacement, floor 2", TUE, &lines).unwrap();
        assert_eq!(
            same,
            SameWork {
                title: "Switch replacement, floor 2".into(),
                target: net,
                minutes: Minutes(150),
                days: 2,
                on_day: false,
                open_tickets: vec!["INC0045678".into()],
            }
        );
        assert!(should_ask(Some(&same)));
        assert_eq!(same_work_target(Some(&same), None), None);
        assert_eq!(same_work_target(Some(&same), Some(false)), None);
        assert_eq!(same_work_target(Some(&same), Some(true)), Some(net));
        // Logged today already: it joins without asking.
        let today =
            same_work("Switch replacement, floor 2", date!(2026 - 10 - 26), &lines).unwrap();
        assert!(today.on_day && !should_ask(Some(&today)));
        assert_eq!(same_work_target(Some(&today), None), Some(net));
        assert_eq!(same_work("New thing", TUE, &lines), None);
        assert_eq!(same_work("  ", TUE, &lines), None);
        // Later days don't count.
        assert_eq!(
            same_work("Printer queue", date!(2026 - 10 - 25), &lines),
            None
        );
    }

    #[test]
    fn time_so_far_on_a_ticket() {
        assert_eq!(ticket_so_far(&[]), None);
        assert_eq!(
            ticket_so_far(&[
                (date!(2026 - 10 - 27), Minutes(30)),
                (date!(2026 - 10 - 26), Minutes(75)),
            ]),
            Some(SoFar {
                first: date!(2026 - 10 - 26),
                last: date!(2026 - 10 - 27),
                minutes: Minutes(105)
            })
        );
    }
}
