//! The bell (round 12): what's coming up, in the panel that slides in from the right of the
//! bottom bar. **Now** holds meetings about to start or just started; **Today** holds the
//! rest of today's meetings, the things to follow up and the tickets still open.
//! **Follow up** (round 15, card r15-things): a thing brought up for a day and a pin's chase
//! day are the same idea, so each card shows once, from the earlier of the two, and a ticket
//! shown there is left out of "still open".
//! The badge counts everything in the panel. Repeating things that are overdue, due soon or
//! need their expiry date moved join them (round 15), and after a day or more away the
//! things that came due meanwhile open the panel as "While you were away".

use serde::Serialize;
use time::Date;

use crate::dates::{format_date, parse_date};
use crate::meetings::{meeting_finished, meeting_today, MeetingToday};
use crate::model::{MeetingSeries, NextUpItem};
use crate::recurring::{came_due_since, due_words, RecurringState};

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
    /// This week's timesheet isn't entered yet (round 15, see `crate::reminder`).
    #[serde(rename_all = "camelCase")]
    Timesheet {
        /// The week's first day.
        week_start: String,
        /// Logged in the week, as decimal hours.
        hours: String,
        /// Entries with no timesheet code yet.
        uncoded: usize,
    },
    /// A thing to follow up today or since an earlier day: brought up ("Follow up"), or a
    /// pin whose chase day has come (see `crate::pins`), once per card.
    #[serde(rename_all = "camelCase")]
    FollowUp {
        /// The card, `kind:id` like a Desk card's key, if it came from one.
        card: Option<String>,
        /// The brought-up list item, if there is one (Done ticks it off).
        next_up: Option<String>,
        /// The day it was due to be followed up.
        on: String,
    },
    /// A repeating thing that needs attention (round 15).
    Due {
        /// The repeating thing's id.
        id: String,
        /// Overdue, due soon or renewed.
        state: RecurringState,
        /// When it is due, `YYYY-MM-DD`.
        next_due: String,
        /// Its warning window in days.
        warn: u32,
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
    // (on, title, card, next-up id), merged by card: the earlier day wins.
    let mut follow: Vec<(Date, String, Option<String>, Option<String>)> = Vec::new();
    let mut brought: Vec<&NextUpItem> = next_up
        .iter()
        .filter(|i| i.done_at.is_none() && i.deleted_at.is_none())
        .filter(|i| i.show_from.is_some_and(|d| d <= today))
        .collect();
    brought.sort_by_key(|i| (i.show_from, i.created_at));
    for i in brought {
        let card = i.about.map(|(k, id)| format!("{}:{id}", k.as_str()));
        let on = i.show_from.unwrap_or(today);
        match follow.iter_mut().find(|f| card.is_some() && f.2 == card) {
            Some(f) => f.0 = f.0.min(on),
            None => follow.push((on, i.text.clone(), card, Some(i.id.to_string()))),
        }
    }
    for (title, card, on) in pins.iter().filter(|(_, _, on)| *on <= today) {
        match follow
            .iter_mut()
            .find(|f| f.2.as_deref() == Some(card.as_str()))
        {
            Some(f) => f.0 = f.0.min(*on),
            None => follow.push((*on, title.clone(), Some(card.clone()), None)),
        }
    }
    follow.sort_by_key(|f| f.0);
    for (on, title, card, id) in &follow {
        items.push(BellItem {
            when: BellWhen::Today,
            title: title.clone(),
            about: BellAbout::FollowUp {
                card: card.clone(),
                next_up: id.clone(),
                on: crate::dates::format_date(*on),
            },
        });
    }
    // A ticket already shown to follow up isn't counted again as still open.
    let open_tickets: Vec<String> = open_tickets
        .iter()
        .filter(|t| !follow.iter().any(|f| f.1.trim().eq_ignore_ascii_case(t)))
        .cloned()
        .collect();
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

/// A repeating thing for the bell: its name, id, state, next due date and warning days.
pub type DueThing = (String, String, RecurringState, Date, u32);

/// The bell's line for a repeating thing, or `None` when it needs nothing yet.
fn due_item(thing: &DueThing, today: Date) -> Option<BellItem> {
    let (name, id, state, next, warn) = thing;
    let left = crate::dates::days_between(today, *next);
    let title = match state {
        RecurringState::Overdue => format!("{name}: {}", due_words(left)),
        RecurringState::DueSoon => format!("{name}: due {}", due_words(left)),
        RecurringState::Renewed => format!("{name}: put in the new expiry date"),
        _ => return None,
    };
    Some(BellItem {
        when: BellWhen::Today,
        title,
        about: BellAbout::Due {
            id: id.clone(),
            state: *state,
            next_due: format_date(*next),
            warn: *warn,
        },
    })
}

/// `items` with the repeating things that need attention added, most urgent first, just
/// before the still-open tickets.
pub fn with_due(mut items: Vec<BellItem>, due: &[DueThing], today: Date) -> Vec<BellItem> {
    let mut sorted: Vec<&DueThing> = due.iter().collect();
    sorted.sort_by_key(|(name, _, state, next, _)| {
        (
            crate::recurring::attention_order(*state),
            *next,
            name.to_lowercase(),
        )
    });
    let lines: Vec<BellItem> = sorted
        .into_iter()
        .filter_map(|t| due_item(t, today))
        .collect();
    let at = items
        .iter()
        .position(|i| matches!(i.about, BellAbout::StillOpen { .. }))
        .unwrap_or(items.len());
    items.splice(at..at, lines);
    items
}

/// "While you were away" (round 15): the bell's items that came due after `last_open` and
/// by `today`: repeating things whose warning window opened or that went overdue, pins
/// whose chase day came and things brought up for a day in between. Meetings and open
/// tickets are left out. Empty when Shiftbook was last open today (or never).
pub fn while_away(items: &[BellItem], last_open: Option<Date>, today: Date) -> Vec<BellItem> {
    let Some(last) = last_open.filter(|d| *d < today) else {
        return Vec::new();
    };
    let new_day = |text: &str| parse_date(text).is_ok_and(|d| d > last && d <= today);
    items
        .iter()
        .filter(|i| match &i.about {
            BellAbout::Due { next_due, warn, .. } => {
                parse_date(next_due).is_ok_and(|d| came_due_since(d, *warn, last, today))
            }
            BellAbout::FollowUp { on, .. } => new_day(on),
            BellAbout::Meeting { .. }
            | BellAbout::StillOpen { .. }
            | BellAbout::MeetingDone { .. }
            | BellAbout::Timesheet { .. } => false,
        })
        .cloned()
        .collect()
}

/// The timesheet reminder, shown first under Now.
pub fn timesheet_item(week_start: Date, hours: &str, uncoded: usize) -> BellItem {
    BellItem {
        when: BellWhen::Now,
        title: "This week's timesheet isn't entered yet".into(),
        about: BellAbout::Timesheet {
            week_start: crate::dates::format_date(week_start),
            hours: hours.into(),
            uncoded,
        },
    }
}

/// `HH:MM` for minutes after midnight.
pub fn clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60 % 24, minutes % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::CardKind;
    use crate::ids::{MeetingSeriesId, NextUpItemId, OwnerId};
    use time::macros::{date, datetime};
    use uuid::Uuid;

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

    fn thing(name: &str, state: RecurringState, next: Date, warn: u32) -> DueThing {
        (name.into(), format!("id-{name}"), state, next, warn)
    }

    #[test]
    fn repeating_things_go_before_open_tickets() {
        let open = vec!["INC1".to_string()];
        let base = bell_items(&[], &[], &[], &open, MON, 600);
        let due = [
            thing(
                "Bins out",
                RecurringState::DueSoon,
                date!(2026 - 10 - 20),
                1,
            ),
            thing(
                "Boiler filter",
                RecurringState::Overdue,
                date!(2026 - 10 - 16),
                7,
            ),
            thing("Passport", RecurringState::Ok, date!(2029 - 08 - 12), 180),
            thing(
                "Car insurance",
                RecurringState::Renewed,
                date!(2027 - 11 - 06),
                30,
            ),
        ];
        let items = with_due(base, &due, MON);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Boiler filter: 3 days late",
                "Car insurance: put in the new expiry date",
                "Bins out: due tomorrow",
                "1 ticket still open"
            ]
        );
    }

    #[test]
    fn while_away_keeps_only_what_came_due_meanwhile() {
        let last = date!(2026 - 10 - 16);
        let next_up = [
            brought("old news", date!(2026 - 10 - 15)),
            brought("ring the garage", date!(2026 - 10 - 18)),
        ];
        let pins = [(
            "Claim 77812".to_string(),
            "tray:c".to_string(),
            date!(2026 - 10 - 17),
        )];
        let base = bell_items(
            &[series("Weekly ops", 600)],
            &next_up,
            &pins,
            &["INC1".into()],
            MON,
            590,
        );
        let due = [
            thing(
                "Bins out",
                RecurringState::Overdue,
                date!(2026 - 10 - 17),
                1,
            ),
            thing(
                "Seen before",
                RecurringState::DueSoon,
                date!(2026 - 10 - 25),
                20,
            ),
        ];
        let items = with_due(base, &due, MON);
        let away = while_away(&items, Some(last), MON);
        let titles: Vec<&str> = away.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Claim 77812", "ring the garage", "Bins out: 2 days late"]
        );
        assert!(while_away(&items, Some(MON), MON).is_empty());
        assert!(while_away(&items, None, MON).is_empty());
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
        assert_eq!(titles, ["waiting longest", "PSUs for the switch"]);
        assert_eq!(
            items[1].about,
            BellAbout::FollowUp {
                card: Some("note:a".into()),
                next_up: None,
                on: "2026-10-19".into()
            }
        );
    }

    #[test]
    fn each_thing_to_follow_up_shows_once() {
        let card = Uuid::now_v7();
        let mut up = brought("INC0045678", MON);
        up.about = Some((CardKind::Tray, card));
        let pins = [(
            "INC0045678".to_string(),
            format!("tray:{card}"),
            date!(2026 - 10 - 15),
        )];
        let open = vec!["INC0045678".to_string(), "RITM0012345".to_string()];
        let items = bell_items(&[], std::slice::from_ref(&up), &pins, &open, MON, 600);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["INC0045678", "1 ticket still open"]);
        assert_eq!(
            items[0].about,
            BellAbout::FollowUp {
                card: Some(format!("tray:{card}")),
                next_up: Some(up.id.to_string()),
                on: "2026-10-15".into()
            }
        );
    }
}
