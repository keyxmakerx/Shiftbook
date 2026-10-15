//! Turning what the user types in the Hours box into whole minutes, and back.
//!
//! Accepted forms (spaces anywhere between parts, any letter case):
//!
//! | Typed            | Meaning                         | Minutes |
//! |------------------|---------------------------------|---------|
//! | `1.5` or `1,5`   | decimal hours                   | 90      |
//! | `1:30`           | hours:minutes (two-digit mins)  | 90      |
//! | `90m`, `90 min`  | minutes                         | 90      |
//! | `2h`, `1.5h`     | hours                           | 120, 90 |
//! | `1h 30m`, `1h30` | hours and minutes               | 90      |
//! | `8`              | a whole number up to 24 is hours| 480     |
//! | `45`             | a whole number over 24 is minutes| 45     |
//!
//! Rules:
//! - Decimal hours are rounded to the **nearest whole minute**, halves rounded
//!   up (`0.01` h = 0.6 min → 1 minute; `1.333` h = 79.98 min → 80 minutes).
//!   At most 6 decimal places are accepted.
//! - Minutes must be whole (`1.5m` is rejected).
//! - In `1:30` and `1h 30m` the minutes part must be under 60.
//! - The result must be between 0 and 24 hours (1440 minutes) inclusive.
//!   Zero is allowed (for example a "done" marker entry with no time).

use crate::error::{preview, CoreError};
use crate::minutes::{Minutes, MAX_ENTRY_MINUTES};

/// The largest whole number typed on its own that is read as hours.
/// Anything larger (e.g. `45`) is read as minutes.
pub const BARE_NUMBER_HOURS_LIMIT: u64 = 24;

const MAX_DECIMALS: u32 = 6;
const MAX_INT_DIGITS: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Number {
    int: u64,
    int_digits: u32,
    frac: u64,
    frac_digits: u32,
    has_separator: bool,
}

impl Number {
    fn is_whole(&self) -> bool {
        !self.has_separator
    }

    /// Hours (possibly fractional) to minutes, rounded to the nearest minute.
    fn hours_to_minutes(&self) -> u128 {
        let scale = 10u128.pow(self.frac_digits);
        let scaled = u128::from(self.int) * 60 * scale + u128::from(self.frac) * 60;
        (scaled + scale / 2) / scale
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    Num(Number),
    Hours,
    Mins,
    Colon,
}

/// Parse the Hours box into minutes. See the module docs for the accepted forms.
pub fn parse_hours(input: &str) -> Result<Minutes, CoreError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CoreError::HoursEmpty);
    }
    if trimmed.starts_with('-') || trimmed.starts_with('\u{2212}') {
        return Err(CoreError::HoursNegative);
    }
    let invalid = || CoreError::HoursInvalid {
        input: preview(trimmed, 30),
    };

    let tokens = tokenize(trimmed).ok_or_else(invalid)?;
    let total: u128 = match tokens.as_slice() {
        [Token::Num(n)] => {
            if n.is_whole() && n.int > BARE_NUMBER_HOURS_LIMIT {
                u128::from(n.int)
            } else {
                n.hours_to_minutes()
            }
        }
        [Token::Num(h), Token::Colon, Token::Num(m)] => {
            if !h.is_whole() || !m.is_whole() || m.int_digits != 2 {
                return Err(invalid());
            }
            u128::from(h.int) * 60 + minutes_part(m)?
        }
        [Token::Num(h), Token::Hours] => h.hours_to_minutes(),
        [Token::Num(h), Token::Hours, Token::Num(m)]
        | [Token::Num(h), Token::Hours, Token::Num(m), Token::Mins] => {
            if !h.is_whole() || !m.is_whole() {
                return Err(invalid());
            }
            u128::from(h.int) * 60 + minutes_part(m)?
        }
        [Token::Num(m), Token::Mins] => {
            if !m.is_whole() {
                return Err(invalid());
            }
            u128::from(m.int)
        }
        _ => return Err(invalid()),
    };

    if total > u128::from(MAX_ENTRY_MINUTES) {
        return Err(CoreError::HoursTooLarge);
    }
    u32::try_from(total)
        .map(Minutes)
        .map_err(|_| CoreError::HoursTooLarge)
}

fn minutes_part(m: &Number) -> Result<u128, CoreError> {
    if m.int >= 60 {
        Err(CoreError::HoursMinutesPartTooBig { minutes: m.int })
    } else {
        Ok(u128::from(m.int))
    }
}

