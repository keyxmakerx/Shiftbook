//! Tickets kept in step everywhere they appear (the 27 Oct review): back-dated work, removed
//! tickets, repeating things and a long mix of actions, each checked with
//! [`crate::items::assert_in_step`].

use shiftbook_core::ids::OwnerId;
use shiftbook_core::minutes::Minutes;
use time::macros::date;
use time::Date;

use crate::db::{database_path, open_encrypted, test_dek};
use crate::fsutil::test_dir::TestDir;
use crate::items::assert_in_step;
use crate::recurring::RecurringInput;
use crate::test_rt::block_on;
use crate::work::EntryInput;
use crate::SqliteStore;

async fn setup(dir: &TestDir) -> (SqliteStore, OwnerId) {
    let s = SqliteStore::new(
        open_encrypted(&database_path(dir.path()), &test_dek(7))
            .await
            .unwrap(),
    );
    let me = s.initialize_local_owner("Me").await.unwrap().id;
    (s, me)
}

fn input(date: Date, minutes: u32, description: &str) -> EntryInput {
    EntryInput {
        date,
        minutes: Minutes(minutes),
        description: description.into(),
        project: None,
        recurring: None,
        category: None,
        next_step: None,
        timesheet_code_id: None,
    }
}

fn certs() -> RecurringInput {
    RecurringInput {
        name: "Certificates".into(),
        recurring_type: None,
        frequency_id: None,
        expiration_date: None,
        last_done_before_tracker: None,
        warn_days_before: None,
        alerts_enabled: true,
        notes: String::new(),
        timesheet_code_id: None,
    }
}

async fn carried(s: &SqliteStore, me: OwnerId, day: Date) -> Vec<String> {
    s.tray_carried(me, day)
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.item.ticket_number)
        .collect()
}

#[test]
fn back_dated_work_leaves_a_closed_ticket_closed() {
    let dir = TestDir::new("items-backdate");
    block_on(async {
        let (s, me) = setup(&dir).await;
        let day = s.tray_add(me, date!(2026 - 10 - 20), "INC1").await.unwrap();
        s.tray_set_done(me, day.items[0].id, true).await.unwrap();
        // Forgotten work from the week before, on the same ticket: on an entry and in a tray.
        let rec = s
            .create_entry(
                me,
                &input(date!(2026 - 10 - 10), 30, "Earlier work"),
                "INC1",
            )
            .await
            .unwrap();
        s.tray_add(me, date!(2026 - 10 - 12), "INC1").await.unwrap();
        assert!(
            rec.tickets[0].done,
            "back-dated work takes the ticket as it is"
        );
        assert!(s.tray_day(me, date!(2026 - 10 - 20)).await.unwrap().items[0].done);
        assert!(s.tray_day(me, date!(2026 - 10 - 12)).await.unwrap().items[0].done);
        assert!(carried(&s, me, date!(2026 - 10 - 27)).await.is_empty());
        assert_in_step(&s, me).await;

        // Working on it again after it was closed still reopens it.
        s.tray_add(me, date!(2026 - 10 - 22), "INC1").await.unwrap();
        assert_eq!(carried(&s, me, date!(2026 - 10 - 27)).await, ["INC1"]);
        assert_in_step(&s, me).await;
    });
}

#[test]
fn back_dated_work_on_an_open_ticket_keeps_it_open() {
    let dir = TestDir::new("items-backdate-open");
    block_on(async {
        let (s, me) = setup(&dir).await;
        s.tray_add(me, date!(2026 - 10 - 20), "INC2").await.unwrap();
        let rec = s
            .create_entry(me, &input(date!(2026 - 10 - 10), 30, "Earlier"), "INC2")
            .await
            .unwrap();
        assert!(!rec.tickets[0].done);
        assert_eq!(carried(&s, me, date!(2026 - 10 - 27)).await, ["INC2"]);
        assert_in_step(&s, me).await;
    });
}

