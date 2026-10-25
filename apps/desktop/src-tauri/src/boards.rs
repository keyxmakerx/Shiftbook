//! Commands for boards (rounds 8 and 12): the full screen you step into from the Desk, with
//! the boards inside it, its cards and, for a project, its time. Which records belong to
//! which board is a rule in `shiftbook_core::boards`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use shiftbook_core::boards::{boards_for, Board, Facts, BOARD_DAYS};
use shiftbook_core::canvas::{auto_place, CardKind, Drawer};
use shiftbook_core::hours::format_hours;
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::EntryTarget;
use shiftbook_core::notes::NoteKind;
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::{Date, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::api::{date_arg, links_for};
use crate::session::{CommandError, VaultSession};
use shiftbook_app::desk::{
    card_for, entry_card, inbox_card, note_card, tray_card, CardView, Pending,
};

type Res<T> = Result<T, CommandError>;

/// One step on the way down from the Desk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrumbView {
    key: String,
    title: String,
}

/// A board inside a board, for the list and the tree.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildView {
    key: String,
    title: String,
    /// A project's colour.
    color: Option<String>,
    /// Cards on it.
    count: usize,
    /// The boards inside it (only filled in the tree).
    children: Vec<ChildView>,
}

/// A board as its full screen shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardView {
    key: String,
    title: String,
    /// A project's colour, for the tint.
    color: Option<String>,
    /// From the Desk down to this board.
    trail: Vec<CrumbView>,
    /// It has a canvas of its own (the lists of boards don't).
    has_canvas: bool,
    /// The boards inside it.
    children: Vec<ChildView>,
    /// Its cards, newest first, each with its spot on this board.
    cards: Vec<CardView>,
    /// For a project: every minute logged against it, as hours.
    hours: Option<String>,
    /// For a project: its tickets still open.
    open_tickets: usize,
    /// For an archived project.
    archived: bool,
    /// Every board from the Desk down, for the tree beside it.
    tree: Vec<ChildView>,
}

/// What a note adds to deciding its boards.
#[derive(Default)]
struct NoteFacts {
    kind: Option<NoteKind>,
    title: String,
    archived: bool,
}

/// What the boards are made from: every record that could be a card, with which boards each
/// shows on, and the names of things.
struct Boards {
    records: Vec<(Pending, Vec<Board>)>,
    projects: Vec<shiftbook_core::model::Project>,
    series: Vec<(Uuid, String)>,
    folders: Vec<(Uuid, String)>,
}

impl Boards {
    fn count(&self, board: Board) -> usize {
        self.records
            .iter()
            .filter(|(_, on)| on.contains(&board))
            .count()
    }

    fn title(&self, board: Board) -> Option<String> {
        Some(match board {
            Board::Desk => "Desk".into(),
            Board::Projects => "Projects".into(),
            Board::Meetings => "Meetings".into(),
            Board::Daily => "Daily notes".into(),
            Board::Done => "Done tickets".into(),
            Board::Archive => "Archive".into(),
            Board::Project(id) => self
                .projects
                .iter()
                .find(|p| *p.id.as_uuid() == id)?
                .name
                .clone(),
            Board::Series(id) => self.series.iter().find(|(s, _)| *s == id)?.1.clone(),
            Board::Folder(id) => self.folders.iter().find(|(f, _)| *f == id)?.1.clone(),
        })
    }

    fn color(&self, board: Board) -> Option<String> {
        match board {
            Board::Project(id) => self
                .projects
                .iter()
                .find(|p| *p.id.as_uuid() == id)
                .and_then(|p| p.color.clone()),
            _ => None,
        }
    }

    fn child(&self, board: Board, children: Vec<ChildView>) -> ChildView {
        let count = match board {
            Board::Projects | Board::Meetings => children.len(),
            Board::Archive => {
                self.count(board) + self.projects.iter().filter(|p| p.archived).count()
            }
            _ => self.count(board),
        };
        ChildView {
            key: board.key(),
            title: self.title(board).unwrap_or_default(),
            color: self.color(board),
            count,
            children,
        }
    }

