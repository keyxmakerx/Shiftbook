//! Commands for the Canvas (the Desk): the app's own records as cards, and where they sit.
//! Which records show and where unmoved cards go are rules in `shiftbook_core::canvas`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use shiftbook_core::canvas::{tidy_groups, Area, CardKind, Drawer};
use shiftbook_core::ids::ProjectId;
use shiftbook_storage::canvas::{CanvasSpot, CardRef};
use tauri::State;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::api::{date_arg, links_for};
use crate::session::{CommandError, VaultSession};

use shiftbook_app::desk::{card_ref, desk, drawers, CardRefView, CardView, DeskView};

type Res<T> = Result<T, CommandError>;

/// Maps to `DataApi.canvas()`: everything on the Desk. Entries and Done tickets from the last
/// week show on their own; older ones only if the owner moved them. Live notes and Inbox
/// scraps always show. The day's "Tickets" roll-up entry doesn't: its tickets are cards.
#[tauri::command]
pub async fn canvas(s: State<'_, VaultSession>, today: String) -> Res<DeskView> {
    let (store, owner) = s.data().await?;
    Ok(desk(&store, owner, date_arg(&today)?).await?.0)
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

/// One card, as the screen sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct CardArg {
    kind: String,
    id: String,
}

/// Maps to `DataApi.canvasFileMany()`: file several cards in one drawer at once, all or
/// nothing (round 15: Tidy up puts them in the Archive).
#[tauri::command]
pub async fn canvas_file_many(
    s: State<'_, VaultSession>,
    cards: Vec<CardArg>,
    drawer: String,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let refs = cards
        .iter()
        .map(|c| ref_arg(&c.kind, &c.id))
        .collect::<Res<Vec<CardRef>>>()?;
    Ok(store.file_cards(owner, &refs, drawer_arg(&drawer)?).await?)
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WasAt {
    kind: CardKind,
    id: String,
    x: i32,
    y: i32,
}

/// What Tidy into stacks did.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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

#[cfg(test)]
mod tests {
    use super::*;
    use shiftbook_app::desk::DONE_PILE;
    use shiftbook_core::canvas::CardKind;
    use shiftbook_core::minutes::Minutes;
    use time::macros::date;

    /// The whole Desk from a real (encrypted) database: the same work folds into one card,
    /// its tray ticket folds in, a ticket done today goes on the Done today pile.
    #[test]
    fn the_desk_from_a_real_database() {
        use crate::session::VaultSession;
        use crate::test_support::{block_on, TempDir};
        use shiftbook_core::notes::NoteKind;
        use shiftbook_storage::notes::NoteInput;
        use shiftbook_storage::work::EntryInput;

        let dir = TempDir::new("desk");
        let session = VaultSession::new(Some(dir.0.clone()));
        block_on(async {
            session
                .setup(crate::session::take_secret("a long enough password".into()))
                .await
                .unwrap();
            let (store, me) = session.data().await.unwrap();
            let today = date!(2026 - 10 - 22);
            let p = store.create_project(me, "Network", None).await.unwrap();
            let entry = |date, minutes| EntryInput {
                date,
                minutes: Minutes(minutes),
                description: "Network closet".into(),
                project: Some(p.id),
                recurring: None,
                category: None,
                next_step: None,
                timesheet_code_id: None,
            };
            store
                .create_entry(me, &entry(date!(2026 - 10 - 20), 60), "CHG0146615")
                .await
                .unwrap();
            store.create_entry(me, &entry(today, 30), "").await.unwrap();
            store.tray_add(me, today, "chg0146615").await.unwrap();
            let day = store.tray_add(me, today, "INC0099001").await.unwrap();
            let done = day
                .items
                .iter()
                .find(|i| i.ticket_number == "INC0099001")
                .unwrap()
                .id;
            store.tray_set_done(me, done, true).await.unwrap();
            store
                .create_note(
                    me,
                    &NoteInput {
                        kind: NoteKind::Jot,
                        title: String::new(),
                        body: "Order patch cables".into(),
                        project: None,
                        tags: String::new(),
                    },
                )
                .await
                .unwrap();

            let (desk, _) = desk(&store, me, today).await.unwrap();
            let work: Vec<&CardView> = desk
                .cards
                .iter()
                .filter(|c| c.kind == CardKind::Entry)
                .collect();
            assert_eq!(work.len(), 1, "the same work is one card");
            assert_eq!(work[0].work.len(), 1);
            assert_eq!(work[0].work_hours.as_deref(), Some("1.5"));
            assert_eq!(work[0].folded.len(), 1, "its tray ticket folds in");

            let pile: Vec<&CardView> = desk
                .cards
                .iter()
                .filter(|c| c.stack.as_deref() == Some(DONE_PILE))
                .collect();
            assert_eq!(pile.len(), 1);
            assert!(desk.stacks.iter().any(|s| s.done && s.count == 1));
            assert!(desk.cards.iter().any(|c| c.kind == CardKind::Note));
            assert_eq!(desk.cards.len(), 3, "work, the done ticket, the jot");
        });
    }
}
