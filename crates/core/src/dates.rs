//! Calendar dates (`YYYY-MM-DD`, local) and timestamps (UTC, RFC 3339).
//!
//! Dates use [`time::Date`]; timestamps use [`time::OffsetDateTime`]. The
//! `serde` helper modules here make both serialize in the text form the
//! storage layer and the UI expect.

use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, Duration, Month, OffsetDateTime};

use crate::error::{preview, CoreError};

pub use time::Date as LocalDate;
pub use time::OffsetDateTime as Timestamp;

/// Earliest year the app accepts for a user-entered date.
pub const MIN_YEAR: i32 = 1900;
/// Latest year the app accepts for a user-entered date.
pub const MAX_YEAR: i32 = 2999;

// `iso_date` serializes `Date` as "YYYY-MM-DD"; `iso_date::option` handles `Option<Date>`.
time::serde::format_description!(iso_date, Date, "[year]-[month]-[day]");

/// Serde helpers for timestamps as RFC 3339 text in UTC.
pub mod rfc3339 {
    pub use time::serde::rfc3339::*;
}

/// Parse a `YYYY-MM-DD` date (surrounding spaces ignored), years 1900–2999.
pub fn parse_date(text: &str) -> Result<Date, CoreError> {
    let trimmed = text.trim();
    let format = format_description!("[year]-[month]-[day]");
    let date = Date::parse(trimmed, &format).map_err(|_| CoreError::DateInvalid {
        input: preview(trimmed, 20),
    })?;
    if !(MIN_YEAR..=MAX_YEAR).contains(&date.year()) {
        return Err(CoreError::DateInvalid {
            input: preview(trimmed, 20),
        });
    }
    Ok(date)
}

/// Format a date as `YYYY-MM-DD`.
pub fn format_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// Format a timestamp as RFC 3339 in UTC, e.g. `2026-10-15T08:30:00Z`.
pub fn format_timestamp(ts: OffsetDateTime) -> String {
    let utc = ts.to_offset(time::UtcOffset::UTC);
    utc.format(&Rfc3339).unwrap_or_else(|_| {
        // Only fails for years outside 0..=9999, which the app never creates.
        String::from("0000-01-01T00:00:00Z")
    })
}

/// Days from `from` to `to` (negative when `to` is earlier).
pub fn days_between(from: Date, to: Date) -> i64 {
    (to - from).whole_days()
}

/// `date` plus (or minus) a number of days, clamped to the supported range.
pub fn add_days(date: Date, days: i64) -> Date {
    date.saturating_add(Duration::days(days))
}

/// Number of days in a month.
pub fn days_in_month(year: i32, month: Month) -> u8 {
    match month {
        Month::January
        | Month::March
        | Month::May
        | Month::July
        | Month::August
        | Month::October
        | Month::December => 31,
        Month::April | Month::June | Month::September | Month::November => 30,
        Month::February => {
            if time::util::is_leap_year(year) {
                29
            } else {
                28
            }
        }
    }
}

/// `date` plus a number of months. When the day does not exist in the target
/// month it is clamped to the month's last day (31 Jan + 1 month = 28/29 Feb),
/// like Excel's `EDATE`. Years beyond the supported range are clamped.
pub fn add_months(date: Date, months: u32) -> Date {
    let start = i64::from(date.year()) * 12 + i64::from(u8::from(date.month())) - 1;
    let target = start + i64::from(months);
    let max_index = i64::from(time::Date::MAX.year()) * 12 + 11;
    if target > max_index {
        return Date::MAX;
    }
    let year = i32::try_from(target.div_euclid(12)).unwrap_or(i32::MAX);
    let month_number = u8::try_from(target.rem_euclid(12) + 1).unwrap_or(12);
    let month = Month::try_from(month_number).unwrap_or(Month::December);
    let day = date.day().min(days_in_month(year, month));
    Date::from_calendar_date(year, month, day).unwrap_or(Date::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    #[test]
    fn parses_iso_dates() {
        assert_eq!(parse_date("2026-10-15"), Ok(date!(2026 - 10 - 15)));
        assert_eq!(parse_date(" 2024-02-29 "), Ok(date!(2024 - 02 - 29)));
    }

    #[test]
    fn rejects_bad_dates() {
        for bad in [
            "",
            "2026-13-01",
            "2023-02-29",
            "15/10/2026",
            "2026-1-5",
            "1899-12-31",
            "3000-01-01",
            "tomorrow",
        ] {
            assert!(
                matches!(parse_date(bad), Err(CoreError::DateInvalid { .. })),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn formats_dates_and_timestamps() {
        assert_eq!(format_date(date!(2026 - 01 - 05)), "2026-01-05");
        assert_eq!(
            format_timestamp(datetime!(2026-10-15 08:30:00 UTC)),
            "2026-10-15T08:30:00Z"
        );
        assert_eq!(
            format_timestamp(datetime!(2026-10-15 10:30:00 +2)),
            "2026-10-15T08:30:00Z"
        );
    }

    #[test]
    fn days_between_and_add_days() {
        assert_eq!(
            days_between(date!(2026 - 10 - 15), date!(2026 - 10 - 20)),
            5
        );
        assert_eq!(
            days_between(date!(2026 - 10 - 15), date!(2026 - 10 - 10)),
            -5
        );
        assert_eq!(add_days(date!(2024 - 02 - 28), 1), date!(2024 - 02 - 29));
        assert_eq!(add_days(date!(2026 - 01 - 01), -1), date!(2025 - 12 - 31));
    }

    #[test]
    fn month_lengths() {
        assert_eq!(days_in_month(2024, Month::February), 29);
        assert_eq!(days_in_month(2023, Month::February), 28);
        assert_eq!(days_in_month(2000, Month::February), 29);
        assert_eq!(days_in_month(1900, Month::February), 28);
        assert_eq!(days_in_month(2026, Month::April), 30);
        assert_eq!(days_in_month(2026, Month::December), 31);
    }

    #[test]
    fn add_months_clamps_to_month_end() {
        let cases = [
            (date!(2026 - 01 - 31), 1, date!(2026 - 02 - 28)),
            (date!(2024 - 01 - 31), 1, date!(2024 - 02 - 29)),
            (date!(2024 - 02 - 29), 12, date!(2025 - 02 - 28)),
            (date!(2024 - 02 - 29), 48, date!(2028 - 02 - 29)),
            (date!(2026 - 03 - 31), 1, date!(2026 - 04 - 30)),
            (date!(2026 - 08 - 31), 6, date!(2027 - 02 - 28)),
            (date!(2026 - 11 - 30), 3, date!(2027 - 02 - 28)),
            (date!(2026 - 12 - 15), 1, date!(2027 - 01 - 15)),
            (date!(2026 - 10 - 15), 0, date!(2026 - 10 - 15)),
            (date!(2026 - 10 - 15), 24, date!(2028 - 10 - 15)),
        ];
        for (start, months, expected) in cases {
            assert_eq!(add_months(start, months), expected, "{start} + {months}");
        }
    }

    #[test]
    fn add_months_saturates_far_future() {
        assert_eq!(add_months(date!(2026 - 01 - 01), u32::MAX), Date::MAX);
    }
}
