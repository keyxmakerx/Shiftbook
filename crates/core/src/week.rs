//! "Week of": the first day of the week a date falls in.
//!
//! Weeks start on Monday by default (SPEC §4); the start day is a setting.

use serde::{Deserialize, Serialize};
use time::{Date, Weekday};

use crate::dates::add_days;

/// The day a week starts on. Defaults to Monday.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WeekStart(pub Weekday);

impl Default for WeekStart {
    fn default() -> Self {
        Self(Weekday::Monday)
    }
}

/// The first day of the week containing `date`: the start day on or before it.
pub fn week_of(date: Date, start: WeekStart) -> Date {
    let back = (i64::from(date.weekday().number_days_from_monday())
        - i64::from(start.0.number_days_from_monday()))
    .rem_euclid(7);
    add_days(date, -back)
}

/// The seven dates of the week that starts on `week_start`.
pub fn week_days(week_start: Date) -> [Date; 7] {
    let mut days = [week_start; 7];
    for (offset, day) in (0i64..).zip(days.iter_mut()) {
        *day = add_days(week_start, offset);
    }
    days
}

/// True when `date` falls in the week that starts on `week_start`.
pub fn in_week(date: Date, week_start: Date) -> bool {
    date >= week_start && date <= add_days(week_start, 6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn monday_start_by_default() {
        let monday = WeekStart::default();
        // 2026-10-12 is a Monday.
        let cases = [
            (date!(2026 - 10 - 12), date!(2026 - 10 - 12)),
            (date!(2026 - 10 - 13), date!(2026 - 10 - 12)),
            (date!(2026 - 10 - 15), date!(2026 - 10 - 12)),
            (date!(2026 - 10 - 18), date!(2026 - 10 - 12)),
            (date!(2026 - 10 - 19), date!(2026 - 10 - 19)),
            // across a year boundary: 2027-01-01 is a Friday
            (date!(2027 - 01 - 01), date!(2026 - 12 - 28)),
            // across a month boundary: 2026-03-01 is a Sunday
            (date!(2026 - 03 - 01), date!(2026 - 02 - 23)),
            // leap day 2024-02-29 is a Thursday
            (date!(2024 - 02 - 29), date!(2024 - 02 - 26)),
        ];
        for (input, expected) in cases {
            assert_eq!(week_of(input, monday), expected, "{input}");
        }
    }

    #[test]
    fn configurable_start() {
        let sunday = WeekStart(Weekday::Sunday);
        let saturday = WeekStart(Weekday::Saturday);
        // 2026-10-15 is a Thursday.
        assert_eq!(
            week_of(date!(2026 - 10 - 15), sunday),
            date!(2026 - 10 - 11)
        );
        assert_eq!(
            week_of(date!(2026 - 10 - 11), sunday),
            date!(2026 - 10 - 11)
        );
        assert_eq!(
            week_of(date!(2026 - 10 - 10), sunday),
            date!(2026 - 10 - 04)
        );
        assert_eq!(
            week_of(date!(2026 - 10 - 15), saturday),
            date!(2026 - 10 - 10)
        );
        assert_eq!(
            week_of(date!(2026 - 10 - 16), saturday),
            date!(2026 - 10 - 10)
        );
    }

    #[test]
    fn every_day_maps_to_a_start_day_within_six_days() {
        let starts = [
            Weekday::Monday,
            Weekday::Tuesday,
            Weekday::Wednesday,
            Weekday::Thursday,
            Weekday::Friday,
            Weekday::Saturday,
            Weekday::Sunday,
        ];
        for start in starts {
            for offset in 0..21 {
                let d = add_days(date!(2026 - 10 - 01), offset);
                let w = week_of(d, WeekStart(start));
                assert_eq!(w.weekday(), start);
                assert!(w <= d && crate::dates::days_between(w, d) < 7);
            }
        }
    }

    #[test]
    fn days_of_week() {
        let days = week_days(date!(2026 - 12 - 28));
        assert_eq!(days[0], date!(2026 - 12 - 28));
        assert_eq!(days[6], date!(2027 - 01 - 03));
        assert!(in_week(date!(2027 - 01 - 03), date!(2026 - 12 - 28)));
        assert!(!in_week(date!(2027 - 01 - 04), date!(2026 - 12 - 28)));
        assert!(!in_week(date!(2026 - 12 - 27), date!(2026 - 12 - 28)));
    }
}
