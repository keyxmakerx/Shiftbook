//! Commands for the Canvas (the Desk): the app's own records as cards, and where they sit.
//! Which records show and where unmoved cards go are rules in `shiftbook_core::canvas`.

use std::collections::HashMap;

use serde::Serialize;
use shiftbook_core::canvas::{auto_place, is_recent, CardKind, RECENT_DAYS};
use shiftbook_core::ids::{TimeEntryId, TrayItemId};
use shiftbook_storage::canvas::CanvasSpot;
use shiftbook_storage::StorageError;
use tauri::State;
use time::{Duration, OffsetDateTime};
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
    entry: Option<EntryView>,
    tray: Option<TrayCardView>,
    note: Option<NoteView>,
    inbox: Option<InboxView>,
}

/// A record that can be a card, before it gets a spot.
struct Pending {
    kind: CardKind,
    id: Uuid,
    /// For newest-first auto-placing.
    created: OffsetDateTime,
    card: CardView,
}

fn pending(kind: CardKind, id: Uuid, created: OffsetDateTime) -> impl FnOnce(CardView) -> Pending {
    move |card| Pending {
        kind,
        id,
        created,
        card,
    }
}

fn blank(kind: CardKind, id: Uuid) -> CardView {
    CardView {
        kind,
        id: id.to_string(),
        x: 0,
        y: 0,
        z: 0,
        placed: false,
        entry: None,
        tray: None,
        note: None,
        inbox: None,
    }
}

fn skip_missing<T>(found: Result<T, StorageError>) -> Res<Option<T>> {
    match found {
        Ok(v) => Ok(Some(v)),
        Err(StorageError::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Maps to `DataApi.canvas()`: every card on the Desk. Entries and Done tickets from the last
/// week show on their own; older ones only if the owner moved them. Live notes and Inbox
/// scraps always show. The day's "Tickets" roll-up entry doesn't: its tickets are cards.
#[tauri::command]
pub async fn canvas(s: State<'_, VaultSession>, today: String) -> Res<Vec<CardView>> {
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
        if rec.is_tray || !is_recent(rec.entry.date, today) {
            continue;
        }
        let (id, created) = (*rec.entry.id.as_uuid(), rec.entry.created_at);
        let mut card = blank(CardKind::Entry, id);
        card.entry = Some(entry_view(rec, &fields));
        all.push(pending(CardKind::Entry, id, created)(card));
    }
    for item in store.tray_between(owner, from, today).await? {
        let (id, created, date) = (*item.id.as_uuid(), item.created_at, item.date);
        let mut card = blank(CardKind::Tray, id);
        card.tray = Some(TrayCardView {
            item: tray_item_view(item, &fields),
            date: shiftbook_core::dates::format_date(date),
        });
        all.push(pending(CardKind::Tray, id, created)(card));
    }
    for note in store.list_notes(owner, false).await? {
        let (id, created) = (*note.id.as_uuid(), note.created_at);
        let mut card = blank(CardKind::Note, id);
        card.note = Some(note_view(note, now, after));
        all.push(pending(CardKind::Note, id, created)(card));
    }
    for scrap in store.list_inbox(owner).await? {
        let (id, created) = (*scrap.id.as_uuid(), scrap.created_at);
        let mut card = blank(CardKind::Inbox, id);
        card.inbox = Some(inbox_view(scrap));
        all.push(pending(CardKind::Inbox, id, created)(card));
    }

    // Older entries and Done tickets the owner moved stay where they put them.
    let shown: std::collections::HashSet<(CardKind, Uuid)> =
        all.iter().map(|p| (p.kind, p.id)).collect();
    for spot in spots.values() {
        if shown.contains(&(spot.kind, spot.record_id)) {
            continue;
        }
        match spot.kind {
            CardKind::Entry => {
                let id = TimeEntryId::from_uuid(spot.record_id);
                if let Some(rec) = skip_missing(store.get_entry(owner, id).await)? {
                    if rec.is_tray {
                        continue;
                    }
                    let created = rec.entry.created_at;
                    let mut card = blank(CardKind::Entry, spot.record_id);
                    card.entry = Some(entry_view(rec, &fields));
                    all.push(pending(CardKind::Entry, spot.record_id, created)(card));
                }
            }
            CardKind::Tray => {
                let id = TrayItemId::from_uuid(spot.record_id);
                if let Some(item) = store.tray_item(owner, id).await? {
                    let (created, date) = (item.created_at, item.date);
                    let mut card = blank(CardKind::Tray, spot.record_id);
                    card.tray = Some(TrayCardView {
                        item: tray_item_view(item, &fields),
                        date: shiftbook_core::dates::format_date(date),
                    });
                    all.push(pending(CardKind::Tray, spot.record_id, created)(card));
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
            placed.push(p.card);
        } else {
            loose.push(p);
        }
    }
    loose.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
    let taken: Vec<(i32, i32)> = placed.iter().map(|c| (c.x, c.y)).collect();
    let spots_for_loose = auto_place(loose.len(), &taken);
    for (p, (x, y)) in loose.into_iter().zip(spots_for_loose) {
        let mut card = p.card;
        card.x = x;
        card.y = y;
        placed.push(card);
    }
    placed.sort_by_key(|c| c.z);
    Ok(placed)
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
