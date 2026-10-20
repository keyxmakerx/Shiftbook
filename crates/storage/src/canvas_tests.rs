//! Tests for the Canvas layout against a real encrypted database.

use shiftbook_core::canvas::{Area, CardKind, MAX_COORD, MIN_AREA};
use shiftbook_core::ids::OwnerId;
use time::macros::date;
use uuid::Uuid;

use crate::canvas::pair;
use crate::db::{database_path, open_encrypted, test_dek};
use crate::fsutil::test_dir::TestDir;
use crate::test_rt::block_on;
use crate::SqliteStore;

async fn setup(dir: &TestDir) -> (SqliteStore, OwnerId, OwnerId) {
    let s = SqliteStore::new(
        open_encrypted(&database_path(dir.path()), &test_dek(8))
            .await
            .unwrap(),
    );
    let me = s.initialize_local_owner("Me").await.unwrap().id;
    let other = OwnerId::new();
    sqlx::query(
        "INSERT INTO users (id, display_name, created_at, updated_at) \
         VALUES ($1, 'Other', '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z')",
    )
    .bind(other.to_string())
    .execute(s.pool())
    .await
    .unwrap();
    (s, me, other)
}

#[test]
fn moving_cards_saves_spots_on_top() {
    let dir = TestDir::new("canvas");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        assert!(s.canvas_spots(me).await.unwrap().is_empty());
        let a = Uuid::now_v7();
        let b = Uuid::now_v7();
        let first = s.move_card(me, CardKind::Note, a, 100, 200).await.unwrap();
        let second = s.move_card(me, CardKind::Entry, b, -40, 10).await.unwrap();
        assert!(second.z > first.z);

        // Moving again updates the same card and puts it back on top.
        let again = s.move_card(me, CardKind::Note, a, 300, 50).await.unwrap();
        assert!(again.z > second.z);
        let spots = s.canvas_spots(me).await.unwrap();
        assert_eq!(spots.len(), 2);
        assert_eq!(spots.last().unwrap().record_id, a);
        assert_eq!((spots[1].x, spots[1].y), (300, 50));

        // The same id as another kind is a different card; other owners see nothing.
        s.move_card(me, CardKind::Inbox, a, 0, 0).await.unwrap();
        assert_eq!(s.canvas_spots(me).await.unwrap().len(), 3);
        assert!(s.canvas_spots(other).await.unwrap().is_empty());

        // Far-off spots are clamped.
        let far = s
            .move_card(me, CardKind::Tray, b, i32::MAX, i32::MIN)
            .await
            .unwrap();
        assert_eq!((far.x, far.y), (MAX_COORD, -MAX_COORD));
    });
}

#[test]
fn tray_between_and_tray_item() {
    let dir = TestDir::new("canvas-tray");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        s.tray_add(me, date!(2026 - 10 - 15), "INC0000001")
            .await
            .unwrap();
        let day = s
            .tray_add(me, date!(2026 - 10 - 19), "INC0000002")
            .await
            .unwrap();
        let list = s
            .tray_between(me, date!(2026 - 10 - 13), date!(2026 - 10 - 19))
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].ticket_number, "INC0000001");
        assert!(s
            .tray_between(other, date!(2026 - 10 - 13), date!(2026 - 10 - 19))
            .await
            .unwrap()
            .is_empty());
        let id = day.items[0].id;
        assert_eq!(
            s.tray_item(me, id).await.unwrap().unwrap().ticket_number,
            "INC0000002"
        );
        assert!(s.tray_item(other, id).await.unwrap().is_none());
        s.tray_remove(me, id).await.unwrap();
        assert!(s.tray_item(me, id).await.unwrap().is_none());
    });
}

