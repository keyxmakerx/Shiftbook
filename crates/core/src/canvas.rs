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

// ---------- project areas (step 2) ----------

/// Smallest project area, in canvas units.
pub const MIN_AREA: (i32, i32) = (CARD_W + 2 * GAP, CARD_H + 3 * GAP);
/// Largest project area either way.
pub const MAX_AREA_SIDE: i32 = 40_000;

/// A project area: a tinted region; cards dropped inside get its project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

impl Area {
    /// Keep an area on the canvas and between the smallest and largest sizes.
    pub fn clean(self) -> Area {
        let (x, y) = clamp_spot(self.x, self.y);
        Area {
            x,
            y,
            w: self.w.clamp(MIN_AREA.0, MAX_AREA_SIDE),
            h: self.h.clamp(MIN_AREA.1, MAX_AREA_SIDE),
        }
    }

    /// True when a card at `spot` (its top-left corner) has its centre inside the area.
    pub fn holds(self, spot: (i32, i32)) -> bool {
        let (cx, cy) = (spot.0 + CARD_W / 2, spot.1 + CARD_H / 2);
        cx >= self.x && cx < self.x + self.w && cy >= self.y && cy < self.y + self.h
    }
}

/// The area a card at `spot` belongs to: the last (topmost) one holding its centre.
pub fn area_at(areas: &[Area], spot: (i32, i32)) -> Option<usize> {
    areas.iter().rposition(|a| a.holds(spot))
}

/// A new area's spot when it's made from the menu at `(x, y)`: a few cards wide.
pub fn new_area(x: i32, y: i32) -> Area {
    Area {
        x,
        y,
        w: 2 * (CARD_W + GAP) + GAP,
        h: 2 * (CARD_H + GAP) + 2 * GAP,
    }
    .clean()
}

// ---------- links (step 2) ----------

/// Why two cards are linked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub enum LinkReason {
    /// Both mention this ticket.
    Ticket(String),
    /// Both use this uncommon word.
    Word(String),
}

/// What a card says, for finding links: its tickets and its words.
#[derive(Debug, Clone, Default)]
pub struct LinkText {
    /// Ticket numbers it carries (any case).
    pub tickets: Vec<String>,
    /// Its title and text.
    pub text: String,
}

/// A word worth linking on: long enough and not an everyday word.
fn link_words(text: &str) -> Vec<String> {
    const COMMON: &[&str] = &[
        "about",
        "after",
        "again",
        "before",
        "being",
        "check",
        "could",
        "daily",
        "doing",
        "done",
        "every",
        "first",
        "follow",
        "from",
        "have",
        "here",
        "into",
        "jot",
        "just",
        "later",
        "meeting",
        "might",
        "minutes",
        "need",
        "next",
        "note",
        "notes",
        "other",
        "should",
        "some",
        "still",
        "that",
        "their",
        "them",
        "then",
        "there",
        "these",
        "they",
        "thing",
        "things",
        "this",
        "those",
        "ticket",
        "tickets",
        "today",
        "tomorrow",
        "update",
        "what",
        "when",
        "where",
        "which",
        "while",
        "with",
        "work",
        "worked",
        "would",
        "yesterday",
        "your",
    ];
    let mut out: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 5 && !w.chars().all(|c| c.is_ascii_digit()))
        .filter(|w| !COMMON.contains(&w.as_str()))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Links drawn automatically between cards: any two that share a ticket (also one written in a
/// note's text), and any two that are the only cards using the same uncommon word. A pair is
/// linked once, by its first reason; a card gets at most [`MAX_WORD_LINKS`] word links.
pub fn auto_links(cards: &[LinkText]) -> Vec<(usize, usize, LinkReason)> {
    use std::collections::{BTreeMap, BTreeSet};
    let tickets: Vec<BTreeSet<String>> = cards
        .iter()
        .map(|c| {
            let mut set: BTreeSet<String> =
                c.tickets.iter().map(|t| t.trim().to_uppercase()).collect();
            set.extend(c.text.split_whitespace().filter_map(crate::capture::ticket));
            set.retain(|t| !t.is_empty());
            set
        })
        .collect();
    let mut out = Vec::new();
    let mut linked = BTreeSet::new();
    for a in 0..cards.len() {
        for b in a + 1..cards.len() {
            if let Some(t) = tickets[a].intersection(&tickets[b]).next() {
                linked.insert((a, b));
                out.push((a, b, LinkReason::Ticket(t.clone())));
            }
        }
    }
    let mut users: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, c) in cards.iter().enumerate() {
        for w in link_words(&c.text) {
            users.entry(w).or_default().push(i);
        }
    }
    let mut per_card = vec![0usize; cards.len()];
    for (word, who) in users {
        if let [a, b] = who[..] {
            if linked.contains(&(a, b))
                || per_card[a] >= MAX_WORD_LINKS
                || per_card[b] >= MAX_WORD_LINKS
            {
                continue;
            }
            linked.insert((a, b));
            per_card[a] += 1;
            per_card[b] += 1;
            out.push((a, b, LinkReason::Word(word)));
        }
    }
    out
}

