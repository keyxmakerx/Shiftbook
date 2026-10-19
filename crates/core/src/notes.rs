//! Notes: kinds, tags, titles and the auto-archive rule (decided on the r7-notes card).
//!
//! Archived notes leave the lists but Find still finds them. Auto-archive is a setting:
//! a Jot, Daily or Meeting note that hasn't changed for that many days moves to the
//! archive, and for the last [`ARCHIVE_WARN_DAYS`] days before that the note says so,
//! with a Keep button. Pages and MOPs are reference material and never move on their own.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::{preview, CoreError};
use crate::validate::MAX_NAME_CHARS;

/// Longest note title.
pub const MAX_TITLE_CHARS: usize = MAX_NAME_CHARS;
/// Longest note body (a long OneNote page fits comfortably).
pub const MAX_BODY_CHARS: usize = 200_000;
/// Most tags on one note.
pub const MAX_TAGS: usize = 20;
/// Longest tag.
pub const MAX_TAG_CHARS: usize = 40;
/// Longest title made from the first line of the body.
const TITLE_FROM_BODY_CHARS: usize = 80;
/// Title for a note with nothing in it yet.
pub const UNTITLED: &str = "Untitled note";

/// Auto-archive choices offered in Settings, in days (0 = off).
pub const AUTO_ARCHIVE_CHOICES: [u32; 6] = [0, 30, 60, 90, 180, 365];
/// Default: notes move after 90 days without a change.
pub const DEFAULT_AUTO_ARCHIVE_DAYS: u32 = 90;
/// How many days before moving a note shows its warning.
pub const ARCHIVE_WARN_DAYS: u32 = 7;

/// What kind of note it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    /// A quick scrap.
    Jot,
    /// The note for one day.
    Daily,
    /// Meeting notes.
    Meeting,
    /// Anything to keep for reference.
    Page,
    /// A method of procedure.
    Mop,
}

impl NoteKind {
    /// Every kind, in the order the filters show them.
    pub const ALL: [NoteKind; 5] = [
        NoteKind::Jot,
        NoteKind::Daily,
        NoteKind::Meeting,
        NoteKind::Page,
        NoteKind::Mop,
    ];

    /// The text stored in `notes.kind`.
    pub const fn as_str(self) -> &'static str {
        match self {
            NoteKind::Jot => "jot",
            NoteKind::Daily => "daily",
            NoteKind::Meeting => "meeting",
            NoteKind::Page => "page",
            NoteKind::Mop => "mop",
        }
    }

    /// True for the kinds that auto-archive moves (not Pages or MOPs).
    pub const fn auto_archives(self) -> bool {
        matches!(self, NoteKind::Jot | NoteKind::Daily | NoteKind::Meeting)
    }
}

impl FromStr for NoteKind {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        NoteKind::ALL
            .into_iter()
            .find(|k| k.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| CoreError::NotAChoice {
                value: preview(s, 30),
            })
    }
}

/// Read tags typed as `network, #Guest WiFi, network`: split on commas, trim, drop a
/// leading `#`, squeeze inner spaces and drop repeats (ignoring case, first spelling wins).
pub fn parse_tags(text: &str) -> Result<Vec<String>, CoreError> {
    let mut tags: Vec<String> = Vec::new();
    for part in text.split(',') {
        let tag = part
            .trim()
            .trim_start_matches('#')
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if tag.is_empty() || tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            continue;
        }
        if tag.chars().count() > MAX_TAG_CHARS {
            return Err(CoreError::TooLong {
                field: "A tag",
                max: MAX_TAG_CHARS,
            });
        }
        tags.push(tag);
    }
    if tags.len() > MAX_TAGS {
        return Err(CoreError::TooManyItems {
            field: "tags",
            max: MAX_TAGS,
        });
    }
    Ok(tags)
}

/// Tags as stored in `notes.tags`.
pub fn join_tags(tags: &[String]) -> String {
    tags.join(", ")
}

/// The title to save: the typed one, else the body's first line (shortened), else
/// "Untitled note".
pub fn note_title(title: &str, body: &str) -> Result<String, CoreError> {
    let title = title.trim();
    if !title.is_empty() {
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(CoreError::TooLong {
                field: "Title",
                max: MAX_TITLE_CHARS,
            });
        }
        return Ok(title.to_string());
    }
    Ok(body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map_or_else(
            || UNTITLED.to_string(),
            |l| preview(l, TITLE_FROM_BODY_CHARS),
        ))
}

