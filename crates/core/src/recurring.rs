//! Recurring work calculations and alert state (SPEC §4).
//!
//! - last done = the later of "last done before tracker" and the latest date
//!   of its live entries with status Done
//! - warn = warn days before, or 30 when not set (0 is kept as 0)
//! - interval = frequency months, then days (months first, clamped to the
//!   month end: 31 Jan + 1 month = 28/29 Feb)
//! - next due:
//!   - with an expiration date: if last done ≥ expiration − warn and there is
//!     a frequency → expiration + interval (an estimate), else expiration
//!   - without: last done + interval when both exist, else none
//! - state, first match wins: Inactive → Ongoing (alerts disabled) →
//!   Ongoing (no expiration and no frequency) → Needs a date → Overdue
//!   (days left < 0) → Due soon (days left ≤ warn) → Renewed – update
//!   expiration date → OK
//!
//! A frequency of 0 months and 0 days counts as "no frequency".

use serde::{Deserialize, Serialize};
use time::Date;

use crate::dates::{add_days, add_months, days_between};
use crate::ids::RecurringTaskId;
use crate::minutes::Minutes;
use crate::model::{is_done, EntryTarget, Frequency, RecurringTask, TimeEntry};

/// Warn this many days ahead when `warn_days_before` is not set.
pub const DEFAULT_WARN_DAYS: u32 = 30;

/// A repeat interval: add `months` (clamped to month end), then `days`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Interval {
    /// Whole months.
    pub months: u32,
    /// Extra days.
    pub days: u32,
}

impl Interval {
    /// An interval, or `None` for 0 months and 0 days.
    pub fn new(months: u32, days: u32) -> Option<Self> {
        (months > 0 || days > 0).then_some(Self { months, days })
    }

    /// The interval of a frequency, or `None` if missing or zero.
    pub fn of(frequency: Option<&Frequency>) -> Option<Self> {
        frequency.and_then(|f| Self::new(f.months, f.days))
    }

    /// `date` plus this interval.
    pub fn add_to(self, date: Date) -> Date {
        add_days(add_months(date, self.months), i64::from(self.days))
    }
}

/// The alert state of a recurring task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurringState {
    /// Not active.
    Inactive,
    /// Routine or open-ended: collects hours, never alerts.
    Ongoing,
    /// Alerts on but no next due date can be worked out.
    NeedsDate,
    /// Next due date has passed.
    Overdue,
    /// Next due date is within the warning window.
    DueSoon,
    /// Done recently but the expiration date was not moved forward.
    Renewed,
    /// Nothing to do yet.
    Ok,
}

impl RecurringState {
    /// The text shown on the badge.
    pub const fn label(self) -> &'static str {
        match self {
            RecurringState::Inactive => "Inactive",
            RecurringState::Ongoing => "Ongoing",
            RecurringState::NeedsDate => "Needs a date",
            RecurringState::Overdue => "Overdue",
            RecurringState::DueSoon => "Due soon",
            RecurringState::Renewed => "Renewed – update expiration date",
            RecurringState::Ok => "OK",
        }
    }
}

/// `warn_days_before`, or 30 when not set.
pub fn warn_days(warn_days_before: Option<u32>) -> u32 {
    warn_days_before.unwrap_or(DEFAULT_WARN_DAYS)
}

/// The later of the pre-tracker date and the given Done dates.
pub fn last_done(
    before_tracker: Option<Date>,
    done_dates: impl IntoIterator<Item = Date>,
) -> Option<Date> {
    done_dates.into_iter().chain(before_tracker).max()
}

/// Dates of live entries for `task` whose status is Done.
pub fn done_dates(task: RecurringTaskId, entries: &[TimeEntry]) -> impl Iterator<Item = Date> + '_ {
    entries
        .iter()
        .filter(move |e| {
            e.is_live() && e.target == EntryTarget::Recurring(task) && is_done(e.status.as_deref())
        })
        .map(|e| e.date)
}

/// Total minutes of live entries for `task`.
pub fn total_minutes(task: RecurringTaskId, entries: &[TimeEntry]) -> Minutes {
    entries
        .iter()
        .filter(|e| e.is_live() && e.target == EntryTarget::Recurring(task))
        .map(|e| e.minutes)
        .sum()
}

/// True when the work was done inside the warning window before expiration
/// (last done ≥ expiration − warn).
fn renewed(expiration: Date, last_done: Option<Date>, warn: u32) -> bool {
    let threshold = add_days(expiration, -i64::from(warn));
    last_done.is_some_and(|d| d >= threshold)
}

