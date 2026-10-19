//! Commands for the Canvas (the Desk): the app's own records as cards, and where they sit.
//! Which records show and where unmoved cards go are rules in `shiftbook_core::canvas`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use shiftbook_core::canvas::{
    auto_links, auto_place, is_recent, timeline, Area, CardKind, LinkReason, LinkText, RECENT_DAYS,
};
use shiftbook_core::dates::format_date;
use shiftbook_core::ids::{ProjectId, TimeEntryId, TrayItemId};
use shiftbook_core::model::{CustomField, InboxItem, Note, TrayItem};
use shiftbook_storage::canvas::{pair, CanvasSpot, CardRef};
use shiftbook_storage::work::EntryRecord;
use shiftbook_storage::StorageError;
use tauri::State;
use time::{Date, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::api::{
    date_arg, entry_view, inbox_view, links_for, tray_item_view, EntryView, InboxView, TrayItemView,
};
use crate::notes::{note_view, NoteView};
use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

/// A Done ticket on the Canvas, with the day it belongs to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayCardView {
    item: TrayItemView,
    date: String,
}

/// One card: where it sits and the record it shows (exactly one of the four is set).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    kind: CardKind,
    id: String,
    x: i32,
    y: i32,
    z: i64,
    /// The owner moved it (false: placed automatically this time).
    placed: bool,
    /// Its spot when the Desk is laid out as a timeline.
    timeline_x: i32,
    timeline_y: i32,
    entry: Option<EntryView>,
    tray: Option<TrayCardView>,
    note: Option<NoteView>,
    inbox: Option<InboxView>,
}

/// A project's area.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaView {
    project_id: ProjectId,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

/// One end of a link.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRefView {
    kind: CardKind,
    id: String,
}

/// A line between two cards: automatic (with its reason) or drawn by hand.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    a: CardRefView,
    b: CardRefView,
    reason: Option<LinkReason>,
    drawn: bool,
}

/// A day on the timeline and where its column starts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AxisDay {
    date: String,
    x: i32,
}

/// Everything on the Desk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeskView {
    /// Bottom first.
    cards: Vec<CardView>,
    areas: Vec<AreaView>,
    links: Vec<LinkView>,
    /// The timeline's days, oldest first.
    axis: Vec<AxisDay>,
}

/// A record that can be a card, before it gets a spot.
struct Pending {
    kind: CardKind,
    id: Uuid,
    /// For newest-first auto-placing and the timeline order within a day.
    created: OffsetDateTime,
    /// Its day on the timeline.
    date: Date,
    /// What it says, for links.
    link: LinkText,
    card: CardView,
}

fn blank(kind: CardKind, id: Uuid) -> CardView {
    CardView {
        kind,
        id: id.to_string(),
        x: 0,
        y: 0,
        z: 0,
        placed: false,
        timeline_x: 0,
        timeline_y: 0,
        entry: None,
        tray: None,
        note: None,
        inbox: None,
    }
}

fn entry_card(rec: EntryRecord, fields: &[CustomField]) -> Pending {
    let id = *rec.entry.id.as_uuid();
    let link = LinkText {
        tickets: rec
            .tickets
            .iter()
            .map(|t| t.ticket_number.clone())
            .collect(),
        text: format!(
            "{} {}",
            rec.entry.description,
            rec.entry.next_step.as_deref().unwrap_or("")
        ),
    };
    let (created, date) = (rec.entry.created_at, rec.entry.date);
    let mut card = blank(CardKind::Entry, id);
    card.entry = Some(entry_view(rec, fields));
    Pending {
        kind: CardKind::Entry,
        id,
        created,
        date,
        link,
        card,
    }
}

fn tray_card(item: TrayItem, fields: &[CustomField]) -> Pending {
    let id = *item.id.as_uuid();
    let link = LinkText {
        tickets: vec![item.ticket_number.clone()],
        text: item.note.clone().unwrap_or_default(),
    };
    let (created, date) = (item.created_at, item.date);
    let mut card = blank(CardKind::Tray, id);
    card.tray = Some(TrayCardView {
        item: tray_item_view(item, fields),
        date: format_date(date),
    });
    Pending {
        kind: CardKind::Tray,
        id,
        created,
        date,
        link,
        card,
    }
}

fn note_card(note: Note, now: OffsetDateTime, after: u32) -> Pending {
    let id = *note.id.as_uuid();
    let link = LinkText {
        tickets: Vec::new(),
        text: format!("{} {}", note.title, note.body),
    };
    let created = note.created_at;
    let mut card = blank(CardKind::Note, id);
    card.note = Some(note_view(note, now, after));
    Pending {
        kind: CardKind::Note,
        id,
        created,
        date: created.date(),
        link,
        card,
    }
}

