//! Commands for pins (see `shiftbook_core::pins`): chips on the bottom bar for entries,
//! tickets and notes kept in view, with an optional day to chase them.

use serde::Serialize;
use shiftbook_core::canvas::CardKind;
use shiftbook_core::dates::{format_date, parse_date};
use shiftbook_core::ids::{NoteId, OwnerId, TimeEntryId, TrayItemId};
use shiftbook_core::pins::{is_due, pin_order, Pin};
use shiftbook_storage::SqliteStore;
use tauri::State;
use uuid::Uuid;

use crate::session::{CommandError, VaultSession};
use shiftbook_app::desk::skip_missing;

type Res<T> = Result<T, CommandError>;

/// One chip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinView {
    kind: CardKind,
    id: String,
    /// What the chip says.
    title: String,
    /// The day to chase it, if any.
    chase_on: Option<String>,
    /// Its chase date has come: the chip is orange and the bell lists it.
    due: bool,
}

/// The line a pinned record shows, or `None` when the record is gone (removed or deleted).
async fn title_of(store: &SqliteStore, owner: OwnerId, pin: &Pin) -> Res<Option<String>> {
    let id = pin.record_id;
    Ok(match pin.kind {
        CardKind::Entry => skip_missing(store.get_entry(owner, TimeEntryId::from_uuid(id)).await)?
            .filter(|r| r.entry.is_live())
            .map(|r| {
                if r.entry.description.trim().is_empty() {
                    "Untitled entry".to_string()
                } else {
                    r.entry.description
                }
            }),
        CardKind::Tray => store
            .tray_item(owner, TrayItemId::from_uuid(id))
            .await?
            .map(|t| match t.note.as_deref().and_then(|n| n.lines().next()) {
                Some(line) if !line.trim().is_empty() => format!("{} · {line}", t.ticket_number),
                _ => t.ticket_number,
            }),
        CardKind::Note => skip_missing(store.get_note(owner, NoteId::from_uuid(id)).await)?
            .filter(|n| n.deleted_at.is_none())
            .map(|n| n.title),
        CardKind::Inbox => None,
    })
}

/// Every pin whose record is still there, in chip order, with its title.
pub(crate) async fn pinned(
    store: &SqliteStore,
    owner: OwnerId,
    today: time::Date,
) -> Res<Vec<(Pin, String)>> {
    let pins = store.pins(owner).await?;
    let mut out = Vec::new();
    for i in pin_order(&pins, today) {
        let pin = pins[i];
        if let Some(title) = title_of(store, owner, &pin).await? {
            out.push((pin, title));
        }
    }
    Ok(out)
}

fn card_args(kind: &str, id: &str) -> Res<(CardKind, Uuid)> {
    Ok((
        kind.parse().map_err(|_| CommandError::InvalidInput)?,
        Uuid::parse_str(id).map_err(|_| CommandError::InvalidInput)?,
    ))
}

/// Maps to `DataApi.pins()`: the chips for the bottom bar, ones due to chase first.
#[tauri::command]
pub async fn pins(s: State<'_, VaultSession>, today: String) -> Res<Vec<PinView>> {
    let (store, owner) = s.data().await?;
    let today = parse_date(&today).map_err(|_| CommandError::InvalidInput)?;
    Ok(pinned(&store, owner, today)
        .await?
        .into_iter()
        .map(|(p, title)| PinView {
            kind: p.kind,
            id: p.record_id.to_string(),
            title,
            chase_on: p.chase_on.map(format_date),
            due: is_due(p.chase_on, today),
        })
        .collect())
}

/// Maps to `DataApi.pin()`: pin a record, or change its chase date (null clears it).
#[tauri::command]
pub async fn pin(
    s: State<'_, VaultSession>,
    kind: String,
    id: String,
    chase_on: Option<String>,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let (kind, id) = card_args(&kind, &id)?;
    let chase = chase_on
        .as_deref()
        .filter(|d| !d.trim().is_empty())
        .map(|d| parse_date(d).map_err(|_| CommandError::InvalidInput))
        .transpose()?;
    Ok(store.pin(owner, kind, id, chase).await?)
}

/// Maps to `DataApi.unpin()`: take a record's chip off the bottom bar.
#[tauri::command]
pub async fn unpin(s: State<'_, VaultSession>, kind: String, id: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    let (kind, id) = card_args(&kind, &id)?;
    Ok(store.unpin(owner, kind, id).await?)
}
