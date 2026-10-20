//! Commands for the Canvas (the Desk): the app's own records as cards, and where they sit.
//! Which records show and where unmoved cards go are rules in `shiftbook_core::canvas`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use shiftbook_core::canvas::{
    area_at, auto_links, auto_place, box_status, is_recent, tidy_groups, timeline, Area, BoxStatus,
    CardKind, Drawer, LinkReason, LinkText, MemberState, RECENT_DAYS,
};
use shiftbook_core::dates::format_date;
use shiftbook_core::ids::{InboxItemId, NoteId, OwnerId, ProjectId, TimeEntryId, TrayItemId};
use shiftbook_core::model::{CustomField, InboxItem, Note, TrayItem};
use shiftbook_storage::canvas::{pair, CanvasSpot, CardRef};
use shiftbook_storage::work::EntryRecord;
use shiftbook_storage::{SqliteStore, StorageError};
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
    /// The box it sits in ("project:<id>" or "label:<id>"), if any.
    in_box: Option<String>,
    /// The pile it's in, if any (it shows at the pile's spot).
    stack: Option<String>,
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
    collapsed: bool,
    /// Cards inside it.
    count: usize,
    status: BoxStatus,
}

/// A plain labelled box.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelView {
    id: String,
    label: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    collapsed: bool,
    /// Cards inside it.
    count: usize,
    status: BoxStatus,
}

/// A pile of cards.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackView {
    id: String,
    name: Option<String>,
    x: i32,
    y: i32,
    count: usize,
}

/// A drawer in the cabinet: a project's or a folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawerView {
    /// `project:<id>` or `folder:<id>`.
    key: String,
    name: String,
    /// The project's colour (folders have none).
    color: Option<String>,
    /// Cards filed in it.
    count: usize,
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
    labels: Vec<LabelView>,
    stacks: Vec<StackView>,
    drawers: Vec<DrawerView>,
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
    /// A ticket still open.
    open: bool,
    /// Its project, for Tidy into stacks.
    project: Option<Uuid>,
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
        in_box: None,
        stack: None,
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
    let project = match rec.entry.target {
        shiftbook_core::model::EntryTarget::Project(p) => Some(*p.as_uuid()),
        _ => None,
    };
    let mut card = blank(CardKind::Entry, id);
    card.entry = Some(entry_view(rec, fields));
    Pending {
        kind: CardKind::Entry,
        id,
        created,
        date,
        link,
        open: false,
        project,
        card,
    }
}