/// The next due date, per the rules in the module docs.
pub fn next_due(
    expiration: Option<Date>,
    last_done: Option<Date>,
    interval: Option<Interval>,
    warn: u32,
) -> Option<Date> {
    match (expiration, interval) {
        (Some(exp), Some(iv)) if renewed(exp, last_done, warn) => Some(iv.add_to(exp)),
        (Some(exp), _) => Some(exp),
        (None, Some(iv)) => last_done.map(|d| iv.add_to(d)),
        (None, None) => None,
    }
}

/// Everything the state depends on, apart from today's date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecurringInput {
    /// The task is active.
    pub active: bool,
    /// Alerts are on (off for routine buckets).
    pub alerts_enabled: bool,
    /// Expiration / due date.
    pub expiration_date: Option<Date>,
    /// Repeat interval (`None` = no frequency).
    pub interval: Option<Interval>,
    /// Last done (see [`last_done`]).
    pub last_done: Option<Date>,
    /// Warning window in days (see [`warn_days`]).
    pub warn_days: u32,
}

/// The calculated dates and state of a recurring task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurringEvaluation {
    /// Next due date, if one can be worked out.
    pub next_due: Option<Date>,
    /// Days from today to the next due date (negative when past).
    pub days_left: Option<i64>,
    /// The alert state.
    pub state: RecurringState,
}

/// Work out next due, days left and the state, in the order of SPEC §4.
pub fn evaluate(input: RecurringInput, today: Date) -> RecurringEvaluation {
    let next = next_due(
        input.expiration_date,
        input.last_done,
        input.interval,
        input.warn_days,
    );
    let days_left = next.map(|d| days_between(today, d));
    let state = if !input.active {
        RecurringState::Inactive
    } else if !input.alerts_enabled || (input.expiration_date.is_none() && input.interval.is_none())
    {
        // Two separate SPEC rules with the same result: alerts disabled, or
        // nothing to schedule from (no expiration and no frequency).
        RecurringState::Ongoing
    } else {
        match days_left {
            None => RecurringState::NeedsDate,
            Some(d) if d < 0 => RecurringState::Overdue,
            Some(d) if d <= i64::from(input.warn_days) => RecurringState::DueSoon,
            Some(_) => match input.expiration_date {
                Some(exp) if renewed(exp, input.last_done, input.warn_days) => {
                    RecurringState::Renewed
                }
                _ => RecurringState::Ok,
            },
        }
    };
    RecurringEvaluation {
        next_due: next,
        days_left,
        state,
    }
}

/// All calculated fields shown for a recurring task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurringSummary {
    /// Last done date.
    pub last_done: Option<Date>,
    /// Warning window used.
    pub warn_days: u32,
    /// Next due date.
    pub next_due: Option<Date>,
    /// Days from today to next due.
    pub days_left: Option<i64>,
    /// Total minutes logged against it.
    pub total_minutes: Minutes,
    /// Alert state.
    pub state: RecurringState,
}

/// Calculate everything for one recurring task. `frequency` is the task's
/// frequency record, looked up by the caller.
pub fn summarize_recurring(
    task: &RecurringTask,
    frequency: Option<&Frequency>,
    entries: &[TimeEntry],
    today: Date,
) -> RecurringSummary {
    let last = last_done(task.last_done_before_tracker, done_dates(task.id, entries));
    let warn = warn_days(task.warn_days_before);
    let eval = evaluate(
        RecurringInput {
            active: task.active,
            alerts_enabled: task.alerts_enabled,
            expiration_date: task.expiration_date,
            interval: Interval::of(frequency),
            last_done: last,
            warn_days: warn,
        },
        today,
    );
    RecurringSummary {
        last_done: last,
        warn_days: warn,
        next_due: eval.next_due,
        days_left: eval.days_left,
        total_minutes: total_minutes(task.id, entries),
        state: eval.state,
    }
}

/// The "last done" date that makes a repeating thing next come round on `next` (round 15:
/// the user says when it's next due, not when it was last done). Months are clamped to the
/// month end, so some dates can't be reached exactly (monthly onto 30 March: no day in
/// February plus a month gives it); then the result is the latest day that comes round
/// before `next`, so a reminder is early rather than late.
pub fn last_done_for_next(next: Date, interval: Interval) -> Date {
    let span = i64::from(interval.months) * 31 + i64::from(interval.days) + 2;
    let mut day = add_days(next, -span);
    let mut best = day;
    while day < next {
        if interval.add_to(day) <= next {
            best = day;
        } else {
            break;
        }
        day = add_days(day, 1);
    }
    best
}

