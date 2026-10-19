//! Saved responses: short messages kept for pasting into tickets. The Done tickets tray has a
//! button that lists them, and picking one copies it. Nothing is matched to a ticket; the user
//! picks.

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

#[cfg(test)]
mod tests {
    use super::*;

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