fn tray_card(item: TrayItem, fields: &[CustomField]) -> Pending {
    let id = *item.id.as_uuid();
    let link = LinkText {
        tickets: vec![item.ticket_number.clone()],
        text: item.note.clone().unwrap_or_default(),
    };
    let (created, date, open) = (item.created_at, item.date, !item.done);
    let project = item.project_id.map(|p| *p.as_uuid());
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
        open,
        project,
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
    let project = note.project_id.map(|p| *p.as_uuid());
    let mut card = blank(CardKind::Note, id);
    card.note = Some(note_view(note, now, after));
    Pending {
        kind: CardKind::Note,
        id,
        created,
        date: created.date(),
        link,
        open: false,
        project,
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
        open: false,
        project: None,
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

/// The card for a record the layout knows about, if the record is still there.
async fn card_for(
    store: &SqliteStore,
    owner: OwnerId,
    kind: CardKind,
    id: Uuid,
    fields: &[CustomField],
    now: OffsetDateTime,
    after: u32,
) -> Res<Option<Pending>> {
    Ok(match kind {
        CardKind::Entry => skip_missing(store.get_entry(owner, TimeEntryId::from_uuid(id)).await)?
            .filter(|rec| !rec.is_tray && rec.entry.is_live())
            .map(|rec| entry_card(rec, fields)),
        CardKind::Tray => store
            .tray_item(owner, TrayItemId::from_uuid(id))
            .await?
            .map(|i| tray_card(i, fields)),
        CardKind::Note => skip_missing(store.get_note(owner, NoteId::from_uuid(id)).await)?
            .filter(|n| n.deleted_at.is_none())
            .map(|n| note_card(n, now, after)),
        CardKind::Inbox => {
            let wanted = InboxItemId::from_uuid(id);
            store
                .list_inbox(owner)
                .await?
                .into_iter()
                .find(|i| i.id == wanted)
                .map(inbox_card)
        }
    })
}

/// The drawers that exist: one per project (archived ones only while they hold cards, so
/// nothing filed gets lost) and each live folder, with the cards filed in each.
async fn drawers(
    store: &SqliteStore,
    owner: OwnerId,
    spots: &HashMap<(CardKind, Uuid), CanvasSpot>,
    fields: &[CustomField],
    now: OffsetDateTime,
    after: u32,
) -> Res<(Vec<DrawerView>, HashMap<String, Vec<Pending>>)> {
    let projects = store.list_projects(owner).await?;
    let folders = store.canvas_folders(owner).await?;
    let mut filed: HashMap<String, Vec<Pending>> = HashMap::new();
    for spot in spots.values() {
        let Some(drawer) = spot.drawer else { continue };
        let exists = match drawer {
            Drawer::Project(id) => projects.iter().any(|p| *p.id.as_uuid() == id),
            Drawer::Folder(id) => folders.iter().any(|f| f.id == id),
        };
        if !exists {
            continue;
        }
        if let Some(p) =
            card_for(store, owner, spot.kind, spot.record_id, fields, now, after).await?
        {
            filed.entry(drawer.key()).or_default().push(p);
        }
    }
    let count = |key: &str| filed.get(key).map_or(0, Vec::len);
    let mut views: Vec<DrawerView> = projects
        .iter()
        .map(|p| (Drawer::Project(*p.id.as_uuid()).key(), p))
        .filter(|(key, p)| !p.archived || count(key) > 0)
        .map(|(key, p)| DrawerView {
            count: count(&key),
            key,
            name: p.name.clone(),
            color: p.color.clone(),
        })
        .collect();
    views.extend(folders.into_iter().map(|f| {
        let key = Drawer::Folder(f.id).key();
        DrawerView {
            count: count(&key),
            key,
            name: f.name,
            color: None,
        }
    }));
    Ok((views, filed))
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
    Ok(desk(&store, owner, date_arg(&today)?).await?.0)
}

/// Everything on the Desk, with each card's project (for Tidy into stacks).
async fn desk(
    store: &SqliteStore,
    owner: OwnerId,
    today: Date,
) -> Res<(DeskView, Vec<Option<Uuid>>)> {
    let from = today - Duration::days(RECENT_DAYS - 1);
    let fields = links_for(store, owner).await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let now = OffsetDateTime::now_utc();
    let spots: HashMap<(CardKind, Uuid), CanvasSpot> = store
        .canvas_spots(owner)
        .await?
        .into_iter()
        .map(|s| ((s.kind, s.record_id), s))
        .collect();

    // Filed cards are in their drawers, not on the Desk.
    let (drawer_views, filed) = drawers(store, owner, &spots, &fields, now, after).await?;
    let filed_ids: HashSet<(CardKind, Uuid)> =
        filed.values().flatten().map(|p| (p.kind, p.id)).collect();

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
    all.retain(|p| !filed_ids.contains(&(p.kind, p.id)));

    // Older entries and Done tickets the owner moved stay where they put them.
    let shown: HashSet<(CardKind, Uuid)> = all.iter().map(|p| (p.kind, p.id)).collect();
    for spot in spots.values() {
        if shown.contains(&(spot.kind, spot.record_id))
            || filed_ids.contains(&(spot.kind, spot.record_id))
        {
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

    // Piles: a card in a live pile shows at the pile's spot. A pile with fewer than two of
    // its cards on the Desk shows as loose cards.
    let piles: HashMap<Uuid, shiftbook_storage::piles::CanvasStack> = store
        .canvas_stacks(owner)
        .await?
        .into_iter()
        .map(|st| (st.id, st))
        .collect();
    let mut pile_size: HashMap<Uuid, usize> = HashMap::new();
    for p in &all {
        if let Some(st) = spots.get(&(p.kind, p.id)).and_then(|sp| sp.stack) {
            if piles.contains_key(&st) {
                *pile_size.entry(st).or_default() += 1;
            }
        }
    }
    let pile_of = |kind: CardKind, id: Uuid| {
        spots
            .get(&(kind, id))
            .and_then(|sp| sp.stack)
            .filter(|st| pile_size.get(st).copied().unwrap_or(0) >= 2)
            .and_then(|st| piles.get(&st))
    };

    // Moved cards keep their spots; the rest are placed newest first around them.
    let mut placed = Vec::new();
    let mut loose = Vec::new();
    for mut p in all {
        if let Some(spot) = spots.get(&(p.kind, p.id)) {
            p.card.x = spot.x;
            p.card.y = spot.y;
            p.card.z = spot.z;
            p.card.placed = true;
            if let Some(pile) = pile_of(p.kind, p.id) {
                p.card.x = pile.x;
                p.card.y = pile.y;
                p.card.stack = Some(pile.id.to_string());
            }
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

    // Boxes: project areas, then labelled boxes (a card in two counts for the one on top).
    let project_areas = store.canvas_areas(owner).await?;
    let label_boxes = store.canvas_labels(owner).await?;
    let boxes: Vec<Area> = project_areas
        .iter()
        .map(|a| a.area)
        .chain(label_boxes.iter().map(|l| l.area))
        .collect();
    let keys: Vec<String> = project_areas
        .iter()
        .map(|a| format!("project:{}", a.project))
        .chain(label_boxes.iter().map(|l| format!("label:{}", l.id)))
        .collect();
    let brought: HashSet<(CardKind, Uuid)> = store
        .next_up_all(owner)
        .await?
        .into_iter()
        .filter(|i| i.done_at.is_none())
        .filter_map(|i| i.about)
        .collect();
    let mut members: Vec<Vec<MemberState>> = vec![Vec::new(); boxes.len()];
    for p in placed.iter_mut() {
        if let Some(b) = area_at(&boxes, (p.card.x, p.card.y)) {
            members[b].push(MemberState {
                open: p.open,
                brought_up: brought.contains(&(p.kind, p.id)),
                today: p.date == today,
            });
            p.card.in_box = Some(keys[b].clone());
        }
    }
    let areas = project_areas
        .iter()
        .enumerate()
        .map(|(i, a)| AreaView {
            project_id: a.project,
            x: a.area.x,
            y: a.area.y,
            w: a.area.w,
            h: a.area.h,
            collapsed: a.collapsed,
            count: members[i].len(),
            status: box_status(&members[i]),
        })
        .collect();
    let first_label = project_areas.len();
    let labels = label_boxes
        .into_iter()
        .enumerate()
        .map(|(i, l)| LabelView {
            id: l.id.to_string(),
            label: l.label,
            x: l.area.x,
            y: l.area.y,
            w: l.area.w,
            h: l.area.h,
            collapsed: l.collapsed,
            count: members[first_label + i].len(),
            status: box_status(&members[first_label + i]),
        })
        .collect();
    let projects: Vec<Option<Uuid>> = placed.iter().map(|p| p.project).collect();
    let stacks = piles
        .values()
        .filter(|st| pile_size.get(&st.id).copied().unwrap_or(0) >= 2)
        .map(|st| StackView {
            id: st.id.to_string(),
            name: st.name.clone(),
            x: st.x,
            y: st.y,
            count: pile_size[&st.id],
        })
        .collect();
    let view = DeskView {
        cards: placed.into_iter().map(|p| p.card).collect(),
        areas,
        labels,
        stacks,
        drawers: drawer_views,
        links,
        axis: days
            .into_iter()
            .map(|(d, x)| AxisDay {
                date: format_date(d),
                x,
            })
            .collect(),
    };
    Ok((view, projects))
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

/// Maps to `DataApi.canvasAreaCollapse()`: fold a project's area up to a count, or open it.
#[tauri::command]
pub async fn canvas_area_collapse(
    s: State<'_, VaultSession>,
    project_id: String,
    collapsed: bool,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let project: ProjectId = project_id.parse().map_err(|_| CommandError::InvalidInput)?;
    Ok(store.set_area_collapsed(owner, project, collapsed).await?)
}

/// Maps to `DataApi.canvasLabelSave()`: make a labelled box (`id` null) or change one.
/// Returns its id.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn canvas_label_save(
    s: State<'_, VaultSession>,
    id: Option<String>,
    label: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    collapsed: bool,
) -> Res<String> {
    let (store, owner) = s.data().await?;
    let id = id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| CommandError::InvalidInput)?;
    let saved = store
        .save_label(owner, id, &label, Area { x, y, w, h }, collapsed)
        .await?;
    Ok(saved.id.to_string())
}

/// Maps to `DataApi.canvasLabelRemove()`: take a labelled box off the Desk (its cards stay),
/// or with `removed` false put it back (Undo).
#[tauri::command]
pub async fn canvas_label_remove(s: State<'_, VaultSession>, id: String, removed: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    let id = Uuid::parse_str(&id).map_err(|_| CommandError::InvalidInput)?;
    Ok(store.set_label_deleted(owner, id, removed).await?)
}

fn drawer_arg(text: &str) -> Res<Drawer> {
    Drawer::parse(text).ok_or(CommandError::InvalidInput)
}

fn uuid_arg(text: &str) -> Res<Uuid> {
    Uuid::parse_str(text).map_err(|_| CommandError::InvalidInput)
}

/// Maps to `DataApi.canvasStack()`: drop a card on another one; they pile up (or it joins
/// that card's pile). `onto_x`/`onto_y` is where the card under it shows. Returns the pile.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn canvas_stack(
    s: State<'_, VaultSession>,
    kind: String,
    id: String,
    onto_kind: String,
    onto_id: String,
    onto_x: i32,
    onto_y: i32,
) -> Res<String> {
    let (store, owner) = s.data().await?;
    let pile = store
        .stack_cards(
            owner,
            ref_arg(&kind, &id)?,
            ref_arg(&onto_kind, &onto_id)?,
            (onto_x, onto_y),
        )
        .await?;
    Ok(pile.to_string())
}

/// Maps to `DataApi.canvasUnstack()`: take a card off its pile, to `x`, `y`.
#[tauri::command]
pub async fn canvas_unstack(
    s: State<'_, VaultSession>,
    kind: String,
    id: String,
    x: i32,
    y: i32,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .unstack_card(owner, ref_arg(&kind, &id)?, x, y)
        .await?)
}

/// Maps to `DataApi.canvasStackMove()`.
#[tauri::command]
pub async fn canvas_stack_move(s: State<'_, VaultSession>, id: String, x: i32, y: i32) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.move_stack(owner, uuid_arg(&id)?, x, y).await?)
}

/// Maps to `DataApi.canvasStackRename()`: blank shows the count again.
#[tauri::command]
pub async fn canvas_stack_rename(s: State<'_, VaultSession>, id: String, name: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.rename_stack(owner, uuid_arg(&id)?, &name).await?)
}

/// Maps to `DataApi.canvasStackDissolve()`: take a pile apart.
#[tauri::command]
pub async fn canvas_stack_dissolve(s: State<'_, VaultSession>, id: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.dissolve_stack(owner, uuid_arg(&id)?).await?)
}

/// Maps to `DataApi.canvasFile()`: file a card in a drawer, off the Desk.
#[tauri::command]
pub async fn canvas_file(
    s: State<'_, VaultSession>,
    kind: String,
    id: String,
    drawer: String,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .file_card(owner, ref_arg(&kind, &id)?, drawer_arg(&drawer)?)
        .await?)
}

/// Maps to `DataApi.canvasFileStack()`: file a whole pile. Returns the cards filed.
#[tauri::command]
pub async fn canvas_file_stack(
    s: State<'_, VaultSession>,
    id: String,
    drawer: String,
) -> Res<Vec<CardRefView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .file_stack(owner, uuid_arg(&id)?, drawer_arg(&drawer)?)
        .await?
        .into_iter()
        .map(|(k, i)| card_ref(k, i))
        .collect())
}

