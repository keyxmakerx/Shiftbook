//! One-line capture: "1.5h firewall cleanup CHG0012345 #network yesterday".
//!
//! The line is split on spaces and each piece is recognised, in this order:
//!
//! 1. **Hours** — the first run of one to four words that the Hours box accepts
//!    (`1.5`, `1:30`, `90m`, `2 hours`, `1h 30m`, ...). A plain whole number counts
//!    only as the **first** word and only up to 24 (`8 standup` is 8 hours, but
//!    `45 errors` and `room 12` are words). A plain decimal like `1.5` counts
//!    anywhere. Later times are kept as words.
//! 2. **Tickets** — letters then digits, like `INC0045678`, `CHG0012345` or
//!    `PROJ-123`: 2 to 10 letters, an optional dash, then 2 or more digits.
//!    Surrounding punctuation is ignored and `INC1,INC2` gives two tickets.
//!    Lower-case letters count only with 4 or more digits (`inc0045678` yes,
//!    `win10` no), so ordinary words are not mistaken for tickets. Tickets are
//!    upper-cased.
//! 3. **Project** — `#name`, matched against the given projects ignoring case:
//!    whole name, then name starts with it, then name without spaces starts with
//!    it, then any word of the name starts with it. An unknown `#name` is kept in
//!    the description and reported in `project_query`.
//! 4. **Day** — `today`, `yesterday`, or a weekday (`mon`/`monday` ... `sun`)
//!    meaning that day of the current week. The first day word wins.
//! 5. Everything else, in order, is the **description**.

use serde::Serialize;
use time::{Date, Weekday};

use crate::dates::{add_days, iso_date};
use crate::hours::{parse_hours, BARE_NUMBER_HOURS_LIMIT};
use crate::ids::ProjectId;
use crate::minutes::Minutes;
use crate::tickets::MAX_TICKET_CHARS;
use crate::week::{week_of, WeekStart};

/// Longest run of words tried as one time (`1 hour 30 minutes` is four).
const MAX_HOURS_WORDS: usize = 4;

/// A project the `#name` word can match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureProject<'a> {
    /// Its id.
    pub id: ProjectId,
    /// Its name as shown.
    pub name: &'a str,
}

/// What a capture line means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    /// How long, if a time was found.
    pub minutes: Option<Minutes>,
    /// The words that were read as the time, e.g. `1h 30m`.
    pub hours_text: Option<String>,
    /// Tickets found, upper-cased, without repeats.
    pub tickets: Vec<String>,
    /// The project a `#name` matched.
    pub project_id: Option<ProjectId>,
    /// The `#name` text (without `#`) when it matched no project.
    pub project_query: Option<String>,
    /// The day the work belongs to (today unless a day word was used).
    #[serde(with = "iso_date")]
    pub date: Date,
    /// The day word that set the date, as typed.
    pub day_word: Option<String>,
    /// Everything else.
    pub description: String,
}

/// Read a capture line. `today` is the user's local date; weekday words mean
/// that day in the week containing `today`.
pub fn parse_capture(
    text: &str,
    today: Date,
    week_start: WeekStart,
    projects: &[CaptureProject<'_>],
) -> Capture {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut used = vec![false; words.len()];
    let mut capture = Capture {
        minutes: None,
        hours_text: None,
        tickets: Vec::new(),
        project_id: None,
        project_query: None,
        date: today,
        day_word: None,
        description: String::new(),
    };

    if let Some((start, len, minutes)) = find_hours(&words) {
        for flag in &mut used[start..start + len] {
            *flag = true;
        }
        capture.minutes = Some(minutes);
        capture.hours_text = Some(words[start..start + len].join(" "));
    }

    let mut rest = Vec::new();
    for (i, word) in words.iter().enumerate() {
        if used[i] {
            continue;
        }
        if let Some(found) = tickets_in_word(word) {
            for ticket in found {
                if !capture
                    .tickets
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case(&ticket))
                {
                    capture.tickets.push(ticket);
                }
            }
            continue;
        }
        if capture.project_id.is_none() && capture.project_query.is_none() {
            if let Some(query) = word.strip_prefix('#') {
                let query = trim_punctuation(query);
                if !query.is_empty() {
                    match match_project(query, projects) {
                        Some(id) => {
                            capture.project_id = Some(id);
                            continue;
                        }
                        None => capture.project_query = Some(query.to_string()),
                    }
                }
            }
        }
        if capture.day_word.is_none() {
            if let Some(date) = day_word_date(word, today, week_start) {
                capture.date = date;
                capture.day_word = Some(trim_punctuation(word).to_string());
                continue;
            }
        }
        rest.push(*word);
    }
    capture.description = rest.join(" ");
    if capture.project_id.is_none() && capture.project_query.is_none() {
        capture.project_id = project_named_in(&capture.description, projects);
    }
    capture
}

