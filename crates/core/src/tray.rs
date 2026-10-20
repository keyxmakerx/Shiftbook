//! The "Done tickets" tray: paste tickets as you finish them through the day.
//!
//! Each tray ticket carries its own minutes (15 by default, changeable in steps
//! of 5 from 0 to 8 hours). All of a day's tray tickets roll up into **one**
//! time entry for that day, titled "Tickets", whose minutes are the sum of the
//! tray tickets (capped at 24 hours) and whose tickets are the tray's tickets.
//! When the tray for a day becomes empty, that day's roll-up entry is removed.
//!
//! A ticket with a project rolls up into that project's own "Tickets" entry for the day
//! instead (one per project per day), so the timesheet shows its time under the project.
//! A ticket can be **still open**: worked on but not finished. Its minutes count on the
//! day they were spent, and it comes back in later trays until it is ticked done.

use time::Date;

use crate::capture::ticket;
use crate::error::CoreError;
use crate::ids::TimeEntryId;
use crate::minutes::{Minutes, MAX_ENTRY_MINUTES};
use crate::tickets::add_tickets;

/// Minutes a newly pasted tray ticket gets.
pub const DEFAULT_TRAY_MINUTES: u32 = 15;
/// The stepper moves in steps of this many minutes.
pub const TRAY_MINUTES_STEP: u32 = 5;
/// Most minutes one tray ticket can carry.
pub const MAX_TRAY_MINUTES: u32 = 8 * 60;
/// Title of the roll-up entry.
pub const TRAY_ENTRY_TITLE: &str = "Tickets";

/// Longest note on a tray ticket. It began as a few words; the notepad under each ticket
/// (round 11) makes it a full note.
pub const MAX_TRAY_NOTE_CHARS: usize = 10_000;

/// Clean a tray ticket's note: trimmed, blank means no note, at most
/// [`MAX_TRAY_NOTE_CHARS`] characters.
pub fn clean_tray_note(note: &str) -> Result<Option<String>, CoreError> {
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_TRAY_NOTE_CHARS {
        return Err(CoreError::TooLong {
            field: "The note",
            max: MAX_TRAY_NOTE_CHARS,
        });
    }
    Ok(Some(trimmed.to_string()))
}

/// Check a tray ticket's minutes: 0 to 480.
pub fn validate_tray_minutes(minutes: u32) -> Result<Minutes, CoreError> {
    if minutes > MAX_TRAY_MINUTES {
        Err(CoreError::OutOfRange {
            field: "Minutes",
            min: 0,
            max: i64::from(MAX_TRAY_MINUTES),
        })
    } else {
        Ok(Minutes(minutes))
    }
}

/// Step a tray ticket's minutes up or down by `steps` (negative = down),
/// staying within 0 to 480.
pub fn step_tray_minutes(current: Minutes, steps: i32) -> Minutes {
    let next = i64::from(current.get()) + i64::from(steps) * i64::from(TRAY_MINUTES_STEP);
    let clamped = next.clamp(0, i64::from(MAX_TRAY_MINUTES));
    Minutes(u32::try_from(clamped).unwrap_or(0))
}

/// The tickets in `pasted` that aren't already in the day's tray (repeats are
/// matched ignoring case), upper-cased. Uses the same splitting rules as entry
/// tickets. When some pieces look like tickets (see [`crate::capture::ticket`])
/// only those are kept, so pasting `INC0045678 Printer jam` adds one ticket;
/// when none do, every piece is kept (for unusual ticket formats).
pub fn new_tray_tickets<S: AsRef<str>>(
    existing: &[S],
    pasted: &str,
) -> Result<Vec<String>, CoreError> {
    let added = add_tickets(existing, pasted)?;
    let looks: Vec<String> = added.iter().filter_map(|t| ticket(t)).collect();
    if looks.is_empty() {
        Ok(added.into_iter().map(|t| t.to_uppercase()).collect())
    } else {
        Ok(looks)
    }
}

/// What the day's roll-up entry should hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rollup {
    /// Sum of the tray tickets' minutes, at most 24 hours.
    pub minutes: Minutes,
    /// The tray's tickets, in tray order.
    pub tickets: Vec<String>,
}

/// Roll up a day's tray: `None` when the tray is empty.
pub fn rollup<S: AsRef<str>>(items: &[(S, Minutes)]) -> Option<Rollup> {
    if items.is_empty() {
        return None;
    }
    let total: Minutes = items.iter().map(|(_, m)| *m).sum();
    Some(Rollup {
        minutes: Minutes(total.get().min(MAX_ENTRY_MINUTES)),
        tickets: items.iter().map(|(t, _)| t.as_ref().to_string()).collect(),
    })
}