/// Maps to `DataApi.canvasUnfile()`: back on the Desk at `x`, `y`.
#[tauri::command]
pub async fn canvas_unfile(
    s: State<'_, VaultSession>,
    kind: String,
    id: String,
    x: i32,
    y: i32,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.unfile_card(owner, ref_arg(&kind, &id)?, x, y).await?)
}

/// Maps to `DataApi.canvasDrawer()`: what's filed in a drawer, newest first.
#[tauri::command]
pub async fn canvas_drawer(s: State<'_, VaultSession>, drawer: String) -> Res<Vec<CardView>> {
    let (store, owner) = s.data().await?;
    let key = drawer_arg(&drawer)?.key();
    let fields = links_for(&store, owner).await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    let spots: HashMap<(CardKind, Uuid), CanvasSpot> = store
        .canvas_spots(owner)
        .await?
        .into_iter()
        .map(|s| ((s.kind, s.record_id), s))
        .collect();
    let (_, mut filed) = drawers(
        &store,
        owner,
        &spots,
        &fields,
        OffsetDateTime::now_utc(),
        after,
    )
    .await?;
    let mut cards = filed.remove(&key).unwrap_or_default();
    cards.sort_by_key(|p| std::cmp::Reverse(p.created));
    Ok(cards.into_iter().map(|p| p.card).collect())
}

