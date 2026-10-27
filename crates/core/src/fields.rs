//! Custom fields the user adds to entries, tickets or projects.
//!
//! Kinds: text, number, choice (from a list of options) and link. A field can
//! have a **link template** such as `https://example.atlassian.net/browse/{ticket}`
//! and a **ticket prefix** such as `PROJ-`: tickets starting with the prefix
//! (ignoring case) get a "copy link" option. Links are only shown and copied,
//! never opened (the app makes no network requests).

use serde::{Deserialize, Serialize};

use crate::error::{preview, CoreError};
use crate::validate::{validate_required, MAX_LINE_CHARS};

/// Longest field name.
pub const MAX_FIELD_NAME_CHARS: usize = 60;
/// Most options in a choice field.
pub const MAX_OPTIONS: usize = 50;
/// Longest single option.
pub const MAX_OPTION_CHARS: usize = 60;
/// Longest link template.
pub const MAX_TEMPLATE_CHARS: usize = 500;
/// Longest ticket prefix.
pub const MAX_PREFIX_CHARS: usize = 20;
/// Where the ticket goes in a link template.
pub const TICKET_PLACEHOLDER: &str = "{ticket}";

/// What kind of value a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// Free text.
    Text,
    /// A number (whole or decimal).
    Number,
    /// One of a list of options.
    Choice,
    /// A link built from a template, e.g. a Jira issue.
    Link,
}

impl FieldKind {
    /// The text stored in `custom_fields.kind`.
    pub const fn as_str(self) -> &'static str {
        match self {
            FieldKind::Text => "text",
            FieldKind::Number => "number",
            FieldKind::Choice => "choice",
            FieldKind::Link => "link",
        }
    }

    /// Parse the stored text.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "text" => FieldKind::Text,
            "number" => FieldKind::Number,
            "choice" => FieldKind::Choice,
            "link" => FieldKind::Link,
            _ => return None,
        })
    }
}

/// Which records a field appears on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum AppliesTo {
    /// Time entries.
    Entry,
    /// Tickets.
    Ticket,
    /// Projects.
    Project,
}

impl AppliesTo {
    /// The text stored in `custom_fields.applies_to`.
    pub const fn as_str(self) -> &'static str {
        match self {
            AppliesTo::Entry => "entry",
            AppliesTo::Ticket => "ticket",
            AppliesTo::Project => "project",
        }
    }

    /// Parse the stored text.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "entry" => AppliesTo::Entry,
            "ticket" => AppliesTo::Ticket,
            "project" => AppliesTo::Project,
            _ => return None,
        })
    }
}

/// A field definition after checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanField {
    /// Trimmed name.
    pub name: String,
    /// Options, one per line (choice fields only; empty otherwise).
    pub options: Vec<String>,
    /// Link template (`None` when blank).
    pub link_template: Option<String>,
    /// Ticket prefix (`None` when blank).
    pub ticket_prefix: Option<String>,
}

/// Split option text on new lines and commas, trimmed, without blanks or repeats.
pub fn parse_options(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for piece in text.split(['\n', ',']) {
        let p = piece.trim();
        if !p.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(p)) {
            out.push(p.to_string());
        }
    }
    out
}

/// Store options as one per line.
pub fn join_options(options: &[String]) -> String {
    options.join("\n")
}

fn check_template(template: &str) -> Result<(), CoreError> {
    let lower = template.to_ascii_lowercase();
    let scheme_ok = lower.starts_with("https://") || lower.starts_with("http://");
    if !scheme_ok
        || !template.contains(TICKET_PLACEHOLDER)
        || template.chars().count() > MAX_TEMPLATE_CHARS
        || template
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(CoreError::LinkTemplateInvalid);
    }
    Ok(())
}

