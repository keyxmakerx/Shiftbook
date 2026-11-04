//! Commands for the wiki (round 18, see `shiftbook_core::wiki`): the side list of spaces and
//! their pages, one page with what links to it, earlier versions, spaces, moving pages, and
//! writing a space out as Markdown files.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use shiftbook_app::views::{note_view, NoteView};
use shiftbook_core::dates::format_timestamp;
use shiftbook_core::ids::{NoteId, OwnerId};
use shiftbook_core::model::Note;
use shiftbook_core::notes::NoteKind;
use shiftbook_core::wiki::{export_paths, links_to, tree, Placed};
use shiftbook_storage::wiki::{in_wiki, WikiPlace};
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

fn id_arg<T: std::str::FromStr>(text: &str) -> Res<T> {
    text.parse().map_err(|_| CommandError::InvalidInput)
}

/// One page in the side list.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WikiRowView {
    id: String,
    title: String,
    /// 0 for a page at the top of its space.
    depth: u32,
    /// A MOP template.
    template: bool,
}

/// A space and its pages, as a tree.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WikiSpaceView {
    id: String,
    name: String,
    color: Option<String>,
    pages: Vec<WikiRowView>,
}

/// The wiki's side list.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WikiView {
    spaces: Vec<WikiSpaceView>,
    /// Pages in no space (Pages written before the wiki, or from a removed space).
    loose: Vec<WikiRowView>,
    /// MOP templates, by title.
    templates: Vec<WikiRowView>,
}

/// A step on the way down to a page.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WikiStepView {
    id: String,
    title: String,
}

/// A note or ticket that links to a page.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WikiLinkerView {
    id: String,
    title: String,
    /// The note's kind, e.g. "page" or "meeting".
    kind: NoteKind,
}

/// A ticket whose notepad links to a page.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WikiTicketView {
    number: String,
    done: bool,
}

/// One page, full.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WikiPageView {
    note: NoteView,
    /// Its space, if any.
    space_id: Option<String>,
    space_name: Option<String>,
    /// The pages above it, the top one first.
    trail: Vec<WikiStepView>,
    /// The pages right under it.
    children: Vec<WikiStepView>,
    /// Notes that link to it with `[[its title]]`, newest first.
    linked_from: Vec<WikiLinkerView>,
    /// Tickets whose notepad links to it.
    on_tickets: Vec<WikiTicketView>,
    /// How many earlier versions are kept.
    versions: u32,
}

/// An earlier version of a page.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct NoteVersionView {
    id: String,
    title: String,
    body: String,
    /// When it was kept (UTC, RFC 3339).
    at: String,
}

fn placed(places: &[WikiPlace]) -> Vec<Placed> {
    places
        .iter()
        .map(|p| Placed {
            id: p.page,
            parent: p.parent,
            position: p.position,
        })
        .collect()
}

fn rows(pages: &[&Note], places: &[WikiPlace]) -> Vec<WikiRowView> {
    let by_id: HashMap<Uuid, &Note> = pages.iter().map(|n| (*n.id.as_uuid(), *n)).collect();
    // Siblings at the same position fall back to title order.
    let mut mine: Vec<WikiPlace> = places
        .iter()
        .filter(|p| by_id.contains_key(&p.page))
        .copied()
        .collect();
    mine.sort_by(|a, b| {
        let t = |id: Uuid| by_id[&id].title.to_lowercase();
        (a.position, t(a.page)).cmp(&(b.position, t(b.page)))
    });
    tree(&placed(&mine))
        .into_iter()
        .map(|r| {
            let n = by_id[&r.id];
            WikiRowView {
                id: n.id.to_string(),
                title: n.title.clone(),
                depth: u32::try_from(r.depth).unwrap_or(u32::MAX),
                template: n.template,
            }
        })
        .collect()
}