/// One folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderView {
    id: String,
    name: String,
}

/// Maps to `DataApi.canvasFolderSave()`: make a folder (`id` null) or rename one.
#[tauri::command]
pub async fn canvas_folder_save(
    s: State<'_, VaultSession>,
    id: Option<String>,
    name: String,
) -> Res<FolderView> {
    let (store, owner) = s.data().await?;
    let id = id.as_deref().map(uuid_arg).transpose()?;
    let f = store.save_folder(owner, id, &name).await?;
    Ok(FolderView {
        id: f.id.to_string(),
        name: f.name,
    })
}

/// Maps to `DataApi.canvasFolderRemove()`: remove a folder (its cards show on the Desk while
/// it's gone), or with `removed` false bring it back.
#[tauri::command]
pub async fn canvas_folder_remove(
    s: State<'_, VaultSession>,
    id: String,
    removed: bool,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_folder_deleted(owner, uuid_arg(&id)?, removed)
        .await?)
}

/// A card's spot before Tidy into stacks, for Undo.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WasAt {
    kind: CardKind,
    id: String,
    x: i32,
    y: i32,
}

/// What Tidy into stacks did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TidyView {
    /// The piles it made.
    stacks: Vec<String>,
    /// Where their cards were.
    before: Vec<WasAt>,
}

