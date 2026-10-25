//! The Desk (the Canvas) as the screen shows it: which records are cards, where each sits,
//! piles, boxes, links and the timeline, put together from `shiftbook-storage` and the
//! rules in `shiftbook_core::canvas`. Shared by the desktop app and the Phase 2 server.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use shiftbook_core::canvas::{
    area_at, auto_links, auto_place, box_status, is_recent, stays_in_just_today, ticket_home, timeline, tray_place,
    work_groups, Area, BoxStatus, CardKind, Drawer, LinkReason, LinkText, MemberState, TrayPlace,
    ORIGIN, RECENT_DAYS,
};
use shiftbook_core::dates::format_date;
use shiftbook_core::hours::format_hours;
use shiftbook_core::ids::{InboxItemId, NoteId, OwnerId, ProjectId, TimeEntryId, TrayItemId};
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::{CustomField, InboxItem, Note, TrayItem};
use shiftbook_storage::canvas::{pair, CanvasSpot, CardRef};
use shiftbook_storage::work::EntryRecord;
use shiftbook_storage::{SqliteStore, StorageError};
use time::{Date, Duration, OffsetDateTime};
use uuid::Uuid;

use crate::views::{
    entry_view, inbox_view, links_for, note_view, tray_item_view, EntryView, InboxView, NoteView,
    TrayItemView,
};

type Result<T> = std::result::Result<T, StorageError>;

/// A Done ticket on the Canvas, with the day it belongs to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayCardView {
    pub item: TrayItemView,
    pub date: String,
}

/// One card: where it sits and the record it shows (exactly one of the four is set).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    pub kind: CardKind,
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub z: i64,
    /// The owner moved it (false: placed automatically this time).
    pub placed: bool,
    /// Its spot when the Desk is laid out as a timeline.
    pub timeline_x: i32,
    pub timeline_y: i32,
    /// The box it sits in ("project:<id>" or "label:<id>"), if any.
    pub in_box: Option<String>,
    /// The pile it's in, if any (it shows at the pile's spot).
    pub stack: Option<String>,
    pub entry: Option<EntryView>,
    pub tray: Option<TrayCardView>,
    pub note: Option<NoteView>,
    pub inbox: Option<InboxView>,
    /// For an entry: the same work on other days, folded into this card (round 14), newest
    /// first. This card shows the newest.
    pub work: Vec<EntryView>,
    /// The time across this card and `work`, as hours (only when there is other work).
    pub work_hours: Option<String>,
    /// Tickets from the Tickets tray with a number this work has, folded into this card.
    pub folded: Vec<TrayCardView>,
    /// Stays bright under "Just today" (round 15): from today, still open, brought up for
    /// today or pinned.
    pub just_today: bool,
}

/// A project's area.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaView {
    pub project_id: ProjectId,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub collapsed: bool,
    /// Cards inside it.
    pub count: usize,
    pub status: BoxStatus,
}

/// A plain labelled box.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelView {
    pub id: String,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub collapsed: bool,
    /// Cards inside it.
    pub count: usize,
    pub status: BoxStatus,
}

/// A pile of cards.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackView {
    pub id: String,
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub count: usize,
    /// The "Done today" pile Shiftbook keeps (id `done`): it can be moved and fanned out,
    /// not renamed, taken apart or filed.
    pub done: bool,
}

/// The id of the "Done today" pile.
pub const DONE_PILE: &str = "done";

/// A drawer in the cabinet: a project's or a folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawerView {
    /// `project:<id>` or `folder:<id>`.
    pub key: String,
    pub name: String,
    /// The project's colour (folders have none).
    pub color: Option<String>,
    /// Cards filed in it.
    pub count: usize,
}

/// One end of a link.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardRefView {
    pub kind: CardKind,
    pub id: String,
}

/// A line between two cards: automatic (with its reason) or drawn by hand.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    pub a: CardRefView,
    pub b: CardRefView,
    pub reason: Option<LinkReason>,
    pub drawn: bool,
}

