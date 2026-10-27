//! "Where was I?": when to offer a way back into the work, and what to offer.
//!
//! The card shows after an unlock, after a break of [`BREAK_MINUTES`] or more, or when
//! the day changes while Shiftbook is open; never in the middle of work. It offers the
//! open "next up" list first (things brought up for today, then the wrap-up list), and
//! otherwise the last thing logged.

use serde::Serialize;
use time::Date;

use crate::model::{NextUpItem, TimeEntry};

/// A gap without keyboard or mouse use at least this long counts as a break.
pub const BREAK_MINUTES: i64 = 30;
/// How far back to look for the last thing logged.
pub const LOOK_BACK_DAYS: i64 = 14;

/// What brought the user back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum ResumeTrigger {
    /// Shiftbook was just unlocked.
    Unlocked,
    /// Activity again after a quiet stretch.
    Break,
    /// The date changed while Shiftbook was open.
    NewDay,
}

impl ResumeTrigger {
    /// Parse the stored or sent name.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "unlocked" => Some(Self::Unlocked),
            "break" => Some(Self::Break),
            "new_day" => Some(Self::NewDay),
            _ => None,
        }
    }
}

/// True when the card should show for this trigger. A break only counts once it has
/// lasted [`BREAK_MINUTES`]; `away_minutes` is ignored for the other triggers.
pub fn resume_due(trigger: ResumeTrigger, away_minutes: i64) -> bool {
    match trigger {
        ResumeTrigger::Unlocked | ResumeTrigger::NewDay => true,
        ResumeTrigger::Break => away_minutes >= BREAK_MINUTES,
    }
}