/// Maps to `DataApi.canvasTidy()`: pile loose cards by project (rule in
/// `shiftbook_core::canvas::tidy_groups`). Cards in a box or a pile stay as they are.
#[tauri::command]
pub async fn canvas_tidy(s: State<'_, VaultSession>, today: String) -> Res<TidyView> {
    let (store, owner) = s.data().await?;
    let (view, projects) = desk(&store, owner, date_arg(&today)?).await?;
    let loose: Vec<usize> = (0..view.cards.len())
        .filter(|&i| view.cards[i].in_box.is_none() && view.cards[i].stack.is_none())
        .collect();
    let keys: Vec<Option<Uuid>> = loose.iter().map(|&i| projects[i]).collect();
    let mut done = TidyView {
        stacks: Vec::new(),
        before: Vec::new(),
    };
    for group in tidy_groups(&keys) {
        let cards: Vec<&CardView> = group.iter().map(|&g| &view.cards[loose[g]]).collect();
        let first = cards[0];
        let first_ref = ref_arg(first.kind.as_str(), &first.id)?;
        let mut pile = None;
        for c in &cards[1..] {
            pile = Some(
                store
                    .stack_cards(
                        owner,
                        ref_arg(c.kind.as_str(), &c.id)?,
                        first_ref,
                        (first.x, first.y),
                    )
                    .await?,
            );
        }
        if let Some(p) = pile {
            done.stacks.push(p.to_string());
        }
        done.before.extend(cards.iter().map(|c| WasAt {
            kind: c.kind,
            id: c.id.clone(),
            x: c.x,
            y: c.y,
        }));
    }
    Ok(done)
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