/// A day on the timeline and where its column starts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AxisDay {
    pub date: String,
    pub x: i32,
}

/// Everything on the Desk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeskView {
    /// Bottom first.
    pub cards: Vec<CardView>,
    pub areas: Vec<AreaView>,
    pub labels: Vec<LabelView>,
    pub stacks: Vec<StackView>,
    pub drawers: Vec<DrawerView>,
    pub links: Vec<LinkView>,
    /// The timeline's days, oldest first.
    pub axis: Vec<AxisDay>,
}

/// A record that can be a card, before it gets a spot.
pub struct Pending {
    pub kind: CardKind,
    pub id: Uuid,
    /// For newest-first auto-placing and the timeline order within a day.
    pub created: OffsetDateTime,
    /// Its day on the timeline.
    pub date: Date,
    /// What it says, for links.
    pub link: LinkText,
    /// A ticket still open.
    pub open: bool,
    /// Its project, for Tidy into stacks.
    pub project: Option<Uuid>,
    /// For entries: the title and minutes, to fold the same work into one card.
    pub title: String,
    pub minutes: u32,
    /// The record whose saved spot this card uses: a folded-in older card's when this one
    /// has none of its own yet.
    pub spot_of: Option<Uuid>,
    pub card: CardView,
}

pub fn blank(kind: CardKind, id: Uuid) -> CardView {
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
        work: Vec::new(),
        work_hours: None,
        folded: Vec::new(),
        just_today: false,
    }
}

pub fn entry_card(rec: EntryRecord, fields: &[CustomField]) -> Pending {
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
    let (title, minutes) = (rec.entry.description.clone(), rec.entry.minutes.0);
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
        title,
        minutes,
        spot_of: None,
        card,
    }
}

pub fn tray_card(item: TrayItem, fields: &[CustomField]) -> Pending {
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
        title: String::new(),
        minutes: 0,
        spot_of: None,
        card,
    }
}

pub fn note_card(note: Note, now: OffsetDateTime, after: u32) -> Pending {
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
        title: String::new(),
        minutes: 0,
        spot_of: None,
        card,
    }
}

pub fn inbox_card(scrap: InboxItem) -> Pending {
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
        title: String::new(),
        minutes: 0,
        spot_of: None,
        card,
    }
}

