//! Meeting series (round 12): a meeting that repeats on set weekdays at a set time, so the
//! bell can say "Weekly ops in 15 minutes" and open the day's meeting note. A series is
//! only a reminder; its notes are ordinary meeting notes titled by
//! [`meeting_note_title`].

use serde::Serialize;
use time::Date;

use crate::error::CoreError;
use crate::notes::daily_note_title;
use crate::validate::validate_required;

/// Longest series name.
pub const MAX_SERIES_NAME_CHARS: usize = 80;
/// Most minutes ahead a reminder can start.
pub const MAX_REMIND_MINUTES: u32 = 120;
/// Minutes a reminder starts ahead unless set otherwise.
pub const DEFAULT_REMIND_MINUTES: u32 = 15;
/// A meeting still shows as on for this long after it starts.
pub const STARTED_SHOWS_MINUTES: u32 = 15;
/// Shortest a meeting can usually last.
pub const MIN_LENGTH_MINUTES: u32 = 5;
/// Longest a meeting can usually last: 8 hours.
pub const MAX_LENGTH_MINUTES: u32 = 8 * 60;
/// How long a meeting usually lasts unless set otherwise.
pub const DEFAULT_LENGTH_MINUTES: u32 = 30;
/// Short weekday names in stored order (Monday first).
pub const DAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// Read the stored weekdays, e.g. `mon,thu`; `None` when a name is unknown or none is set.
pub fn parse_days(text: &str) -> Option<[bool; 7]> {
    let mut days = [false; 7];
    for name in text.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let i = DAY_NAMES
            .iter()
            .position(|d| d.eq_ignore_ascii_case(name))?;
        days[i] = true;
    }
    days.contains(&true).then_some(days)
}

/// Write weekdays in the stored form.
pub fn format_days(days: &[bool; 7]) -> String {
    DAY_NAMES
        .iter()
        .zip(days)
        .filter(|(_, on)| **on)
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(",")
}

/// Check a series before saving: a name, at least one weekday, a time of day in minutes
/// (0 to 1439) and a reminder of 0 to [`MAX_REMIND_MINUTES`] minutes ahead. Returns the
/// trimmed name.
pub fn validate_series(
    name: &str,
    days: &[bool; 7],
    at: u32,
    remind_minutes: u32,
) -> Result<String, CoreError> {
    let name = validate_required("The name", name, MAX_SERIES_NAME_CHARS)?;
    if !days.contains(&true) {
        return Err(CoreError::Required { field: "A day" });
    }
    if at >= 24 * 60 {
        return Err(CoreError::OutOfRange {
            field: "The time",
            min: 0,
            max: 24 * 60 - 1,
        });
    }
    if remind_minutes > MAX_REMIND_MINUTES {
        return Err(CoreError::OutOfRange {
            field: "Remind me",
            min: 0,
            max: i64::from(MAX_REMIND_MINUTES),
        });
    }
    Ok(name)
}

/// Check how long a series usually lasts: [`MIN_LENGTH_MINUTES`] to [`MAX_LENGTH_MINUTES`].
pub fn validate_length(length_minutes: u32) -> Result<(), CoreError> {
    if (MIN_LENGTH_MINUTES..=MAX_LENGTH_MINUTES).contains(&length_minutes) {
        Ok(())
    } else {
        Err(CoreError::OutOfRange {
            field: "Usually lasts",
            min: i64::from(MIN_LENGTH_MINUTES),
            max: i64::from(MAX_LENGTH_MINUTES),
        })
    }
}

/// True when today's meeting of a series is over (its usual length has passed) and its
/// time hasn't been logged or skipped today, so the bell offers "Log 30m".
pub fn meeting_finished(
    days: &[bool; 7],
    at: u32,
    length_minutes: u32,
    handled_on: Option<Date>,
    today: Date,
    now: u32,
) -> bool {
    days[usize::from(today.weekday().number_days_from_monday())]
        && now >= at + length_minutes
        && handled_on != Some(today)
}

/// Where today's meeting of a series stands at `now` (minutes since local midnight).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "minutes")]
pub enum MeetingToday {
    /// Later today, before its reminder starts.
    Later,
    /// Starts in this many minutes (the reminder is on).
    Soon(u32),
    /// Started this many minutes ago (up to [`STARTED_SHOWS_MINUTES`]).
    Started(u32),
}

