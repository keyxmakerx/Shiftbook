//! Notes commands. Thin like `api`: parse, call `shiftbook-storage`, return camelCase
//! views. Each maps 1:1 to a `DataApi` method in `web/src/lib/api/`.

use serde::{Deserialize, Serialize};
use shiftbook_core::dates::parse_date;
use shiftbook_core::ids::OwnerId;
use shiftbook_core::model::{Note, Project};
use shiftbook_core::notes::{
    daily_note_title, is_daily_note_for, join_tags, search_text, NoteKind,
};
use shiftbook_core::search::rank;
use shiftbook_storage::notes::NoteInput;
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::OffsetDateTime;

pub(crate) use shiftbook_app::views::{note_view, NoteView};

use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

fn id_arg<T: std::str::FromStr>(text: &str) -> Res<T> {
    text.parse().map_err(|_| CommandError::InvalidInput)
}

/// The notes list.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct NoteListView {
    /// Notes to show: best match first when searching, else most recently changed.
    notes: Vec<NoteView>,
    /// Archived notes that match but were left out (Include archived is off).
    hidden_archived: usize,
}

/// What the UI sends to write or change a note.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteArgs {
    kind: String,
    title: String,
    body: String,
    project_id: Option<String>,
    tags: String,
}

impl NoteArgs {
    fn input(&self) -> Res<NoteInput> {
        Ok(NoteInput {
            kind: id_arg(&self.kind)?,
            title: self.title.clone(),
            body: self.body.clone(),
            project: self
                .project_id
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .map(id_arg)
                .transpose()?,
            tags: self.tags.clone(),
        })
    }
}

/// Move due notes to the archive, then return the auto-archive setting.
async fn tidy(store: &SqliteStore, owner: OwnerId, now: OffsetDateTime) -> Res<u32> {
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    store.auto_archive_notes(owner, now, after).await?;
    Ok(after)
}

fn ranked(query: &str, notes: Vec<Note>, projects: &[Project], limit: usize) -> Vec<Note> {
    rank(
        query,
        notes.into_iter().map(|n| {
            let project = n
                .project_id
                .and_then(|id| projects.iter().find(|p| p.id == id))
                .map(|p| p.name.as_str());
            let text = search_text(&n.title, &n.tags, project, &n.body);
            (n, text)
        }),
        limit,
    )
}

/// Notes for Find: matches from every note, archived ones included, best first.
pub(crate) async fn find_notes(
    store: &SqliteStore,
    owner: OwnerId,
    query: &str,
    limit: usize,
) -> Res<Vec<NoteView>> {
    let now = OffsetDateTime::now_utc();
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let projects = store.list_projects(owner).await?;
    let notes = store.list_notes(owner, true).await?;
    Ok(ranked(query, notes, &projects, limit)
        .into_iter()
        .map(|n| note_view(n, now, after))
        .collect())
}

/// Maps to `DataApi.listNotes()`: notes of one kind (or all), matching `query` when it
/// isn't blank. Moves notes that auto-archive says are due first.
#[tauri::command]
pub async fn list_notes(
    s: State<'_, VaultSession>,
    query: String,
    kind: Option<String>,
    include_archived: bool,
) -> Res<NoteListView> {
    let (store, owner) = s.data().await?;
    let now = OffsetDateTime::now_utc();
    let after = tidy(&store, owner, now).await?;
    let kind: Option<NoteKind> = kind.as_deref().map(id_arg).transpose()?;
    let notes: Vec<Note> = store
        .list_notes(owner, true)
        .await?
        .into_iter()
        .filter(|n| kind.is_none_or(|k| n.kind == k))
        .collect();
    let notes = if query.trim().is_empty() {
        notes
    } else {
        let projects = store.list_projects(owner).await?;
        let limit = notes.len();
        ranked(&query, notes, &projects, limit)
    };
    let (shown, hidden): (Vec<Note>, Vec<Note>) = notes
        .into_iter()
        .partition(|n| include_archived || n.archived_at.is_none());
    Ok(NoteListView {
        notes: shown
            .into_iter()
            .map(|n| note_view(n, now, after))
            .collect(),
        hidden_archived: if query.trim().is_empty() {
            0
        } else {
            hidden.len()
        },
    })
}

