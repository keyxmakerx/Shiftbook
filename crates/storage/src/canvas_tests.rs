//! Tests for the Canvas layout against a real encrypted database.

use shiftbook_core::canvas::{CardKind, MAX_COORD};
use shiftbook_core::ids::OwnerId;
use time::macros::date;
use uuid::Uuid;

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