/// Today's meeting of a series, if it meets today and isn't long over.
pub fn meeting_today(
    days: &[bool; 7],
    at: u32,
    remind_minutes: u32,
    today: Date,
    now: u32,
) -> Option<MeetingToday> {
    if !days[usize::from(today.weekday().number_days_from_monday())] {
        return None;
    }
    if now < at {
        let ahead = at - now;
        return Some(if ahead <= remind_minutes {
            MeetingToday::Soon(ahead)
        } else {
            MeetingToday::Later
        });
    }
    let since = now - at;
    (since <= STARTED_SHOWS_MINUTES).then_some(MeetingToday::Started(since))
}

/// The title of a series' meeting note for a day, e.g. "Weekly ops · Mon 19 Oct 2026".
pub fn meeting_note_title(name: &str, date: Date) -> String {
    let day = daily_note_title(date);
    let day = day.strip_prefix("Daily · ").unwrap_or(&day);
    format!("{} · {day}", name.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    const MON: Date = date!(2026 - 10 - 19);

    #[test]
    fn days_round_trip() {
        let days = parse_days(" Mon, thu ").unwrap();
        assert_eq!(days, [true, false, false, true, false, false, false]);
        assert_eq!(format_days(&days), "mon,thu");
        assert_eq!(parse_days("mon,funday"), None);
        assert_eq!(parse_days(" , "), None);
    }

    #[test]
    fn series_rules() {
        let mon = [true, false, false, false, false, false, false];
        assert_eq!(
            validate_series(" Weekly ops ", &mon, 600, 15),
            Ok("Weekly ops".into())
        );
        assert!(validate_series("  ", &mon, 600, 15).is_err());
        assert!(validate_series("Ops", &[false; 7], 600, 15).is_err());
        assert!(validate_series("Ops", &mon, 1440, 15).is_err());
        assert!(validate_series("Ops", &mon, 600, MAX_REMIND_MINUTES + 1).is_err());
    }

    #[test]
    fn where_todays_meeting_stands() {
        let mon = [true, false, false, false, false, false, false];
        let at = 10 * 60;
        assert_eq!(
            meeting_today(&mon, at, 15, MON, 9 * 60),
            Some(MeetingToday::Later)
        );
        assert_eq!(
            meeting_today(&mon, at, 15, MON, at - 15),
            Some(MeetingToday::Soon(15))
        );
        assert_eq!(
            meeting_today(&mon, at, 15, MON, at - 1),
            Some(MeetingToday::Soon(1))
        );
        assert_eq!(
            meeting_today(&mon, at, 15, MON, at),
            Some(MeetingToday::Started(0))
        );
        assert_eq!(
            meeting_today(&mon, at, 15, MON, at + 15),
            Some(MeetingToday::Started(15))
        );
        assert_eq!(meeting_today(&mon, at, 15, MON, at + 16), None);
        assert_eq!(
            meeting_today(&mon, at, 15, date!(2026 - 10 - 20), 9 * 60),
            None
        );
        assert_eq!(
            meeting_today(&mon, at, 0, MON, at - 1),
            Some(MeetingToday::Later)
        );
    }

    #[test]
    fn a_meeting_is_finished_once_its_length_has_passed_until_logged() {
        let mon = [true, false, false, false, false, false, false];
        let at = 10 * 60;
        assert!(!meeting_finished(&mon, at, 30, None, MON, at + 29));
        assert!(meeting_finished(&mon, at, 30, None, MON, at + 30));
        assert!(meeting_finished(&mon, at, 30, None, MON, 23 * 60));
        assert!(meeting_finished(
            &mon,
            at,
            30,
            Some(date!(2026 - 10 - 12)),
            MON,
            at + 30
        ));
        assert!(!meeting_finished(&mon, at, 30, Some(MON), MON, at + 30));
        assert!(!meeting_finished(
            &mon,
            at,
            30,
            None,
            date!(2026 - 10 - 20),
            at + 30
        ));
        assert!(validate_length(30).is_ok());
        assert!(validate_length(4).is_err());
        assert!(validate_length(MAX_LENGTH_MINUTES + 1).is_err());
    }

    #[test]
    fn meeting_notes_are_titled_by_series_and_day() {
        assert_eq!(
            meeting_note_title(" Weekly ops ", MON),
            "Weekly ops · Mon 19 Oct 2026"
        );
    }
}
