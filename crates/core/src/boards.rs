//! Boards (rounds 8 and 12): places to step into from the Desk. Each has a full screen with
//! three views of the same things: Notebook (its notes), Board (its own canvas) and List.
//! Shiftbook makes **Projects** (a board per project), **Meetings** (a board per meeting
//! series) and **Daily notes**; each folder in the cabinet is a board too, and the
//! **Done tickets** board and the **Archive** sit at the bottom of the tree.

use uuid::Uuid;

use crate::canvas::{CardKind, Drawer};
use crate::notes::NoteKind;

/// How far back a project's board reaches for entries and tickets (notes always show).
pub const BOARD_DAYS: i64 = 90;

/// A board, by what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Board {
    /// The Desk itself (the root).
    Desk,
    /// Every project, as boards.
    Projects,
    /// One project's board: its entries, tickets and notes.
    Project(Uuid),
    /// Every meeting series, as boards.
    Meetings,
    /// One meeting series' board: its meeting notes.
    Series(Uuid),
    /// The Daily notes.
    Daily,
    /// A folder from the cabinet: what's filed in it.
    Folder(Uuid),
    /// Tickets ticked done, newest first (they leave the Desk at the end of their day).
    Done,
    /// Archived things, with Bring back.
    Archive,
}

impl Board {
    /// The stored and sent form, e.g. `project:<id>`, `daily`, `desk`.
    pub fn key(self) -> String {
        match self {
            Board::Desk => "desk".into(),
            Board::Projects => "projects".into(),
            Board::Project(id) => format!("project:{id}"),
            Board::Meetings => "meetings".into(),
            Board::Series(id) => format!("series:{id}"),
            Board::Daily => "daily".into(),
            Board::Folder(id) => format!("folder:{id}"),
            Board::Done => "done".into(),
            Board::Archive => "archive".into(),
        }
    }

    /// Read the stored form.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        match text {
            "desk" => return Some(Board::Desk),
            "projects" => return Some(Board::Projects),
            "meetings" => return Some(Board::Meetings),
            "daily" => return Some(Board::Daily),
            "done" => return Some(Board::Done),
            "archive" => return Some(Board::Archive),
            _ => {}
        }
        let (kind, id) = text.split_once(':')?;
        let id = Uuid::parse_str(id).ok()?;
        match kind {
            "project" => Some(Board::Project(id)),
            "series" => Some(Board::Series(id)),
            "folder" => Some(Board::Folder(id)),
            _ => None,
        }
    }

    /// The board it sits in (`None` for the Desk): the breadcrumbs walk up these.
    pub fn parent(self) -> Option<Board> {
        match self {
            Board::Desk => None,
            Board::Project(_) => Some(Board::Projects),
            Board::Series(_) => Some(Board::Meetings),
            Board::Projects
            | Board::Meetings
            | Board::Daily
            | Board::Folder(_)
            | Board::Done
            | Board::Archive => Some(Board::Desk),
        }
    }

    /// The way from the Desk down to this board, the Desk first.
    pub fn trail(self) -> Vec<Board> {
        let mut trail = vec![self];
        while let Some(up) = trail.last().and_then(|b| b.parent()) {
            trail.push(up);
        }
        trail.reverse();
        trail
    }

    /// True for boards that have a canvas of their own (the lists of boards don't).
    pub fn has_canvas(self) -> bool {
        matches!(
            self,
            Board::Project(_) | Board::Series(_) | Board::Daily | Board::Folder(_)
        )
    }
}

/// True when a note titled `title` belongs to the meeting series called `series`: its notes
/// are titled "<series> · <day>" (see `crate::meetings::meeting_note_title`).
pub fn in_series(title: &str, series: &str) -> bool {
    let series = series.trim();
    !series.is_empty()
        && title
            .strip_prefix(series)
            .is_some_and(|rest| rest.starts_with(" · "))
}

/// Web addresses written in a note, in order and without repeats (round 12: links in notes).
/// Only `http://` and `https://` count; trailing punctuation is left off.
pub fn links_in(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || c == '<' || c == '>' || c == '"') {
        let lower = word.to_ascii_lowercase();
        let Some(start) = lower.find("https://").or_else(|| lower.find("http://")) else {
            continue;
        };
        let link = word[start..].trim_end_matches(['.', ',', ';', ':', ')', ']', '!', '?', '\'']);
        if link.len() > "https://".len() && !out.iter().any(|l| l == link) {
            out.push(link.to_string());
        }
    }
    out
}

