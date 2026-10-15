//! Splitting pasted ticket text into separate tickets (SPEC §2.3).
//!
//! Separators: new lines, commas, semicolons, tabs and any run of spaces.
//! Each ticket is trimmed, empty pieces are dropped, and repeats are removed
//! keeping the first one (repeats are matched ignoring letter case, so
//! `INC1` and `inc1` count as the same ticket).

use crate::error::{preview, CoreError};

/// Longest single ticket, in characters.
pub const MAX_TICKET_CHARS: usize = 64;
/// Most tickets on one entry.
pub const MAX_TICKETS_PER_ENTRY: usize = 100;

fn is_separator(c: char) -> bool {
    c.is_whitespace() || c == ',' || c == ';'
}

/// Check one ticket (already trimmed and non-empty).
fn check_ticket(ticket: &str) -> Result<(), CoreError> {
    if ticket.chars().any(char::is_control) {
        return Err(CoreError::TicketInvalid {
            preview: preview(ticket, 20),
        });
    }
    if ticket.chars().count() > MAX_TICKET_CHARS {
        return Err(CoreError::TicketTooLong {
            preview: preview(ticket, 20),
            max: MAX_TICKET_CHARS,
        });
    }
    Ok(())
}

/// Split pasted text into unique tickets, in the order they appear.
/// Errors if any ticket is too long or the result has more than
/// [`MAX_TICKETS_PER_ENTRY`] tickets. Empty input gives an empty list.
pub fn split_tickets(text: &str) -> Result<Vec<String>, CoreError> {
    add_tickets::<&str>(&[], text)
}

/// The tickets in `pasted` that are not already in `existing`, checking that
/// the entry would not go over [`MAX_TICKETS_PER_ENTRY`] in total.
pub fn add_tickets<S: AsRef<str>>(existing: &[S], pasted: &str) -> Result<Vec<String>, CoreError> {
    let mut seen: Vec<String> = existing
        .iter()
        .map(|t| t.as_ref().trim().to_lowercase())
        .collect();
    let mut added = Vec::new();
    for piece in pasted.split(is_separator) {
        let ticket = piece.trim();
        if ticket.is_empty() {
            continue;
        }
        check_ticket(ticket)?;
        let key = ticket.to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        added.push(ticket.to_string());
    }
    let count = existing.len() + added.len();
    if count > MAX_TICKETS_PER_ENTRY {
        return Err(CoreError::TooManyTickets {
            count,
            max: MAX_TICKETS_PER_ENTRY,
        });
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_every_separator() {
        let cases: &[(&str, &[&str])] = &[
            ("", &[]),
            ("   ", &[]),
            (",;\n\t", &[]),
            ("INC1", &["INC1"]),
            ("  INC1  ", &["INC1"]),
            ("INC1\nINC2", &["INC1", "INC2"]),
            ("INC1\r\nINC2\r\n", &["INC1", "INC2"]),
            ("INC1,INC2", &["INC1", "INC2"]),
            ("INC1, INC2 ,INC3", &["INC1", "INC2", "INC3"]),
            ("INC1;INC2", &["INC1", "INC2"]),
            ("INC1\tINC2", &["INC1", "INC2"]),
            ("INC1    INC2", &["INC1", "INC2"]),
            ("INC1,,;;\n\nINC2", &["INC1", "INC2"]),
            (
                "CHG-100\nREQ 200; TASK_3,\tINC#4",
                &["CHG-100", "REQ", "200", "TASK_3", "INC#4"],
            ),
            ("INC1\u{a0}INC2", &["INC1", "INC2"]),
        ];
        for &(input, expected) in cases {
            assert_eq!(
                split_tickets(input),
                Ok(expected.iter().map(|s| s.to_string()).collect()),
                "{input:?}"
            );
        }
    }

    #[test]
    fn removes_repeats_keeping_order() {
        assert_eq!(
            split_tickets("B2, A1, B2, C3, A1, b2"),
            Ok(vec!["B2".into(), "A1".into(), "C3".into()])
        );
    }

    #[test]
    fn caps_ticket_length() {
        let ok = "X".repeat(MAX_TICKET_CHARS);
        assert_eq!(split_tickets(&ok), Ok(vec![ok.clone()]));
        let multibyte = "é".repeat(MAX_TICKET_CHARS);
        assert_eq!(split_tickets(&multibyte), Ok(vec![multibyte.clone()]));
        let long = "X".repeat(MAX_TICKET_CHARS + 1);
        assert!(matches!(
            split_tickets(&format!("INC1 {long}")),
            Err(CoreError::TicketTooLong { max: 64, .. })
        ));
    }

    #[test]
    fn caps_ticket_count() {
        let hundred: Vec<String> = (1..=100).map(|n| format!("T{n}")).collect();
        assert_eq!(split_tickets(&hundred.join("\n")).map(|v| v.len()), Ok(100));
        let with_repeats = format!("{}\nT1\nT2", hundred.join(","));
        assert_eq!(split_tickets(&with_repeats).map(|v| v.len()), Ok(100));
        let too_many: Vec<String> = (1..=101).map(|n| format!("T{n}")).collect();
        assert_eq!(
            split_tickets(&too_many.join(" ")),
            Err(CoreError::TooManyTickets {
                count: 101,
                max: 100
            })
        );
    }

    #[test]
    fn rejects_control_characters() {
        assert!(matches!(
            split_tickets("INC\u{7}1"),
            Err(CoreError::TicketInvalid { .. })
        ));
        assert!(matches!(
            split_tickets("INC\u{0}1"),
            Err(CoreError::TicketInvalid { .. })
        ));
    }

    #[test]
    fn adds_only_new_tickets_to_an_entry() {
        let existing = vec!["INC1".to_string(), "INC2".to_string()];
        assert_eq!(
            add_tickets(&existing, "inc2, INC3\nINC1 INC4"),
            Ok(vec!["INC3".into(), "INC4".into()])
        );
        assert_eq!(add_tickets(&existing, ""), Ok(vec![]));
        let full: Vec<String> = (1..=99).map(|n| format!("T{n}")).collect();
        assert_eq!(add_tickets(&full, "NEW").map(|v| v.len()), Ok(1));
        assert_eq!(
            add_tickets(&full, "NEW1 NEW2"),
            Err(CoreError::TooManyTickets {
                count: 101,
                max: 100
            })
        );
        assert_eq!(add_tickets(&full, "T1 T2"), Ok(vec![]));
    }
}
