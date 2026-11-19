//! Booking time off for several days at once (round 21, idea approved 26 October).
//!
//! A day off is an ordinary entry called "Holiday" with the day's usual hours, plus a mark
//! on the day: the ribbon draws it grey and the bell stays quiet. Booking a range books
//! every work day in it (a day whose weekday has a target) that has nothing logged yet;
//! weekends and days already logged are skipped, never overwritten.

use serde::Serialize;
use time::Date;

use crate::dates::{add_days, iso_date};
use crate::error::CoreError;
use crate::hours::format_hours;
use crate::minutes::Minutes;

/// What a day off is called.
pub const TIME_OFF_TITLE: &str = "Holiday";
/// The longest range that can be booked in one go, in days.
pub const MAX_TIME_OFF_DAYS: i64 = 62;
/// The hours a single day off gets when its weekday has no target (8 h).
pub const DEFAULT_DAY_OFF_MINUTES: u32 = 480;

/// Why a day in the range isn't booked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum OffSkip {
    /// Its weekday has no target (a weekend, usually).
    Free,
    /// Something is already logged on it.
    Logged,
}

/// One day of a time-off range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct OffDay {
    /// The date.
    #[serde(with = "iso_date")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub date: Date,
    /// The time it gets (zero when skipped).
    pub minutes: Minutes,
    /// Why it is left alone, or `None` when it is booked.
    pub skip: Option<OffSkip>,
}

/// A range of days and what booking it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TimeOffPlan {
    /// Every day of the range, in order.
    pub days: Vec<OffDay>,
    /// How many get booked.
    pub booked: u32,
    /// Their time together.
    pub minutes: Minutes,
    /// That time as decimal hours.
    pub hours: String,
}

impl TimeOffPlan {
    /// The days that get booked, with their time.
    pub fn to_book(&self) -> impl Iterator<Item = (Date, Minutes)> + '_ {
        self.days
            .iter()
            .filter(|d| d.skip.is_none())
            .map(|d| (d.date, d.minutes))
    }
}

fn target(targets: &[u32; 7], date: Date) -> u32 {
    targets[usize::from(date.weekday().number_days_from_monday())]
}

/// Plan time off from `from` to `to` (both included). `targets` are the daily targets in
/// minutes, Monday first; `logged` the days in the range that already have something
/// logged. The range has to run forwards and be at most [`MAX_TIME_OFF_DAYS`] long.
pub fn plan_time_off(
    from: Date,
    to: Date,
    targets: &[u32; 7],
    logged: &[Date],
) -> Result<TimeOffPlan, CoreError> {
    let length = (to - from).whole_days() + 1;
    if length < 1 {
        return Err(CoreError::TimeOffBackwards);
    }
    if length > MAX_TIME_OFF_DAYS {
        return Err(CoreError::TimeOffTooLong);
    }
    let days: Vec<OffDay> = (0..length)
        .map(|n| {
            let date = add_days(from, n);
            let minutes = target(targets, date);
            let skip = if minutes == 0 {
                Some(OffSkip::Free)
            } else if logged.contains(&date) {
                Some(OffSkip::Logged)
            } else {
                None
            };
            OffDay {
                date,
                minutes: Minutes(if skip.is_some() { 0 } else { minutes }),
                skip,
            }
        })
        .collect();
    let minutes: Minutes = days.iter().map(|d| d.minutes).sum();
    Ok(TimeOffPlan {
        booked: days.iter().filter(|d| d.skip.is_none()).count() as u32,
        hours: format_hours(minutes),
        minutes,
        days,
    })
}

/// The time one day off gets when it is booked on its own: the weekday's target, or 8 h
/// when it has none. Booking a single day never skips it, whatever is logged there.
pub fn one_day_off(date: Date, targets: &[u32; 7]) -> Minutes {
    match target(targets, date) {
        0 => Minutes(DEFAULT_DAY_OFF_MINUTES),
        minutes => Minutes(minutes),
    }
}

