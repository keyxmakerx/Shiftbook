//! The Canvas (the Desk): the app's own records laid out as cards the owner arranges.
//!
//! Cards are time entries, Done tickets, notes and Inbox scraps. The Canvas adds only layout:
//! where a card sits. A card the owner has never moved has no saved spot; it gets one from
//! [`auto_place`] each time, in rows from the top left, skipping the spots taken by moved
//! cards. Entries and Done tickets older than [`RECENT_DAYS`] leave the Canvas unless the owner
//! moved them; notes and scraps stay until they are archived or removed (see docs/CANVAS.md).

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::{Date, Duration};

use crate::error::{preview, CoreError};

/// Entries and Done tickets from this many days back (today included) show on their own.
pub const RECENT_DAYS: i64 = 7;
/// A card's size on the Canvas, in canvas units, for placing and overlap checks.
pub const CARD_W: i32 = 220;
/// See [`CARD_W`].
pub const CARD_H: i32 = 120;
/// Space between auto-placed cards.
pub const GAP: i32 = 24;
/// Auto-placed cards fill rows this many cards wide.
pub const ROW_CARDS: i32 = 5;
/// Where auto-placing starts.
pub const ORIGIN: (i32, i32) = (40, 40);
/// Furthest a card can be from the origin either way: keeps numbers sane.
pub const MAX_COORD: i32 = 1_000_000;

/// What a card shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    /// A time entry.
    Entry,
    /// A ticket in the Done tickets tray.
    Tray,
    /// A note of any kind.
    Note,
    /// A scrap waiting in the Inbox.
    Inbox,
}

impl CardKind {
    /// The stored name.
    pub fn as_str(self) -> &'static str {
        match self {
            CardKind::Entry => "entry",
            CardKind::Tray => "tray",
            CardKind::Note => "note",
            CardKind::Inbox => "inbox",
        }
    }
}

impl FromStr for CardKind {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "entry" => Ok(CardKind::Entry),
            "tray" => Ok(CardKind::Tray),
            "note" => Ok(CardKind::Note),
            "inbox" => Ok(CardKind::Inbox),
            other => Err(CoreError::NotAChoice {
                value: preview(other, 30),
            }),
        }
    }
}

/// True when a dated card (an entry or a Done ticket) from `date` shows without having been
/// moved: it's from the last [`RECENT_DAYS`] days, today included.
pub fn is_recent(date: Date, today: Date) -> bool {
    date <= today && date > today - Duration::days(RECENT_DAYS)
}

/// Keep a spot within [`MAX_COORD`] of the origin.
pub fn clamp_spot(x: i32, y: i32) -> (i32, i32) {
    (
        x.clamp(-MAX_COORD, MAX_COORD),
        y.clamp(-MAX_COORD, MAX_COORD),
    )
}

fn overlaps(a: (i32, i32), b: (i32, i32)) -> bool {
    (a.0 - b.0).abs() < CARD_W + GAP / 2 && (a.1 - b.1).abs() < CARD_H + GAP / 2
}

/// Spots for `count` cards that haven't been moved, in reading order, skipping any spot that
/// would overlap a moved card in `taken`. The first card (newest, as the caller orders them)
/// gets the top-left free spot.
pub fn auto_place(count: usize, taken: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut out = Vec::with_capacity(count);
    let mut slot: i32 = 0;
    while out.len() < count {
        let spot = (
            ORIGIN.0 + (slot % ROW_CARDS) * (CARD_W + GAP),
            ORIGIN.1 + (slot / ROW_CARDS) * (CARD_H + GAP),
        );
        slot = slot.saturating_add(1);
        if !taken.iter().any(|t| overlaps(*t, spot)) {
            out.push(spot);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn kinds_round_trip() {
        for k in [
            CardKind::Entry,
            CardKind::Tray,
            CardKind::Note,
            CardKind::Inbox,
        ] {
            assert_eq!(k.as_str().parse::<CardKind>().unwrap(), k);
        }
        assert!("task".parse::<CardKind>().is_err());
    }

    #[test]
    fn recent_is_the_last_seven_days() {
        let today = date!(2026 - 10 - 19);
        assert!(is_recent(today, today));
        assert!(is_recent(date!(2026 - 10 - 13), today));
        assert!(!is_recent(date!(2026 - 10 - 12), today));
        assert!(!is_recent(date!(2026 - 10 - 20), today), "future days wait");
    }

    #[test]
    fn auto_places_in_rows_from_the_top_left() {
        let spots = auto_place(6, &[]);
        assert_eq!(spots[0], ORIGIN);
        assert_eq!(spots[1], (ORIGIN.0 + CARD_W + GAP, ORIGIN.1));
        assert_eq!(
            spots[5],
            (ORIGIN.0, ORIGIN.1 + CARD_H + GAP),
            "sixth starts row two"
        );
    }

    #[test]
    fn auto_place_steps_around_moved_cards() {
        // A moved card sitting on the first two slots pushes new cards along.
        let taken = [(ORIGIN.0 + 100, ORIGIN.1)];
        let spots = auto_place(2, &taken);
        assert_eq!(spots[0], (ORIGIN.0 + 2 * (CARD_W + GAP), ORIGIN.1));
        assert!(spots.iter().all(|s| !overlaps(*s, taken[0])));
        // Far-away cards don't matter.
        assert_eq!(auto_place(1, &[(5_000, 5_000)])[0], ORIGIN);
    }

    #[test]
    fn spots_are_clamped() {
        assert_eq!(clamp_spot(i32::MAX, -i32::MAX), (MAX_COORD, -MAX_COORD));
        assert_eq!(clamp_spot(10, -20), (10, -20));
    }
}
