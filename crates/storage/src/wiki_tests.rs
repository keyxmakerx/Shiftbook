//! Tests for the wiki's spaces, places, earlier versions and the move-in, against a real
//! encrypted database.

use shiftbook_core::dates::parse_date;
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

#[test]
fn a_mop_on_its_ticket() {
    use crate::change::MopFrom;
    use shiftbook_core::change::MopStatus;
    use time::macros::date;

    let dir = TestDir::new("mop-runs");
    block_on(async {
        let (s, me) = setup(&dir).await;
        s.tray_add(me, date!(2026 - 11 - 04), "CHG0146615")
            .await
            .unwrap();
        let mut tpl = NoteInput {
            kind: NoteKind::Mop,
            body: "# {{ticket}} at {{site}}\n1. Tell {{who}}\n2. Swap it".into(),
            ..page("Change for {{ticket}}")
        };
        tpl.template = true;
        let tpl = s.create_note(me, &tpl).await.unwrap();
        let values = vec![("site".to_string(), "Site B".to_string())];
        let mop = s
            .attach_mop(
                me,
                "chg0146615",
                MopFrom::Template {
                    id: tpl.id,
                    values,
                    title: String::new(),
                },
            )
            .await
            .unwrap();
        assert_eq!(mop.kind, NoteKind::Mop);
        assert_eq!(mop.title, "Change for CHG0146615");
        assert_eq!(
            mop.body,
            "# CHG0146615 at Site B\n1. Tell {{who}}\n2. Swap it"
        );
        // A ticket that isn't there: refused.
        assert!(s
            .attach_mop(me, "CHG404", MopFrom::Blank(String::new()))
            .await
            .is_err());

        let runs = s.mop_runs(me, Some("CHG0146615")).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, MopStatus::Draft);
        assert_eq!(runs[0].template, Some(*tpl.id.as_uuid()));
        assert!(s
            .templates_last_used(me)
            .await
            .unwrap()
            .contains_key(tpl.id.as_uuid()));

        s.set_mop_status(me, mop.id, MopStatus::Running)
            .await
            .unwrap();
        s.set_mop_window(me, mop.id, Some((date!(2026 - 11 - 04), 22 * 60)))
            .await
            .unwrap();
        assert!(s
            .set_mop_window(me, mop.id, Some((date!(2026 - 11 - 04), 24 * 60)))
            .await
            .is_err());
        let keys = shiftbook_core::mop::tick_keys(&mop.body);
        s.tick_mop(me, mop.id, &[keys[1].clone()]).await.unwrap();
        let run = s.mop_run(me, mop.id).await.unwrap().unwrap();
        assert_eq!(run.status, MopStatus::Running);
        assert!(run.started_at.is_some());
        assert_eq!(run.window, Some((date!(2026 - 11 - 04), 22 * 60)));
        assert_eq!(run.tick_times.len(), 1);
        assert_eq!(run.tick_times[0].0, keys[1]);
        // Unticking drops its time.
        s.tick_mop(me, mop.id, &[]).await.unwrap();
        assert!(s
            .mop_run(me, mop.id)
            .await
            .unwrap()
            .unwrap()
            .tick_times
            .is_empty());

        // From a page: copied in, the page stays.
        let how = make(&s, me, "Stack swap").await;
        let copy = s
            .attach_mop(me, "CHG0146615", MopFrom::Page(how))
            .await
            .unwrap();
        assert_eq!(copy.body, "About Stack swap");
        assert_eq!(s.get_note(me, how).await.unwrap().kind, NoteKind::Page);
        assert_eq!(s.mop_runs(me, None).await.unwrap().len(), 2);
    });
}

#[test]
fn a_mop_written_before_its_ticket() {
    let dir = TestDir::new("mop-link");
    block_on(async {
        let (s, me) = setup(&dir).await;
        let mop = s
            .create_note(
                me,
                &NoteInput {
                    kind: NoteKind::Mop,
                    ..page("Firewall swap")
                },
            )
            .await
            .unwrap();
        assert!(s.mop_run(me, mop.id).await.unwrap().is_none());
        // The ticket doesn't exist yet: it's made.
        let today = parse_date("2026-11-12").unwrap();
        assert!(s.link_mop(me, mop.id, " chg0099 ", today).await.unwrap());
        let run = s.mop_run(me, mop.id).await.unwrap().unwrap();
        assert_eq!(run.ticket, "CHG0099");
        // A ticket made just now is on no day, so it went on today's Tickets tray: it has a
        // card to show the MOP on, and the Tickets page can find it.
        let tray = s.tray_day(me, today).await.unwrap();
        assert_eq!(tray.items.len(), 1);
        assert_eq!(tray.items[0].ticket_number, "CHG0099");
        let listed = s
            .item_summaries(me, Some(&["chg0099".to_string()]))
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        // Already on a ticket: refused.
        assert!(s.link_mop(me, mop.id, "CHG0100", today).await.is_err());
        // Not a MOP, or two tickets at once: refused.
        let jot = make(&s, me, "Jot").await;
        assert!(s.link_mop(me, jot, "CHG0100", today).await.is_err());
        let other = s
            .create_note(
                me,
                &NoteInput {
                    kind: NoteKind::Mop,
                    ..page("Other")
                },
            )
            .await
            .unwrap();
        assert!(s.link_mop(me, other.id, "CHG1 CHG2", today).await.is_err());
        assert!(s.link_mop(me, other.id, "  ", today).await.is_err());
        // A ticket already on a day stays where it is: nothing is added to today.
        let later = parse_date("2026-11-13").unwrap();
        assert!(!s.link_mop(me, other.id, "CHG0099", later).await.unwrap());
        assert!(s.tray_day(me, later).await.unwrap().items.is_empty());
    });
}