fn inbox_card(scrap: InboxItem) -> Pending {
    let id = *scrap.id.as_uuid();
    let link = LinkText {
        tickets: Vec::new(),
        text: scrap.text.clone(),
    };
    let created = scrap.created_at;
    let mut card = blank(CardKind::Inbox, id);
    card.inbox = Some(inbox_view(scrap));
    Pending {
        kind: CardKind::Inbox,
        id,
        created,
        date: created.date(),
        link,
        card,
    }
}

fn skip_missing<T>(found: Result<T, StorageError>) -> Res<Option<T>> {
    match found {
        Ok(v) => Ok(Some(v)),
        Err(StorageError::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn card_ref(kind: CardKind, id: Uuid) -> CardRefView {
    CardRefView {
        kind,
        id: id.to_string(),
    }
}

/// Maps to `DataApi.canvas()`: everything on the Desk. Entries and Done tickets from the last
/// week show on their own; older ones only if the owner moved them. Live notes and Inbox
/// scraps always show. The day's "Tickets" roll-up entry doesn't: its tickets are cards.
#[tauri::command]
pub async fn canvas(s: State<'_, VaultSession>, today: String) -> Res<DeskView> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    let from = today - Duration::days(RECENT_DAYS - 1);
    let fields = links_for(&store, owner).await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let now = OffsetDateTime::now_utc();
    let spots: HashMap<(CardKind, Uuid), CanvasSpot> = store
        .canvas_spots(owner)
        .await?
        .into_iter()
        .map(|s| ((s.kind, s.record_id), s))
        .collect();

    let mut all: Vec<Pending> = Vec::new();
    for rec in store.list_entries(owner, from, today).await? {
        if !rec.is_tray && is_recent(rec.entry.date, today) {
            all.push(entry_card(rec, &fields));
        }
    }
    for item in store.tray_between(owner, from, today).await? {
        all.push(tray_card(item, &fields));
    }
    for note in store.list_notes(owner, false).await? {
        all.push(note_card(note, now, after));
    }
    for scrap in store.list_inbox(owner).await? {
        all.push(inbox_card(scrap));
    }

    // Older entries and Done tickets the owner moved stay where they put them.
    let shown: HashSet<(CardKind, Uuid)> = all.iter().map(|p| (p.kind, p.id)).collect();
    for spot in spots.values() {
        if shown.contains(&(spot.kind, spot.record_id)) {
            continue;
        }
        match spot.kind {
            CardKind::Entry => {
                let id = TimeEntryId::from_uuid(spot.record_id);
                if let Some(rec) = skip_missing(store.get_entry(owner, id).await)? {
                    if !rec.is_tray {
                        all.push(entry_card(rec, &fields));
                    }
                }
            }
            CardKind::Tray => {
                let id = TrayItemId::from_uuid(spot.record_id);
                if let Some(item) = store.tray_item(owner, id).await? {
                    all.push(tray_card(item, &fields));
                }
            }
            // Notes and scraps that aren't live (archived or removed) stay off the Desk.
            CardKind::Note | CardKind::Inbox => {}
        }
    }

    // Moved cards keep their spots; the rest are placed newest first around them.
    let mut placed = Vec::new();
    let mut loose = Vec::new();
    for mut p in all {
        if let Some(spot) = spots.get(&(p.kind, p.id)) {
            p.card.x = spot.x;
            p.card.y = spot.y;
            p.card.z = spot.z;
            p.card.placed = true;
            placed.push(p);
        } else {
            loose.push(p);
        }
    }
    loose.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
    let taken: Vec<(i32, i32)> = placed.iter().map(|p| (p.card.x, p.card.y)).collect();
    let spots_for_loose = auto_place(loose.len(), &taken);
    for (mut p, (x, y)) in loose.into_iter().zip(spots_for_loose) {
        p.card.x = x;
        p.card.y = y;
        placed.push(p);
    }
    placed.sort_by_key(|p| p.card.z);

    // The timeline: by day, and within a day in the order things were made.
    let mut order: Vec<usize> = (0..placed.len()).collect();
    order.sort_by(|&a, &b| {
        (placed[a].date, placed[a].created).cmp(&(placed[b].date, placed[b].created))
    });
    let dates: Vec<Date> = order.iter().map(|&i| placed[i].date).collect();
    let (line, days) = timeline(&dates);
    for (&i, (x, y)) in order.iter().zip(line) {
        placed[i].card.timeline_x = x;
        placed[i].card.timeline_y = y;
    }

    // Links: automatic ones the owner didn't hide, plus the ones they drew.
    let changed = store.canvas_links(owner).await?;
    let hidden: HashSet<(CardRef, CardRef)> = changed
        .iter()
        .filter(|l| !l.drawn)
        .map(|l| (l.a, l.b))
        .collect();
    let texts: Vec<LinkText> = placed.iter().map(|p| p.link.clone()).collect();
    let mut links: Vec<LinkView> = auto_links(&texts)
        .into_iter()
        .filter(|(a, b, _)| {
            !hidden.contains(&pair(
                (placed[*a].kind, placed[*a].id),
                (placed[*b].kind, placed[*b].id),
            ))
        })
        .map(|(a, b, reason)| LinkView {
            a: card_ref(placed[a].kind, placed[a].id),
            b: card_ref(placed[b].kind, placed[b].id),
            reason: Some(reason),
            drawn: false,
        })
        .collect();
    let on_desk: HashSet<CardRef> = placed.iter().map(|p| (p.kind, p.id)).collect();
    for l in changed.iter().filter(|l| l.drawn) {
        if on_desk.contains(&l.a) && on_desk.contains(&l.b) {
            links.push(LinkView {
                a: card_ref(l.a.0, l.a.1),
                b: card_ref(l.b.0, l.b.1),
                reason: None,
                drawn: true,
            });
        }
    }

    let areas = store
        .canvas_areas(owner)
        .await?
        .into_iter()
        .map(|a| AreaView {
            project_id: a.project,
            x: a.area.x,
            y: a.area.y,
            w: a.area.w,
            h: a.area.h,
        })
        .collect();
    Ok(DeskView {
        cards: placed.into_iter().map(|p| p.card).collect(),
        areas,
        links,
        axis: days
            .into_iter()
            .map(|(d, x)| AxisDay {
                date: format_date(d),
                x,
            })
            .collect(),
    })
}

/// Maps to `DataApi.canvasArea()`: put a project's area here (making it the first time).
#[tauri::command]
pub async fn canvas_area(
    s: State<'_, VaultSession>,
    project_id: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let project: ProjectId = project_id.parse().map_err(|_| CommandError::InvalidInput)?;
    store.set_area(owner, project, Area { x, y, w, h }).await?;
    Ok(())
}

/// Maps to `DataApi.canvasAreaRemove()`: take a project's area off the Desk.
#[tauri::command]
pub async fn canvas_area_remove(s: State<'_, VaultSession>, project_id: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    let project: ProjectId = project_id.parse().map_err(|_| CommandError::InvalidInput)?;
    store.remove_area(owner, project).await?;
    Ok(())
}

fn ref_arg(kind: &str, id: &str) -> Res<CardRef> {
    Ok((
        kind.parse().map_err(|_| CommandError::InvalidInput)?,
        Uuid::parse_str(id).map_err(|_| CommandError::InvalidInput)?,
    ))
}

/// Maps to `DataApi.canvasLink()`: draw a link (`drawn`) or hide an automatic one.
#[tauri::command]
pub async fn canvas_link(
    s: State<'_, VaultSession>,
    a_kind: String,
    a_id: String,
    b_kind: String,
    b_id: String,
    drawn: bool,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    store
        .set_link(
            owner,
            ref_arg(&a_kind, &a_id)?,
            ref_arg(&b_kind, &b_id)?,
            drawn,
        )
        .await?;
    Ok(())
}

/// Maps to `DataApi.canvasLinkReset()`: undo a drawn or hidden link.
#[tauri::command]
pub async fn canvas_link_reset(
    s: State<'_, VaultSession>,
    a_kind: String,
    a_id: String,
    b_kind: String,
    b_id: String,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    store
        .reset_link(owner, ref_arg(&a_kind, &a_id)?, ref_arg(&b_kind, &b_id)?)
        .await?;
    Ok(())
}

/// Maps to `DataApi.canvasMove()`: put a card where the owner dropped it, on top.
#[tauri::command]
pub async fn canvas_move(
    s: State<'_, VaultSession>,
    kind: String,
    id: String,
    x: i32,
    y: i32,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let kind: CardKind = kind.parse().map_err(|_| CommandError::InvalidInput)?;
    let id = Uuid::parse_str(&id).map_err(|_| CommandError::InvalidInput)?;
    store.move_card(owner, kind, id, x, y).await?;
    Ok(())
}