    /// The boards inside `board`, each with its own inside boards when `deep`.
    fn children(&self, board: Board, deep: bool) -> Vec<ChildView> {
        let inner = |b: Board| {
            let below = if deep {
                self.children(b, false)
            } else {
                Vec::new()
            };
            self.child(b, below)
        };
        match board {
            Board::Desk => {
                let mut out = vec![inner(Board::Projects), inner(Board::Meetings)];
                out.push(inner(Board::Daily));
                out.extend(self.folders.iter().map(|(f, _)| inner(Board::Folder(*f))));
                out.push(inner(Board::Done));
                out.push(inner(Board::Archive));
                out
            }
            Board::Projects => self
                .projects
                .iter()
                .filter(|p| !p.archived)
                .map(|p| inner(Board::Project(*p.id.as_uuid())))
                .collect(),
            Board::Meetings => self
                .series
                .iter()
                .map(|(s, _)| inner(Board::Series(*s)))
                .collect(),
            Board::Archive => self
                .projects
                .iter()
                .filter(|p| p.archived)
                .map(|p| inner(Board::Project(*p.id.as_uuid())))
                .collect(),
            Board::Project(_)
            | Board::Series(_)
            | Board::Daily
            | Board::Folder(_)
            | Board::Done => Vec::new(),
        }
    }
}

/// Load every record that could be a card and work out the boards each shows on.
async fn load(
    store: &SqliteStore,
    owner: shiftbook_core::ids::OwnerId,
    today: Date,
) -> Res<Boards> {
    let from = today - Duration::days(BOARD_DAYS - 1);
    let fields = links_for(store, owner).await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let now = OffsetDateTime::now_utc();
    let projects = store.list_projects(owner).await?;
    let series: Vec<(Uuid, String)> = store
        .meeting_series(owner)
        .await?
        .into_iter()
        .map(|s| (*s.id.as_uuid(), s.name))
        .collect();
    let folders: Vec<(Uuid, String)> = store
        .canvas_folders(owner)
        .await?
        .into_iter()
        .map(|f| (f.id, f.name))
        .collect();
    let drawers: HashMap<(CardKind, Uuid), Drawer> = store
        .canvas_spots(owner)
        .await?
        .into_iter()
        .filter_map(|s| s.drawer.map(|d| ((s.kind, s.record_id), d)))
        .collect();

    let mut found: Vec<(Pending, NoteFacts)> = Vec::new();
    for rec in store.list_entries(owner, from, today).await? {
        if !rec.is_tray && rec.entry.is_live() {
            found.push((entry_card(rec, &fields), NoteFacts::default()));
        }
    }
    for item in store.tray_between(owner, from, today).await? {
        found.push((tray_card(item, &fields), NoteFacts::default()));
    }
    for note in store.list_notes(owner, true).await? {
        let facts = NoteFacts {
            kind: Some(note.kind),
            title: note.title.clone(),
            archived: note.archived_at.is_some(),
        };
        found.push((note_card(note, now, after), facts));
    }
    for scrap in store.list_inbox(owner).await? {
        found.push((inbox_card(scrap), NoteFacts::default()));
    }
    // Older entries and tickets that were filed still show in their drawer's board.
    let have: HashSet<(CardKind, Uuid)> = found.iter().map(|(p, _)| (p.kind, p.id)).collect();
    for &(kind, id) in drawers.keys() {
        if !have.contains(&(kind, id)) {
            if let Some(p) = card_for(store, owner, kind, id, &fields, now, after).await? {
                found.push((p, NoteFacts::default()));
            }
        }
    }

    let names: Vec<(Uuid, &str)> = series.iter().map(|(id, n)| (*id, n.as_str())).collect();
    let records = found
        .into_iter()
        .map(|(p, n)| {
            let on = boards_for(
                Facts {
                    kind: p.kind,
                    project: p.project,
                    note_kind: n.kind,
                    title: &n.title,
                    archived: n.archived,
                    done: p.kind == CardKind::Tray && !p.open,
                    drawer: drawers.get(&(p.kind, p.id)).copied(),
                },
                &names,
            );
            (p, on)
        })
        .collect();
    Ok(Boards {
        records,
        projects,
        series,
        folders,
    })
}

