//! Tests for round 15's home pieces: Work / Home / Both, repeating things with Mark done,
//! and reusable lists, against a real encrypted database.

use shiftbook_core::home::UseMode;
use shiftbook_core::ids::OwnerId;
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::{EntryTarget, ListName};
use shiftbook_core::recurring::{summarize_recurring, RecurringState};
use time::macros::date;
use time::Date;

use crate::db::{database_path, open_encrypted, test_dek};
use crate::extras::CodeInput;
use crate::fsutil::test_dir::TestDir;
use crate::recurring::RecurringInput;
use crate::repo::{FrequencyRepository, ListOptionRepository};
use crate::test_rt::block_on;
use crate::work::EntryInput;
use crate::{SqliteStore, StorageError};

const DAY: Date = date!(2026 - 10 - 28);

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

async fn shown(s: &SqliteStore, me: OwnerId, list: ListName) -> Vec<String> {
    s.list_options(me, list, false)
        .await
        .unwrap()
        .into_iter()
        .map(|o| o.value)
        .collect()
}

#[test]
fn picking_home_changes_defaults_and_starter_lists() {
    let dir = TestDir::new("home-mode");
    block_on(async {
        let (s, me, _) = setup(&dir).await;
        assert_eq!(s.prefs(me).await.unwrap().use_mode, UseMode::Work);
        s.add_list_option(me, ListName::Category, "Garden", None)
            .await
            .unwrap();

        s.set_use_mode(me, UseMode::Home).await.unwrap();
        let p = s.prefs(me).await.unwrap();
        assert_eq!(p.use_mode, UseMode::Home);
        assert_eq!(p.day_targets, [0; 7]);
        assert_eq!(p.auto_lock_minutes, 60);
        let cats = shown(&s, me, ListName::Category).await;
        assert!(cats.contains(&"Money".to_string()));
        assert!(cats.contains(&"Admin".to_string()));
        assert!(
            cats.contains(&"Garden".to_string()),
            "user values are left alone"
        );
        assert!(!cats.contains(&"Troubleshooting".to_string()));
        let kinds = shown(&s, me, ListName::RecurringType).await;
        assert!(kinds.contains(&"Bills".to_string()));
        assert!(!kinds.contains(&"Patching".to_string()));

        // Back to Work: the work starters show again; the home ones step aside.
        s.set_use_mode(me, UseMode::Work).await.unwrap();
        let p = s.prefs(me).await.unwrap();
        assert_eq!(p.day_targets[0], 480);
        assert_eq!(p.auto_lock_minutes, 15);
        let cats = shown(&s, me, ListName::Category).await;
        assert!(cats.contains(&"Troubleshooting".to_string()));
        assert!(!cats.contains(&"Money".to_string()));
        assert!(cats.contains(&"Garden".to_string()));

        s.set_use_mode(me, UseMode::Both).await.unwrap();
        let cats = shown(&s, me, ListName::Category).await;
        assert!(
            cats.contains(&"Troubleshooting".to_string()) && cats.contains(&"Money".to_string())
        );
        assert_eq!(cats.iter().filter(|c| *c == "Admin").count(), 1);

        assert_eq!(s.last_open(me).await.unwrap(), None);
        s.set_last_open(me, DAY).await.unwrap();
        assert_eq!(s.last_open(me).await.unwrap(), Some(DAY));
    });
}

fn bins(frequency: Option<shiftbook_core::ids::FrequencyId>) -> RecurringInput {
    RecurringInput {
        name: " Bins out ".into(),
        recurring_type: Some("Home upkeep".into()),
        frequency_id: frequency,
        expiration_date: None,
        last_done_before_tracker: Some(date!(2026 - 10 - 22)),
        warn_days_before: Some(1),
        alerts_enabled: true,
        notes: String::new(),
        timesheet_code_id: None,
    }
}