/// What to pick up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeStart<'a> {
    /// The open items brought up for today, then the wrap-up list's; the first is where
    /// to start.
    List(Vec<&'a NextUpItem>),
    /// The last entry logged.
    Entry(&'a TimeEntry),
}

/// The open "next up" items when there are any, otherwise the most recently logged
/// live entry that isn't the tray's roll-up (latest day first, then latest change).
/// Things brought up from a card count from their day on (`show_from` on or before
/// `today`), oldest first, ahead of the wrap-up list in its order.
/// `entries` pairs each entry with whether it is the tray's roll-up. `None` when there
/// is nothing to pick up, so the card stays away.
pub fn resume_start<'a>(
    next_up: &'a [NextUpItem],
    today: Date,
    entries: &'a [(TimeEntry, bool)],
) -> Option<ResumeStart<'a>> {
    let mut open: Vec<&NextUpItem> = next_up
        .iter()
        .filter(|i| i.done_at.is_none() && i.deleted_at.is_none())
        .filter(|i| i.show_from.is_none_or(|d| d <= today))
        .collect();
    if !open.is_empty() {
        open.sort_by_key(|i| (i.show_from.is_none(), i.show_from, i.position, i.created_at));
        return Some(ResumeStart::List(open));
    }
    entries
        .iter()
        .filter(|(e, is_tray)| !is_tray && e.is_live())
        .map(|(e, _)| e)
        .max_by_key(|e| (e.date, e.updated_at))
        .map(ResumeStart::Entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{NextUpItemId, OwnerId, TimeEntryId};
    use crate::minutes::Minutes;
    use crate::model::EntryTarget;
    use time::macros::{date, datetime};
    use time::OffsetDateTime;

    fn item(position: u32, text: &str, done: bool) -> NextUpItem {
        NextUpItem {
            id: NextUpItemId::new(),
            owner_id: OwnerId::new(),
            position,
            text: text.into(),
            show_from: None,
            about: None,
            done_at: done.then_some(datetime!(2026-10-16 09:00 UTC)),
            created_at: datetime!(2026-10-15 17:00 UTC),
            updated_at: datetime!(2026-10-15 17:00 UTC),
            deleted_at: None,
        }
    }

    fn entry(day: time::Date, description: &str, changed: OffsetDateTime) -> TimeEntry {
        TimeEntry {
            id: TimeEntryId::new(),
            owner_id: OwnerId::new(),
            date: day,
            minutes: Minutes(30),
            target: EntryTarget::None,
            category: None,
            description: description.into(),
            status: None,
            priority: None,
            next_step: None,
            timesheet_code_id: None,
            legacy_ticket_text: None,
            created_at: changed,
            updated_at: changed,
            deleted_at: None,
        }
    }

    #[test]
    fn shows_after_unlock_new_day_or_a_long_break_only() {
        assert!(resume_due(ResumeTrigger::Unlocked, 0));
        assert!(resume_due(ResumeTrigger::NewDay, 0));
        assert!(!resume_due(ResumeTrigger::Break, 29));
        assert!(resume_due(ResumeTrigger::Break, 30));
        assert_eq!(ResumeTrigger::parse("break"), Some(ResumeTrigger::Break));
        assert_eq!(ResumeTrigger::parse("lunch"), None);
    }

    #[test]
    fn the_wrapup_list_comes_first_in_order() {
        let list = [
            item(2, "Renew certs", false),
            item(0, "Call the vendor", true),
            item(1, "Chase the printer tickets", false),
        ];
        let entries = [(
            entry(
                date!(2026 - 10 - 16),
                "Firewall review",
                datetime!(2026-10-16 08:00 UTC),
            ),
            false,
        )];
        match resume_start(&list, date!(2026 - 10 - 16), &entries) {
            Some(ResumeStart::List(open)) => {
                let texts: Vec<&str> = open.iter().map(|i| i.text.as_str()).collect();
                assert_eq!(texts, ["Chase the printer tickets", "Renew certs"]);
            }
            other => panic!("expected the list, got {other:?}"),
        }
    }

    #[test]
    fn things_brought_up_come_first_from_their_day() {
        let mut friday = item(0, "call vendor about the spare PSU", false);
        friday.show_from = Some(date!(2026 - 10 - 16));
        let mut monday = item(0, "renew wildcard cert", false);
        monday.show_from = Some(date!(2026 - 10 - 19));
        let list = [item(0, "Chase the printer tickets", false), monday, friday];
        let texts = |today| -> Vec<String> {
            match resume_start(&list, today, &[]) {
                Some(ResumeStart::List(open)) => open.iter().map(|i| i.text.clone()).collect(),
                other => panic!("expected the list, got {other:?}"),
            }
        };
        assert_eq!(
            texts(date!(2026 - 10 - 16)),
            [
                "call vendor about the spare PSU",
                "Chase the printer tickets"
            ]
        );
        assert_eq!(
            texts(date!(2026 - 10 - 19)),
            [
                "call vendor about the spare PSU",
                "renew wildcard cert",
                "Chase the printer tickets"
            ]
        );
    }

    #[test]
    fn otherwise_the_last_thing_logged() {
        let done = [item(0, "Call the vendor", true)];
        let mut deleted = entry(
            date!(2026 - 10 - 16),
            "Deleted",
            datetime!(2026-10-16 12:00 UTC),
        );
        deleted.deleted_at = Some(datetime!(2026-10-16 12:30 UTC));
        let entries = [
            (
                entry(
                    date!(2026 - 10 - 15),
                    "Yesterday",
                    datetime!(2026-10-16 13:00 UTC),
                ),
                false,
            ),
            (
                entry(
                    date!(2026 - 10 - 16),
                    "Earlier today",
                    datetime!(2026-10-16 08:00 UTC),
                ),
                false,
            ),
            (
                entry(
                    date!(2026 - 10 - 16),
                    "Latest today",
                    datetime!(2026-10-16 11:00 UTC),
                ),
                false,
            ),
            (
                entry(
                    date!(2026 - 10 - 16),
                    "Tray",
                    datetime!(2026-10-16 14:00 UTC),
                ),
                true,
            ),
            (deleted, false),
        ];
        match resume_start(&done, date!(2026 - 10 - 16), &entries) {
            Some(ResumeStart::Entry(e)) => assert_eq!(e.description, "Latest today"),
            other => panic!("expected an entry, got {other:?}"),
        }
    }

    #[test]
    fn nothing_to_pick_up_means_no_card() {
        assert_eq!(resume_start(&[], date!(2026 - 10 - 16), &[]), None);
        let tray_only = [(
            entry(
                date!(2026 - 10 - 16),
                "Tray",
                datetime!(2026-10-16 14:00 UTC),
            ),
            true,
        )];
        assert_eq!(resume_start(&[], date!(2026 - 10 - 16), &tray_only), None);
    }
}