#[test]
fn project_areas_are_saved_per_project() {
    let dir = TestDir::new("canvas-areas");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let p = s
            .create_project(me, "Network refresh", None)
            .await
            .unwrap()
            .id;
        assert!(s.canvas_areas(me).await.unwrap().is_empty());
        let area = Area {
            x: 10,
            y: 20,
            w: 600,
            h: 400,
        };
        s.set_area(me, p, area).await.unwrap();
        // Moving again updates the same area; tiny sizes are kept sane.
        let small = s
            .set_area(
                me,
                p,
                Area {
                    x: 50,
                    y: 60,
                    w: 1,
                    h: 1,
                },
            )
            .await
            .unwrap();
        assert_eq!((small.area.w, small.area.h), MIN_AREA);
        let all = s.canvas_areas(me).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!((all[0].area.x, all[0].area.y), (50, 60));
        assert!(s.canvas_areas(other).await.unwrap().is_empty());
        // Someone else's project, or one that doesn't exist, has no area.
        assert!(s.set_area(other, p, area).await.is_err());
        s.remove_area(me, p).await.unwrap();
        assert!(s.canvas_areas(me).await.unwrap().is_empty());
        // And it can come back.
        s.set_area(me, p, area).await.unwrap();
        assert_eq!(s.canvas_areas(me).await.unwrap().len(), 1);
    });
}

#[test]
fn links_are_drawn_hidden_and_reset_in_either_order() {
    let dir = TestDir::new("canvas-links");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let a = (CardKind::Note, Uuid::now_v7());
        let b = (CardKind::Entry, Uuid::now_v7());
        s.set_link(me, a, b, true).await.unwrap();
        let links = s.canvas_links(me).await.unwrap();
        assert_eq!(links.len(), 1);
        assert!(links[0].drawn);
        assert_eq!((links[0].a, links[0].b), pair(a, b));
        // The same pair the other way round is the same link.
        s.set_link(me, b, a, false).await.unwrap();
        let links = s.canvas_links(me).await.unwrap();
        assert_eq!(links.len(), 1);
        assert!(!links[0].drawn);
        assert!(s.canvas_links(other).await.unwrap().is_empty());
        assert!(s.set_link(me, a, a, true).await.is_err(), "not to itself");
        s.reset_link(me, b, a).await.unwrap();
        assert!(s.canvas_links(me).await.unwrap().is_empty());
    });
}

#[test]
fn boxes_fold_up_and_labelled_boxes_are_kept() {
    let dir = TestDir::new("canvas-boxes");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let p = s.create_project(me, "SSL renewals", None).await.unwrap().id;
        let area = Area {
            x: 10,
            y: 20,
            w: 600,
            h: 400,
        };
        assert!(!s.set_area(me, p, area).await.unwrap().collapsed);
        s.set_area_collapsed(me, p, true).await.unwrap();
        // Moving a folded area keeps it folded.
        assert!(s.set_area(me, p, area).await.unwrap().collapsed);
        assert!(s.canvas_areas(me).await.unwrap()[0].collapsed);
        assert!(s.set_area_collapsed(other, p, false).await.is_err());

        let made = s
            .save_label(me, None, "  Waiting on vendors ", area, false)
            .await
            .unwrap();
        assert_eq!(made.label, "Waiting on vendors");
        let moved = Area { x: 300, ..area };
        s.save_label(me, Some(made.id), "Vendors", moved, true)
            .await
            .unwrap();
        let all = s.canvas_labels(me).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(
            (all[0].label.as_str(), all[0].area.x, all[0].collapsed),
            ("Vendors", 300, true)
        );
        assert!(s.canvas_labels(other).await.unwrap().is_empty());
        assert!(s
            .save_label(other, Some(made.id), "Mine", area, false)
            .await
            .is_err());
        assert!(s.save_label(me, None, "   ", area, false).await.is_err());

        s.set_label_deleted(me, made.id, true).await.unwrap();
        assert!(s.canvas_labels(me).await.unwrap().is_empty());
        s.set_label_deleted(me, made.id, false).await.unwrap();
        assert_eq!(s.canvas_labels(me).await.unwrap().len(), 1);
    });
}