/// Maps to `DataApi.wiki()`: the side list.
#[tauri::command]
pub async fn wiki(s: State<'_, VaultSession>) -> Res<WikiView> {
    let (store, owner) = s.data().await?;
    let notes: Vec<Note> = store
        .list_notes(owner, false)
        .await?
        .into_iter()
        .filter(in_wiki)
        .collect();
    let places = store.wiki_places(owner).await?;
    let space_of: HashMap<Uuid, Option<Uuid>> = places.iter().map(|p| (p.page, p.space)).collect();
    let pages = |space: Option<Uuid>| -> Vec<&Note> {
        notes
            .iter()
            .filter(|n| n.kind == NoteKind::Page)
            .filter(|n| space_of.get(n.id.as_uuid()).copied().flatten() == space)
            .collect()
    };
    let spaces = store
        .wiki_spaces(owner)
        .await?
        .into_iter()
        .map(|sp| WikiSpaceView {
            id: sp.id.to_string(),
            name: sp.name,
            color: sp.color,
            pages: rows(&pages(Some(sp.id)), &places),
        })
        .collect();
    // Loose pages have no place row, or one with no space: give each a top-level spot.
    let loose_notes = pages(None);
    let loose_places: Vec<WikiPlace> = loose_notes
        .iter()
        .map(|n| {
            places
                .iter()
                .find(|p| p.page == *n.id.as_uuid())
                .copied()
                .unwrap_or(WikiPlace {
                    page: *n.id.as_uuid(),
                    space: None,
                    parent: None,
                    position: 0,
                })
        })
        .collect();
    let mut templates: Vec<&Note> = notes.iter().filter(|n| n.kind == NoteKind::Mop).collect();
    templates.sort_by_key(|n| n.title.to_lowercase());
    Ok(WikiView {
        spaces,
        loose: rows(&loose_notes, &loose_places),
        templates: templates
            .into_iter()
            .map(|n| WikiRowView {
                id: n.id.to_string(),
                title: n.title.clone(),
                depth: 0,
                template: true,
            })
            .collect(),
    })
}

async fn page_view(store: &SqliteStore, owner: OwnerId, id: NoteId) -> Res<WikiPageView> {
    let note = store.get_note(owner, id).await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let places = store.wiki_places(owner).await?;
    let here = places.iter().find(|p| p.page == *id.as_uuid()).copied();
    let notes = store.list_notes(owner, true).await?;
    let title_of: HashMap<Uuid, &str> = notes
        .iter()
        .map(|n| (*n.id.as_uuid(), n.title.as_str()))
        .collect();
    let step = |id: Uuid| WikiStepView {
        id: id.to_string(),
        title: title_of.get(&id).copied().unwrap_or_default().to_string(),
    };
    let mut trail = Vec::new();
    let mut seen = HashSet::new();
    let mut up = here.and_then(|p| p.parent);
    while let Some(p) = up {
        if !seen.insert(p) || !title_of.contains_key(&p) {
            break;
        }
        trail.push(step(p));
        up = places.iter().find(|w| w.page == p).and_then(|w| w.parent);
    }
    trail.reverse();
    let mut children: Vec<&WikiPlace> = places
        .iter()
        .filter(|p| p.parent == Some(*id.as_uuid()) && title_of.contains_key(&p.page))
        .collect();
    children.sort_by_key(|p| p.position);
    let space_id = here.and_then(|p| p.space);
    let space_name = match space_id {
        Some(sp) => store
            .wiki_spaces(owner)
            .await?
            .into_iter()
            .find(|x| x.id == sp)
            .map(|x| x.name),
        None => None,
    };
    let linked_from = notes
        .iter()
        .filter(|n| n.id != note.id && links_to(&n.body, &note.title))
        .map(|n| WikiLinkerView {
            id: n.id.to_string(),
            title: n.title.clone(),
            kind: n.kind,
        })
        .collect();
    let on_tickets = store
        .item_summaries(owner, None)
        .await?
        .into_iter()
        .filter(|i| i.note.as_deref().is_some_and(|t| links_to(t, &note.title)))
        .map(|i| WikiTicketView {
            number: i.number,
            done: i.done,
        })
        .collect();
    let versions = store.note_versions(owner, id).await?.len();
    Ok(WikiPageView {
        note: note_view(note, OffsetDateTime::now_utc(), after),
        space_id: space_id.map(|s| s.to_string()),
        space_name,
        trail,
        children: children.into_iter().map(|p| step(p.page)).collect(),
        linked_from,
        on_tickets,
        versions: u32::try_from(versions).unwrap_or(u32::MAX),
    })
}

