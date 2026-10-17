//! The one error type returned by core validation and parsing.
//!
//! Messages are written for the person using the app: plain words, no jargon.
//! Nothing in core panics on user input; it returns one of these instead.

use thiserror::Error;

/// Something the user typed or chose cannot be accepted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// The hours box was left empty.
    #[error("Enter how long you worked, for example 1.5 or 1h 30m.")]
    HoursEmpty,

    /// The hours started with a minus sign.
    #[error("Hours can't be negative.")]
    HoursNegative,

    /// The hours text could not be understood.
    #[error("\"{input}\" doesn't look like a time. Try 1.5, 1:30, 90m or 1h 30m.")]
    HoursInvalid {
        /// What the user typed (trimmed, shortened if very long).
        input: String,
    },

    /// The minutes part of `1:75` or `1h 75m` was 60 or more.
    #[error("The minutes part ({minutes}) must be under 60.")]
    HoursMinutesPartTooBig {
        /// The minutes part as typed.
        minutes: u64,
    },

    /// One entry can't be longer than a day.
    #[error("One entry can't be more than 24 hours.")]
    HoursTooLarge,

    /// A required text field was empty.
    #[error("{field} can't be empty.")]
    Required {
        /// The field name as shown to the user, e.g. "Name".
        field: &'static str,
    },

    /// A text field was longer than allowed.
    #[error("{field} is too long (at most {max} characters).")]
    TooLong {
        /// The field name as shown to the user.
        field: &'static str,
        /// The maximum number of characters.
        max: usize,
    },

    /// A list had more items than allowed.
    #[error("Keep {field} to at most {max} items.")]
    TooManyItems {
        /// The list's name as shown to the user.
        field: &'static str,
        /// The most items allowed.
        max: usize,
    },

    /// A number was outside the allowed range.
    #[error("{field} must be between {min} and {max}.")]
    OutOfRange {
        /// The field name as shown to the user.
        field: &'static str,
        /// Smallest allowed value.
        min: i64,
        /// Largest allowed value.
        max: i64,
    },

    /// A date was not a real calendar date in `YYYY-MM-DD` form.
    #[error("\"{input}\" isn't a valid date. Use YYYY-MM-DD, for example 2026-01-31.")]
    DateInvalid {
        /// What was given.
        input: String,
    },

    /// A colour was not `#rrggbb`.
    #[error("\"{input}\" isn't a colour. Use a code like #2563eb.")]
    ColorInvalid {
        /// What was given.
        input: String,
    },

    /// A frequency of "every 0 months and 0 days" was entered.
    #[error("A repeat needs at least one month or one day.")]
    FrequencyEmpty,

    /// One pasted ticket was too long.
    #[error("Ticket \"{preview}\" is too long (at most {max} characters).")]
    TicketTooLong {
        /// The start of the ticket text.
        preview: String,
        /// The maximum number of characters.
        max: usize,
    },

    /// A ticket contained invisible control characters.
    #[error("Ticket \"{preview}\" contains characters that can't be used.")]
    TicketInvalid {
        /// The start of the ticket text, control characters removed.
        preview: String,
    },

    /// Too many tickets for one entry.
    #[error("That's {count} tickets; one entry can hold at most {max}.")]
    TooManyTickets {
        /// How many tickets there would be.
        count: usize,
        /// The maximum per entry.
        max: usize,
    },

    /// A new master password was too short.
    #[error("Use at least {min} characters for your password.")]
    PasswordTooShort {
        /// The minimum number of characters.
        min: usize,
    },

    /// A new master password was too long.
    #[error("That password is too long (at most {max} characters).")]
    PasswordTooLong {
        /// The maximum number of characters.
        max: usize,
    },

    /// A setting name isn't known, or its value isn't one of the allowed ones.
    #[error("That setting can't be set to \"{value}\".")]
    SettingInvalid {
        /// The value given (shortened).
        value: String,
    },

    /// A value isn't one of a choice field's options.
    #[error("\"{value}\" isn't one of the choices for this field.")]
    NotAChoice {
        /// The value given (shortened).
        value: String,
    },

    /// A number field got something that isn't a number.
    #[error("\"{input}\" isn't a number.")]
    NumberInvalid {
        /// What was given (shortened).
        input: String,
    },

    /// A link template must start with http:// or https:// and contain {ticket}.
    #[error("A link needs to start with https:// and contain {{ticket}} where the ticket goes.")]
    LinkTemplateInvalid,

    /// Log work needs a ticket or a few words.
    #[error("Paste a ticket or type what you did first.")]
    NothingToLog,

    /// A text value was not a valid id.
    #[error("\"{input}\" isn't a valid id.")]
    IdInvalid {
        /// What was given.
        input: String,
    },
}

/// Shorten user text for an error message without splitting a character.
pub(crate) fn preview(text: &str, max_chars: usize) -> String {
    let cleaned: String = text.chars().filter(|c| !c.is_control()).collect();
    if cleaned.chars().count() <= max_chars {
        cleaned
    } else {
        let mut out: String = cleaned.chars().take(max_chars).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_shortens_long_text() {
        assert_eq!(preview("abcdef", 3), "abc…");
        assert_eq!(preview("abc", 3), "abc");
        assert_eq!(preview("a\u{7}b", 3), "ab");
        assert_eq!(preview("ééééé", 2), "éé…");
    }

    #[test]
    fn messages_are_plain() {
        assert_eq!(
            CoreError::Required { field: "Name" }.to_string(),
            "Name can't be empty."
        );
        assert_eq!(
            CoreError::TooManyTickets {
                count: 101,
                max: 100
            }
            .to_string(),
            "That's 101 tickets; one entry can hold at most 100."
        );
    }
}