#[test]
fn repeating_things_and_mark_done() {
    let dir = TestDir::new("recurring");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let weekly = s
            .list_frequencies(me)
            .await
            .unwrap()
            .into_iter()
            .find(|f| f.label == "Weekly")
            .unwrap();
        let r = s
            .create_recurring(me, &bins(Some(weekly.id)))
            .await
            .unwrap();
        assert_eq!(r.name, "Bins out");
        assert!(r.active && r.alerts_enabled);
        assert_eq!(s.recurring_all(other).await.unwrap().len(), 0);
        assert!(matches!(
            s.recurring_one(other, r.id).await,
            Err(StorageError::NotFound)
        ));
        // Someone else's frequency can't be used.
        let theirs = s.add_frequency(other, "Odd", 0, 9).await.unwrap();
        assert!(s
            .create_recurring(me, &bins(Some(theirs.id)))
            .await
            .is_err());

        let summary = |entries| summarize_recurring(&r, Some(&weekly), entries, DAY);
        let none = s.recurring_entries(me).await.unwrap();
        assert_eq!(summary(&none).next_due, Some(date!(2026 - 10 - 29)));
        assert_eq!(summary(&none).state, RecurringState::DueSoon);

        // Mark done adds a 0-minute Done entry on the day.
        let marked = s.mark_recurring_done(me, r.id, DAY).await.unwrap();
        assert!(marked.created);
        let entries = s.recurring_entries(me).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].minutes, Minutes(0));
        assert_eq!(entries[0].description, "Bins out");
        assert_eq!(entries[0].target, EntryTarget::Recurring(r.id));
        let after = summary(&entries);
        assert_eq!(after.last_done, Some(DAY));
        assert_eq!(after.next_due, Some(date!(2026 - 11 - 04)));
        assert_eq!(after.state, RecurringState::Ok);

        // It shows on the day, and editing it there keeps what it belongs to.
        let day = s.list_entries(me, DAY, DAY).await.unwrap();
        assert_eq!(day.len(), 1);
        let edited = s
            .update_entry(
                me,
                marked.entry_id,
                &EntryInput {
                    date: DAY,
                    minutes: Minutes(10),
                    description: "Bins and recycling out".into(),
                    project: None,
                    recurring: Some(r.id),
                    category: None,
                    next_step: None,
                    timesheet_code_id: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(edited.entry.target, EntryTarget::Recurring(r.id));

        // Marking again on the same day reuses that entry.
        let again = s.mark_recurring_done(me, r.id, DAY).await.unwrap();
        assert!(!again.created);
        assert_eq!(again.entry_id, marked.entry_id);
        s.undo_recurring_done(me, again).await.unwrap();
        let e = &s.recurring_entries(me).await.unwrap()[0];
        assert_eq!(e.status, None);
        assert_eq!(e.minutes, Minutes(10));
        s.undo_recurring_done(me, marked).await.unwrap();
        assert!(s.recurring_entries(me).await.unwrap().is_empty());
        assert!(matches!(
            s.undo_recurring_done(other, marked).await,
            Err(StorageError::NotFound)
        ));

        // Things that expire: a new expiry date after renewing.
        let mut car = bins(None);
        car.name = "Car insurance".into();
        car.expiration_date = Some(date!(2026 - 11 - 06));
        car.last_done_before_tracker = None;
        let car = s.create_recurring(me, &car).await.unwrap();
        let moved = s
            .set_recurring_expiry(me, car.id, date!(2027 - 11 - 06))
            .await
            .unwrap();
        assert_eq!(moved.expiration_date, Some(date!(2027 - 11 - 06)));

        let mut changed = bins(Some(weekly.id));
        changed.name = "Bins and recycling".into();
        let r2 = s.update_recurring(me, r.id, &changed).await.unwrap();
        assert_eq!(r2.name, "Bins and recycling");
        assert!(s.update_recurring(other, r.id, &changed).await.is_err());

        s.set_recurring_deleted(me, r.id, true).await.unwrap();
        assert_eq!(s.recurring_all(me).await.unwrap().len(), 1);
        assert!(s.mark_recurring_done(me, r.id, DAY).await.is_err());
        s.set_recurring_deleted(me, r.id, false).await.unwrap();
        assert_eq!(s.recurring_all(me).await.unwrap().len(), 2);
        let mut blank = bins(None);
        blank.name = "  ".into();
        assert!(s.create_recurring(me, &blank).await.is_err());
    });
}