/// Most word links one card gets, so the Desk doesn't turn into a web.
pub const MAX_WORD_LINKS: usize = 2;

// ---------- timeline (step 2) ----------

/// A card's top-left corner, in canvas units.
pub type Spot = (i32, i32);

/// Room above the first row of cards for the date labels.
pub const TIMELINE_LABEL: i32 = 44;

/// Most cards stacked in one timeline column; a busier day wraps into more columns.
pub const TIMELINE_ROWS: usize = 5;

/// The timeline: days oldest on the left, each day's cards stacked in the order given, wrapping
/// into another column every [`TIMELINE_ROWS`] cards. Returns each card's spot and where each
/// day starts.
pub fn timeline(dates: &[Date]) -> (Vec<Spot>, Vec<(Date, i32)>) {
    let mut days: Vec<Date> = dates.to_vec();
    days.sort();
    days.dedup();
    let per_day: Vec<usize> = days
        .iter()
        .map(|d| dates.iter().filter(|x| *x == d).count())
        .collect();
    let col_x = |col: usize| {
        ORIGIN.0
            + i32::try_from(col)
                .unwrap_or(i32::MAX / 2)
                .saturating_mul(CARD_W + 2 * GAP)
    };
    // The first column of each day.
    let mut first = Vec::with_capacity(days.len());
    let mut col = 0usize;
    for n in &per_day {
        first.push(col);
        col += n.div_ceil(TIMELINE_ROWS).max(1);
    }
    let axis = days
        .iter()
        .zip(&first)
        .map(|(d, c)| (*d, col_x(*c)))
        .collect();
    let mut used = vec![0usize; days.len()];
    let spots = dates
        .iter()
        .map(|d| {
            let day = days.binary_search(d).unwrap_or(0);
            let n = used[day];
            used[day] += 1;
            let row = i32::try_from(n % TIMELINE_ROWS).unwrap_or(0);
            (
                col_x(first[day] + n / TIMELINE_ROWS),
                ORIGIN.1 + TIMELINE_LABEL + row * (CARD_H + GAP),
            )
        })
        .collect();
    (spots, axis)
}

/// Longest name on a plain labelled box.
pub const MAX_LABEL_CHARS: usize = 60;

/// Clean a labelled box's name: trimmed, required, at most [`MAX_LABEL_CHARS`].
pub fn clean_label(label: &str) -> Result<String, CoreError> {
    crate::validate::validate_required("The box name", label, MAX_LABEL_CHARS)
}

/// How a box's cards stand, for the colour of its count when it is folded up (round 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoxStatus {
    /// Nothing waiting: grey.
    Quiet,
    /// Something from today: the accent colour.
    Today,
    /// A ticket still open or something brought up: orange.
    Waiting,
}

/// What matters about one card in a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MemberState {
    /// A ticket that is still open.
    pub open: bool,
    /// Brought up for a day and not ticked off yet.
    pub brought_up: bool,
    /// Dated today (an entry or a ticket) or made today (a note or a scrap).
    pub today: bool,
}

