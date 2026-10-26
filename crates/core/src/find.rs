//! Find (round 15): reading a day out of what was typed, so "fri", "last friday",
//! "12 oct" or "2026-10-12" offers to go to that day.

use time::{Date, Month};

use crate::capture::{day_word_date, weekday_of};
use crate::dates::{add_days, days_between, parse_date};
use crate::week::WeekStart;

/// A day and month this many days ahead still means this year; further means last year.
pub const AHEAD_DAYS: i64 = 60;

fn month_of(word: &str) -> Option<Month> {
    let word = word.trim_end_matches('.');
    if word.len() < 3 {
        return None;
    }
    const MONTHS: [(&str, Month); 12] = [
        ("january", Month::January),
        ("february", Month::February),
        ("march", Month::March),
        ("april", Month::April),
        ("may", Month::May),
        ("june", Month::June),
        ("july", Month::July),
        ("august", Month::August),
        ("september", Month::September),
        ("october", Month::October),
        ("november", Month::November),
        ("december", Month::December),
    ];
    MONTHS
        .iter()
        .find(|(name, _)| name.starts_with(word) || (word == "sept" && *name == "september"))
        .map(|(_, m)| *m)
}

fn day_number(word: &str) -> Option<u8> {
    let word = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|s| word.strip_suffix(s))
        .unwrap_or(word);
    word.parse().ok().filter(|d| (1..=31).contains(d))
}

/// "12 oct", "oct 12", "12 october 2025": this year's, or last year's when this year's is
/// more than `AHEAD_DAYS` away.
fn day_and_month(words: &[&str], today: Date) -> Option<Date> {
    let (day, month, year) = match words {
        [a, b] | [a, b, _] => match (day_number(a), month_of(b), day_number(b), month_of(a)) {
            (Some(d), Some(m), _, _) => (d, m, words.get(2)),
            (_, _, Some(d), Some(m)) => (d, m, words.get(2)),
            _ => return None,
        },
        _ => return None,
    };
    if let Some(year) = year {
        let year: i32 = year.parse().ok()?;
        return Date::from_calendar_date(year, month, day).ok();
    }
    let this = Date::from_calendar_date(today.year(), month, day).ok();
    match this {
        Some(d) if days_between(today, d) <= AHEAD_DAYS => Some(d),
        _ => Date::from_calendar_date(today.year() - 1, month, day).ok(),
    }
}

/// The day `text` names, or `None` when it isn't a day. "last friday" is the most recent
/// Friday before today, "next friday" the first one after it, and a bare "fri" is the
/// Friday of this week.
pub fn find_day(text: &str, today: Date, week_start: WeekStart) -> Option<Date> {
    let text = text.trim().to_lowercase();
    let text = text
        .strip_prefix("go to ")
        .or_else(|| text.strip_prefix("on "))
        .unwrap_or(&text)
        .trim();
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    match words.as_slice() {
        [one] => parse_date(one)
            .ok()
            .or_else(|| day_word_date(one, today, week_start)),
        ["last", day] => {
            let want = weekday_of(day)?;
            (1..=7)
                .map(|n| add_days(today, -n))
                .find(|d| d.weekday() == want)
        }
        ["next", day] => {
            let want = weekday_of(day)?;
            (1..=7)
                .map(|n| add_days(today, n))
                .find(|d| d.weekday() == want)
        }
        _ => day_and_month(&words, today),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    // Sunday 25 October 2026.
    const TODAY: Date = date!(2026 - 10 - 25);

    fn day(text: &str) -> Option<Date> {
        find_day(text, TODAY, WeekStart::default())
    }

    #[test]
    fn words_and_weekdays() {
        assert_eq!(day("today"), Some(TODAY));
        assert_eq!(day("Yesterday"), Some(date!(2026 - 10 - 24)));
        assert_eq!(day("fri"), Some(date!(2026 - 10 - 23)));
        assert_eq!(day("go to friday"), Some(date!(2026 - 10 - 23)));
        assert_eq!(day("last friday"), Some(date!(2026 - 10 - 23)));
        assert_eq!(day("last sunday"), Some(date!(2026 - 10 - 18)));
        assert_eq!(day("next mon"), Some(date!(2026 - 10 - 26)));
    }

    #[test]
    fn day_and_month_pick_the_near_year() {
        assert_eq!(day("12 oct"), Some(date!(2026 - 10 - 12)));
        assert_eq!(day("Oct 12th"), Some(date!(2026 - 10 - 12)));
        assert_eq!(day("1 dec"), Some(date!(2026 - 12 - 01)));
        assert_eq!(day("3 february"), Some(date!(2026 - 02 - 03)));
        assert_eq!(day("30 december"), Some(date!(2025 - 12 - 30)));
        assert_eq!(day("12 oct 2024"), Some(date!(2024 - 10 - 12)));
        assert_eq!(day("2026-10-12"), Some(date!(2026 - 10 - 12)));
    }

    #[test]
    fn other_text_is_not_a_day() {
        for text in [
            "",
            "printer",
            "back up now",
            "31 feb",
            "12 ma",
            "last week",
            "fr",
        ] {
            assert_eq!(day(text), None, "{text}");
        }
    }
}
