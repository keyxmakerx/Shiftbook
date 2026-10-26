//! The "next up" list: a few things to start with, written at the end of a day and shown
//! by "Where was I?" (first item, the rest as a checklist).
//!
//! **Bring up tomorrow** adds one thing from a card (an entry, a ticket, a note) for the
//! next work day. Those stay apart from the wrap-up list (saving the list doesn't touch
//! them) and keep coming up until ticked.

use time::Date;

use crate::dates::add_days;
use crate::error::CoreError;
use crate::validate::validate_required;
use crate::week::{week_of, WeekStart};

/// Most things on the list.
pub const MAX_NEXT_UP: usize = 7;
/// Longest line on the list.
pub const MAX_NEXT_UP_CHARS: usize = 200;

/// Clean a typed list: trim each line, drop blank ones, check the length and count.
pub fn clean_next_up(lines: &[String]) -> Result<Vec<String>, CoreError> {
    let items = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| validate_required("A line", l, MAX_NEXT_UP_CHARS))
        .collect::<Result<Vec<_>, _>>()?;
    if items.len() > MAX_NEXT_UP {
        return Err(CoreError::TooManyItems {
            field: "the list",
            max: MAX_NEXT_UP,
        });
    }
    Ok(items)
}

/// The day a thing brought up from `today` comes up: the next day with a target
/// (`is_work_day`), looking a week ahead; tomorrow when no day in that week has one.
pub fn next_work_day(today: Date, is_work_day: impl Fn(Date) -> bool) -> Date {
    (1..=7)
        .map(|n| add_days(today, n))
        .find(|d| is_work_day(*d))
        .unwrap_or_else(|| add_days(today, 1))
}

/// The days "Bring up…" offers from `today` (round 15): the next work day, the work days
/// on or after two and four days ahead, and the first work day of next week (the week
/// after tomorrow's, so on a last day of the week it isn't tomorrow again). Repeats are
/// dropped, so a short week can offer fewer.
pub fn bring_up_days(
    today: Date,
    week_start: WeekStart,
    is_work_day: impl Fn(Date) -> bool,
) -> Vec<Date> {
    let next_week = add_days(week_of(add_days(today, 1), week_start), 7);
    let mut days: Vec<Date> = Vec::with_capacity(4);
    for from in [
        add_days(today, 1),
        add_days(today, 2),
        add_days(today, 4),
        next_week,
    ] {
        let day = work_day_from(from, &is_work_day);
        if !days.contains(&day) {
            days.push(day);
        }
    }
    days
}

/// The day a thing brought up to `chosen` comes up: `chosen` when it's a work day,
/// otherwise the next work day after it. `None` for today or earlier.
pub fn bring_up_on(today: Date, chosen: Date, is_work_day: impl Fn(Date) -> bool) -> Option<Date> {
    (chosen > today).then(|| work_day_from(chosen, &is_work_day))
}

/// `day` when it's a work day, else the next work day within a week, else `day` itself.
fn work_day_from(day: Date, is_work_day: &impl Fn(Date) -> bool) -> Date {
    (0..7)
        .map(|n| add_days(day, n))
        .find(|d| is_work_day(*d))
        .unwrap_or(day)
}

/// The line for a card brought up: its title on one line, shortened with "…" to fit.
pub fn bring_up_text(title: &str) -> Result<String, CoreError> {
    let one_line = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let text = if one_line.chars().count() > MAX_NEXT_UP_CHARS {
        let cut: String = one_line.chars().take(MAX_NEXT_UP_CHARS - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        one_line
    };
    validate_required("The line", &text, MAX_NEXT_UP_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn trims_and_drops_blanks() {
        assert_eq!(
            clean_next_up(&lines(&["  Call the vendor ", "", "   ", "Renew certs"])),
            Ok(lines(&["Call the vendor", "Renew certs"]))
        );
        assert_eq!(clean_next_up(&[]), Ok(vec![]));
    }

    #[test]
    fn tomorrow_means_the_next_work_day() {
        use time::macros::date;
        use time::Weekday;
        let weekdays = |d: Date| !matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday);
        assert_eq!(
            next_work_day(date!(2026 - 10 - 19), weekdays),
            date!(2026 - 10 - 20)
        );
        // Friday comes up on Monday.
        assert_eq!(
            next_work_day(date!(2026 - 10 - 23), weekdays),
            date!(2026 - 10 - 26)
        );
        assert_eq!(
            next_work_day(date!(2026 - 10 - 23), |_| false),
            date!(2026 - 10 - 24)
        );
    }

    #[test]
    fn bring_up_offers_a_few_work_days_and_next_week() {
        use time::macros::date;
        use time::Weekday;
        let weekdays = |d: Date| !matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday);
        let monday = WeekStart(Weekday::Monday);
        // From Sunday 25 Oct: Mon 26, Tue 27 (+2), Thu 29 (+4), next week's Monday 2 Nov.
        assert_eq!(
            bring_up_days(date!(2026 - 10 - 25), monday, weekdays),
            vec![
                date!(2026 - 10 - 26),
                date!(2026 - 10 - 27),
                date!(2026 - 10 - 29),
                date!(2026 - 11 - 02),
            ]
        );
        // From Thursday 22 Oct: Fri 23, Sat→Mon 26, Mon 26 again (dropped), Mon 26 again.
        assert_eq!(
            bring_up_days(date!(2026 - 10 - 22), monday, weekdays),
            vec![date!(2026 - 10 - 23), date!(2026 - 10 - 26)]
        );
        // No work days at all: plain days, never an endless search.
        assert_eq!(
            bring_up_days(date!(2026 - 10 - 25), monday, |_| false).len(),
            4
        );
    }

    #[test]
    fn a_picked_day_moves_off_the_weekend_and_must_be_after_today() {
        use time::macros::date;
        use time::Weekday;
        let weekdays = |d: Date| !matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday);
        let today = date!(2026 - 10 - 21);
        assert_eq!(
            bring_up_on(today, date!(2026 - 10 - 29), weekdays),
            Some(date!(2026 - 10 - 29))
        );
        assert_eq!(
            bring_up_on(today, date!(2026 - 10 - 31), weekdays),
            Some(date!(2026 - 11 - 02))
        );
        assert_eq!(bring_up_on(today, today, weekdays), None);
        assert_eq!(bring_up_on(today, date!(2026 - 10 - 01), weekdays), None);
    }

    #[test]
    fn a_card_title_becomes_one_short_line() {
        assert_eq!(
            bring_up_text("  call vendor\nabout the  PSU "),
            Ok("call vendor about the PSU".into())
        );
        let long = bring_up_text(&"word ".repeat(80)).unwrap();
        assert_eq!(long.chars().count(), MAX_NEXT_UP_CHARS);
        assert!(long.ends_with('…'));
        assert!(bring_up_text("   ").is_err());
    }

    #[test]
    fn limits_count_and_length() {
        let eight = lines(&["a", "b", "c", "d", "e", "f", "g", "h"]);
        assert!(matches!(
            clean_next_up(&eight),
            Err(CoreError::TooManyItems { max: 7, .. })
        ));
        assert!(clean_next_up(&eight[..7]).is_ok());
        let long = "x".repeat(MAX_NEXT_UP_CHARS + 1);
        assert!(matches!(
            clean_next_up(&[long]),
            Err(CoreError::TooLong { .. })
        ));
    }
}