#[test]
fn repeating_things_hold_work() {
    let dir = TestDir::new("recurringwork");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let mut cert = bins(None);
        cert.name = "www.example.com certificate".into();
        cert.expiration_date = Some(date!(2026 - 11 - 16));
        let cert = s.create_recurring(me, &cert).await.unwrap();
        let input = |recurring| EntryInput {
            date: DAY,
            minutes: Minutes(90),
            description: "Renew it".into(),
            project: None,
            recurring,
            category: None,
            next_step: None,
            timesheet_code_id: None,
        };
        let rec = s
            .create_entry(me, &input(Some(cert.id)), "RITM0034567")
            .await
            .unwrap();
        assert_eq!(rec.entry.target, EntryTarget::Recurring(cert.id));
        // Work filed under it is not "done": the reminder still stands.
        let entries = s.recurring_entries(me).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, None);
        // Moved off it, it belongs to nothing.
        let moved = s
            .update_entry(me, rec.entry.id, &input(None))
            .await
            .unwrap();
        assert_eq!(moved.entry.target, EntryTarget::None);
        // Someone else's, or a removed one, can't be used.
        let theirs = s.create_recurring(other, &bins(None)).await.unwrap();
        assert!(matches!(
            s.create_entry(me, &input(Some(theirs.id)), "").await,
            Err(StorageError::NotFound)
        ));
        s.set_recurring_deleted(me, cert.id, true).await.unwrap();
        assert!(s
            .update_entry(me, rec.entry.id, &input(Some(cert.id)))
            .await
            .is_err());
    });
}

#[test]
fn lists_tick_reset_and_stay_apart_from_tasks() {
    let dir = TestDir::new("lists");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let l = s
            .create_list(
                me,
                " Before we go away ",
                "- Passports\n- Chargers\n\n[ ] Plants",
            )
            .await
            .unwrap();
        assert_eq!(l.list.title, "Before we go away");
        let texts: Vec<&str> = l.items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(texts, ["Passports", "Chargers", "Plants"]);
        assert!(s.create_list(me, "", "").await.is_err());

        let l = s
            .add_list_items(me, l.list.id, "Lock the back door")
            .await
            .unwrap();
        assert_eq!(l.items.len(), 4);
        assert!(s.add_list_items(me, l.list.id, "  \n ").await.is_err());
        let first = l.items[0].id;
        let l = s.set_list_item_done(me, first, true).await.unwrap();
        assert!(l.items[0].done);
        assert!(s.set_list_item_done(other, first, false).await.is_err());
        let l = s
            .set_list_item_text(me, first, "Passports and tickets")
            .await
            .unwrap();
        assert_eq!(l.items[0].text, "Passports and tickets");
        assert!(l.items[0].done);

        let l = s.reset_list(me, l.list.id).await.unwrap();
        assert!(l.items.iter().all(|i| !i.done));
        assert_eq!(l.items.len(), 4);

        let l = s.set_list_item_deleted(me, first, true).await.unwrap();
        assert_eq!(l.items.len(), 3);
        let l = s.set_list_item_deleted(me, first, false).await.unwrap();
        assert_eq!(l.items.len(), 4);

        let l = s.rename_list(me, l.list.id, "Holiday").await.unwrap();
        assert_eq!(l.list.title, "Holiday");
        assert_eq!(s.lists(me).await.unwrap().len(), 1);
        assert!(s.lists(other).await.unwrap().is_empty());

        // An ordinary task is not a list.
        sqlx::query(
            "INSERT INTO tasks (id, owner_id, title, created_at, updated_at) \
             VALUES ($1, $2, 'Plain task', '2026-10-01T00:00:00Z', '2026-10-01T00:00:00Z')",
        )
        .bind(shiftbook_core::ids::TaskId::new().to_string())
        .bind(me.to_string())
        .execute(s.pool())
        .await
        .unwrap();
        assert_eq!(s.lists(me).await.unwrap().len(), 1);

        s.set_list_deleted(me, l.list.id, true).await.unwrap();
        assert!(s.lists(me).await.unwrap().is_empty());
        assert!(s.add_list_items(me, l.list.id, "x").await.is_err());
        s.set_list_deleted(me, l.list.id, false).await.unwrap();
        assert_eq!(s.lists(me).await.unwrap().len(), 1);
    });
}