/// Mark the ribbon's days off.
pub fn mark_off(weeks: &mut [crate::ribbon::RibbonWeek], off: &[Date]) {
    for day in weeks.iter_mut().flat_map(|w| w.days.iter_mut()) {
        day.off = off.contains(&day.date);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::DEFAULT_DAY_TARGETS;
    use time::macros::date;

    #[test]
    fn a_week_books_its_five_work_days() {
        // Monday 2 to Sunday 8 November 2026.
        let plan = plan_time_off(
            date!(2026 - 11 - 02),
            date!(2026 - 11 - 08),
            &DEFAULT_DAY_TARGETS,
            &[],
        )
        .unwrap();
        assert_eq!(plan.days.len(), 7);
        assert_eq!(plan.booked, 5);
        assert_eq!(plan.minutes, Minutes(2400));
        assert_eq!(plan.hours, "40");
        assert_eq!(plan.days[5].skip, Some(OffSkip::Free));
        assert_eq!(plan.days[6].skip, Some(OffSkip::Free));
        assert_eq!(plan.to_book().count(), 5);
        assert!(plan.to_book().all(|(_, m)| m == Minutes(480)));
    }

    #[test]
    fn days_already_logged_are_left_alone() {
        let plan = plan_time_off(
            date!(2026 - 11 - 02),
            date!(2026 - 11 - 04),
            &DEFAULT_DAY_TARGETS,
            &[date!(2026 - 11 - 03)],
        )
        .unwrap();
        assert_eq!(plan.booked, 2);
        assert_eq!(plan.days[1].skip, Some(OffSkip::Logged));
        assert_eq!(plan.days[1].minutes, Minutes(0));
        let dates: Vec<Date> = plan.to_book().map(|(d, _)| d).collect();
        assert_eq!(dates, [date!(2026 - 11 - 02), date!(2026 - 11 - 04)]);
    }

    #[test]
    fn each_day_gets_its_own_target() {
        // Short Fridays, and Saturdays worked.
        let targets = [480, 480, 480, 480, 240, 120, 0];
        let plan =
            plan_time_off(date!(2026 - 11 - 05), date!(2026 - 11 - 08), &targets, &[]).unwrap();
        let minutes: Vec<u32> = plan.days.iter().map(|d| d.minutes.get()).collect();
        assert_eq!(minutes, [480, 240, 120, 0]);
        assert_eq!(plan.booked, 3);
        assert_eq!(plan.hours, "14");
    }

    #[test]
    fn the_range_runs_forwards_and_has_an_end() {
        let day = date!(2026 - 11 - 02);
        assert_eq!(
            plan_time_off(day, add_days(day, -1), &DEFAULT_DAY_TARGETS, &[]),
            Err(CoreError::TimeOffBackwards)
        );
        assert!(plan_time_off(
            day,
            add_days(day, MAX_TIME_OFF_DAYS - 1),
            &DEFAULT_DAY_TARGETS,
            &[]
        )
        .is_ok());
        assert_eq!(
            plan_time_off(
                day,
                add_days(day, MAX_TIME_OFF_DAYS),
                &DEFAULT_DAY_TARGETS,
                &[]
            ),
            Err(CoreError::TimeOffTooLong)
        );
        // One day is a range too.
        assert_eq!(
            plan_time_off(day, day, &DEFAULT_DAY_TARGETS, &[])
                .unwrap()
                .booked,
            1
        );
    }

    #[test]
    fn a_weekend_with_nothing_to_book() {
        let plan = plan_time_off(
            date!(2026 - 11 - 07),
            date!(2026 - 11 - 08),
            &DEFAULT_DAY_TARGETS,
            &[],
        )
        .unwrap();
        assert_eq!(plan.booked, 0);
        assert_eq!(plan.minutes, Minutes(0));
    }

    #[test]
    fn one_day_takes_its_target_or_eight_hours() {
        let targets = [480, 480, 480, 480, 240, 0, 0];
        assert_eq!(one_day_off(date!(2026 - 11 - 06), &targets), Minutes(240));
        assert_eq!(one_day_off(date!(2026 - 11 - 07), &targets), Minutes(480));
    }
}