/// Maps to `DataApi.getNote()`.
#[tauri::command]
pub async fn get_note(s: State<'_, VaultSession>, id: String) -> Res<NoteView> {
    let (store, owner) = s.data().await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let note = store.get_note(owner, id_arg(&id)?).await?;
    Ok(note_view(note, OffsetDateTime::now_utc(), after))
}

/// Maps to `DataApi.createNote()`.
#[tauri::command]
pub async fn create_note(s: State<'_, VaultSession>, note: NoteArgs) -> Res<NoteView> {
    let (store, owner) = s.data().await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let note = store.create_note(owner, &note.input()?).await?;
    Ok(note_view(note, OffsetDateTime::now_utc(), after))
}

/// Maps to `DataApi.updateNote()`.
#[tauri::command]
pub async fn update_note(s: State<'_, VaultSession>, id: String, note: NoteArgs) -> Res<NoteView> {
    let (store, owner) = s.data().await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let note = store
        .update_note(owner, id_arg(&id)?, &note.input()?)
        .await?;
    Ok(note_view(note, OffsetDateTime::now_utc(), after))
}

/// The Daily note for `date`, archived or not, if there is one.
async fn find_daily(store: &SqliteStore, owner: OwnerId, date: time::Date) -> Res<Option<Note>> {
    Ok(store
        .list_notes(owner, true)
        .await?
        .into_iter()
        .find(|n| is_daily_note_for(n.kind, &n.title, date)))
}

/// Maps to `DataApi.dailyNote()`: the day's Daily note for the Desk's Today tab, or null.
#[tauri::command]
pub async fn daily_note(s: State<'_, VaultSession>, date: String) -> Res<Option<NoteView>> {
    let (store, owner) = s.data().await?;
    let date = parse_date(&date).map_err(|_| CommandError::InvalidInput)?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let now = OffsetDateTime::now_utc();
    Ok(find_daily(&store, owner, date)
        .await?
        .map(|n| note_view(n, now, after)))
}

/// Maps to `DataApi.saveDailyNote()`: write the day's Daily note, creating it the first time
/// something is typed. Its project and tags stay as they are.
#[tauri::command]
pub async fn save_daily_note(
    s: State<'_, VaultSession>,
    date: String,
    body: String,
) -> Res<Option<NoteView>> {
    let (store, owner) = s.data().await?;
    let date = parse_date(&date).map_err(|_| CommandError::InvalidInput)?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let note = match find_daily(&store, owner, date).await? {
        Some(n) => {
            let input = NoteInput {
                kind: NoteKind::Daily,
                title: n.title.clone(),
                body,
                project: n.project_id,
                tags: join_tags(&n.tags),
            };
            Some(store.update_note(owner, n.id, &input).await?)
        }
        None if body.trim().is_empty() => None,
        None => {
            let input = NoteInput {
                kind: NoteKind::Daily,
                title: daily_note_title(date),
                body,
                project: None,
                tags: String::new(),
            };
            Some(store.create_note(owner, &input).await?)
        }
    };
    let now = OffsetDateTime::now_utc();
    Ok(note.map(|n| note_view(n, now, after)))
}

/// Maps to `DataApi.setNoteArchived()` (Archive, or Bring back).
#[tauri::command]
pub async fn set_note_archived(s: State<'_, VaultSession>, id: String, archived: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_note_archived(owner, id_arg(&id)?, archived)
        .await?)
}

/// Maps to `DataApi.setNoteKeep()` (keep out of auto-archive).
#[tauri::command]
pub async fn set_note_keep(s: State<'_, VaultSession>, id: String, keep: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.set_note_keep(owner, id_arg(&id)?, keep).await?)
}

/// Maps to `DataApi.setNoteDeleted()` (delete, or undo a delete).
#[tauri::command]
pub async fn set_note_deleted(s: State<'_, VaultSession>, id: String, deleted: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.set_note_deleted(owner, id_arg(&id)?, deleted).await?)
}