/// The project whose name the words are, or start with (ignoring case, spaces and
/// punctuation): "Huntsville closet" logs to the project "Huntsville Closet", and so does
/// "Huntsville closet re-cabling". The longest matching name wins; names shorter than
/// [`MIN_NAMED_PROJECT`] letters only match exactly.
fn project_named_in(description: &str, projects: &[CaptureProject<'_>]) -> Option<ProjectId> {
    let words = compact(description);
    if words.is_empty() {
        return None;
    }
    projects
        .iter()
        .filter(|p| {
            let name = compact(p.name);
            !name.is_empty()
                && (name == words
                    || (name.chars().count() >= MIN_NAMED_PROJECT && words.starts_with(&name)))
        })
        .max_by_key(|p| compact(p.name).chars().count())
        .map(|p| p.id)
}

/// Shortest project name (letters and digits) that matches the start of a longer description.
pub const MIN_NAMED_PROJECT: usize = 6;

/// The first run of words that reads as a time: (start, word count, minutes).
fn find_hours(words: &[&str]) -> Option<(usize, usize, Minutes)> {
    for start in 0..words.len() {
        if !starts_like_number(words[start]) {
            continue;
        }
        let longest = MAX_HOURS_WORDS.min(words.len() - start);
        for len in (1..=longest).rev() {
            let window = &words[start..start + len];
            let joined = window.join(" ");
            let Ok(minutes) = parse_hours(&joined) else {
                continue;
            };
            if len == 1 && !single_word_is_time(window[0], start) {
                continue;
            }
            return Some((start, len, minutes));
        }
    }
    None
}

fn starts_like_number(word: &str) -> bool {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) if c.is_ascii_digit() => true,
        Some('.') | Some(',') => chars.next().is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    }
}

/// A single word that parsed as a time: plain whole numbers only count as the
/// first word and up to 24; anything with a unit, colon or decimal counts anywhere.
fn single_word_is_time(word: &str, position: usize) -> bool {
    if word.chars().all(|c| c.is_ascii_digit()) {
        return position == 0
            && word
                .parse::<u64>()
                .is_ok_and(|n| n <= BARE_NUMBER_HOURS_LIMIT);
    }
    true
}

