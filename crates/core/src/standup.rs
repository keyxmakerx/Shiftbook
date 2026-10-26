//! "Copy for stand-up" (round 15): what you did on your last day with work, then what you
//! start with today, as plain lines to paste into a chat or read out.

/// Most tickets named after one line; the rest become "+N".
pub const MAX_TICKETS_PER_LINE: usize = 3;
/// Most lines under "today".
pub const MAX_NEXT_LINES: usize = 3;
/// Default headings.
pub const DEFAULT_WORDS: (&str, &str) = ("Yesterday", "Today");

/// One thing done: an entry's title and its tickets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoneLine {
    /// What was done.
    pub title: String,
    /// Its ticket numbers, in order.
    pub tickets: Vec<String>,
}

/// The stand-up text. Entries with the same title (any case) become one line with their
/// tickets merged; tickets closed in the tray become a count; "today" lists the first few
/// things to start with and is left out when there are none.
pub fn standup_text(
    words: (&str, &str),
    done: &[DoneLine],
    tickets_closed: usize,
    next: &[String],
) -> String {
    let mut merged: Vec<DoneLine> = Vec::new();
    for line in done {
        let title = line.title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() {
            continue;
        }
        match merged
            .iter_mut()
            .find(|m| m.title.to_lowercase() == title.to_lowercase())
        {
            Some(m) => {
                for t in &line.tickets {
                    if !m.tickets.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                        m.tickets.push(t.clone());
                    }
                }
            }
            None => merged.push(DoneLine {
                title,
                tickets: line.tickets.clone(),
            }),
        }
    }

    let mut out = vec![words.0.to_string()];
    for m in &merged {
        if m.tickets.is_empty() {
            out.push(format!("- {}", m.title));
        } else {
            let shown = m.tickets[..m.tickets.len().min(MAX_TICKETS_PER_LINE)].join(", ");
            let more = m.tickets.len().saturating_sub(MAX_TICKETS_PER_LINE);
            let more = if more > 0 {
                format!(", +{more}")
            } else {
                String::new()
            };
            out.push(format!("- {} ({shown}{more})", m.title));
        }
    }
    match tickets_closed {
        0 => {}
        1 => out.push("- 1 ticket closed".into()),
        n => out.push(format!("- {n} tickets closed")),
    }
    if merged.is_empty() && tickets_closed == 0 {
        out.push("- Nothing logged".into());
    }
    let next: Vec<&String> = next
        .iter()
        .filter(|n| !n.trim().is_empty())
        .take(MAX_NEXT_LINES)
        .collect();
    if !next.is_empty() {
        out.push(words.1.to_string());
        out.extend(next.into_iter().map(|n| format!("- {}", n.trim())));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(title: &str, tickets: &[&str]) -> DoneLine {
        DoneLine {
            title: title.into(),
            tickets: tickets.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn yesterday_then_today() {
        let text = standup_text(
            DEFAULT_WORDS,
            &[
                line("Switch replacement, floor 2", &["INC0045678"]),
                line("Laptop imaging", &[]),
            ],
            6,
            &["Finish the printer ticket".into()],
        );
        assert_eq!(
            text,
            "Yesterday\n- Switch replacement, floor 2 (INC0045678)\n- Laptop imaging\n\
             - 6 tickets closed\nToday\n- Finish the printer ticket"
        );
    }

    #[test]
    fn same_work_is_one_line_and_long_ticket_lists_are_cut() {
        let text = standup_text(
            ("Done", "Next"),
            &[
                line("Patching", &["A1", "A2"]),
                line("  patching ", &["a2", "A3", "A4"]),
            ],
            1,
            &[],
        );
        assert_eq!(text, "Done\n- Patching (A1, A2, A3, +1)\n- 1 ticket closed");
    }

    #[test]
    fn nothing_logged_and_at_most_three_next() {
        let next: Vec<String> = ["a", " ", "b", "c", "d"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            standup_text(DEFAULT_WORDS, &[], 0, &next),
            "Yesterday\n- Nothing logged\nToday\n- a\n- b\n- c"
        );
    }
}