#[test]
fn removed_tickets_leave_the_items_view() {
    let dir = TestDir::new("items-removed");
    block_on(async {
        let (s, me) = setup(&dir).await;
        // A typo taken out of the tray, and a ticket on an entry that is deleted.
        let day = s
            .tray_add(me, date!(2026 - 10 - 27), "INC00456781")
            .await
            .unwrap();
        s.tray_remove(me, day.items[0].id).await.unwrap();
        let rec = s
            .create_entry(me, &input(date!(2026 - 10 - 27), 30, "x"), "TYPO1")
            .await
            .unwrap();
        s.delete_entry(me, rec.entry.id).await.unwrap();
        // Done on the 20th, open again on the 26th, then the 26th taken out: it's done.
        let d1 = s.tray_add(me, date!(2026 - 10 - 20), "INC9").await.unwrap();
        s.tray_set_done(me, d1.items[0].id, true).await.unwrap();
        let d2 = s.tray_add(me, date!(2026 - 10 - 26), "INC9").await.unwrap();
        s.tray_remove(me, d2.items[0].id).await.unwrap();

        let items = s.item_summaries(me, None).await.unwrap();
        let shown: Vec<(&str, bool)> = items.iter().map(|i| (i.number.as_str(), i.done)).collect();
        assert_eq!(shown, [("INC9", true)]);
        assert!(carried(&s, me, date!(2026 - 10 - 27)).await.is_empty());
        assert_in_step(&s, me).await;

        // Undo brings them back as they were.
        s.tray_restore(me, d2.items[0].id).await.unwrap();
        s.tray_restore(me, day.items[0].id).await.unwrap();
        s.restore_entry(me, rec.entry.id).await.unwrap();
        let open: Vec<String> = s
            .item_summaries(me, None)
            .await
            .unwrap()
            .into_iter()
            .filter(|i| !i.done)
            .map(|i| i.number)
            .collect();
        assert_eq!(open.len(), 3, "{open:?}");
        assert_in_step(&s, me).await;
    });
}

#[test]
fn a_repeating_thing_keeps_its_done_when_its_tickets_move() {
    let dir = TestDir::new("items-recurring");
    block_on(async {
        let (s, me) = setup(&dir).await;
        let r = s.create_recurring(me, &certs()).await.unwrap();
        let d = date!(2026 - 10 - 27);
        let day = s.tray_add(me, d, "CHG1").await.unwrap();
        s.tray_set_recurring(me, day.items[0].id, Some(r.id))
            .await
            .unwrap();
        let marked = s.mark_recurring_done(me, r.id, d).await.unwrap();
        assert!(marked.created, "the Done is its own entry, not the tray's");
        s.tray_set_recurring(me, day.items[0].id, None)
            .await
            .unwrap();
        let (history, _) = s.recurring_history(me, r.id).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0.status.as_deref(), Some("Done"));
        assert_in_step(&s, me).await;
    });
}

#[test]
fn a_mix_of_actions_keeps_every_copy_in_step() {
    let dir = TestDir::new("items-mix");
    block_on(async {
        let (s, me) = setup(&dir).await;
        let p = s.create_project(me, "Rollout", None).await.unwrap();
        let mon = date!(2026 - 10 - 19);
        let tue = date!(2026 - 10 - 20);
        let wed = date!(2026 - 10 - 21);
        let day = s.tray_add(me, mon, "INC1 INC2 REQ3").await.unwrap();
        assert_in_step(&s, me).await;
        s.tray_set_note(me, day.items[0].id, "called the user")
            .await
            .unwrap();
        s.tray_set_project(me, day.items[1].id, Some(p.id))
            .await
            .unwrap();
        s.tray_set_minutes(me, day.items[2].id, 45).await.unwrap();
        assert_in_step(&s, me).await;
        let rec = s
            .create_entry(me, &input(tue, 60, "Printer swap"), "inc1, CHG9")
            .await
            .unwrap();
        assert_in_step(&s, me).await;
        s.set_ticket_note(me, rec.tickets[0].id, "parts ordered")
            .await
            .unwrap();
        s.set_ticket_done(me, rec.tickets[0].id, true)
            .await
            .unwrap();
        assert_in_step(&s, me).await;
        let carry = s.tray_carried(me, wed).await.unwrap();
        for c in carry.iter().filter(|c| c.entry.is_none()) {
            s.tray_carry(me, c.item.id, wed).await.unwrap();
        }
        assert_in_step(&s, me).await;
        let w = s.tray_day(me, wed).await.unwrap();
        s.tray_set_done(me, w.items[0].id, true).await.unwrap();
        s.tray_set_done(me, w.items[0].id, false).await.unwrap();
        s.tray_remove(me, w.items[1].id).await.unwrap();
        assert_in_step(&s, me).await;
        s.remove_ticket(me, rec.tickets[1].id).await.unwrap();
        s.delete_entry(me, rec.entry.id).await.unwrap();
        assert_in_step(&s, me).await;
        s.restore_entry(me, rec.entry.id).await.unwrap();
        let tickets_entry = s.tray_day(me, mon).await.unwrap().entry_id.unwrap();
        s.delete_entry(me, tickets_entry).await.unwrap();
        assert_in_step(&s, me).await;
        s.restore_entry(me, tickets_entry).await.unwrap();
        assert_in_step(&s, me).await;
    });
}