/// Check a note body's length.
pub fn validate_body(body: &str) -> Result<(), CoreError> {
    if body.chars().count() > MAX_BODY_CHARS {
        return Err(CoreError::TooLong {
            field: "The note",
            max: MAX_BODY_CHARS,
        });
    }
    Ok(())
}

/// What auto-archive will do with a note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum ArchiveOutlook {
    /// Stays where it is (auto-archive off, kept, already archived, or a Page or MOP).
    Stays,
    /// Moves in `days_left` days unless it changes or is kept; show the warning.
    Soon {
        /// Whole days until it moves (1 to [`ARCHIVE_WARN_DAYS`]).
        days_left: u32,
    },
    /// Moves now.
    Due,
}

/// The facts about a note that auto-archive looks at.
#[derive(Debug, Clone, Copy)]
pub struct ArchiveFacts {
    /// Its kind.
    pub kind: NoteKind,
    /// Keep was pressed: never moves on its own.
    pub keep: bool,
    /// Already in the archive.
    pub archived: bool,
    /// When it last changed.
    pub updated_at: OffsetDateTime,
}

/// What auto-archive does with a note at `now`, when notes move after `after_days`
/// days without a change (0 = auto-archive is off).
pub fn archive_outlook(note: ArchiveFacts, now: OffsetDateTime, after_days: u32) -> ArchiveOutlook {
    if after_days == 0 || note.keep || note.archived || !note.kind.auto_archives() {
        return ArchiveOutlook::Stays;
    }
    let idle = (now - note.updated_at).whole_days().max(0);
    let idle = u32::try_from(idle).unwrap_or(u32::MAX);
    if idle >= after_days {
        return ArchiveOutlook::Due;
    }
    let days_left = after_days - idle;
    if days_left <= ARCHIVE_WARN_DAYS {
        ArchiveOutlook::Soon { days_left }
    } else {
        ArchiveOutlook::Stays
    }
}

/// The text Find matches a note against: title, tags, project name and body.
pub fn search_text(title: &str, tags: &[String], project: Option<&str>, body: &str) -> String {
    let mut text = String::with_capacity(title.len() + body.len() + 64);
    text.push_str(title);
    for tag in tags {
        text.push(' ');
        text.push_str(tag);
    }
    if let Some(p) = project {
        text.push(' ');
        text.push_str(p);
    }
    text.push(' ');
    text.push_str(body);
    text
}

/// The title of a day's Daily note, e.g. "Daily · Mon 19 Oct 2026". The Desk's Today tab
/// finds and writes the day's Daily note by this title.
pub fn daily_note_title(date: time::Date) -> String {
    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let day = DAYS[usize::from(date.weekday().number_days_from_monday())];
    let month = MONTHS[usize::from(u8::from(date.month()) - 1)];
    format!("Daily · {day} {} {month} {}", date.day(), date.year())
}