/// Check a field definition. Choice fields need at least one option; a link
/// template (any kind may have one) must be `http(s)://...{ticket}...`.
pub fn validate_field(
    name: &str,
    kind: FieldKind,
    options: &str,
    link_template: &str,
    ticket_prefix: &str,
) -> Result<CleanField, CoreError> {
    let name = validate_required("Name", name, MAX_FIELD_NAME_CHARS)?;
    let options = if kind == FieldKind::Choice {
        let list = parse_options(options);
        if list.is_empty() {
            return Err(CoreError::Required { field: "Choices" });
        }
        if list.len() > MAX_OPTIONS {
            return Err(CoreError::OutOfRange {
                field: "Number of choices",
                min: 1,
                max: MAX_OPTIONS as i64,
            });
        }
        if list.iter().any(|o| o.chars().count() > MAX_OPTION_CHARS) {
            return Err(CoreError::TooLong {
                field: "A choice",
                max: MAX_OPTION_CHARS,
            });
        }
        list
    } else {
        Vec::new()
    };
    let template = link_template.trim();
    let link_template = if template.is_empty() {
        None
    } else {
        check_template(template)?;
        Some(template.to_string())
    };
    let prefix = ticket_prefix.trim();
    let ticket_prefix = if prefix.is_empty() {
        None
    } else {
        if prefix.chars().count() > MAX_PREFIX_CHARS || prefix.chars().any(char::is_whitespace) {
            return Err(CoreError::TooLong {
                field: "Ticket prefix",
                max: MAX_PREFIX_CHARS,
            });
        }
        Some(prefix.to_string())
    };
    Ok(CleanField {
        name,
        options,
        link_template,
        ticket_prefix,
    })
}

/// Check a value for a field; returns the value to store (trimmed; a choice is
/// returned spelled as the option). Blank is always allowed (it clears the value).
pub fn validate_value(
    kind: FieldKind,
    options: &[String],
    value: &str,
) -> Result<String, CoreError> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(String::new());
    }
    if v.chars().count() > MAX_LINE_CHARS {
        return Err(CoreError::TooLong {
            field: "Value",
            max: MAX_LINE_CHARS,
        });
    }
    match kind {
        FieldKind::Text | FieldKind::Link => Ok(v.to_string()),
        FieldKind::Number => {
            let normal = v.replace(',', ".");
            let body = normal.strip_prefix('-').unwrap_or(&normal);
            let ok = !body.is_empty()
                && body.chars().filter(|c| *c == '.').count() <= 1
                && body.chars().all(|c| c.is_ascii_digit() || c == '.')
                && body.chars().any(|c| c.is_ascii_digit());
            if ok {
                Ok(normal)
            } else {
                Err(CoreError::NumberInvalid {
                    input: preview(v, 20),
                })
            }
        }
        FieldKind::Choice => options
            .iter()
            .find(|o| o.eq_ignore_ascii_case(v))
            .cloned()
            .ok_or_else(|| CoreError::NotAChoice {
                value: preview(v, 30),
            }),
    }
}

/// True when `ticket` starts with `prefix`, ignoring case. A blank prefix matches nothing.
pub fn ticket_matches_prefix(prefix: &str, ticket: &str) -> bool {
    let prefix = prefix.trim();
    !prefix.is_empty()
        && ticket
            .trim()
            .to_lowercase()
            .starts_with(&prefix.to_lowercase())
}