/// Maps to `DataApi.wikiPage()`: one page with its trail, the pages under it, what links to
/// it and how many earlier versions it has.
#[tauri::command]
pub async fn wiki_page(s: State<'_, VaultSession>, id: String) -> Res<WikiPageView> {
    let (store, owner) = s.data().await?;
    page_view(&store, owner, id_arg(&id)?).await
}

/// Maps to `DataApi.wikiPageByTitle()`: the page a `[[link]]` points at (ignoring case),
/// or null when there's none yet.
#[tauri::command]
pub async fn wiki_page_by_title(s: State<'_, VaultSession>, title: String) -> Res<Option<String>> {
    let (store, owner) = s.data().await?;
    let title = title.trim();
    Ok(store
        .list_notes(owner, true)
        .await?
        .into_iter()
        .filter(in_wiki)
        .find(|n| n.title.trim().eq_ignore_ascii_case(title))
        .map(|n| n.id.to_string()))
}

/// Maps to `DataApi.wikiSpaceSave()`: make a space (null id) or rename one.
#[tauri::command]
pub async fn wiki_space_save(
    s: State<'_, VaultSession>,
    id: Option<String>,
    name: String,
    color: Option<String>,
) -> Res<String> {
    let (store, owner) = s.data().await?;
    let id: Option<Uuid> = id.as_deref().map(id_arg).transpose()?;
    let saved = store
        .save_wiki_space(owner, id, &name, color.as_deref().filter(|c| !c.is_empty()))
        .await?;
    Ok(saved.id.to_string())
}

/// Maps to `DataApi.wikiSpaceDelete()`: remove a space; its pages stay, in no space.
#[tauri::command]
pub async fn wiki_space_delete(s: State<'_, VaultSession>, id: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.delete_wiki_space(owner, id_arg(&id)?).await?)
}

/// Maps to `DataApi.wikiPlace()`: put a page in a space, under a page or at the top.
#[tauri::command]
pub async fn wiki_place(
    s: State<'_, VaultSession>,
    id: String,
    space_id: Option<String>,
    parent_id: Option<String>,
    position: Option<i64>,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let space: Option<Uuid> = space_id.as_deref().map(id_arg).transpose()?;
    let parent: Option<NoteId> = parent_id.as_deref().map(id_arg).transpose()?;
    Ok(store
        .place_wiki_page(owner, id_arg(&id)?, space, parent, position)
        .await?)
}

/// Maps to `DataApi.noteVersions()`: a page's earlier versions, newest first.
#[tauri::command]
pub async fn note_versions(s: State<'_, VaultSession>, id: String) -> Res<Vec<NoteVersionView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .note_versions(owner, id_arg(&id)?)
        .await?
        .into_iter()
        .map(|v| NoteVersionView {
            id: v.id.to_string(),
            title: v.title,
            body: v.body,
            at: format_timestamp(v.at),
        })
        .collect())
}

/// The Markdown files for a space (or the pages in no space): each page's path and text.
pub(crate) async fn space_files(
    store: &SqliteStore,
    owner: OwnerId,
    space: Option<Uuid>,
) -> Res<Vec<(String, String)>> {
    let notes: Vec<Note> = store
        .list_notes(owner, false)
        .await?
        .into_iter()
        .filter(|n| n.kind == NoteKind::Page)
        .collect();
    let places = store.wiki_places(owner).await?;
    let mine: Vec<Placed> = notes
        .iter()
        .filter_map(|n| {
            let p = places.iter().find(|p| p.page == *n.id.as_uuid());
            (p.and_then(|p| p.space) == space).then(|| Placed {
                id: *n.id.as_uuid(),
                parent: p.and_then(|p| p.parent),
                position: p.map_or(0, |p| p.position),
            })
        })
        .collect();
    let titles: HashMap<Uuid, String> = notes
        .iter()
        .map(|n| (*n.id.as_uuid(), n.title.clone()))
        .collect();
    let bodies: HashMap<Uuid, &str> = notes
        .iter()
        .map(|n| (*n.id.as_uuid(), n.body.as_str()))
        .collect();
    Ok(export_paths(&mine, &titles)
        .into_iter()
        .map(|(id, path)| {
            let text = format!(
                "# {}\n\n{}\n",
                titles.get(&id).map(String::as_str).unwrap_or(""),
                bodies.get(&id).copied().unwrap_or("").trim_end()
            );
            (path, text)
        })
        .collect())
}