/// Split the text into numbers, unit words and colons. `None` for anything else.
fn tokenize(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == ':' {
            tokens.push(Token::Colon);
            i += 1;
        } else if c.is_ascii_digit() || c == '.' || c == ',' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == ',')
            {
                i += 1;
            }
            tokens.push(Token::Num(parse_number(&chars[start..i])?));
        } else if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect::<String>().to_lowercase();
            tokens.push(match word.as_str() {
                "h" | "hr" | "hrs" | "hour" | "hours" => Token::Hours,
                "m" | "min" | "mins" | "minute" | "minutes" => Token::Mins,
                _ => return None,
            });
        } else {
            return None;
        }
    }
    Some(tokens)
}

/// Parse digits with at most one `.` or `,` decimal separator.
fn parse_number(chars: &[char]) -> Option<Number> {
    let mut number = Number {
        int: 0,
        int_digits: 0,
        frac: 0,
        frac_digits: 0,
        has_separator: false,
    };
    for &c in chars {
        if c == '.' || c == ',' {
            if number.has_separator {
                return None;
            }
            number.has_separator = true;
        } else {
            let digit = u64::from(c.to_digit(10)?);
            if number.has_separator {
                number.frac_digits += 1;
                if number.frac_digits > MAX_DECIMALS {
                    return None;
                }
                number.frac = number.frac * 10 + digit;
            } else {
                number.int_digits += 1;
                if number.int_digits > MAX_INT_DIGITS {
                    return None;
                }
                number.int = number.int * 10 + digit;
            }
        }
    }
    if number.int_digits + number.frac_digits == 0 {
        return None;
    }
    Some(number)
}

/// Minutes as decimal hours with at most two decimals and no trailing zeros:
/// 90 → `1.5`, 75 → `1.25`, 20 → `0.33`, 0 → `0`. Rounds to the nearest
/// hundredth of an hour, halves up.
pub fn format_hours(minutes: Minutes) -> String {
    let hundredths = (u64::from(minutes.get()) * 100 + 30) / 60;
    let whole = hundredths / 100;
    let rest = hundredths % 100;
    if rest == 0 {
        format!("{whole}")
    } else if rest % 10 == 0 {
        format!("{whole}.{}", rest / 10)
    } else {
        format!("{whole}.{rest:02}")
    }
}

