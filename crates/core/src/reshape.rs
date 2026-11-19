//! Splitting one entry in two, and merging two into one (round 21, idea approved 26
//! October). A split shares the entry's time between two parts; everything else (tickets,
//! notepad) stays on the first. A merge adds the second entry's time, notepad and tickets to
//! the first and removes the second.

use time::Date;

use crate::error::CoreError;
use crate::minutes::{Minutes, MAX_ENTRY_MINUTES};
use crate::tickets::MAX_TICKETS_PER_ENTRY;
use crate::validate::MAX_NOTES_CHARS;

/// Check a split of `total` with `first` minutes for the first part: both parts get at
/// least a minute. Returns the two parts.
pub fn check_split(total: Minutes, first: Minutes) -> Result<(Minutes, Minutes), CoreError> {
    let (total, first) = (total.get(), first.get());
    if total < 2 || first < 1 || first >= total {
        return Err(CoreError::OutOfRange {
            field: "The first part's minutes",
            min: 1,
            max: i64::from(total.saturating_sub(1).max(1)),
        });
    }
    Ok((Minutes(first), Minutes(total - first)))
}

/// One side of a merge.
#[derive(Debug, Clone, Copy)]
pub struct MergePart<'a> {
    /// Its day.
    pub date: Date,
    /// Its time.
    pub minutes: Minutes,
    /// Its notepad.
    pub notes: Option<&'a str>,
    /// Its tickets' numbers.
    pub tickets: &'a [&'a str],
    /// It is a Tickets tray roll-up.
    pub is_tray: bool,
}

/// What the kept entry becomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// Both times together.
    pub minutes: Minutes,
    /// Both notepads, the kept one first, a blank line between.
    pub notes: Option<String>,
    /// Which of the other entry's tickets move over (indexes into its list): the ones the
    /// kept entry doesn't already have.
    pub moved: Vec<usize>,
}

/// Merge `other` into `keep`. They must be on the same day, neither a tray roll-up, and the
/// result has to fit one entry (24 hours, the notepad's length, the ticket limit).
pub fn merge(keep: &MergePart<'_>, other: &MergePart<'_>) -> Result<Merged, CoreError> {
    if keep.is_tray || other.is_tray {
        return Err(CoreError::ReshapeTray);
    }
    if keep.date != other.date {
        return Err(CoreError::MergeDays);
    }
    let minutes = keep.minutes.get().saturating_add(other.minutes.get());
    if minutes > MAX_ENTRY_MINUTES {
        return Err(CoreError::HoursTooLarge);
    }
    let parts: Vec<&str> = [keep.notes, other.notes]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    let notes = (!parts.is_empty()).then(|| parts.join("\n\n"));
    if notes
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_NOTES_CHARS)
    {
        return Err(CoreError::TooLong {
            field: "Notes",
            max: MAX_NOTES_CHARS,
        });
    }
    let has = |number: &str| keep.tickets.iter().any(|t| t.eq_ignore_ascii_case(number));
    let moved: Vec<usize> = other
        .tickets
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            !has(t)
                && !other.tickets[..*i]
                    .iter()
                    .any(|b| b.eq_ignore_ascii_case(t))
        })
        .map(|(i, _)| i)
        .collect();
    let count = keep.tickets.len() + moved.len();
    if count > MAX_TICKETS_PER_ENTRY {
        return Err(CoreError::TooManyTickets {
            count,
            max: MAX_TICKETS_PER_ENTRY,
        });
    }
    Ok(Merged {
        minutes: Minutes(minutes),
        notes,
        moved,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn a_split_leaves_both_parts_time() {
        assert_eq!(
            check_split(Minutes(90), Minutes(30)),
            Ok((Minutes(30), Minutes(60)))
        );
        assert!(check_split(Minutes(90), Minutes(0)).is_err());
        assert!(check_split(Minutes(90), Minutes(90)).is_err());
        assert!(check_split(Minutes(1), Minutes(1)).is_err());
    }

    fn part<'a>(minutes: u32, notes: Option<&'a str>, tickets: &'a [&'a str]) -> MergePart<'a> {
        MergePart {
            date: date!(2026 - 11 - 18),
            minutes: Minutes(minutes),
            notes,
            tickets,
            is_tray: false,
        }
    }

    #[test]
    fn a_merge_adds_time_notes_and_new_tickets() {
        let keep = part(45, Some("flap at 08:12"), &["INC0041233"]);
        let other = part(30, Some(" peer says fixed "), &["inc0041233", "CHG0051877"]);
        let merged = merge(&keep, &other).unwrap();
        assert_eq!(merged.minutes, Minutes(75));
        assert_eq!(
            merged.notes.as_deref(),
            Some("flap at 08:12\n\npeer says fixed")
        );
        assert_eq!(merged.moved, [1]);
        // Nothing written anywhere: no notepad.
        assert_eq!(
            merge(&part(5, None, &[]), &part(5, Some("  "), &[]))
                .unwrap()
                .notes,
            None
        );
    }

    #[test]
    fn a_merge_needs_the_same_day_and_has_to_fit() {
        let keep = part(45, None, &[]);
        let mut other = part(30, None, &[]);
        other.date = date!(2026 - 11 - 17);
        assert_eq!(merge(&keep, &other), Err(CoreError::MergeDays));
        let mut tray = part(30, None, &[]);
        tray.is_tray = true;
        assert_eq!(merge(&keep, &tray), Err(CoreError::ReshapeTray));
        assert_eq!(
            merge(&part(MAX_ENTRY_MINUTES, None, &[]), &part(1, None, &[])),
            Err(CoreError::HoursTooLarge)
        );
    }
}