fn trim_punctuation(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Tickets in one word (`INC1`, `(INC1),` or `INC1,INC2`), or `None` if any part
/// isn't a ticket.
fn tickets_in_word(word: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = word
        .split([',', ';', '/'])
        .map(trim_punctuation)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    parts.into_iter().map(ticket).collect()
}

/// The tickets in pasted text when every word is a ticket (`INC1 INC2`, one per line, or
/// `INC1, INC2`), upper-cased and without repeats; `None` when any word isn't one or there
/// are none. Used by "paste anywhere on the Desk" (round 15): tickets go to the Tickets
/// tray, anything else becomes a jot.
pub fn pasted_tickets(text: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        for t in tickets_in_word(word)? {
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// One ticket, upper-cased, if `text` looks like one (see the module docs).
pub fn ticket(text: &str) -> Option<String> {
    let text = trim_punctuation(text);
    if text.chars().count() > MAX_TICKET_CHARS {
        return None;
    }
    let letters: String = text
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let after_letters = &text[letters.len()..];
    let digits = after_letters.strip_prefix('-').unwrap_or(after_letters);
    let letters_ok = (2..=10).contains(&letters.len());
    let digits_ok = digits.len() >= 2 && digits.chars().all(|c| c.is_ascii_digit());
    if !letters_ok || !digits_ok {
        return None;
    }
    let upper = letters.chars().all(|c| c.is_ascii_uppercase());
    if !upper && digits.len() < 4 {
        return None;
    }
    Some(text.to_ascii_uppercase())
}

fn compact(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// The project a `#name` query means, by the tiers in the module docs.
pub fn match_project(query: &str, projects: &[CaptureProject<'_>]) -> Option<ProjectId> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    let tiers: [&dyn Fn(&str) -> bool; 4] = [
        &|name: &str| name.trim().to_lowercase() == q,
        &|name: &str| name.trim().to_lowercase().starts_with(&q),
        &|name: &str| compact(name).starts_with(&compact(&q)) && !compact(&q).is_empty(),
        &|name: &str| {
            name.to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| !w.is_empty() && w.starts_with(&q))
        },
    ];
    tiers
        .iter()
        .find_map(|test| projects.iter().find(|p| test(p.name)).map(|p| p.id))
}

fn weekday_of(word: &str) -> Option<Weekday> {
    Some(match word {
        "mon" | "monday" => Weekday::Monday,
        "tue" | "tues" | "tuesday" => Weekday::Tuesday,
        "wed" | "weds" | "wednesday" => Weekday::Wednesday,
        "thu" | "thur" | "thurs" | "thursday" => Weekday::Thursday,
        "fri" | "friday" => Weekday::Friday,
        "sat" | "saturday" => Weekday::Saturday,
        "sun" | "sunday" => Weekday::Sunday,
        _ => return None,
    })
}

/// The date a day word means, or `None` if it isn't one.
pub fn day_word_date(word: &str, today: Date, week_start: WeekStart) -> Option<Date> {
    let word = trim_punctuation(word).to_lowercase();
    match word.as_str() {
        "today" => Some(today),
        "yesterday" => Some(add_days(today, -1)),
        other => {
            let day = weekday_of(other)?;
            let start = week_of(today, week_start);
            let offset = (i64::from(day.number_days_from_monday())
                - i64::from(week_start.0.number_days_from_monday()))
            .rem_euclid(7);
            Some(add_days(start, offset))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    // Thursday 15 October 2026; the week (Monday start) began on the 12th.
    const TODAY: Date = date!(2026 - 10 - 15);

    fn projects() -> Vec<(ProjectId, &'static str)> {
        ["Network refresh", "Data centre", "Patching", "Net tools"]
            .into_iter()
            .map(|n| (ProjectId::new(), n))
            .collect()
    }

    fn parse_with(text: &str, list: &[(ProjectId, &'static str)]) -> Capture {
        let refs: Vec<CaptureProject<'_>> = list
            .iter()
            .map(|(id, name)| CaptureProject { id: *id, name })
            .collect();
        parse_capture(text, TODAY, WeekStart::default(), &refs)
    }

    fn parse(text: &str) -> Capture {
        parse_with(text, &[])
    }

    #[test]
    fn the_example_line() {
        let list = projects();
        let c = parse_with("1.5h firewall cleanup CHG0012345 #network yesterday", &list);
        assert_eq!(c.minutes, Some(Minutes(90)));
        assert_eq!(c.hours_text.as_deref(), Some("1.5h"));
        assert_eq!(c.tickets, vec!["CHG0012345"]);
        assert_eq!(c.project_id, Some(list[0].0));
        assert_eq!(c.project_query, None);
        assert_eq!(c.date, date!(2026 - 10 - 14));
        assert_eq!(c.day_word.as_deref(), Some("yesterday"));
        assert_eq!(c.description, "firewall cleanup");
    }

    #[test]
    fn a_line_that_is_a_project_name_logs_to_that_project() {
        let list = vec![
            (ProjectId::new(), "Branch Network Closet (Comms Room)"),
            (ProjectId::new(), "Branch Network Closet"),
            (ProjectId::new(), "Lab"),
        ];
        let c = parse_with("1h branch network closet (comms room)", &list);
        assert_eq!(c.project_id, Some(list[0].0), "the exact name");
        let c = parse_with("branch network closet re-cabling", &list);
        assert_eq!(c.project_id, Some(list[1].0), "starts with a name");
        assert_eq!(
            c.description, "branch network closet re-cabling",
            "the words stay"
        );
        assert_eq!(
            parse_with("Lab", &list).project_id,
            Some(list[2].0),
            "short names match exactly"
        );
        assert_eq!(
            parse_with("Labels printed", &list).project_id,
            None,
            "but not as a start"
        );
        assert_eq!(parse_with("fix printer", &list).project_id, None);
        // A #tag still wins.
        let c = parse_with("branch network closet #lab", &list);
        assert_eq!(c.project_id, Some(list[2].0));
    }

    #[test]
    fn every_hours_form_is_found() {
        let cases: &[(&str, u32, &str)] = &[
            ("1.5 fix printer", 90, "fix printer"),
            ("fix printer 1.5", 90, "fix printer"),
            ("fix printer 1,5", 90, "fix printer"),
            ("1:30 fix printer", 90, "fix printer"),
            ("fix printer 90m", 90, "fix printer"),
            ("fix printer 90 min", 90, "fix printer"),
            ("fix printer 2h", 120, "fix printer"),
            ("fix printer 2 hours", 120, "fix printer"),
            ("fix printer 1h 30m", 90, "fix printer"),
            ("fix printer 1h30", 90, "fix printer"),
            ("fix printer 1 hour 30 minutes", 90, "fix printer"),
            ("fix 1 h 30 m printer", 90, "fix printer"),
            ("8 standup", 480, "standup"),
            ("24 on call", 1440, "on call"),
            ("0 marker", 0, "marker"),
            (".5 call", 30, "call"),
        ];
        for &(text, minutes, description) in cases {
            let c = parse(text);
            assert_eq!(c.minutes, Some(Minutes(minutes)), "{text}");
            assert_eq!(c.description, description, "{text}");
        }
    }

    #[test]
    fn plain_whole_numbers_only_lead_and_only_up_to_24() {
        for text in [
            "45 errors on the site",
            "room 12 projector",
            "fixed 3 printers",
        ] {
            let c = parse(text);
            assert_eq!(c.minutes, None, "{text}");
            assert_eq!(c.description, text, "{text}");
        }
        let c = parse("25 users reset");
        assert_eq!(c.minutes, None);
    }

    #[test]
    fn only_the_first_time_is_used() {
        let c = parse("2h meeting then 30m notes");
        assert_eq!(c.minutes, Some(Minutes(120)));
        assert_eq!(c.description, "meeting then 30m notes");
    }

    #[test]
    fn too_long_times_are_words() {
        let c = parse("printer 25h");
        assert_eq!(c.minutes, None);
        assert_eq!(c.description, "printer 25h");
    }

    #[test]
    fn tickets() {
        let c = parse("INC0045678 CHG0012345 PROJ-123 inc0045690 AB12");
        assert_eq!(
            c.tickets,
            vec!["INC0045678", "CHG0012345", "PROJ-123", "INC0045690", "AB12"]
        );
        assert_eq!(c.description, "");

        let c = parse("check (INC0045678), and INC1111,INC2222; plus INC1111");
        assert_eq!(c.tickets, vec!["INC0045678", "INC1111", "INC2222"]);
        assert_eq!(c.description, "check and plus");
    }

    #[test]
    fn words_that_are_not_tickets() {
        for word in [
            "win10",
            "mp3",
            "A1234",
            "ABCDEFGHIJK12",
            "INC1",
            "PROJ-",
            "1234",
            "proj-123",
            "INC12a",
            "INC--12",
        ] {
            assert_eq!(ticket(word), None, "{word}");
            assert!(parse(word).tickets.is_empty(), "{word}");
        }
        assert_eq!(ticket("ABCDEFGHIJ12").as_deref(), Some("ABCDEFGHIJ12"));
        assert_eq!(ticket("proj-1234").as_deref(), Some("PROJ-1234"));
        let long = format!("AB{}", "1".repeat(70));
        assert_eq!(ticket(&long), None);
    }

    #[test]
    fn project_matching_tiers() {
        let list = projects();
        let id = |i: usize| Some(list[i].0);
        assert_eq!(parse_with("#patching", &list).project_id, id(2));
        assert_eq!(parse_with("#PAT", &list).project_id, id(2));
        assert_eq!(
            parse_with("#net", &list).project_id,
            id(0),
            "first prefix wins"
        );
        assert_eq!(
            parse_with("#nettools", &list).project_id,
            id(3),
            "spaces ignored"
        );
        assert_eq!(
            parse_with("#centre", &list).project_id,
            id(1),
            "word prefix"
        );
        assert_eq!(parse_with("#refresh,", &list).project_id, id(0));
        let c = parse_with("cabling #zzz", &list);
        assert_eq!(c.project_id, None);
        assert_eq!(c.project_query.as_deref(), Some("zzz"));
        assert_eq!(c.description, "cabling #zzz");
        let c = parse_with("# alone", &list);
        assert_eq!(c.project_id, None);
        assert_eq!(c.description, "# alone");
    }

    #[test]
    fn day_words() {
        let cases: &[(&str, Date)] = &[
            ("today", TODAY),
            ("Yesterday", date!(2026 - 10 - 14)),
            ("mon", date!(2026 - 10 - 12)),
            ("Monday", date!(2026 - 10 - 12)),
            ("tues", date!(2026 - 10 - 13)),
            ("wed", date!(2026 - 10 - 14)),
            ("thu", TODAY),
            ("fri", date!(2026 - 10 - 16)),
            ("sat", date!(2026 - 10 - 17)),
            ("sunday.", date!(2026 - 10 - 18)),
        ];
        for &(word, expected) in cases {
            let c = parse(&format!("1h call {word}"));
            assert_eq!(c.date, expected, "{word}");
            assert_eq!(c.description, "call", "{word}");
        }
        let c = parse("1h call mon then tue");
        assert_eq!(c.date, date!(2026 - 10 - 12));
        assert_eq!(c.description, "call then tue");
        assert_eq!(parse("call").date, TODAY);
    }

    #[test]
    fn day_words_follow_the_week_start() {
        let sunday = WeekStart(Weekday::Sunday);
        assert_eq!(
            day_word_date("sun", TODAY, sunday),
            Some(date!(2026 - 10 - 11))
        );
        assert_eq!(
            day_word_date("sat", TODAY, sunday),
            Some(date!(2026 - 10 - 17))
        );
        assert_eq!(day_word_date("someday", TODAY, sunday), None);
    }

    #[test]
    fn empty_and_spaces() {
        let c = parse("   ");
        assert_eq!(c.minutes, None);
        assert!(c.tickets.is_empty());
        assert_eq!(c.description, "");
        assert_eq!(c.date, TODAY);
    }

    #[test]
    fn serializes_for_the_ui() {
        let c = parse("1h INC0045678 call");
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["minutes"], 60);
        assert_eq!(json["hoursText"], "1h");
        assert_eq!(json["date"], "2026-10-15");
        assert_eq!(json["tickets"][0], "INC0045678");
    }

    #[test]
    fn pasted_text_is_tickets_only_when_every_word_is_one() {
        assert_eq!(
            pasted_tickets("INC0045712\nritm0012399, INC0045712"),
            Some(vec!["INC0045712".into(), "RITM0012399".into()])
        );
        assert_eq!(pasted_tickets("guest wifi drops again INC0045712"), None);
        assert_eq!(pasted_tickets("   "), None);
        assert_eq!(
            pasted_tickets("PROJ-123 (CHG0012345)"),
            Some(vec!["PROJ-123".into(), "CHG0012345".into()])
        );
    }
}