/// The day a thing starts to need attention: its warning window opens `warn` days before
/// it is due.
pub fn alert_from(next_due: Date, warn: u32) -> Date {
    add_days(next_due, -i64::from(warn))
}

/// True when a thing due on `next_due` started needing attention, or went overdue, after
/// `last_open` and by `today` ("While you were away", round 15).
pub fn came_due_since(next_due: Date, warn: u32, last_open: Date, today: Date) -> bool {
    let new = |d: Date| d > last_open && d <= today;
    new(alert_from(next_due, warn)) || new(add_days(next_due, 1))
}

/// Sort position for the Repeating view and the bell: overdue first, then things whose
/// expiry date needs moving, due soon, needing a date, fine, routine and retired last.
pub const fn attention_order(state: RecurringState) -> u8 {
    match state {
        RecurringState::Overdue => 0,
        RecurringState::Renewed => 1,
        RecurringState::DueSoon => 2,
        RecurringState::NeedsDate => 3,
        RecurringState::Ok => 4,
        RecurringState::Ongoing => 5,
        RecurringState::Inactive => 6,
    }
}

/// When it's due, in plain words: "today", "tomorrow", "in 9 days", "in 4 months",
/// "yesterday", "3 days late".
pub fn due_words(days_left: i64) -> String {
    match days_left {
        0 => "today".into(),
        1 => "tomorrow".into(),
        -1 => "yesterday".into(),
        d if d < 0 => format!("{} days late", -d),
        d if d < 60 => format!("in {d} days"),
        d if d < 730 => format!("in {} months", d / 30),
        d => format!("in {} years", d / 365),
    }
}

/// One round of a repeating thing: the work up to and including a Done (a renewal, a
/// service), or the work since the last Done that isn't finished yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PastTime {
    /// When it was done, or for the round in progress, the last day worked on it.
    #[serde(with = "crate::dates::iso_date")]
    pub date: Date,
    /// False for the round still in progress.
    pub done: bool,
    /// Minutes logged in the round.
    pub minutes: Minutes,
    /// Ticket numbers worked in the round, first seen first, each once.
    pub tickets: Vec<String>,
    /// The new expiry date, when the round ended with Renewed….
    #[serde(with = "crate::dates::iso_date::option")]
    pub renewed_to: Option<Date>,
}