/// A box's status: waiting when any card is still open or brought up, otherwise today when
/// any card is from today, otherwise quiet (also when it's empty).
pub fn box_status(members: &[MemberState]) -> BoxStatus {
    if members.iter().any(|m| m.open || m.brought_up) {
        BoxStatus::Waiting
    } else if members.iter().any(|m| m.today) {
        BoxStatus::Today
    } else {
        BoxStatus::Quiet
    }
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

    #[test]
    fn areas_hold_cards_by_their_centre() {
        let a = Area {
            x: 0,
            y: 0,
            w: 600,
            h: 400,
        };
        assert!(a.holds((0, 0)));
        assert!(a.holds((450, 300)), "the centre is still inside");
        assert!(!a.holds((520, 0)), "the centre is past the right edge");
        let b = Area {
            x: 300,
            y: 0,
            w: 600,
            h: 400,
        };
        assert_eq!(area_at(&[a, b], (400, 10)), Some(1), "the top area wins");
        assert_eq!(area_at(&[a, b], (5000, 10)), None);
    }

    #[test]
    fn areas_are_kept_to_sane_sizes() {
        let tiny = Area {
            x: 0,
            y: 0,
            w: 5,
            h: -9,
        }
        .clean();
        assert_eq!((tiny.w, tiny.h), MIN_AREA);
        let huge = Area {
            x: i32::MAX,
            y: 0,
            w: i32::MAX,
            h: 10_000,
        }
        .clean();
        assert_eq!((huge.x, huge.w), (MAX_COORD, MAX_AREA_SIDE));
        let made = new_area(100, 100);
        assert!(made.holds((110, 110)));
    }

    fn lt(tickets: &[&str], text: &str) -> LinkText {
        LinkText {
            tickets: tickets.iter().map(|t| t.to_string()).collect(),
            text: text.into(),
        }
    }

    #[test]
    fn links_shared_tickets_also_inside_note_text() {
        let cards = [
            lt(&["CHG0012345"], "Firewall rule cleanup"),
            lt(&[], "Rollback plan for chg0012345 if the rules break"),
            lt(&["INC0000001"], "Printer"),
        ];
        let links = auto_links(&cards);
        assert_eq!(links, vec![(0, 1, LinkReason::Ticket("CHG0012345".into()))]);
    }

    #[test]
    fn links_uncommon_words_used_by_exactly_two_cards() {
        let cards = [
            lt(&[], "Core switch cutover plan"),
            lt(&[], "cutover went fine"),
            lt(&[], "optics arrived"),
            lt(&[], "spare optics for the switch"),
            lt(&[], "optics again"),
        ];
        let links = auto_links(&cards);
        assert!(links.contains(&(0, 1, LinkReason::Word("cutover".into()))));
        assert!(links.contains(&(0, 3, LinkReason::Word("switch".into()))));
        assert!(
            !links
                .iter()
                .any(|l| l.2 == LinkReason::Word("optics".into())),
            "three cards use it"
        );
    }

    #[test]
    fn word_links_are_capped_per_card() {
        let hub = lt(&[], "alpha bravo charlie delta");
        let cards = [
            hub,
            lt(&[], "alpha"),
            lt(&[], "bravo"),
            lt(&[], "charlie"),
            lt(&[], "delta"),
        ];
        let links = auto_links(&cards);
        assert_eq!(links.iter().filter(|l| l.0 == 0).count(), MAX_WORD_LINKS);
    }

    #[test]
    fn timeline_has_a_column_per_day_oldest_first() {
        let d1 = date!(2026 - 10 - 13);
        let d2 = date!(2026 - 10 - 15);
        let (spots, axis) = timeline(&[d2, d1, d2]);
        assert_eq!(axis.iter().map(|a| a.0).collect::<Vec<_>>(), [d1, d2]);
        assert_eq!(
            spots[1].0, axis[0].1,
            "the older card is in the first column"
        );
        assert_eq!(spots[0].0, spots[2].0);
        assert!(spots[2].1 > spots[0].1, "same-day cards stack downwards");
        assert_eq!(spots[0].1, ORIGIN.1 + TIMELINE_LABEL);
    }

    #[test]
    fn a_busy_day_wraps_into_more_columns() {
        let d1 = date!(2026 - 10 - 13);
        let d2 = date!(2026 - 10 - 14);
        let mut dates = vec![d1; TIMELINE_ROWS + 2];
        dates.push(d2);
        let (spots, axis) = timeline(&dates);
        let step = CARD_W + 2 * GAP;
        assert_eq!(
            spots[TIMELINE_ROWS].0,
            spots[0].0 + step,
            "the sixth card starts a second column"
        );
        assert_eq!(spots[TIMELINE_ROWS].1, spots[0].1, "at the top");
        assert_eq!(
            axis[1].1,
            axis[0].1 + 2 * step,
            "the next day starts after both columns"
        );
        assert_eq!(spots[TIMELINE_ROWS + 2].0, axis[1].1);
    }

    #[test]
    fn boxes_show_whether_anything_is_waiting() {
        let quiet = MemberState::default();
        let today = MemberState {
            today: true,
            ..quiet
        };
        let open = MemberState {
            open: true,
            ..quiet
        };
        let brought = MemberState {
            brought_up: true,
            ..quiet
        };
        assert_eq!(box_status(&[]), BoxStatus::Quiet);
        assert_eq!(box_status(&[quiet, quiet]), BoxStatus::Quiet);
        assert_eq!(box_status(&[quiet, today]), BoxStatus::Today);
        assert_eq!(box_status(&[today, open]), BoxStatus::Waiting);
        assert_eq!(box_status(&[brought]), BoxStatus::Waiting);
    }

    #[test]
    fn labelled_boxes_need_a_short_name() {
        assert_eq!(
            clean_label("  Waiting on vendors "),
            Ok("Waiting on vendors".into())
        );
        assert!(clean_label("   ").is_err());
        assert!(clean_label(&"x".repeat(MAX_LABEL_CHARS + 1)).is_err());
    }
}