/// What storage should do to the day's roll-up entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollupAction {
    /// No roll-up entry yet: create one.
    Create(Rollup),
    /// Update the existing roll-up entry.
    Update {
        /// The entry to update.
        entry: TimeEntryId,
        /// What it should hold.
        rollup: Rollup,
    },
    /// The tray is empty: remove the roll-up entry.
    Remove(TimeEntryId),
    /// Empty tray and no entry: nothing to do.
    Nothing,
}

/// Decide what to do with the day's roll-up entry.
pub fn plan_rollup<S: AsRef<str>>(
    items: &[(S, Minutes)],
    existing: Option<TimeEntryId>,
) -> RollupAction {
    match (rollup(items), existing) {
        (Some(rollup), None) => RollupAction::Create(rollup),
        (Some(rollup), Some(entry)) => RollupAction::Update { entry, rollup },
        (None, Some(entry)) => RollupAction::Remove(entry),
        (None, None) => RollupAction::Nothing,
    }
}

/// Plan every roll-up entry of a day when tray tickets can have a project. `items` are
/// the day's tray tickets as (ticket, minutes, project) in tray order; `existing` are the
/// day's roll-up entries by the project they hold tickets for (`None` = the plain
/// "Tickets" entry). Groups come in the order their first ticket was pasted, then the
/// entries whose group is now empty, to be removed.
pub fn plan_rollups<S: AsRef<str>, P: Copy + Eq>(
    items: &[(S, Minutes, Option<P>)],
    existing: &[(Option<P>, TimeEntryId)],
) -> Vec<(Option<P>, RollupAction)> {
    let mut groups: Vec<Option<P>> = Vec::new();
    for (_, _, project) in items {
        if !groups.contains(project) {
            groups.push(*project);
        }
    }
    let entry_for = |group: Option<P>| {
        existing
            .iter()
            .find(|(g, _)| *g == group)
            .map(|(_, id)| *id)
    };
    let mut plans: Vec<(Option<P>, RollupAction)> = groups
        .iter()
        .map(|group| {
            let members: Vec<(&str, Minutes)> = items
                .iter()
                .filter(|(_, _, p)| p == group)
                .map(|(t, m, _)| (t.as_ref(), *m))
                .collect();
            (*group, plan_rollup(&members, entry_for(*group)))
        })
        .collect();
    for (group, id) in existing {
        if !groups.contains(group) {
            plans.push((*group, RollupAction::Remove(*id)));
        }
    }
    plans
}

