//! Pins (round 14): an entry, a ticket or a note kept in view as a chip on the bottom bar,
//! e.g. "PSUs for the Meraki switch: asked Sam to ship them". A pin can have a chase date:
//! from that day its chip turns orange and the bell lists it, until it's chased again or
//! unpinned.

use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::canvas::CardKind;
use crate::error::CoreError;

/// Most pins at once, so the bottom bar stays one tidy row.
pub const MAX_PINS: usize = 12;

/// One pinned record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    /// What's pinned.
    pub kind: CardKind,
    /// The record's id.
    pub record_id: Uuid,
    /// The day to chase it, if any.
    pub chase_on: Option<Date>,
    /// When it was pinned (UTC).
    pub created_at: OffsetDateTime,
}

/// Entries, tickets and notes can be pinned (they open in a window); Inbox scraps can't.
pub fn can_pin(kind: CardKind) -> bool {
    matches!(kind, CardKind::Entry | CardKind::Tray | CardKind::Note)
}

/// Check a new pin: a kind that can be pinned, and room for one more when it isn't pinned
/// already (`pinned` counts the pins there are now).
pub fn check_pin(kind: CardKind, already: bool, pinned: usize) -> Result<(), CoreError> {
    if !can_pin(kind) {
        return Err(CoreError::NotAChoice {
            value: kind.as_str().to_string(),
        });
    }
    if !already && pinned >= MAX_PINS {
        return Err(CoreError::TooManyItems {
            field: "pins",
            max: MAX_PINS,
        });
    }
    Ok(())
}

/// True from the pin's chase date on.
pub fn is_due(chase_on: Option<Date>, today: Date) -> bool {
    chase_on.is_some_and(|d| d <= today)
}

/// The order chips show in: ones due to chase first (the longest-waiting first), then the
/// rest in the order they were pinned.
pub fn pin_order(pins: &[Pin], today: Date) -> Vec<usize> {
    let mut order: Vec<usize> = (0..pins.len()).collect();
    order.sort_by_key(|&i| {
        let p = &pins[i];
        let due = is_due(p.chase_on, today);
        (!due, if due { p.chase_on } else { None }, p.created_at)
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    const TUE: Date = date!(2026 - 10 - 20);

    fn pin(chase_on: Option<Date>, minute: u8) -> Pin {
        Pin {
            kind: CardKind::Note,
            record_id: Uuid::now_v7(),
            chase_on,
            created_at: datetime!(2026-10-20 09:00 UTC) + time::Duration::minutes(minute.into()),
        }
    }

    #[test]
    fn only_records_with_a_window_can_be_pinned() {
        assert!(can_pin(CardKind::Note) && can_pin(CardKind::Tray) && can_pin(CardKind::Entry));
        assert!(check_pin(CardKind::Inbox, false, 0).is_err());
        assert!(check_pin(CardKind::Note, false, MAX_PINS - 1).is_ok());
        assert!(check_pin(CardKind::Note, false, MAX_PINS).is_err());
        // Changing the chase date of a pin that's there already is fine when full.
        assert!(check_pin(CardKind::Note, true, MAX_PINS).is_ok());
    }

    #[test]
    fn due_pins_come_first() {
        assert!(!is_due(None, TUE));
        assert!(is_due(Some(TUE), TUE));
        assert!(!is_due(Some(date!(2026 - 10 - 21)), TUE));
        let pins = [
            pin(None, 0),
            pin(Some(date!(2026 - 10 - 27)), 1),
            pin(Some(TUE), 2),
            pin(Some(date!(2026 - 10 - 16)), 3),
        ];
        assert_eq!(pin_order(&pins, TUE), [3, 2, 0, 1]);
    }
}
