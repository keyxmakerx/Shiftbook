//! The bell (round 12): what's coming up, in the panel that slides in from the right of the
//! bottom bar. **Now** holds meetings about to start or just started; **Today** holds the
//! rest of today's meetings, the things brought up for today, pins due to chase and the
//! tickets still open.
//! The badge counts everything in the panel. Recurring tasks join once they have screens.

use serde::Serialize;
use time::Date;

use crate::meetings::{meeting_finished, meeting_today, MeetingToday};
use crate::model::{MeetingSeries, NextUpItem};

/// Which part of the panel an item is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BellWhen {
    /// Needs you now.
    Now,
    /// Later today.
    Today,
}

/// What an item is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BellAbout {
    /// A meeting of a series today.
    Meeting {
        /// The series.
        series: String,
        /// Its start, minutes after local midnight.
        at: u32,
        /// Where it stands.
        state: MeetingToday,
    },
    /// A meeting that just finished: log its usual length, other time, or skip (round 15).
    MeetingDone {
        /// The series.
        series: String,
        /// Its name, what the entry is called.
        name: String,
        /// How long it usually lasts, the time "Log" offers.
        minutes: u32,
        /// The project its time goes to, if set.
        project: Option<String>,
    },
    /// A thing brought up for today ("Bring up tomorrow").
    BroughtUp {
        /// The list item.
        id: String,
        /// The day it was brought up for.
        from: String,
    },
    /// A pin whose chase date has come (see `crate::pins`).
    Chase {
        /// The pinned record, `kind:id` like a Desk card's key.
        card: String,
        /// Its chase date.
        on: String,
    },
    /// Tickets worked on but not finished.
    StillOpen {
        /// Their numbers, oldest first.
        tickets: Vec<String>,
    },
}

/// One thing in the bell's panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BellItem {
    /// Now or later today.
    pub when: BellWhen,
    /// The line to show.
    pub title: String,
    /// What it is about.
    pub about: BellAbout,
}

/// A pin to chase: its title, `kind:id` and chase date.
pub type ChasePin = (String, String, Date);

/// Everything for the bell at `now` (minutes after local midnight on `today`): meetings in
/// start order (one that just finished offers to log its time instead), then things brought up (oldest first), then pins due to chase (longest
/// waiting first), then still-open tickets as one item.
pub fn bell_items(
    meetings: &[MeetingSeries],
    next_up: &[NextUpItem],
    pins: &[ChasePin],
    open_tickets: &[String],
    today: Date,
    now: u32,
) -> Vec<BellItem> {
    let mut items: Vec<BellItem> = Vec::new();
    let live = meetings.iter().filter(|m| m.deleted_at.is_none());
    let mut finished: Vec<&MeetingSeries> = live
        .clone()
        .filter(|m| meeting_finished(&m.days, m.at, m.length_minutes, m.handled_on, today, now))
        .collect();
    finished.sort_by_key(|m| (m.at, m.name.to_lowercase()));
    for m in &finished {
        items.push(BellItem {
            when: BellWhen::Now,
            title: format!(
                "{}, {} to {}",
                m.name,
                clock(m.at),
                clock(m.at + m.length_minutes)
            ),
            about: BellAbout::MeetingDone {
                series: m.id.to_string(),
                name: m.name.clone(),
                minutes: m.length_minutes,
                project: m.project_id.map(|p| p.to_string()),
            },
        });
    }
    let mut todays: Vec<(&MeetingSeries, MeetingToday)> = live
        .filter(|m| now < m.at + m.length_minutes)
        .filter_map(|m| meeting_today(&m.days, m.at, m.remind_minutes, today, now).map(|s| (m, s)))
        .collect();
    todays.sort_by_key(|(m, _)| (m.at, m.name.to_lowercase()));
    for (m, state) in todays {
        let (when, title) = match state {
            MeetingToday::Later => (BellWhen::Today, format!("{} at {}", m.name, clock(m.at))),
            MeetingToday::Soon(n) => (
                BellWhen::Now,
                format!("{} in {n} minute{}", m.name, if n == 1 { "" } else { "s" }),
            ),
            MeetingToday::Started(_) => (BellWhen::Now, format!("{} has started", m.name)),
        };
        items.push(BellItem {
            when,
            title,
            about: BellAbout::Meeting {
                series: m.id.to_string(),
                at: m.at,
                state,
            },
        });
    }
    let mut brought: Vec<&NextUpItem> = next_up
        .iter()
        .filter(|i| i.done_at.is_none() && i.deleted_at.is_none())
        .filter(|i| i.show_from.is_some_and(|d| d <= today))
        .collect();
    brought.sort_by_key(|i| (i.show_from, i.created_at));
    for i in brought {
        items.push(BellItem {
            when: BellWhen::Today,
            title: i.text.clone(),
            about: BellAbout::BroughtUp {
                id: i.id.to_string(),
                from: i
                    .show_from
                    .map(crate::dates::format_date)
                    .unwrap_or_default(),
            },
        });
    }
    let mut chase: Vec<&ChasePin> = pins.iter().filter(|(_, _, on)| *on <= today).collect();
    chase.sort_by_key(|(_, _, on)| *on);
    for (title, card, on) in chase {
        items.push(BellItem {
            when: BellWhen::Today,
            title: format!("Chase: {title}"),
            about: BellAbout::Chase {
                card: card.clone(),
                on: crate::dates::format_date(*on),
            },
        });
    }
    if !open_tickets.is_empty() {
        let n = open_tickets.len();
        items.push(BellItem {
            when: BellWhen::Today,
            title: format!("{n} ticket{} still open", if n == 1 { "" } else { "s" }),
            about: BellAbout::StillOpen {
                tickets: open_tickets.to_vec(),
            },
        });
    }
    items.sort_by_key(|i| i.when);
    items
}

