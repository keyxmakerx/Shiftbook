//! Tests for notes, the archive and auto-archive against a real encrypted database.

use shiftbook_core::ids::{NoteId, OwnerId};
use shiftbook_core::notes::NoteKind;
use time::macros::datetime;
use time::Duration;

use crate::db::{database_path, open_encrypted, test_dek};
use crate::fsutil::test_dir::TestDir;
use crate::notes::NoteInput;
use crate::test_rt::block_on;
use crate::{SqliteStore, StorageError};

async fn setup(dir: &TestDir) -> (SqliteStore, OwnerId, OwnerId) {
    let s = SqliteStore::new(
        open_encrypted(&database_path(dir.path()), &test_dek(7))
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

fn note(kind: NoteKind, title: &str, body: &str) -> NoteInput {
    NoteInput {
        kind,
        title: title.into(),
        body: body.into(),
        project: None,
        tags: String::new(),
    }
}

/// Pretend a note last changed at `when`.
async fn age(s: &SqliteStore, id: NoteId, when: &str) {
    sqlx::query("UPDATE notes SET updated_at = $1 WHERE id = $2")
        .bind(when)
        .bind(id.to_string())
        .execute(s.pool())
        .await
        .unwrap();
}

#[test]
fn write_edit_and_list() {
    let dir = TestDir::new("notes-crud");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let project = s.create_project(me, "Site A", None).await.unwrap();
        let mut input = note(NoteKind::Page, "", "Guest WiFi\nPassword is on the fridge");
        input.tags = "network, #wifi, Network".into();
        input.project = Some(project.id);
        let n = s.create_note(me, &input).await.unwrap();
        assert_eq!(n.title, "Guest WiFi", "title from the first line");
        assert_eq!(n.tags, ["network", "wifi"]);
        assert_eq!(n.project_id, Some(project.id));
        assert!(!n.keep);
        assert_eq!(n.archived_at, None);

        input.title = "Guest Wi-Fi".into();
        input.kind = NoteKind::Jot;
        input.project = None;
        let changed = s.update_note(me, n.id, &input).await.unwrap();
        assert_eq!(changed.title, "Guest Wi-Fi");
        assert_eq!(changed.kind, NoteKind::Jot);
        assert_eq!(changed.project_id, None);

        let second = s
            .create_note(me, &note(NoteKind::Meeting, "Ops sync", ""))
            .await
            .unwrap();
        let list = s.list_notes(me, false).await.unwrap();
        assert_eq!(
            list.iter().map(|n| n.id).collect::<Vec<_>>(),
            [second.id, n.id],
            "most recently changed first"
        );
        assert!(s.list_notes(other, true).await.unwrap().is_empty());
        assert!(matches!(
            s.get_note(other, n.id).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            s.update_note(other, n.id, &input).await,
            Err(StorageError::NotFound)
        ));
    });
}

#[test]
fn bad_input_is_refused() {
    let dir = TestDir::new("notes-bad");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let mut input = note(NoteKind::Jot, "x", "");
        input.tags = "a".repeat(41);
        assert!(matches!(
            s.create_note(me, &input).await,
            Err(StorageError::Invalid(_))
        ));
        // Someone else's project.
        let theirs = s.create_project(other, "Theirs", None).await.unwrap();
        let mut input = note(NoteKind::Jot, "x", "");
        input.project = Some(theirs.id);
        assert!(matches!(
            s.create_note(me, &input).await,
            Err(StorageError::NotFound)
        ));
        assert!(s.list_notes(me, true).await.unwrap().is_empty());
    });
}

#[test]
fn archive_keep_and_delete() {
    let dir = TestDir::new("notes-archive");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let n = s
            .create_note(me, &note(NoteKind::Meeting, "Ops", "docks"))
            .await
            .unwrap();
        s.set_note_archived(me, n.id, true).await.unwrap();
        assert!(s.list_notes(me, false).await.unwrap().is_empty());
        let all = s.list_notes(me, true).await.unwrap();
        assert!(all[0].archived_at.is_some(), "still there with archived");
        s.set_note_archived(me, n.id, false).await.unwrap();
        assert_eq!(s.list_notes(me, false).await.unwrap().len(), 1);

        s.set_note_keep(me, n.id, true).await.unwrap();
        assert!(s.get_note(me, n.id).await.unwrap().keep);

        assert!(matches!(
            s.set_note_archived(other, n.id, true).await,
            Err(StorageError::NotFound)
        ));
        s.set_note_deleted(me, n.id, true).await.unwrap();
        assert!(s.list_notes(me, true).await.unwrap().is_empty());
        assert!(matches!(
            s.set_note_deleted(me, n.id, true).await,
            Err(StorageError::NotFound)
        ));
        s.set_note_deleted(me, n.id, false).await.unwrap();
        assert_eq!(s.list_notes(me, true).await.unwrap().len(), 1, "undo");
    });
}

#[test]
fn auto_archive_moves_only_due_notes() {
    let dir = TestDir::new("notes-auto");
    block_on(async {
        let (s, me, _) = setup(&dir).await;
        let now = datetime!(2026-10-17 12:00 UTC);
        let old = now - Duration::days(120);
        let old_text = "2026-06-19T12:00:00Z";
        assert_eq!(shiftbook_core::dates::format_timestamp(old), old_text);

        let meeting = s
            .create_note(me, &note(NoteKind::Meeting, "Old sync", ""))
            .await
            .unwrap();
        let page = s
            .create_note(me, &note(NoteKind::Page, "How the UPS works", ""))
            .await
            .unwrap();
        let kept = s
            .create_note(me, &note(NoteKind::Jot, "Kept", ""))
            .await
            .unwrap();
        let fresh = s
            .create_note(me, &note(NoteKind::Jot, "Fresh", ""))
            .await
            .unwrap();
        s.set_note_keep(me, kept.id, true).await.unwrap();
        for id in [meeting.id, page.id, kept.id] {
            age(&s, id, old_text).await;
        }
        age(&s, fresh.id, "2026-10-16T12:00:00Z").await;

        assert_eq!(s.auto_archive_notes(me, now, 0).await.unwrap(), 0, "off");
        assert_eq!(s.auto_archive_notes(me, now, 90).await.unwrap(), 1);
        let live: Vec<_> = s
            .list_notes(me, false)
            .await
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();
        assert!(!live.contains(&meeting.id));
        assert!(live.contains(&page.id) && live.contains(&kept.id) && live.contains(&fresh.id));
        let moved = s.get_note(me, meeting.id).await.unwrap();
        assert_eq!(moved.updated_at, old, "moving isn't a change");
        assert_eq!(
            s.auto_archive_notes(me, now, 90).await.unwrap(),
            0,
            "only once"
        );
    });
}
