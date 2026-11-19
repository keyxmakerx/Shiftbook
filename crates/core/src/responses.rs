//! Saved responses: short messages kept for pasting into tickets. The Done tickets tray has a
//! button that lists them, and picking one copies it. Nothing is matched to a ticket; the user
//! picks.
//!
//! Round 21: a response can hold blanks. `{ticket}` and `{date}` fill themselves in when the
//! response is copied for a ticket (ticking one done offers "Copy a response"), and `{name}`
//! takes a name the user is asked for.

use time::Date;

use crate::error::CoreError;
use crate::validate::validate_required;

/// Most saved responses.
pub const MAX_RESPONSES: usize = 30;
/// Longest name shown in the menu.
pub const MAX_RESPONSE_NAME_CHARS: usize = 60;
/// Longest message.
pub const MAX_RESPONSE_TEXT_CHARS: usize = 4_000;

/// One response as typed: a short name for the menu and the message that gets copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseDraft {
    /// Name shown in the menu (blank means "use the start of the message").
    pub name: String,
    /// The message copied to the clipboard.
    pub text: String,
}

/// The name to use when none was typed: the message's first line, cut to fit.
fn name_from_text(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    if first.chars().count() <= MAX_RESPONSE_NAME_CHARS {
        first.to_string()
    } else {
        let cut: String = first.chars().take(MAX_RESPONSE_NAME_CHARS - 1).collect();
        format!("{}…", cut.trim_end())
    }
}

/// Clean a typed list: drop rows with no message, name the unnamed ones from their message,
/// and check lengths and the count. The message keeps its line breaks; only the ends are
/// trimmed.
pub fn clean_responses(drafts: &[ResponseDraft]) -> Result<Vec<ResponseDraft>, CoreError> {
    let items = drafts
        .iter()
        .filter(|d| !d.text.trim().is_empty())
        .map(|d| {
            let text = validate_required("A response", &d.text, MAX_RESPONSE_TEXT_CHARS)?;
            let name = if d.name.trim().is_empty() {
                name_from_text(&text)
            } else {
                validate_required("A response name", &d.name, MAX_RESPONSE_NAME_CHARS)?
            };
            Ok(ResponseDraft { name, text })
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    if items.len() > MAX_RESPONSES {
        return Err(CoreError::TooManyItems {
            field: "saved responses",
            max: MAX_RESPONSES,
        });
    }
    Ok(items)
}

/// A day as a response says it: "Wed 18 Nov".
fn response_day(date: Date) -> String {
    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let day = DAYS[usize::from(date.weekday().number_days_from_monday())];
    let month = MONTHS[usize::from(u8::from(date.month()) - 1)];
    format!("{day} {} {month}", date.day())
}

/// True when the message has a `{name}` blank, so the user is asked for a name first.
pub fn asks_name(text: &str) -> bool {
    text.to_lowercase().contains("{name}")
}

/// `text` with its blanks filled in: `{ticket}` with the ticket number, `{date}` with today
/// ("Wed 18 Nov") and `{name}` with the name given. Capitals in a blank don't matter. A blank
/// with nothing to put in it (no ticket, no name) stays as it was typed, and so does
/// anything else in braces.
pub fn fill_response(text: &str, ticket: Option<&str>, today: Date, name: Option<&str>) -> String {
    let ticket = ticket.map(str::trim).filter(|t| !t.is_empty());
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let Some(close) = after.find('}') else {
            rest = after;
            break;
        };
        let blank = &after[..=close];
        let filled = match blank[1..close].to_lowercase().as_str() {
            "ticket" => ticket.map(str::to_string),
            "date" => Some(response_day(today)),
            "name" => name.map(str::to_string),
            _ => None,
        };
        match filled {
            Some(value) => {
                out.push_str(&value);
                rest = &after[close + 1..];
            }
            // Not a blank we know: keep the brace and look again after it.
            None => {
                out.push('{');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn fills_the_ticket_the_date_and_the_name() {
        let text = "Hi {name}, {ticket} is fixed. Confirmed on {date}. Closing {TICKET}.";
        assert_eq!(
            fill_response(
                text,
                Some("INC0041240"),
                date!(2026 - 11 - 18),
                Some(" Sam ")
            ),
            "Hi Sam, INC0041240 is fixed. Confirmed on Wed 18 Nov. Closing INC0041240."
        );
    }

    #[test]
    fn leaves_a_blank_with_nothing_to_fill_it() {
        let text = "Hi {name}, {ticket} is fixed on {date}. See {runbook} and {.";
        assert_eq!(
            fill_response(text, None, date!(2026 - 11 - 2), Some("  ")),
            "Hi {name}, {ticket} is fixed on Mon 2 Nov. See {runbook} and {."
        );
        assert_eq!(
            fill_response("No blanks here.", Some("X-1"), date!(2026 - 11 - 2), None),
            "No blanks here."
        );
    }

    #[test]
    fn knows_when_to_ask_for_a_name() {
        assert!(asks_name("Hi {Name},"));
        assert!(!asks_name("Hi there, {ticket} is done."));
    }

    fn d(name: &str, text: &str) -> ResponseDraft {
        ResponseDraft {
            name: name.into(),
            text: text.into(),
        }
    }

    #[test]
    fn drops_empty_rows_and_trims() {
        let out = clean_responses(&[
            d("  No reply ", "  No reply after 3 tries. Closing.\n"),
            d("Blank", "   "),
            d("", ""),
        ])
        .unwrap();
        assert_eq!(out, vec![d("No reply", "No reply after 3 tries. Closing.")]);
    }

    #[test]
    fn keeps_line_breaks_inside_the_message() {
        let out = clean_responses(&[d("Fixed", "Fixed.\n\nConfirmed with the user.")]).unwrap();
        assert_eq!(out[0].text, "Fixed.\n\nConfirmed with the user.");
    }

    #[test]
    fn names_unnamed_responses_from_the_first_line() {
        let out =
            clean_responses(&[d(" ", "Duplicate ticket, closing.\nSee the other one.")]).unwrap();
        assert_eq!(out[0].name, "Duplicate ticket, closing.");
        let long = "x".repeat(MAX_RESPONSE_NAME_CHARS + 20);
        let out = clean_responses(&[d("", &long)]).unwrap();
        assert_eq!(out[0].name.chars().count(), MAX_RESPONSE_NAME_CHARS);
        assert!(out[0].name.ends_with('…'));
    }

    #[test]
    fn limits_lengths_and_count() {
        let long_name = "n".repeat(MAX_RESPONSE_NAME_CHARS + 1);
        assert!(matches!(
            clean_responses(&[d(&long_name, "ok")]),
            Err(CoreError::TooLong { .. })
        ));
        let long_text = "t".repeat(MAX_RESPONSE_TEXT_CHARS + 1);
        assert!(matches!(
            clean_responses(&[d("ok", &long_text)]),
            Err(CoreError::TooLong { .. })
        ));
        let many: Vec<_> = (0..=MAX_RESPONSES)
            .map(|i| d("", &format!("r{i}")))
            .collect();
        assert!(matches!(
            clean_responses(&many),
            Err(CoreError::TooManyItems {
                max: MAX_RESPONSES,
                ..
            })
        ));
        assert!(clean_responses(&many[..MAX_RESPONSES]).is_ok());
    }
}