/// `HH:MM` for minutes after midnight.
pub fn clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60 % 24, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{MeetingSeriesId, NextUpItemId, OwnerId};
    use time::macros::{date, datetime};

    const MON: Date = date!(2026 - 10 - 19);

    fn series(name: &str, at: u32) -> MeetingSeries {
        MeetingSeries {
            id: MeetingSeriesId::new(),
            owner_id: OwnerId::new(),
            name: name.into(),
            days: [true, false, true, false, false, false, false],
            at,
            remind_minutes: 15,
            length_minutes: 30,
            project_id: None,
            handled_on: None,
            created_at: datetime!(2026-10-01 09:00 UTC),
            updated_at: datetime!(2026-10-01 09:00 UTC),
            deleted_at: None,
        }
    }

    fn brought(text: &str, from: Date) -> NextUpItem {
        NextUpItem {
            id: NextUpItemId::new(),
            owner_id: OwnerId::new(),
            position: 0,
            text: text.into(),
            show_from: Some(from),
            about: None,
            done_at: None,
            created_at: datetime!(2026-10-16 17:00 UTC),
            updated_at: datetime!(2026-10-16 17:00 UTC),
            deleted_at: None,
        }
    }

    #[test]
    fn now_first_then_today() {
        let meetings = [series("All-hands", 14 * 60), series("Weekly ops", 10 * 60)];
        let next_up = [
            brought("call vendor about the PSU", date!(2026 - 10 - 16)),
            brought("not yet", date!(2026 - 10 - 20)),
        ];
        let open = vec!["RITM0034567".to_string(), "INC0045711".to_string()];
        let items = bell_items(&meetings, &next_up, &[], &open, MON, 9 * 60 + 50);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Weekly ops in 10 minutes",
                "All-hands at 14:00",
                "call vendor about the PSU",
                "2 tickets still open"
            ]
        );
        assert_eq!(items[0].when, BellWhen::Now);
        assert!(items[1..].iter().all(|i| i.when == BellWhen::Today));
    }

    #[test]
    fn a_finished_meeting_offers_to_log_its_time_until_handled() {
        let mut ops = series("Weekly ops", 10 * 60);
        let items = bell_items(std::slice::from_ref(&ops), &[], &[], &[], MON, 10 * 60 + 31);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].when, BellWhen::Now);
        assert_eq!(items[0].title, "Weekly ops, 10:00 to 10:30");
        assert_eq!(
            items[0].about,
            BellAbout::MeetingDone {
                series: ops.id.to_string(),
                name: "Weekly ops".into(),
                minutes: 30,
                project: None
            }
        );
        // A short meeting shows as finished rather than started.
        ops.length_minutes = 10;
        let items = bell_items(std::slice::from_ref(&ops), &[], &[], &[], MON, 10 * 60 + 12);
        assert!(matches!(
            items[0].about,
            BellAbout::MeetingDone { minutes: 10, .. }
        ));
        ops.handled_on = Some(MON);
        assert!(bell_items(&[ops], &[], &[], &[], MON, 10 * 60 + 12).is_empty());
    }

    #[test]
    fn quiet_when_nothing_is_due() {
        let mut gone = series("Old", 10 * 60);
        gone.deleted_at = Some(datetime!(2026-10-10 09:00 UTC));
        let mut done = brought("done already", MON);
        done.done_at = Some(datetime!(2026-10-19 08:00 UTC));
        assert!(bell_items(&[gone], &[done], &[], &[], MON, 9 * 60).is_empty());
        // Tuesday: neither series meets, and one open ticket reads in the singular.
        let items = bell_items(
            &[series("Weekly ops", 600)],
            &[],
            &[],
            &["INC1".into()],
            date!(2026 - 10 - 20),
            590,
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "1 ticket still open");
        assert_eq!(clock(9 * 60 + 5), "09:05");
    }

    #[test]
    fn pins_show_from_their_chase_date() {
        let pins = [
            ("PSUs for the switch".to_string(), "note:a".to_string(), MON),
            (
                "later".to_string(),
                "note:b".to_string(),
                date!(2026 - 10 - 26),
            ),
            (
                "waiting longest".to_string(),
                "tray:c".to_string(),
                date!(2026 - 10 - 14),
            ),
        ];
        let items = bell_items(&[], &[], &pins, &[], MON, 600);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Chase: waiting longest", "Chase: PSUs for the switch"]
        );
        assert_eq!(
            items[1].about,
            BellAbout::Chase {
                card: "note:a".into(),
                on: "2026-10-19".into()
            }
        );
    }
}