#[test]
fn cards_pile_up_and_come_apart() {
    let dir = TestDir::new("canvas-piles");
    block_on(async {
        let (s, me, _) = setup(&dir).await;
        let (a, b, c) = (
            (CardKind::Entry, Uuid::now_v7()),
            (CardKind::Note, Uuid::now_v7()),
            (CardKind::Tray, Uuid::now_v7()),
        );
        // b was never moved: the pile starts where b showed.
        let pile = s.stack_cards(me, a, b, (300, 200)).await.unwrap();
        let piles = s.canvas_stacks(me).await.unwrap();
        assert_eq!((piles.len(), piles[0].x, piles[0].y), (1, 300, 200));
        assert!(s.stack_cards(me, a, a, (0, 0)).await.is_err());
        // c joins by being dropped on a pile member.
        assert_eq!(s.stack_cards(me, c, a, (0, 0)).await.unwrap(), pile);
        let spots = s.canvas_spots(me).await.unwrap();
        assert!(spots.iter().all(|sp| sp.stack == Some(pile)));

        s.rename_stack(me, pile, " Printers ").await.unwrap();
        assert_eq!(
            s.canvas_stacks(me).await.unwrap()[0].name.as_deref(),
            Some("Printers")
        );
        s.rename_stack(me, pile, " ").await.unwrap();
        assert_eq!(s.canvas_stacks(me).await.unwrap()[0].name, None);
        s.move_stack(me, pile, 500, 600).await.unwrap();

        // Taking cards off: with one left the pile comes apart where it was.
        s.unstack_card(me, c, 50, 60).await.unwrap();
        assert_eq!(s.canvas_stacks(me).await.unwrap().len(), 1);
        s.move_card(me, a.0, a.1, 10, 10).await.unwrap();
        assert!(s.canvas_stacks(me).await.unwrap().is_empty());
        let spots = s.canvas_spots(me).await.unwrap();
        let left = spots.iter().find(|sp| sp.record_id == b.1).unwrap();
        assert_eq!((left.x, left.y, left.stack), (500, 600, None));

        // Taking a pile apart spreads its cards out.
        let pile = s.stack_cards(me, a, b, (100, 100)).await.unwrap();
        s.dissolve_stack(me, pile).await.unwrap();
        assert!(s.canvas_stacks(me).await.unwrap().is_empty());
        assert!(s
            .canvas_spots(me)
            .await
            .unwrap()
            .iter()
            .all(|sp| sp.stack.is_none()));
    });
}

#[test]
fn cards_and_piles_are_filed_in_drawers() {
    let dir = TestDir::new("canvas-drawers");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let folder = s.save_folder(me, None, " Reference ").await.unwrap();
        assert_eq!(folder.name, "Reference");
        let drawer = shiftbook_core::canvas::Drawer::Folder(folder.id);
        let (a, b, c) = (
            (CardKind::Entry, Uuid::now_v7()),
            (CardKind::Note, Uuid::now_v7()),
            (CardKind::Note, Uuid::now_v7()),
        );
        s.file_card(me, a, drawer).await.unwrap();
        let pile = s.stack_cards(me, b, c, (0, 0)).await.unwrap();
        assert_eq!(s.file_stack(me, pile, drawer).await.unwrap().len(), 2);
        assert!(s.canvas_stacks(me).await.unwrap().is_empty());
        let spots = s.canvas_spots(me).await.unwrap();
        assert_eq!(
            spots.iter().filter(|sp| sp.drawer == Some(drawer)).count(),
            3
        );

        s.unfile_card(me, a, 40, 40).await.unwrap();
        let back = s.canvas_spots(me).await.unwrap();
        let a_spot = back.iter().find(|sp| sp.record_id == a.1).unwrap();
        assert_eq!((a_spot.drawer, a_spot.x), (None, 40));

        s.save_folder(me, Some(folder.id), "Old stuff")
            .await
            .unwrap();
        assert_eq!(s.canvas_folders(me).await.unwrap()[0].name, "Old stuff");
        assert!(s.canvas_folders(other).await.unwrap().is_empty());
        assert!(s.save_folder(other, Some(folder.id), "Mine").await.is_err());
        assert!(s.save_folder(me, None, "  ").await.is_err());
        s.set_folder_deleted(me, folder.id, true).await.unwrap();
        assert!(s.canvas_folders(me).await.unwrap().is_empty());
        s.set_folder_deleted(me, folder.id, false).await.unwrap();
        assert_eq!(s.canvas_folders(me).await.unwrap().len(), 1);
    });
}