/// True when `note` is the Daily note for `date`.
pub fn is_daily_note_for(kind: NoteKind, title: &str, date: time::Date) -> bool {
    kind == NoteKind::Daily && title == daily_note_title(date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;
    use time::Duration;

    #[test]
    fn daily_note_titles() {
        use time::macros::date;
        assert_eq!(
            daily_note_title(date!(2026 - 10 - 19)),
            "Daily · Mon 19 Oct 2026"
        );
        assert_eq!(
            daily_note_title(date!(2027 - 01 - 03)),
            "Daily · Sun 3 Jan 2027"
        );
        let d = date!(2026 - 10 - 19);
        assert!(is_daily_note_for(
            NoteKind::Daily,
            "Daily · Mon 19 Oct 2026",
            d
        ));
        assert!(!is_daily_note_for(
            NoteKind::Jot,
            "Daily · Mon 19 Oct 2026",
            d
        ));
        assert!(!is_daily_note_for(
            NoteKind::Daily,
            "Daily · Tue 20 Oct 2026",
            d
        ));
    }

    #[test]
    fn kinds_round_trip_and_reject_unknown() {
        for k in NoteKind::ALL {
            assert_eq!(k.as_str().parse::<NoteKind>().unwrap(), k);
        }
        assert_eq!(" MOP ".parse::<NoteKind>().unwrap(), NoteKind::Mop);
        assert!("notebook".parse::<NoteKind>().is_err());
        assert!(NoteKind::Meeting.auto_archives());
        assert!(!NoteKind::Page.auto_archives());
        assert!(!NoteKind::Mop.auto_archives());
    }

    #[test]
    fn tags_are_cleaned() {
        assert_eq!(
            parse_tags(" network, #Guest   WiFi,,NETWORK ,  ").unwrap(),
            vec!["network", "Guest WiFi"]
        );
        assert!(parse_tags("").unwrap().is_empty());
        assert!(parse_tags(&"x".repeat(MAX_TAG_CHARS + 1)).is_err());
        let many: Vec<String> = (0..=MAX_TAGS).map(|i| format!("t{i}")).collect();
        assert!(parse_tags(&many.join(",")).is_err());
        assert_eq!(join_tags(&["a".into(), "b c".into()]), "a, b c");
        assert_eq!(
            parse_tags(&join_tags(&["a".into(), "b c".into()])).unwrap(),
            ["a", "b c"]
        );
    }

    #[test]
    fn title_falls_back_to_first_line() {
        assert_eq!(note_title("  UPS swap ", "body").unwrap(), "UPS swap");
        assert_eq!(
            note_title("", "\n\n  Docks ordered \nmore").unwrap(),
            "Docks ordered"
        );
        assert_eq!(note_title(" ", "  \n ").unwrap(), UNTITLED);
        let long = "a".repeat(100);
        assert_eq!(note_title("", &long).unwrap().chars().count(), 81);
        assert!(note_title(&"t".repeat(MAX_TITLE_CHARS + 1), "").is_err());
        assert!(validate_body(&"b".repeat(MAX_BODY_CHARS)).is_ok());
        assert!(validate_body(&"b".repeat(MAX_BODY_CHARS + 1)).is_err());
    }

    fn facts(kind: NoteKind, idle_days: i64) -> (ArchiveFacts, OffsetDateTime) {
        let now = datetime!(2026-10-17 12:00 UTC);
        (
            ArchiveFacts {
                kind,
                keep: false,
                archived: false,
                updated_at: now - Duration::days(idle_days) - Duration::hours(1),
            },
            now,
        )
    }

    #[test]
    fn auto_archive_warns_then_moves() {
        let (f, now) = facts(NoteKind::Meeting, 10);
        assert_eq!(archive_outlook(f, now, 90), ArchiveOutlook::Stays);
        let (f, now) = facts(NoteKind::Meeting, 83);
        assert_eq!(
            archive_outlook(f, now, 90),
            ArchiveOutlook::Soon { days_left: 7 }
        );
        let (f, now) = facts(NoteKind::Jot, 89);
        assert_eq!(
            archive_outlook(f, now, 90),
            ArchiveOutlook::Soon { days_left: 1 }
        );
        let (f, now) = facts(NoteKind::Daily, 90);
        assert_eq!(archive_outlook(f, now, 90), ArchiveOutlook::Due);
        let (f, now) = facts(NoteKind::Daily, 400);
        assert_eq!(archive_outlook(f, now, 30), ArchiveOutlook::Due);
    }

    #[test]
    fn auto_archive_leaves_some_notes_alone() {
        let (f, now) = facts(NoteKind::Meeting, 400);
        assert_eq!(archive_outlook(f, now, 0), ArchiveOutlook::Stays, "off");
        let kept = ArchiveFacts { keep: true, ..f };
        assert_eq!(archive_outlook(kept, now, 90), ArchiveOutlook::Stays);
        let archived = ArchiveFacts {
            archived: true,
            ..f
        };
        assert_eq!(archive_outlook(archived, now, 90), ArchiveOutlook::Stays);
        for kind in [NoteKind::Page, NoteKind::Mop] {
            let (f, now) = facts(kind, 400);
            assert_eq!(archive_outlook(f, now, 90), ArchiveOutlook::Stays);
        }
        // A clock that went backwards doesn't archive anything.
        let (f, now) = facts(NoteKind::Jot, -5);
        assert_eq!(archive_outlook(f, now, 30), ArchiveOutlook::Stays);
    }

    #[test]
    fn search_text_has_everything() {
        let t = search_text("UPS", &["power".into()], Some("Site A"), "swap the battery");
        assert_eq!(t, "UPS power Site A swap the battery");
        assert_eq!(search_text("T", &[], None, "b"), "T b");
    }
}