/// Maps to `DataApi.board()`: one board's full screen. Entries and tickets from the last
/// [`BOARD_DAYS`] days show (older ones once filed); notes always do.
#[tauri::command]
pub async fn board(s: State<'_, VaultSession>, today: String, key: String) -> Res<BoardView> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    let board = Board::parse(&key).ok_or(CommandError::InvalidInput)?;
    let all = load(&store, owner, today).await?;
    let title = all.title(board).ok_or(CommandError::NotFound)?;

    let trail = board
        .trail()
        .into_iter()
        .map(|b| CrumbView {
            key: b.key(),
            title: all.title(b).unwrap_or_default(),
        })
        .collect();
    let children = all.children(board, false);
    let tree = all.children(Board::Desk, true);
    let color = all.color(board);

    let (hours, open_tickets, archived) = match board {
        Board::Project(id) => {
            let since = Date::from_calendar_date(2000, time::Month::January, 1)
                .map_err(|_| CommandError::InvalidInput)?;
            let minutes: u32 = store
                .list_entries(owner, since, today)
                .await?
                .iter()
                .filter(|r| r.entry.is_live())
                .filter(|r| matches!(r.entry.target, EntryTarget::Project(p) if *p.as_uuid() == id))
                .map(|r| r.entry.minutes.0)
                .sum();
            let open = all
                .records
                .iter()
                .filter(|(p, on)| p.kind == CardKind::Tray && p.open && on.contains(&board))
                .count();
            let archived = all
                .projects
                .iter()
                .any(|p| *p.id.as_uuid() == id && p.archived);
            (Some(format_hours(Minutes(minutes))), open, archived)
        }
        _ => (None, 0, false),
    };

    // Its cards: moved ones keep their spots here; the rest are placed newest first.
    let spots: HashMap<(CardKind, Uuid), shiftbook_storage::boards::BoardSpot> =
        if board.has_canvas() {
            store
                .board_spots(owner, board)
                .await?
                .into_iter()
                .map(|s| ((s.kind, s.record_id), s))
                .collect()
        } else {
            HashMap::new()
        };
    let mut mine: Vec<Pending> = all
        .records
        .into_iter()
        .filter(|(_, on)| on.contains(&board))
        .map(|(p, _)| p)
        .collect();
    mine.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
    let taken: Vec<(i32, i32)> = mine
        .iter()
        .filter_map(|p| spots.get(&(p.kind, p.id)).map(|s| (s.x, s.y)))
        .collect();
    let loose = mine
        .iter()
        .filter(|p| !spots.contains_key(&(p.kind, p.id)))
        .count();
    let mut free = auto_place(loose, &taken).into_iter();
    let cards = mine
        .into_iter()
        .map(|mut p| {
            if let Some(s) = spots.get(&(p.kind, p.id)) {
                (p.card.x, p.card.y, p.card.z, p.card.placed) = (s.x, s.y, s.z, true);
            } else if let Some((x, y)) = free.next() {
                (p.card.x, p.card.y) = (x, y);
            }
            p.card
        })
        .collect();

    Ok(BoardView {
        key: board.key(),
        color,
        title,
        trail,
        has_canvas: board.has_canvas(),
        children,
        cards,
        hours,
        open_tickets,
        archived,
        tree,
    })
}

/// Maps to `DataApi.boardMove()`: put a card where it was dropped on a board's canvas.
#[tauri::command]
pub async fn board_move(
    s: State<'_, VaultSession>,
    key: String,
    kind: String,
    id: String,
    x: i32,
    y: i32,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let board = Board::parse(&key).ok_or(CommandError::InvalidInput)?;
    let kind: CardKind = kind.parse().map_err(|_| CommandError::InvalidInput)?;
    let id = Uuid::parse_str(&id).map_err(|_| CommandError::InvalidInput)?;
    store.move_on_board(owner, board, kind, id, x, y).await?;
    Ok(())
}
