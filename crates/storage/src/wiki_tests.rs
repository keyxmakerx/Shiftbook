//! Tests for the wiki's spaces, places, earlier versions and the move-in, against a real
//! encrypted database.

use shiftbook_core::ids::{NoteId, OwnerId};
use shiftbook_core::notes::NoteKind;
use shiftbook_core::wiki::MAX_VERSIONS;

use crate::db::{database_path, open_encrypted, test_dek};
use crate::fsutil::test_dir::TestDir;
use crate::notes::NoteInput;
use crate::test_rt::block_on;
use crate::SqliteStore;

async fn setup(dir: &TestDir) -> (SqliteStore, OwnerId) {
    let s = SqliteStore::new(
        open_encrypted(&database_path(dir.path()), &test_dek(18))
            .await
            .unwrap(),
    );
    let me = s.initialize_local_owner("Me").await.unwrap().id;
    (s, me)
}

fn page(title: &str) -> NoteInput {
    NoteInput {
        kind: NoteKind::Page,
        title: title.into(),
        body: format!("About {title}"),
        project: None,
        tags: String::new(),
        template: false,
    }
}

async fn make(s: &SqliteStore, me: OwnerId, title: &str) -> NoteId {
    s.create_note(me, &page(title)).await.unwrap().id
}

#[test]
fn spaces_and_pages_under_pages() {
    let dir = TestDir::new("wiki-places");
    block_on(async {
        let (s, me) = setup(&dir).await;
        let net = s
            .save_wiki_space(me, None, " Network ", None)
            .await
            .unwrap();
        let srv = s
            .save_wiki_space(me, None, "Servers", Some("#8a6a2e"))
            .await
            .unwrap();
        assert_eq!(net.name, "Network");
        assert!(srv.position > net.position);
        assert!(s.save_wiki_space(me, None, "  ", None).await.is_err());

        let switches = make(&s, me, "Switches").await;
        let swap = make(&s, me, "Stack swap").await;
        s.place_wiki_page(me, switches, Some(net.id), None, None)
            .await
            .unwrap();
        s.place_wiki_page(me, swap, Some(net.id), Some(switches), None)
            .await
            .unwrap();
        // Under a page in another space: refused.
        let lone = make(&s, me, "Certificates").await;
        assert!(s
            .place_wiki_page(me, lone, Some(srv.id), Some(switches), None)
            .await
            .is_err());
        // Under itself: refused.
        assert!(s
            .place_wiki_page(me, switches, Some(net.id), Some(swap), None)
            .await
            .is_err());

        // Moving a page to another space takes the pages under it along.
        s.place_wiki_page(me, switches, Some(srv.id), None, None)
            .await
            .unwrap();
        let places = s.wiki_places(me).await.unwrap();
        let of = |id: NoteId| *places.iter().find(|p| p.page == *id.as_uuid()).unwrap();
        assert_eq!(of(swap).space, Some(srv.id));
        assert_eq!(of(swap).parent, Some(*switches.as_uuid()));

        // A jot isn't a wiki page.
        let jot = s
            .create_note(
                me,
                &NoteInput {
                    kind: NoteKind::Jot,
                    ..page("Quick")
                },
            )
            .await
            .unwrap();
        assert!(s
            .place_wiki_page(me, jot.id, Some(net.id), None, None)
            .await
            .is_err());

        // Removing a space keeps its pages, under "Not in a space".
        s.delete_wiki_space(me, srv.id).await.unwrap();
        let places = s.wiki_places(me).await.unwrap();
        assert!(places.iter().all(|p| p.space.is_none()));
        assert_eq!(s.wiki_spaces(me).await.unwrap().len(), 1);
    });
}

#[test]
fn saving_a_page_keeps_earlier_versions() {
    let dir = TestDir::new("wiki-versions");
    block_on(async {
        let (s, me) = setup(&dir).await;
        let id = make(&s, me, "VPN fixes").await;
        let mut input = page("VPN fixes");
        input.body = "Check the MTU".into();
        s.update_note(me, id, &input).await.unwrap();
        // A second save straight after doesn't make another version.
        input.body = "Check the MTU, then the client".into();
        s.update_note(me, id, &input).await.unwrap();
        let versions = s.note_versions(me, id).await.unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].body, "About VPN fixes");

        // Older versions than the gap make room for a new one; only the newest are kept.
        for i in 0..MAX_VERSIONS + 3 {
            sqlx::query("UPDATE note_versions SET created_at = '2026-01-01T00:00:00Z'")
                .execute(s.pool())
                .await
                .unwrap();
            input.body = format!("Take {i}");
            s.update_note(me, id, &input).await.unwrap();
        }
        assert_eq!(s.note_versions(me, id).await.unwrap().len(), MAX_VERSIONS);

        // Jots keep no versions.
        let jot = s
            .create_note(
                me,
                &NoteInput {
                    kind: NoteKind::Jot,
                    ..page("Quick")
                },
            )
            .await
            .unwrap();
        let mut j = NoteInput {
            kind: NoteKind::Jot,
            ..page("Quick")
        };
        j.body = "changed".into();
        s.update_note(me, jot.id, &j).await.unwrap();
        assert!(s.note_versions(me, jot.id).await.unwrap().is_empty());
    });
}

#[test]
fn old_mops_move_into_the_wiki_once() {
    let dir = TestDir::new("wiki-move-in");
    block_on(async {
        let (s, me) = setup(&dir).await;
        let mop = s
            .create_note(
                me,
                &NoteInput {
                    kind: NoteKind::Mop,
                    ..page("Swap PSU")
                },
            )
            .await
            .unwrap();
        let mut tpl = NoteInput {
            kind: NoteKind::Mop,
            body: "Swap at [[site]] for [[ticket]]".into(),
            ..page("Swap at [[site]]")
        };
        tpl.template = true;
        let tpl = s.create_note(me, &tpl).await.unwrap();
        // The move-in runs on unlock; the first unlock already happened in setup, before
        // these existed, so clear its mark and unlock again.
        sqlx::query("DELETE FROM settings WHERE key = 'wiki.moved_in'")
            .execute(s.pool())
            .await
            .unwrap();
        s.initialize_local_owner("Me").await.unwrap();

        let moved = s.get_note(me, mop.id).await.unwrap();
        assert_eq!(moved.kind, NoteKind::Page);
        let spaces = s.wiki_spaces(me).await.unwrap();
        assert_eq!(spaces.len(), 1);
        assert_eq!(spaces[0].name, "MOPs");
        assert_eq!(
            s.wiki_places(me).await.unwrap()[0].space,
            Some(spaces[0].id)
        );
        let tpl = s.get_note(me, tpl.id).await.unwrap();
        assert_eq!(tpl.body, "Swap at {{site}} for {{ticket}}");
        assert_eq!(tpl.title, "Swap at {{site}}");
        assert_eq!(tpl.kind, NoteKind::Mop);

        // Only once: a MOP written later stays a MOP.
        let later = s
            .create_note(
                me,
                &NoteInput {
                    kind: NoteKind::Mop,
                    ..page("Later")
                },
            )
            .await
            .unwrap();
        s.initialize_local_owner("Me").await.unwrap();
        assert_eq!(s.get_note(me, later.id).await.unwrap().kind, NoteKind::Mop);
        assert_eq!(s.wiki_spaces(me).await.unwrap().len(), 1);
    });
}