/// Minutes as hours and minutes: 90 → `1h 30m`, 45 → `45m`, 120 → `2h`, 0 → `0m`.
pub fn format_hours_minutes(minutes: Minutes) -> String {
    let h = minutes.get() / 60;
    let m = minutes.get() % 60;
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_all_documented_forms() {
        let cases: &[(&str, u32)] = &[
            // decimal hours
            ("1.5", 90),
            ("1,5", 90),
            (" 1.25 ", 75),
            ("0.5", 30),
            (".5", 30),
            (",25", 15),
            ("2.", 120),
            ("0.25", 15),
            ("0.75", 45),
            ("1.0", 60),
            ("24", 1440),
            ("24.0", 1440),
            ("0", 0),
            ("0.0", 0),
            // bare whole numbers
            ("1", 60),
            ("8", 480),
            ("23", 1380),
            ("25", 25),
            ("45", 45),
            ("90", 90),
            ("1440", 1440),
            // colon
            ("1:30", 90),
            ("0:45", 45),
            ("00:05", 5),
            ("12:00", 720),
            ("24:00", 1440),
            (" 1 : 30 ", 90),
            // minutes
            ("90m", 90),
            ("90 m", 90),
            ("90min", 90),
            ("90 mins", 90),
            ("5 minutes", 5),
            ("1 minute", 1),
            ("0m", 0),
            ("1440m", 1440),
            ("90M", 90),
            // hours with unit
            ("2h", 120),
            ("2 h", 120),
            ("2H", 120),
            ("2hr", 120),
            ("2 hrs", 120),
            ("1 hour", 60),
            ("2 hours", 120),
            ("1.5h", 90),
            ("1,5 h", 90),
            ("0.25 hours", 15),
            // hours and minutes
            ("1h 30m", 90),
            ("1h30m", 90),
            ("1h30", 90),
            ("1h 30", 90),
            ("1 h 30 m", 90),
            ("1 hour 30 minutes", 90),
            ("1hr 5min", 65),
            ("0h 45m", 45),
            ("23h 59m", 1439),
            ("24h 0m", 1440),
            // tabs and other whitespace
            ("\t1h\u{a0}30m\n", 90),
        ];
        for &(input, expected) in cases {
            assert_eq!(parse_hours(input), Ok(Minutes(expected)), "input {input:?}");
        }
    }

    #[test]
    fn rounds_decimals_to_nearest_minute() {
        let cases: &[(&str, u32)] = &[
            ("0.01", 1),     // 0.6 min
            ("0.008", 0),    // 0.48 min
            ("0.0083", 0),   // 0.498 min
            ("0.00834", 1),  // 0.5004 min
            ("1.333", 80),   // 79.98 min
            ("1.3333", 80),  // 79.998 min
            ("0.1", 6),      // exact
            ("0.33", 20),    // 19.8 min
            ("0.17", 10),    // 10.2 min
            ("2.999", 180),  // 179.94 min
            ("0.125", 8),    // 7.5 min, half rounds up
            ("0.041667", 3), // 2.50002 min
            ("1.333333", 80),
        ];
        for &(input, expected) in cases {
            assert_eq!(parse_hours(input), Ok(Minutes(expected)), "input {input:?}");
        }
    }

    #[test]
    fn rejects_empty() {
        for input in ["", " ", "\t\n"] {
            assert_eq!(parse_hours(input), Err(CoreError::HoursEmpty), "{input:?}");
        }
    }

    #[test]
    fn rejects_negative() {
        for input in ["-1", " -1.5", "-90m", "-1:30", "\u{2212}2"] {
            assert_eq!(
                parse_hours(input),
                Err(CoreError::HoursNegative),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_more_than_a_day() {
        for input in [
            "24.01",
            "25h",
            "1441m",
            "1441",
            "24:01",
            "24h 1m",
            "24.5",
            "999999999",
        ] {
            assert_eq!(
                parse_hours(input),
                Err(CoreError::HoursTooLarge),
                "{input:?}"
            );
        }
    }

    #[test]
    fn rejects_minutes_part_of_60_or_more() {
        assert_eq!(
            parse_hours("1:75"),
            Err(CoreError::HoursMinutesPartTooBig { minutes: 75 })
        );
        assert_eq!(
            parse_hours("1h 60m"),
            Err(CoreError::HoursMinutesPartTooBig { minutes: 60 })
        );
        assert_eq!(
            parse_hours("1h90"),
            Err(CoreError::HoursMinutesPartTooBig { minutes: 90 })
        );
    }

    #[test]
    fn rejects_nonsense() {
        for input in [
            "abc",
            "1.5.2",
            "1,5,2",
            "1.,5",
            ".",
            ",",
            "h",
            "m",
            "1x",
            "1 day",
            "1:3",
            "1:300",
            ":30",
            "1:",
            "1::30",
            "1.5:30",
            "1:30.5",
            "1.5m",
            "1.5h 30m",
            "1h 30.5m",
            "30m 1h",
            "1h 2h",
            "1m 30",
            "1 2",
            "1h 30m 10",
            "+1",
            "1e2",
            "0x10",
            "1/2",
            "½",
            "١",
            "1.1234567",
            "1234567890",
            "1h30mm",
            "one hour",
            "1 h r",
        ] {
            assert!(
                matches!(parse_hours(input), Err(CoreError::HoursInvalid { .. })),
                "{input:?} gave {:?}",
                parse_hours(input)
            );
        }
    }

    #[test]
    fn invalid_message_quotes_short_input() {
        let err = parse_hours(&"x".repeat(200)).unwrap_err();
        let message = err.to_string();
        assert!(message.len() < 150, "{message}");
    }

    #[test]
    fn format_hours_table() {
        let cases: &[(u32, &str)] = &[
            (0, "0"),
            (1, "0.02"),
            (10, "0.17"),
            (15, "0.25"),
            (20, "0.33"),
            (30, "0.5"),
            (40, "0.67"),
            (45, "0.75"),
            (60, "1"),
            (75, "1.25"),
            (90, "1.5"),
            (100, "1.67"),
            (480, "8"),
            (1440, "24"),
            (6000, "100"),
        ];
        for &(minutes, expected) in cases {
            assert_eq!(format_hours(Minutes(minutes)), expected, "{minutes} min");
        }
    }

    #[test]
    fn format_hours_round_trips_quarter_hours() {
        for minutes in (0..=1440).step_by(15) {
            let text = format_hours(Minutes(minutes));
            assert_eq!(parse_hours(&format!("{text}h")), Ok(Minutes(minutes)));
        }
    }

    #[test]
    fn format_hours_minutes_table() {
        assert_eq!(format_hours_minutes(Minutes(0)), "0m");
        assert_eq!(format_hours_minutes(Minutes(45)), "45m");
        assert_eq!(format_hours_minutes(Minutes(60)), "1h");
        assert_eq!(format_hours_minutes(Minutes(90)), "1h 30m");
        assert_eq!(format_hours_minutes(Minutes(1439)), "23h 59m");
    }

    #[test]
    fn format_hours_minutes_round_trips() {
        for minutes in 0..=1440 {
            let text = format_hours_minutes(Minutes(minutes));
            assert_eq!(parse_hours(&text), Ok(Minutes(minutes)), "{text}");
        }
    }
}