/// Which still-open tickets from earlier days come back in `today`'s tray. `earlier` are
/// open tray tickets as (ticket, day); only those from before `today` count, the latest
/// day wins for a ticket seen more than once (ignoring case), and a ticket already in
/// today's tray (`today_tickets`) doesn't come back twice. Returns indexes into `earlier`,
/// oldest day first.
pub fn carried_open<S: AsRef<str>, T: AsRef<str>>(
    earlier: &[(S, Date)],
    today_tickets: &[T],
    today: Date,
) -> Vec<usize> {
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let mut keep: Vec<usize> = Vec::new();
    for (i, (ticket, day)) in earlier.iter().enumerate() {
        let ticket = ticket.as_ref();
        if *day >= today || today_tickets.iter().any(|t| same(t.as_ref(), ticket)) {
            continue;
        }
        match keep
            .iter()
            .position(|&k| same(earlier[k].0.as_ref(), ticket))
        {
            Some(at) if earlier[keep[at]].1 < *day => keep[at] = i,
            Some(_) => {}
            None => keep.push(i),
        }
    }
    keep.sort_by_key(|&i| (earlier[i].1, i));
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_notes_are_trimmed_and_limited() {
        assert_eq!(
            clean_tray_note("  printer on 3rd floor "),
            Ok(Some("printer on 3rd floor".into()))
        );
        assert_eq!(clean_tray_note("   "), Ok(None));
        assert!(clean_tray_note(&"x".repeat(MAX_TRAY_NOTE_CHARS)).is_ok());
        assert!(matches!(
            clean_tray_note(&"x".repeat(MAX_TRAY_NOTE_CHARS + 1)),
            Err(CoreError::TooLong { .. })
        ));
    }

    #[test]
    fn minutes_rules() {
        assert_eq!(validate_tray_minutes(0), Ok(Minutes(0)));
        assert_eq!(validate_tray_minutes(480), Ok(Minutes(480)));
        assert!(validate_tray_minutes(481).is_err());
        assert_eq!(step_tray_minutes(Minutes(15), 1), Minutes(20));
        assert_eq!(step_tray_minutes(Minutes(15), -3), Minutes(0));
        assert_eq!(step_tray_minutes(Minutes(5), -10), Minutes(0));
        assert_eq!(step_tray_minutes(Minutes(478), 1), Minutes(480));
        assert_eq!(DEFAULT_TRAY_MINUTES, 15);
    }

    #[test]
    fn pasting_splits_and_skips_repeats() {
        let existing = ["INC0045601"];
        let added =
            new_tray_tickets(&existing, "inc0045601\nINC0045602, inc0045603 INC0045602").unwrap();
        assert_eq!(added, vec!["INC0045602", "INC0045603"]);
        assert!(new_tray_tickets::<&str>(&[], "   ").unwrap().is_empty());
        assert_eq!(
            new_tray_tickets::<&str>(&[], "INC0045678 - Printer jam on 2nd floor").unwrap(),
            vec!["INC0045678"]
        );
        assert_eq!(
            new_tray_tickets::<&str>(&[], "case#4 case#5").unwrap(),
            vec!["CASE#4", "CASE#5"]
        );
        assert!(new_tray_tickets::<&str>(&[], &"X".repeat(65)).is_err());
    }

    #[test]
    fn tickets_with_a_project_roll_up_under_it() {
        let (plain, net) = (TimeEntryId::new(), TimeEntryId::new());
        let items = [
            ("INC1", Minutes(15), None),
            ("INC2", Minutes(30), Some(7)),
            ("INC3", Minutes(15), None),
            ("INC4", Minutes(10), Some(7)),
        ];
        let plans = plan_rollups(&items, &[(None, plain)]);
        assert_eq!(plans.len(), 2);
        assert_eq!(
            plans[0],
            (
                None,
                RollupAction::Update {
                    entry: plain,
                    rollup: Rollup {
                        minutes: Minutes(30),
                        tickets: vec!["INC1".into(), "INC3".into()],
                    },
                }
            )
        );
        assert_eq!(
            plans[1],
            (
                Some(7),
                RollupAction::Create(Rollup {
                    minutes: Minutes(40),
                    tickets: vec!["INC2".into(), "INC4".into()],
                })
            )
        );
        // Moving the last plain ticket to the project removes the plain entry.
        let moved = [("INC2", Minutes(30), Some(7))];
        let plans = plan_rollups(&moved, &[(None, plain), (Some(7), net)]);
        assert!(matches!(
            plans[0],
            (Some(7), RollupAction::Update { entry, .. }) if entry == net
        ));
        assert_eq!(plans[1], (None, RollupAction::Remove(plain)));
        assert!(plan_rollups::<&str, u8>(&[], &[]).is_empty());
    }

    #[test]
    fn open_tickets_come_back_until_ticked() {
        let today = time::macros::date!(2026 - 10 - 20);
        let earlier = [
            ("RITM1", time::macros::date!(2026 - 10 - 15)),
            ("INC2", time::macros::date!(2026 - 10 - 16)),
            ("ritm1", time::macros::date!(2026 - 10 - 19)),
            ("INC3", today),
            ("INC4", time::macros::date!(2026 - 10 - 19)),
        ];
        // INC4 is already in today's tray; INC3 is today's own; RITM1's latest day wins.
        assert_eq!(carried_open(&earlier, &["inc4"], today), vec![1, 2]);
        assert!(carried_open::<&str, &str>(&[], &[], today).is_empty());
    }

    #[test]
    fn rollup_sums_minutes() {
        let items = [
            ("INC1", Minutes(15)),
            ("INC2", Minutes(30)),
            ("INC3", Minutes(0)),
        ];
        assert_eq!(
            rollup(&items),
            Some(Rollup {
                minutes: Minutes(45),
                tickets: vec!["INC1".into(), "INC2".into(), "INC3".into()],
            })
        );
        assert_eq!(rollup::<&str>(&[]), None);
        let many: Vec<(String, Minutes)> =
            (0..4).map(|i| (format!("INC{i}"), Minutes(480))).collect();
        assert_eq!(rollup(&many).unwrap().minutes, Minutes(MAX_ENTRY_MINUTES));
    }

    #[test]
    fn plan_actions() {
        let id = TimeEntryId::new();
        let items = [("INC1", Minutes(15))];
        assert!(
            matches!(plan_rollup(&items, None), RollupAction::Create(r) if r.minutes == Minutes(15))
        );
        assert!(matches!(
            plan_rollup(&items, Some(id)),
            RollupAction::Update { entry, .. } if entry == id
        ));
        assert_eq!(plan_rollup::<&str>(&[], Some(id)), RollupAction::Remove(id));
        assert_eq!(plan_rollup::<&str>(&[], None), RollupAction::Nothing);
    }
}
