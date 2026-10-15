//! The "Done tickets" tray: paste tickets as you finish them through the day.
//!
//! Each tray ticket carries its own minutes (15 by default, changeable in steps
//! of 5 from 0 to 8 hours). All of a day's tray tickets roll up into **one**
//! time entry for that day, titled "Tickets", whose minutes are the sum of the
//! tray tickets (capped at 24 hours) and whose tickets are the tray's tickets.
//! When the tray for a day becomes empty, that day's roll-up entry is removed.

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

#[cfg(test)]
mod tests {
    use super::*;

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