/// What decides which boards a record shows on.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    /// What the card shows.
    pub kind: CardKind,
    /// Its project, if any (an entry's target, a ticket's or a note's project).
    pub project: Option<Uuid>,
    /// For notes: their kind.
    pub note_kind: Option<NoteKind>,
    /// For notes: their title (meeting notes are found by it).
    pub title: &'a str,
    /// For notes: moved to the archive.
    pub archived: bool,
    /// For tray tickets: ticked done.
    pub done: bool,
    /// The drawer it's filed in on the Desk, if any.
    pub drawer: Option<Drawer>,
}

/// The boards a record shows on, given the meeting series (id, name). An archived note
/// shows only in the Archive; a filed card shows in its drawer's board as well as its others.
pub fn boards_for(facts: Facts<'_>, series: &[(Uuid, &str)]) -> Vec<Board> {
    if facts.kind == CardKind::Note && facts.archived {
        return vec![Board::Archive];
    }
    let mut out = Vec::new();
    if let Some(p) = facts.project {
        out.push(Board::Project(p));
    }
    if facts.kind == CardKind::Tray && facts.done {
        out.push(Board::Done);
    }
    if facts.kind == CardKind::Note {
        if facts.note_kind == Some(NoteKind::Daily) {
            out.push(Board::Daily);
        }
        out.extend(
            series
                .iter()
                .filter(|(_, name)| in_series(facts.title, name))
                .map(|(id, _)| Board::Series(*id)),
        );
    }
    match facts.drawer {
        Some(Drawer::Folder(f)) => out.push(Board::Folder(f)),
        Some(Drawer::Project(d)) if facts.project != Some(d) => out.push(Board::Project(d)),
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        let id = Uuid::now_v7();
        for b in [
            Board::Desk,
            Board::Projects,
            Board::Project(id),
            Board::Meetings,
            Board::Series(id),
            Board::Daily,
            Board::Folder(id),
            Board::Done,
            Board::Archive,
        ] {
            assert_eq!(Board::parse(&b.key()), Some(b));
        }
        assert_eq!(Board::parse("shelf"), None);
        assert_eq!(Board::parse("project:nope"), None);
    }

    #[test]
    fn breadcrumbs_walk_up_to_the_desk() {
        let id = Uuid::now_v7();
        assert_eq!(
            Board::Project(id).trail(),
            [Board::Desk, Board::Projects, Board::Project(id)]
        );
        assert_eq!(Board::Daily.trail(), [Board::Desk, Board::Daily]);
        assert_eq!(Board::Desk.trail(), [Board::Desk]);
        assert!(Board::Series(id).has_canvas());
        assert!(!Board::Meetings.has_canvas());
    }

    #[test]
    fn series_notes_are_found_by_title() {
        assert!(in_series("Weekly ops · Mon 19 Oct 2026", "Weekly ops"));
        assert!(!in_series(
            "Weekly ops review · Mon 19 Oct 2026",
            "Weekly ops"
        ));
        assert!(!in_series("Weekly ops", "Weekly ops"));
        assert!(!in_series("anything · x", " "));
    }

    #[test]
    fn records_show_on_the_boards_they_belong_to() {
        let (p, s, f) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let series = [(s, "Weekly ops")];
        let note = |kind, title, archived| Facts {
            kind: CardKind::Note,
            project: Some(p),
            note_kind: Some(kind),
            title,
            archived,
            done: false,
            drawer: None,
        };
        assert_eq!(
            boards_for(note(NoteKind::Meeting, "Weekly ops · Mon", false), &series),
            [Board::Project(p), Board::Series(s)]
        );
        assert_eq!(
            boards_for(note(NoteKind::Daily, "Daily · Mon", false), &series),
            [Board::Project(p), Board::Daily]
        );
        assert_eq!(
            boards_for(note(NoteKind::Daily, "Daily · Mon", true), &series),
            [Board::Archive]
        );
        let ticket = Facts {
            kind: CardKind::Tray,
            project: None,
            note_kind: None,
            title: "Weekly ops · Mon",
            archived: false,
            done: false,
            drawer: Some(Drawer::Folder(f)),
        };
        assert_eq!(boards_for(ticket, &series), [Board::Folder(f)]);
        let finished = Facts {
            done: true,
            drawer: None,
            ..ticket
        };
        assert_eq!(boards_for(finished, &series), [Board::Done]);
        let scrap = Facts {
            kind: CardKind::Inbox,
            drawer: Some(Drawer::Project(p)),
            ..ticket
        };
        assert_eq!(boards_for(scrap, &series), [Board::Project(p)]);
        let loose = Facts {
            drawer: None,
            ..scrap
        };
        assert!(boards_for(loose, &series).is_empty());
    }

    #[test]
    fn links_are_picked_out_of_notes() {
        let text = "See https://wiki.example/page, and (http://intranet/x). Again: https://wiki.example/page";
        assert_eq!(
            links_in(text),
            ["https://wiki.example/page", "http://intranet/x"]
        );
        assert!(links_in("no links, just https:// alone").is_empty());
    }
}