/// Each round of `task`, newest first. `entries` pairs every entry with the expiry date it
/// renewed to (if any); `tickets` are (date, ticket number) pairs worked on the thing. A
/// round runs from the day after one Done up to and including the next; work after the
/// last Done is the round in progress, shown only when there is some.
pub fn past_times(
    task: RecurringTaskId,
    entries: &[(TimeEntry, Option<Date>)],
    tickets: &[(Date, String)],
) -> Vec<PastTime> {
    let mine: Vec<&(TimeEntry, Option<Date>)> = entries
        .iter()
        .filter(|(e, _)| e.is_live() && e.target == EntryTarget::Recurring(task))
        .collect();
    let mut ends: Vec<(Date, Option<Date>)> = mine
        .iter()
        .filter(|(e, _)| is_done(e.status.as_deref()))
        .map(|(e, to)| (e.date, *to))
        .collect();
    ends.sort();
    // One round per Done day; a renewal on that day wins over a plain Done.
    let mut rounds: Vec<PastTime> = Vec::new();
    for (date, to) in ends {
        match rounds.last_mut() {
            Some(last) if last.date == date => last.renewed_to = last.renewed_to.or(to),
            _ => rounds.push(PastTime {
                date,
                done: true,
                minutes: Minutes(0),
                tickets: Vec::new(),
                renewed_to: to,
            }),
        }
    }
    let mut open = PastTime {
        date: Date::MIN,
        done: false,
        minutes: Minutes(0),
        tickets: Vec::new(),
        renewed_to: None,
    };
    let mut open_used = false;
    let done_days: Vec<Date> = rounds.iter().map(|r| r.date).collect();
    let index = |d: Date| done_days.iter().position(|end| d <= *end);
    let mut sorted_entries = mine.clone();
    sorted_entries.sort_by_key(|(e, _)| e.date);
    let mut sorted_tickets: Vec<&(Date, String)> = tickets.iter().collect();
    sorted_tickets.sort_by_key(|(d, _)| *d);
    for (e, _) in sorted_entries {
        match index(e.date) {
            Some(i) => rounds[i].minutes = rounds[i].minutes + e.minutes,
            None => {
                open.minutes = open.minutes + e.minutes;
                open.date = open.date.max(e.date);
                open_used = true;
            }
        }
    }
    for (d, number) in sorted_tickets {
        let round = match index(*d) {
            Some(i) => &mut rounds[i],
            None => {
                open.date = open.date.max(*d);
                open_used = true;
                &mut open
            }
        };
        if !round.tickets.iter().any(|t| t.eq_ignore_ascii_case(number)) {
            round.tickets.push(number.clone());
        }
    }
    if open_used {
        rounds.push(open);
    }
    rounds.reverse();
    rounds
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{FrequencyId, OwnerId, ProjectId, TimeEntryId};
    use crate::model::STATUS_DONE;
    use time::macros::{date, datetime};
    use RecurringState::*;

    const TODAY: Date = date!(2026 - 10 - 15);
    const MONTHLY: Option<Interval> = Some(Interval { months: 1, days: 0 });
    const YEARLY: Option<Interval> = Some(Interval {
        months: 12,
        days: 0,
    });
    const DAILY: Option<Interval> = Some(Interval { months: 0, days: 1 });

    /// An active, alerting task with nothing set; tests change what they need.
    fn base() -> RecurringInput {
        RecurringInput {
            active: true,
            alerts_enabled: true,
            expiration_date: None,
            interval: None,
            last_done: None,
            warn_days: DEFAULT_WARN_DAYS,
        }
    }

    fn state(input: RecurringInput) -> RecurringState {
        evaluate(input, TODAY).state
    }

    // ---- One test per state, written from the SPEC §4 text ----

    #[test]
    fn inactive_wins_over_everything() {
        let input = RecurringInput {
            active: false,
            expiration_date: Some(date!(2020 - 01 - 01)),
            interval: MONTHLY,
            ..base()
        };
        assert_eq!(state(input), Inactive);
        assert_eq!(
            state(RecurringInput {
                active: false,
                alerts_enabled: false,
                ..base()
            }),
            Inactive
        );
    }

    #[test]
    fn alerts_disabled_is_ongoing_even_when_overdue() {
        let input = RecurringInput {
            alerts_enabled: false,
            expiration_date: Some(date!(2020 - 01 - 01)),
            interval: MONTHLY,
            ..base()
        };
        assert_eq!(state(input), Ongoing);
    }

    #[test]
    fn no_expiration_and_no_frequency_is_ongoing() {
        assert_eq!(state(base()), Ongoing);
        assert_eq!(
            state(RecurringInput {
                last_done: Some(date!(2020 - 01 - 01)),
                ..base()
            }),
            Ongoing
        );
    }

    #[test]
    fn last_done_for_next_comes_round_on_next() {
        let weekly = Interval::new(0, 7).unwrap();
        let next = date!(2026 - 10 - 29);
        let last = last_done_for_next(next, weekly);
        assert_eq!(last, date!(2026 - 10 - 22));
        assert_eq!(weekly.add_to(last), next);
        let yearly = YEARLY.unwrap();
        let birthday = date!(2027 - 11 - 17);
        assert_eq!(
            yearly.add_to(last_done_for_next(birthday, yearly)),
            birthday
        );
        let monthly = MONTHLY.unwrap();
        for d in [
            date!(2026 - 03 - 31),
            date!(2026 - 02 - 28),
            date!(2026 - 12 - 01),
        ] {
            let back = monthly.add_to(last_done_for_next(d, monthly));
            assert!(
                back <= d && days_between(back, d) <= 3,
                "{d} came back as {back}"
            );
        }
        // Monthly onto 31 March can only reach 28 March: early, never late.
        assert_eq!(
            monthly.add_to(last_done_for_next(date!(2026 - 03 - 31), monthly)),
            date!(2026 - 03 - 28)
        );
        let both = Interval::new(1, 3).unwrap();
        let n = date!(2026 - 07 - 15);
        assert_eq!(both.add_to(last_done_for_next(n, both)), n);
    }

    #[test]
    fn what_came_due_while_away() {
        let away = date!(2026 - 10 - 20);
        // Due 25 Oct, warn 3: the window opened on the 22nd, while away.
        assert!(came_due_since(
            date!(2026 - 10 - 25),
            3,
            away,
            date!(2026 - 10 - 25)
        ));
        // Window opened before the last open and not overdue yet: already seen.
        assert!(!came_due_since(
            date!(2026 - 10 - 22),
            5,
            away,
            date!(2026 - 10 - 22)
        ));
        // ...but went overdue while away: new again.
        assert!(came_due_since(
            date!(2026 - 10 - 22),
            5,
            away,
            date!(2026 - 10 - 24)
        ));
        // Nothing new when opened the same day.
        assert!(!came_due_since(date!(2026 - 10 - 20), 0, away, away));
        // Not yet in its window.
        assert!(!came_due_since(
            date!(2026 - 12 - 01),
            7,
            away,
            date!(2026 - 10 - 24)
        ));
    }

    #[test]
    fn plain_due_words_and_order() {
        assert_eq!(due_words(0), "today");
        assert_eq!(due_words(1), "tomorrow");
        assert_eq!(due_words(-1), "yesterday");
        assert_eq!(due_words(-3), "3 days late");
        assert_eq!(due_words(9), "in 9 days");
        assert_eq!(due_words(125), "in 4 months");
        assert_eq!(due_words(1000), "in 2 years");
        assert!(attention_order(Overdue) < attention_order(DueSoon));
        assert!(attention_order(DueSoon) < attention_order(Ok));
        assert!(attention_order(Ok) < attention_order(Inactive));
    }

    #[test]
    fn frequency_but_never_done_needs_a_date() {
        let input = RecurringInput {
            interval: MONTHLY,
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.state, NeedsDate);
        assert_eq!(eval.next_due, None);
        assert_eq!(eval.days_left, None);
    }

    #[test]
    fn overdue_from_expiration_date() {
        let input = RecurringInput {
            expiration_date: Some(date!(2026 - 10 - 14)),
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.state, Overdue);
        assert_eq!(eval.days_left, Some(-1));
    }

    #[test]
    fn overdue_from_last_done_plus_interval() {
        let input = RecurringInput {
            interval: MONTHLY,
            last_done: Some(date!(2026 - 09 - 01)),
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.next_due, Some(date!(2026 - 10 - 01)));
        assert_eq!(eval.days_left, Some(-14));
        assert_eq!(eval.state, Overdue);
    }

    #[test]
    fn due_soon_inside_warning_window() {
        let input = RecurringInput {
            expiration_date: Some(date!(2026 - 10 - 25)),
            interval: YEARLY,
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.days_left, Some(10));
        assert_eq!(eval.state, DueSoon);
    }

    #[test]
    fn renewed_when_done_but_expiration_not_moved() {
        // Expires 1 Nov; warn 30 → window opens 2 Oct; done 5 Oct.
        let input = RecurringInput {
            expiration_date: Some(date!(2026 - 11 - 01)),
            interval: YEARLY,
            last_done: Some(date!(2026 - 10 - 05)),
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.next_due, Some(date!(2027 - 11 - 01)));
        assert_eq!(eval.days_left, Some(382));
        assert_eq!(eval.state, Renewed);
        assert_eq!(Renewed.label(), "Renewed – update expiration date");
    }

    #[test]
    fn ok_with_expiration_far_away() {
        let input = RecurringInput {
            expiration_date: Some(date!(2027 - 06 - 01)),
            interval: YEARLY,
            last_done: Some(date!(2026 - 06 - 01)),
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.next_due, Some(date!(2027 - 06 - 01)));
        assert_eq!(eval.state, Ok);
    }

    #[test]
    fn ok_without_expiration() {
        let input = RecurringInput {
            interval: YEARLY,
            last_done: Some(date!(2026 - 10 - 01)),
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.next_due, Some(date!(2027 - 10 - 01)));
        assert_eq!(eval.state, Ok);
    }

    #[test]
    fn expiration_without_frequency_still_alerts() {
        // Only "no expiration AND no frequency" is Ongoing.
        let input = RecurringInput {
            expiration_date: Some(date!(2027 - 06 - 01)),
            ..base()
        };
        assert_eq!(state(input), Ok);
    }

    // ---- Boundaries ----

    fn with_days_left(days: i64, warn: u32) -> RecurringState {
        state(RecurringInput {
            expiration_date: Some(add_days(TODAY, days)),
            warn_days: warn,
            ..base()
        })
    }

    #[test]
    fn days_left_boundaries() {
        let cases = [
            (-365, 30, Overdue),
            (-1, 30, Overdue),
            (0, 30, DueSoon), // due today is not overdue
            (1, 30, DueSoon),
            (29, 30, DueSoon),
            (30, 30, DueSoon), // exactly warn
            (31, 30, Ok),      // warn + 1
            (0, 0, DueSoon),
            (1, 0, Ok),
            (-1, 0, Overdue),
            (7, 7, DueSoon),
            (8, 7, Ok),
        ];
        for (days, warn, expected) in cases {
            assert_eq!(
                with_days_left(days, warn),
                expected,
                "days_left {days}, warn {warn}"
            );
        }
    }

    #[test]
    fn expiration_minus_warn_boundary_for_next_due() {
        let exp = date!(2026 - 12 - 31);
        // warn 30 → threshold 2026-12-01
        assert_eq!(
            next_due(Some(exp), Some(date!(2026 - 12 - 01)), YEARLY, 30),
            Some(date!(2027 - 12 - 31)),
            "done exactly on the threshold counts as renewed"
        );
        assert_eq!(
            next_due(Some(exp), Some(date!(2026 - 11 - 30)), YEARLY, 30),
            Some(exp),
            "done one day before the threshold does not"
        );
        assert_eq!(
            next_due(Some(exp), Some(date!(2027 - 02 - 01)), YEARLY, 30),
            Some(date!(2027 - 12 - 31)),
            "done after expiration counts as renewed"
        );
        assert_eq!(
            next_due(Some(exp), Some(date!(2026 - 12 - 01)), None, 30),
            Some(exp),
            "no frequency: stays on expiration"
        );
        assert_eq!(next_due(Some(exp), None, YEARLY, 30), Some(exp));
    }

    #[test]
    fn expiration_minus_warn_boundary_for_state() {
        // Expires 2027-01-31 (108 days away), warn 30 → threshold 2027-01-01.
        let exp = Some(date!(2027 - 01 - 31));
        let at_threshold = RecurringInput {
            expiration_date: exp,
            interval: YEARLY,
            last_done: Some(date!(2027 - 01 - 01)),
            ..base()
        };
        assert_eq!(state(at_threshold), Renewed);
        let before = RecurringInput {
            last_done: Some(date!(2026 - 12 - 31)),
            ..at_threshold
        };
        assert_eq!(state(before), Ok);
    }

    #[test]
    fn renewed_but_next_estimate_is_close_is_due_soon() {
        // Daily frequency: renewed estimate is expiration + 1 day, inside the window.
        let input = RecurringInput {
            expiration_date: Some(date!(2026 - 10 - 20)),
            interval: DAILY,
            last_done: Some(date!(2026 - 10 - 14)),
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.next_due, Some(date!(2026 - 10 - 21)));
        assert_eq!(eval.state, DueSoon);
    }

    #[test]
    fn renewed_and_past_estimate_is_overdue() {
        let input = RecurringInput {
            expiration_date: Some(date!(2026 - 09 - 01)),
            interval: MONTHLY,
            last_done: Some(date!(2026 - 08 - 20)),
            ..base()
        };
        let eval = evaluate(input, TODAY);
        assert_eq!(eval.next_due, Some(date!(2026 - 10 - 01)));
        assert_eq!(eval.state, Overdue);
    }

    #[test]
    fn zero_warn_window_threshold_is_the_expiration_date() {
        let exp = date!(2026 - 12 - 31);
        assert_eq!(
            next_due(Some(exp), Some(exp), YEARLY, 0),
            Some(date!(2027 - 12 - 31))
        );
        assert_eq!(
            next_due(Some(exp), Some(date!(2026 - 12 - 30)), YEARLY, 0),
            Some(exp)
        );
    }

    // ---- Intervals and month arithmetic ----

    #[test]
    fn interval_adds_months_then_days_with_clamping() {
        let cases = [
            (date!(2026 - 01 - 31), 1, 0, date!(2026 - 02 - 28)),
            (date!(2024 - 01 - 31), 1, 0, date!(2024 - 02 - 29)),
            (date!(2024 - 02 - 29), 12, 0, date!(2025 - 02 - 28)),
            (date!(2024 - 02 - 29), 0, 365, date!(2025 - 02 - 28)),
            (date!(2023 - 02 - 28), 0, 366, date!(2024 - 02 - 29)),
            (date!(2026 - 01 - 31), 1, 15, date!(2026 - 03 - 15)),
            (date!(2026 - 05 - 31), 3, 0, date!(2026 - 08 - 31)),
            (date!(2026 - 05 - 31), 6, 0, date!(2026 - 11 - 30)),
            (date!(2026 - 10 - 15), 0, 7, date!(2026 - 10 - 22)),
            (date!(2026 - 12 - 31), 0, 1, date!(2027 - 01 - 01)),
            (date!(2026 - 10 - 15), 24, 0, date!(2028 - 10 - 15)),
        ];
        for (start, months, days, expected) in cases {
            let iv = Interval::new(months, days);
            assert_eq!(
                iv.map(|i| i.add_to(start)),
                Some(expected),
                "{start} + {months}m {days}d"
            );
        }
    }

    #[test]
    fn next_due_leap_years() {
        assert_eq!(
            next_due(None, Some(date!(2024 - 02 - 29)), YEARLY, 30),
            Some(date!(2025 - 02 - 28))
        );
        assert_eq!(
            next_due(None, Some(date!(2027 - 01 - 31)), MONTHLY, 30),
            Some(date!(2027 - 02 - 28))
        );
        assert_eq!(
            next_due(None, Some(date!(2028 - 01 - 31)), MONTHLY, 30),
            Some(date!(2028 - 02 - 29))
        );
        assert_eq!(
            next_due(
                Some(date!(2024 - 02 - 29)),
                Some(date!(2024 - 02 - 20)),
                YEARLY,
                30
            ),
            Some(date!(2025 - 02 - 28))
        );
    }

    #[test]
    fn zero_interval_counts_as_no_frequency() {
        assert_eq!(Interval::new(0, 0), None);
        let input = RecurringInput {
            interval: Interval::new(0, 0),
            last_done: Some(date!(2026 - 01 - 01)),
            ..base()
        };
        assert_eq!(state(input), Ongoing);
    }

    #[test]
    fn warn_defaults_to_30() {
        assert_eq!(warn_days(None), 30);
        assert_eq!(warn_days(Some(0)), 0);
        assert_eq!(warn_days(Some(14)), 14);
    }

    #[test]
    fn last_done_takes_the_latest() {
        let d1 = date!(2026 - 01 - 01);
        let d2 = date!(2026 - 06 - 01);
        assert_eq!(last_done(None, []), None);
        assert_eq!(last_done(Some(d1), []), Some(d1));
        assert_eq!(last_done(None, [d2, d1]), Some(d2));
        assert_eq!(last_done(Some(d2), [d1]), Some(d2));
        assert_eq!(last_done(Some(d1), [d2]), Some(d2));
    }

    #[test]
    fn past_times_split_at_each_done() {
        let t = task();
        let other = task();
        let on = |d: Date, m: u32, st: Option<&str>| entry(EntryTarget::Recurring(t.id), d, m, st);
        let entries = vec![
            (on(date!(2024 - 10 - 20), 30, None), None),
            (
                on(date!(2024 - 10 - 28), 30, Some(STATUS_DONE)),
                Some(date!(2025 - 11 - 18)),
            ),
            (on(date!(2025 - 11 - 03), 120, None), None),
            (
                on(date!(2025 - 11 - 03), 0, Some(STATUS_DONE)),
                Some(date!(2026 - 11 - 16)),
            ),
            // A plain Done the same day doesn't make a second round.
            (on(date!(2025 - 11 - 03), 0, Some(STATUS_DONE)), None),
            (on(date!(2026 - 10 - 26), 90, None), None),
            (
                entry(
                    EntryTarget::Recurring(other.id),
                    date!(2026 - 10 - 26),
                    60,
                    None,
                ),
                None,
            ),
        ];
        let tickets = vec![
            (date!(2024 - 10 - 20), "RITM0009874".to_string()),
            (date!(2024 - 10 - 28), "ritm0009874".to_string()),
            (date!(2025 - 11 - 01), "RITM0021190".to_string()),
            (date!(2026 - 10 - 26), "RITM0034567".to_string()),
        ];
        let rounds = past_times(t.id, &entries, &tickets);
        type Shape<'a> = (Date, bool, u32, Vec<&'a str>, Option<Date>);
        let shape: Vec<Shape> = rounds
            .iter()
            .map(|r| {
                (
                    r.date,
                    r.done,
                    r.minutes.get(),
                    r.tickets.iter().map(String::as_str).collect(),
                    r.renewed_to,
                )
            })
            .collect();
        assert_eq!(
            shape,
            vec![
                (date!(2026 - 10 - 26), false, 90, vec!["RITM0034567"], None),
                (
                    date!(2025 - 11 - 03),
                    true,
                    120,
                    vec!["RITM0021190"],
                    Some(date!(2026 - 11 - 16))
                ),
                (
                    date!(2024 - 10 - 28),
                    true,
                    60,
                    vec!["RITM0009874"],
                    Some(date!(2025 - 11 - 18))
                ),
            ]
        );
        // Nothing logged: no rounds. Nothing since the last Done: no round in progress.
        assert!(past_times(other.id, &[], &[]).is_empty());
        assert!(past_times(t.id, &entries[..5], &tickets[..3])
            .iter()
            .all(|r| r.done));
    }

    // ---- From records ----

    fn entry(target: EntryTarget, date: Date, minutes: u32, status: Option<&str>) -> TimeEntry {
        TimeEntry {
            id: TimeEntryId::new(),
            owner_id: OwnerId::new(),
            date,
            minutes: Minutes(minutes),
            target,
            category: None,
            description: String::new(),
            status: status.map(String::from),
            priority: None,
            next_step: None,
            timesheet_code_id: None,
            legacy_ticket_text: None,
            created_at: datetime!(2026-10-15 09:00 UTC),
            updated_at: datetime!(2026-10-15 09:00 UTC),
            deleted_at: None,
        }
    }

    fn task() -> RecurringTask {
        RecurringTask {
            id: RecurringTaskId::new(),
            owner_id: OwnerId::new(),
            name: "Renew web certificate".into(),
            recurring_type: Some("Certificate renewal".into()),
            frequency_id: None,
            expiration_date: None,
            warn_days_before: None,
            last_done_before_tracker: Some(date!(2025 - 10 - 01)),
            alerts_enabled: true,
            active: true,
            notes: String::new(),
            default_timesheet_code_id: None,
            created_at: datetime!(2026-01-01 09:00 UTC),
            updated_at: datetime!(2026-01-01 09:00 UTC),
            deleted_at: None,
        }
    }

    fn frequency(months: u32, days: u32) -> Frequency {
        Frequency {
            id: FrequencyId::new(),
            owner_id: OwnerId::new(),
            label: "Yearly".into(),
            months,
            days,
            sort_order: 0,
            created_at: datetime!(2026-01-01 09:00 UTC),
            updated_at: datetime!(2026-01-01 09:00 UTC),
            deleted_at: None,
        }
    }

    #[test]
    fn summary_from_records() {
        let t = task();
        let me = EntryTarget::Recurring(t.id);
        let mut deleted_done = entry(me, date!(2026 - 10 - 10), 30, Some("Done"));
        deleted_done.deleted_at = Some(datetime!(2026-10-11 09:00 UTC));
        let entries = vec![
            entry(me, date!(2026 - 03 - 01), 60, Some("done")),
            entry(me, date!(2026 - 09 - 01), 45, Some("In progress")),
            entry(me, date!(2026 - 08 - 01), 15, None),
            entry(
                EntryTarget::Recurring(RecurringTaskId::new()),
                date!(2026 - 10 - 01),
                90,
                Some("Done"),
            ),
            entry(
                EntryTarget::Project(ProjectId::new()),
                date!(2026 - 10 - 01),
                90,
                Some("Done"),
            ),
            deleted_done,
        ];
        let yearly = frequency(12, 0);
        let s = summarize_recurring(&t, Some(&yearly), &entries, TODAY);
        assert_eq!(s.last_done, Some(date!(2026 - 03 - 01)));
        assert_eq!(s.warn_days, 30);
        assert_eq!(s.next_due, Some(date!(2027 - 03 - 01)));
        assert_eq!(s.total_minutes, Minutes(120));
        assert_eq!(s.state, Ok);

        let s = summarize_recurring(&t, None, &entries, TODAY);
        assert_eq!(s.state, Ongoing, "no expiration and no frequency");

        let s = summarize_recurring(&t, Some(&frequency(0, 0)), &entries, TODAY);
        assert_eq!(s.state, Ongoing, "a zero frequency is no frequency");
    }

    #[test]
    fn routine_bucket_is_ongoing() {
        let mut t = task();
        t.name = "General tickets".into();
        t.alerts_enabled = false;
        t.last_done_before_tracker = None;
        let s = summarize_recurring(&t, Some(&frequency(0, 1)), &[], TODAY);
        assert_eq!(s.state, Ongoing);
    }

    #[test]
    fn labels() {
        let all = [
            (Inactive, "Inactive"),
            (Ongoing, "Ongoing"),
            (NeedsDate, "Needs a date"),
            (Overdue, "Overdue"),
            (DueSoon, "Due soon"),
            (Renewed, "Renewed – update expiration date"),
            (Ok, "OK"),
        ];
        for (s, label) in all {
            assert_eq!(s.label(), label);
        }
    }
}