fn encode_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The link for `value` (a ticket or a field value): the template with
/// `{ticket}` replaced by the value, percent-encoded.
pub fn build_link(template: &str, value: &str) -> String {
    template.replace(TICKET_PLACEHOLDER, &encode_component(value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip() {
        for k in [
            FieldKind::Text,
            FieldKind::Number,
            FieldKind::Choice,
            FieldKind::Link,
        ] {
            assert_eq!(FieldKind::parse(k.as_str()), Some(k));
        }
        for a in [AppliesTo::Entry, AppliesTo::Ticket, AppliesTo::Project] {
            assert_eq!(AppliesTo::parse(a.as_str()), Some(a));
        }
        assert_eq!(FieldKind::parse("Text"), None);
        assert_eq!(AppliesTo::parse("task"), None);
    }

    #[test]
    fn options_are_split_and_cleaned() {
        assert_eq!(
            parse_options("Low\nMedium, High\n\nlow , "),
            vec!["Low", "Medium", "High"]
        );
        assert_eq!(join_options(&["a".into(), "b".into()]), "a\nb");
    }

    #[test]
    fn field_rules() {
        let f = validate_field(
            " Jira ",
            FieldKind::Link,
            "ignored",
            " https://example.atlassian.net/browse/{ticket} ",
            " PROJ- ",
        )
        .unwrap();
        assert_eq!(f.name, "Jira");
        assert!(f.options.is_empty());
        assert_eq!(
            f.link_template.as_deref(),
            Some("https://example.atlassian.net/browse/{ticket}")
        );
        assert_eq!(f.ticket_prefix.as_deref(), Some("PROJ-"));

        assert_eq!(
            validate_field("", FieldKind::Text, "", "", ""),
            Err(CoreError::Required { field: "Name" })
        );
        assert_eq!(
            validate_field("Size", FieldKind::Choice, " , ", "", ""),
            Err(CoreError::Required { field: "Choices" })
        );
        let choice = validate_field("Size", FieldKind::Choice, "S\nM\nL", "", "").unwrap();
        assert_eq!(choice.options, vec!["S", "M", "L"]);
        assert_eq!(choice.link_template, None);
        for bad in [
            "javascript:alert(1){ticket}",
            "https://example.com/browse/",
            "ftp://example.com/{ticket}",
            "https://exa mple.com/{ticket}",
        ] {
            assert_eq!(
                validate_field("J", FieldKind::Link, "", bad, ""),
                Err(CoreError::LinkTemplateInvalid),
                "{bad}"
            );
        }
        assert!(validate_field("J", FieldKind::Link, "", "", "TOO LONG PREFIX").is_err());
        let many: String = (0..51).map(|i| format!("o{i}\n")).collect();
        assert!(validate_field("J", FieldKind::Choice, &many, "", "").is_err());
    }

    #[test]
    fn value_rules() {
        let opts = vec!["Low".to_string(), "High".to_string()];
        assert_eq!(
            validate_value(FieldKind::Choice, &opts, " high "),
            Ok("High".into())
        );
        assert!(matches!(
            validate_value(FieldKind::Choice, &opts, "Mid"),
            Err(CoreError::NotAChoice { .. })
        ));
        assert_eq!(
            validate_value(FieldKind::Number, &[], "1,5"),
            Ok("1.5".into())
        );
        assert_eq!(
            validate_value(FieldKind::Number, &[], "-3"),
            Ok("-3".into())
        );
        for bad in ["1.2.3", "abc", "-", ".", "1e5"] {
            assert!(
                matches!(
                    validate_value(FieldKind::Number, &[], bad),
                    Err(CoreError::NumberInvalid { .. })
                ),
                "{bad}"
            );
        }
        assert_eq!(
            validate_value(FieldKind::Text, &[], "  hi "),
            Ok("hi".into())
        );
        assert_eq!(
            validate_value(FieldKind::Number, &[], "  "),
            Ok(String::new())
        );
        assert!(validate_value(FieldKind::Text, &[], &"x".repeat(2001)).is_err());
    }

    #[test]
    fn prefixes_and_links() {
        assert!(ticket_matches_prefix("PROJ-", "proj-123"));
        assert!(!ticket_matches_prefix("PROJ-", "INC0045678"));
        assert!(!ticket_matches_prefix(" ", "PROJ-1"));
        assert_eq!(
            build_link(
                "https://example.atlassian.net/browse/{ticket}",
                " PROJ-123 "
            ),
            "https://example.atlassian.net/browse/PROJ-123"
        );
        assert_eq!(
            build_link("https://x.test/?q={ticket}&a={ticket}", "a b/é"),
            "https://x.test/?q=a%20b%2F%C3%A9&a=a%20b%2F%C3%A9"
        );
    }
}