/// A record that has gone (deleted since) counts as nothing, not an error.
pub fn skip_missing<T>(found: std::result::Result<T, StorageError>) -> Result<Option<T>> {
    match found {
        Ok(v) => Ok(Some(v)),
        Err(StorageError::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The card for a record the layout knows about, if the record is still there.
pub async fn card_for(
    store: &SqliteStore,
    owner: OwnerId,
    kind: CardKind,
    id: Uuid,
    fields: &[CustomField],
    now: OffsetDateTime,
    after: u32,
) -> Result<Option<Pending>> {
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
pub async fn drawers(
    store: &SqliteStore,
    owner: OwnerId,
    spots: &HashMap<(CardKind, Uuid), CanvasSpot>,
    fields: &[CustomField],
    now: OffsetDateTime,
    after: u32,
) -> Result<(Vec<DrawerView>, HashMap<String, Vec<Pending>>)> {
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

/// Fold the same work on several days into one card (see `shiftbook_core::canvas::work_groups`):
/// the newest entry shows, the older ones go in its `work`, and its time is the total. A tray
/// ticket with a number the work has folds in too (`ticket_home`). Other cards pass through.
pub fn fold_same_work(
    all: Vec<Pending>,
    spots: &HashMap<(CardKind, Uuid), CanvasSpot>,
) -> Vec<Pending> {
    let (mut entries, rest): (Vec<Pending>, Vec<Pending>) =
        all.into_iter().partition(|p| p.kind == CardKind::Entry);
    entries.sort_by_key(|p| std::cmp::Reverse((p.date, p.created)));
    let keys: Vec<(&str, Option<Uuid>)> = entries
        .iter()
        .map(|p| (p.title.as_str(), p.project))
        .collect();
    let groups = work_groups(&keys);
    let mut slots: Vec<Option<Pending>> = entries.into_iter().map(Some).collect();
    let mut cards: Vec<Pending> = Vec::new();
    for group in groups {
        let Some(mut head) = slots[group[0]].take() else {
            continue;
        };
        let mut minutes = head.minutes;
        for &i in &group[1..] {
            let Some(mut older) = slots[i].take() else {
                continue;
            };
            minutes = minutes.saturating_add(older.minutes);
            if head.spot_of.is_none()
                && !spots.contains_key(&(CardKind::Entry, head.id))
                && spots.contains_key(&(CardKind::Entry, older.id))
            {
                head.spot_of = Some(older.id);
            }
            for t in older.link.tickets.drain(..) {
                if !head.link.tickets.iter().any(|h| h.eq_ignore_ascii_case(&t)) {
                    head.link.tickets.push(t);
                }
            }
            if let Some(e) = older.card.entry.take() {
                head.card.work.push(e);
            }
        }
        if group.len() > 1 {
            head.card.work_hours = Some(format_hours(Minutes(minutes)));
        }
        cards.push(head);
    }
    let numbers: Vec<Vec<String>> = cards.iter().map(|p| p.link.tickets.clone()).collect();
    let mut others = Vec::new();
    for mut p in rest {
        if p.kind == CardKind::Tray {
            let home = p
                .link
                .tickets
                .first()
                .and_then(|n| ticket_home(n, &numbers));
            // Only a ticket that has a home gives up its card (taking it any sooner left an
            // empty card behind for the rest).
            if let Some(h) = home {
                if let Some(tray) = p.card.tray.take() {
                    cards[h].open |= p.open;
                    cards[h].card.folded.push(tray);
                    continue;
                }
            }
        }
        others.push(p);
    }
    cards.extend(others);
    cards
}

pub fn card_ref(kind: CardKind, id: Uuid) -> CardRefView {
    CardRefView {
        kind,
        id: id.to_string(),
    }
}

/// Everything on the Desk, with each card's project (for Tidy into stacks).
pub async fn desk(
    store: &SqliteStore,
    owner: OwnerId,
    today: Date,
) -> Result<(DeskView, Vec<Option<Uuid>>)> {
    let from = today - Duration::days(RECENT_DAYS - 1);
    let fields = links_for(store, owner).await?;
    let prefs = store.prefs(owner).await?;
    let after = prefs.notes_auto_archive_days;
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

    // Tickets: open ones stay loose, today's done ones gather in the Done today pile, and done
    // ones from before today leave the Desk (they're on the Done tickets board and in Find).
    let place = |p: &Pending| {
        if p.kind == CardKind::Tray {
            tray_place(!p.open, p.date, today, spots.contains_key(&(p.kind, p.id)))
        } else {
            TrayPlace::Loose
        }
    };
    let mut done_today: Vec<Pending> = Vec::new();
    let mut rest = Vec::new();
    for p in all {
        match place(&p) {
            TrayPlace::Loose => rest.push(p),
            TrayPlace::DonePile => done_today.push(p),
            TrayPlace::Off => {}
        }
    }
    let mut all = rest;

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
                    if tray_place(item.done, item.date, today, true) != TrayPlace::Off {
                        all.push(tray_card(item, &fields));
                    }
                }
            }
            // Notes and scraps that aren't live (archived or removed) stay off the Desk.
            CardKind::Note | CardKind::Inbox => {}
        }
    }

    // The same work on several days shows as one card, and tray tickets it has fold in.
    let all = fold_same_work(all, &spots);

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
        if let Some(st) = spots
            .get(&(p.kind, p.spot_of.unwrap_or(p.id)))
            .and_then(|sp| sp.stack)
        {
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
        if let Some(spot) = spots.get(&(p.kind, p.spot_of.unwrap_or(p.id))) {
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
    let mut taken: Vec<(i32, i32)> = placed.iter().map(|p| (p.card.x, p.card.y)).collect();
    // The Done today pile sits where the owner left it, or takes the first free spot.
    let pile_needs_spot = !done_today.is_empty() && prefs.done_pile.is_none();
    taken.extend(prefs.done_pile.filter(|_| !done_today.is_empty()));
    let mut free = auto_place(loose.len() + usize::from(pile_needs_spot), &taken).into_iter();
    let done_spot = match prefs.done_pile {
        Some(at) => at,
        None if pile_needs_spot => free.next().unwrap_or(ORIGIN),
        None => ORIGIN,
    };
    for (mut p, (x, y)) in loose.into_iter().zip(free) {
        p.card.x = x;
        p.card.y = y;
        placed.push(p);
    }
    placed.sort_by_key(|p| p.card.z);
    // Oldest at the bottom of the pile, so the latest one done shows on top.
    done_today.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.cmp(&b.id)));
    let done_count = done_today.len();
    let top_z = placed.last().map_or(0, |p| p.card.z);
    for (i, mut p) in done_today.into_iter().enumerate() {
        (p.card.x, p.card.y) = done_spot;
        p.card.z = top_z + 1 + i as i64;
        p.card.stack = Some(DONE_PILE.to_string());
        placed.push(p);
    }

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
    let brought_today: HashSet<(CardKind, Uuid)> = store
        .next_up_all(owner)
        .await?
        .into_iter()
        .filter(|i| i.done_at.is_none() && i.show_from.is_some_and(|d| d <= today))
        .filter_map(|i| i.about)
        .collect();
    let pinned: HashSet<(CardKind, Uuid)> = store
        .pins(owner)
        .await?
        .into_iter()
        .map(|p| (p.kind, p.record_id))
        .collect();
    for p in placed.iter_mut() {
        let state = MemberState {
            open: p.open,
            brought_up: brought_today.contains(&(p.kind, p.id)),
            today: p.date == today,
        };
        p.card.just_today = stays_in_just_today(state, pinned.contains(&(p.kind, p.id)));
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
    let mut stacks: Vec<StackView> = piles
        .values()
        .filter(|st| pile_size.get(&st.id).copied().unwrap_or(0) >= 2)
        .map(|st| StackView {
            id: st.id.to_string(),
            name: st.name.clone(),
            x: st.x,
            y: st.y,
            count: pile_size[&st.id],
            done: false,
        })
        .collect();
    if done_count > 0 {
        stacks.push(StackView {
            id: DONE_PILE.to_string(),
            name: Some("Done today".to_string()),
            x: done_spot.0,
            y: done_spot.1,
            count: done_count,
            done: true,
        });
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    fn tray(number: &str) -> Pending {
        tray_card(
            TrayItem {
                id: TrayItemId::new(),
                owner_id: OwnerId::new(),
                date: date!(2026 - 10 - 22),
                ticket_number: number.into(),
                minutes: Minutes(15),
                note: None,
                done: false,
                project_id: None,
                created_at: datetime!(2026-10-22 09:00 UTC),
                updated_at: datetime!(2026-10-22 09:00 UTC),
                deleted_at: None,
            },
            &[],
        )
    }

    #[test]
    fn a_ticket_without_a_home_keeps_its_card() {
        let mut entry = tray("CHG0146615");
        // An entry card whose work has CHG0146615 (only its kind and tickets matter here).
        entry.kind = CardKind::Entry;
        entry.title = "Network closet".into();
        let out = fold_same_work(
            vec![entry, tray("chg0146615"), tray("INC0099001")],
            &HashMap::new(),
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].card.folded.len(), 1);
        let loose = &out[1];
        assert_eq!(loose.kind, CardKind::Tray);
        assert!(
            loose.card.tray.is_some(),
            "an unmatched ticket keeps what its card shows"
        );
    }
}