#[test]
fn tray_tickets_count_under_a_repeating_thing() {
    let dir = TestDir::new("recurringtray");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let mut cert = bins(None);
        cert.name = "www.example.com certificate".into();
        let cert = s.create_recurring(me, &cert).await.unwrap();
        let day = s.tray_add(me, DAY, "RITM0034567 INC0045678").await.unwrap();
        let ritm = day.items[0].clone();
        let day = s
            .tray_set_recurring(me, ritm.id, Some(cert.id))
            .await
            .unwrap();
        assert_eq!(day.items[0].recurring_id, Some(cert.id));
        assert_eq!(day.items[0].project_id, None);
        // Its minutes moved to a roll-up logged against the certificate.
        let entries = s.list_entries(me, DAY, DAY).await.unwrap();
        let rollup = entries
            .iter()
            .find(|e| e.entry.target == EntryTarget::Recurring(cert.id))
            .unwrap();
        assert!(rollup.is_tray);
        assert_eq!(rollup.entry.minutes, Minutes(15));
        let plain = entries
            .iter()
            .find(|e| e.is_tray && e.entry.target == EntryTarget::None)
            .unwrap();
        assert_eq!(plain.tickets.len(), 1);
        // Editing the roll-up keeps it with the certificate.
        let kept = s
            .update_entry(
                me,
                rollup.entry.id,
                &EntryInput {
                    date: DAY,
                    minutes: Minutes(5),
                    description: "Renewal".into(),
                    project: None,
                    recurring: None,
                    category: None,
                    next_step: None,
                    timesheet_code_id: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(kept.entry.target, EntryTarget::Recurring(cert.id));
        assert_eq!(kept.entry.minutes, Minutes(15));
        // Carried to the next day, it stays with the certificate.
        let next = date!(2026 - 10 - 23);
        let carried = s.tray_carry(me, ritm.id, next).await.unwrap();
        assert_eq!(carried.items[0].recurring_id, Some(cert.id));
        // Deleting the roll-up takes only its own tickets out; Undo brings them back.
        s.delete_entry(me, rollup.entry.id).await.unwrap();
        let left = s.tray_day(me, DAY).await.unwrap();
        assert_eq!(left.items.len(), 1);
        assert_eq!(left.items[0].ticket_number, "INC0045678");
        s.restore_entry(me, rollup.entry.id).await.unwrap();
        assert_eq!(s.tray_day(me, DAY).await.unwrap().items.len(), 2);
        // Back to the plain entry; someone else's repeating thing is refused.
        let theirs = s.create_recurring(other, &bins(None)).await.unwrap();
        assert!(s
            .tray_set_recurring(me, ritm.id, Some(theirs.id))
            .await
            .is_err());
        let day = s.tray_set_recurring(me, ritm.id, None).await.unwrap();
        assert!(day.items.iter().all(|i| i.recurring_id.is_none()));
        let entries = s.list_entries(me, DAY, DAY).await.unwrap();
        assert!(entries
            .iter()
            .all(|e| e.entry.target != EntryTarget::Recurring(cert.id)));
    });
}

#[test]
fn repeating_things_and_kinds_keep_codes() {
    let dir = TestDir::new("recurringcodes");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let code = |c: &str| CodeInput {
            area: "Operations".into(),
            phase_label: String::new(),
            code: c.into(),
            sort_order: 0,
            hidden: false,
        };
        let certs = s
            .add_timesheet_code(me, &code("Certificates"))
            .await
            .unwrap();
        let own = s
            .add_timesheet_code(me, &code("Web renewals"))
            .await
            .unwrap();

        // A thing's own code is saved, changed and cleared.
        let mut input = bins(None);
        input.timesheet_code_id = Some(own.id);
        let thing = s.create_recurring(me, &input).await.unwrap();
        assert_eq!(thing.default_timesheet_code_id, Some(own.id));
        input.timesheet_code_id = None;
        let thing = s.update_recurring(me, thing.id, &input).await.unwrap();
        assert_eq!(thing.default_timesheet_code_id, None);
        // Someone else's code can't be used.
        let mut theirs = bins(None);
        theirs.timesheet_code_id = Some(own.id);
        assert!(matches!(
            s.create_recurring(other, &theirs).await,
            Err(StorageError::NotFound)
        ));

        // A kind's code: one per kind, ignoring case.
        s.set_kind_code(me, "Certificates", Some(own.id))
            .await
            .unwrap();
        s.set_kind_code(me, " certificates ", Some(certs.id))
            .await
            .unwrap();
        assert_eq!(
            s.list_kind_codes(me).await.unwrap(),
            vec![("Certificates".to_string(), certs.id)]
        );
        assert!(s.list_kind_codes(other).await.unwrap().is_empty());
        assert!(matches!(
            s.set_kind_code(other, "Certificates", Some(certs.id)).await,
            Err(StorageError::NotFound)
        ));
        s.set_kind_code(me, "CERTIFICATES", None).await.unwrap();
        assert!(s.list_kind_codes(me).await.unwrap().is_empty());
    });
}
