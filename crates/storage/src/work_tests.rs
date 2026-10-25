//! Tests for the alpha repositories (projects, entries, tickets, tray, Log work,
//! timesheet codes, tick-offs, custom fields, Inbox, preferences) against a real
//! encrypted database.

use shiftbook_core::capture::{parse_capture, CaptureProject};
use shiftbook_core::fields::{AppliesTo, FieldKind};
use shiftbook_core::ids::{OwnerId, TimeEntryId};
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::EntryTarget;
use shiftbook_core::prefs::{Motion, Theme, KEY_COMMENT_SEPARATOR, KEY_MOTION, KEY_THEME};
use shiftbook_core::responses::ResponseDraft;
use shiftbook_core::week::WeekStart;
use shiftbook_core::CoreError;
use time::macros::date;
use time::Date;

use crate::db::{database_path, open_encrypted, test_dek};
use crate::extras::{CodeInput, FieldInput};
use crate::fsutil::test_dir::TestDir;
use crate::test_rt::block_on;
use crate::work::{EntryInput, ProjectInput};
use crate::{SqliteStore, StorageError};

const DAY: Date = date!(2026 - 10 - 15);

async fn setup(dir: &TestDir) -> (SqliteStore, OwnerId, OwnerId) {
    let s = SqliteStore::new(
        open_encrypted(&database_path(dir.path()), &test_dek(5))
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

fn input(date: Date, minutes: u32, description: &str) -> EntryInput {
    EntryInput {
        date,
        minutes: Minutes(minutes),
        description: description.into(),
        project: None,
        category: None,
        next_step: None,
        timesheet_code_id: None,
    }
}

fn numbers(rec: &crate::work::EntryRecord) -> Vec<String> {
    rec.tickets
        .iter()
        .map(|t| t.ticket_number.clone())
        .collect()
}

#[test]
fn projects_colours_names_and_archive() {
    let dir = TestDir::new("projects");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let a = s.create_project(me, " Network ", None).await.unwrap();
        assert_eq!(a.name, "Network");
        assert_eq!(a.color.as_deref(), Some("#3d5bb0"));
        let b = s.create_project(me, "Data centre", None).await.unwrap();
        assert_eq!(b.color.as_deref(), Some("#148c79"), "next unused colour");
        let c = s
            .create_project(me, "Admin", Some("#ABCDEF"))
            .await
            .unwrap();
        assert_eq!(c.color.as_deref(), Some("#abcdef"));
        assert!(matches!(
            s.create_project(me, "network", None).await,
            Err(StorageError::Duplicate(_))
        ));
        assert!(matches!(
            s.create_project(me, "  ", None).await,
            Err(StorageError::Invalid(CoreError::Required { .. }))
        ));
        assert!(s.create_project(me, "X", Some("red")).await.is_err());

        let code = s
            .add_timesheet_code(
                me,
                &CodeInput {
                    area: "Ops".into(),
                    phase_label: String::new(),
                    code: "1 - Support".into(),
                    sort_order: 0,
                    hidden: false,
                },
            )
            .await
            .unwrap();
        let edited = s
            .update_project(
                me,
                a.id,
                &ProjectInput {
                    name: "Network refresh".into(),
                    color: Some("#112233".into()),
                    default_timesheet_code_id: Some(code.id),
                    archived: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(edited.name, "Network refresh");
        assert_eq!(edited.color.as_deref(), Some("#112233"));
        assert_eq!(edited.default_timesheet_code_id, Some(code.id));
        assert!(edited.archived);
        let names: Vec<String> = s
            .list_projects(me)
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(
            names,
            ["Admin", "Data centre", "Network refresh"],
            "archived last"
        );

        // Another owner sees and changes nothing of mine.
        assert!(s.list_projects(other).await.unwrap().is_empty());
        let mut edit = ProjectInput {
            name: "Mine now".into(),
            color: None,
            default_timesheet_code_id: None,
            archived: false,
        };
        assert!(matches!(
            s.update_project(other, b.id, &edit).await,
            Err(StorageError::NotFound)
        ));
        // ...nor use my codes.
        let theirs = s.create_project(other, "Theirs", None).await.unwrap();
        edit.default_timesheet_code_id = Some(code.id);
        assert!(matches!(
            s.update_project(other, theirs.id, &edit).await,
            Err(StorageError::NotFound)
        ));
    });
}

#[test]
fn entries_and_tickets() {
    let dir = TestDir::new("entries");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let p = s.create_project(me, "Network", None).await.unwrap();
        let mut first = input(DAY, 90, " Firewall cleanup ");
        first.project = Some(p.id);
        first.next_step = Some("Remove the old rule Friday".into());
        let rec = s
            .create_entry(me, &first, "CHG0012345\nINC1, inc1; INC2")
            .await
            .unwrap();
        assert_eq!(rec.entry.description, "Firewall cleanup");
        assert_eq!(rec.entry.target, EntryTarget::Project(p.id));
        assert_eq!(numbers(&rec), ["CHG0012345", "INC1", "INC2"]);
        assert!(!rec.is_tray);
        s.create_entry(me, &input(date!(2026 - 10 - 14), 30, "Yesterday"), "")
            .await
            .unwrap();
        s.create_entry(me, &input(date!(2026 - 10 - 20), 30, "Later"), "")
            .await
            .unwrap();

        let week = s
            .list_entries(me, date!(2026 - 10 - 12), date!(2026 - 10 - 18))
            .await
            .unwrap();
        let titles: Vec<&str> = week.iter().map(|r| r.entry.description.as_str()).collect();
        assert_eq!(titles, ["Yesterday", "Firewall cleanup"]);
        assert_eq!(week[1].tickets.len(), 3);
        assert!(s.list_entries(other, DAY, DAY).await.unwrap().is_empty());

        // Minutes: add, take away (never below 0), refuse over 24 h.
        let id = rec.entry.id;
        assert_eq!(
            s.add_entry_minutes(me, id, 15).await.unwrap().entry.minutes,
            Minutes(105)
        );
        assert_eq!(
            s.add_entry_minutes(me, id, -500)
                .await
                .unwrap()
                .entry
                .minutes,
            Minutes(0)
        );
        assert!(matches!(
            s.add_entry_minutes(me, id, 1441).await,
            Err(StorageError::Invalid(CoreError::HoursTooLarge))
        ));
        assert!(matches!(
            s.add_entry_minutes(other, id, 15).await,
            Err(StorageError::NotFound)
        ));

        // Update.
        let mut edit = input(DAY, 60, "Firewall rules");
        edit.category = Some(" Meetings ".into());
        let updated = s.update_entry(me, id, &edit).await.unwrap();
        assert_eq!(updated.entry.minutes, Minutes(60));
        assert_eq!(updated.entry.target, EntryTarget::None);
        assert_eq!(updated.entry.category.as_deref(), Some("Meetings"));
        assert_eq!(updated.entry.next_step, None);
        let mut bad = edit.clone();
        bad.project = Some(shiftbook_core::ids::ProjectId::new());
        assert!(matches!(
            s.update_entry(me, id, &bad).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            s.update_entry(other, id, &edit).await,
            Err(StorageError::NotFound)
        ));

        // Tickets.
        let more = s.add_entry_tickets(me, id, "INC2 INC3,INC4").await.unwrap();
        assert_eq!(
            numbers(&more),
            ["CHG0012345", "INC1", "INC2", "INC3", "INC4"]
        );
        let less = s.remove_ticket(me, more.tickets[0].id).await.unwrap();
        assert_eq!(numbers(&less), ["INC1", "INC2", "INC3", "INC4"]);
        assert!(matches!(
            s.remove_ticket(other, more.tickets[1].id).await,
            Err(StorageError::NotFound)
        ));

        // Delete and restore.
        s.delete_entry(me, id).await.unwrap();
        assert!(matches!(
            s.get_entry(me, id).await,
            Err(StorageError::NotFound)
        ));
        assert_eq!(s.list_entries(me, DAY, DAY).await.unwrap().len(), 0);
        let back = s.restore_entry(me, id).await.unwrap();
        assert_eq!(back.tickets.len(), 4);
        assert!(matches!(
            s.restore_entry(me, id).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            s.delete_entry(other, id).await,
            Err(StorageError::NotFound)
        ));
    });
}

#[test]
fn tray_rolls_up_into_one_entry_per_day() {
    let dir = TestDir::new("tray");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let day = s
            .tray_add(me, DAY, "INC0045601\nINC0045602, INC0045601")
            .await
            .unwrap();
        assert_eq!(day.items.len(), 2);
        assert_eq!(day.minutes, Minutes(30));
        let entry_id = day.entry_id.unwrap();
        let rec = s.get_entry(me, entry_id).await.unwrap();
        assert!(rec.is_tray);
        assert_eq!(rec.entry.description, "Tickets");
        assert_eq!(numbers(&rec), ["INC0045601", "INC0045602"]);

        let day = s
            .tray_add(me, DAY, "inc0045602 INC0045603 - printer")
            .await
            .unwrap();
        assert_eq!(day.items.len(), 3, "repeat skipped, words ignored");
        assert_eq!(day.entry_id, Some(entry_id), "same roll-up entry");
        let day = s.tray_set_minutes(me, day.items[0].id, 45).await.unwrap();
        assert_eq!(day.minutes, Minutes(75));
        assert!(s.tray_set_minutes(me, day.items[0].id, 481).await.is_err());
        assert!(matches!(
            s.tray_set_minutes(other, day.items[0].id, 5).await,
            Err(StorageError::NotFound)
        ));

        // A few words on a ticket; blank clears them; minutes are untouched.
        let day = s
            .tray_set_note(me, day.items[1].id, "  reset the printer ")
            .await
            .unwrap();
        assert_eq!(day.items[1].note.as_deref(), Some("reset the printer"));
        assert_eq!(day.minutes, Minutes(75));
        let day = s.tray_set_note(me, day.items[1].id, " ").await.unwrap();
        assert_eq!(day.items[1].note, None);
        assert!(s
            .tray_set_note(
                me,
                day.items[1].id,
                &"x".repeat(shiftbook_core::tray::MAX_TRAY_NOTE_CHARS + 1)
            )
            .await
            .is_err());
        assert!(matches!(
            s.tray_set_note(other, day.items[1].id, "hi").await,
            Err(StorageError::NotFound)
        ));

        // The roll-up keeps the user's title but not their minutes.
        let mut edit = input(DAY, 999, "Ticket queue");
        edit.date = date!(2026 - 10 - 01);
        let renamed = s.update_entry(me, entry_id, &edit).await.unwrap();
        assert_eq!(renamed.entry.description, "Ticket queue");
        assert_eq!(renamed.entry.minutes, Minutes(75));
        assert_eq!(renamed.entry.date, DAY);
        assert!(matches!(
            s.add_entry_minutes(me, entry_id, 15).await,
            Err(StorageError::TrayManaged)
        ));
        assert!(matches!(
            s.add_entry_tickets(me, entry_id, "X1").await,
            Err(StorageError::TrayManaged)
        ));

        let removed = s.tray_remove(me, day.items[1].id).await.unwrap();
        assert_eq!(removed.items.len(), 2);
        assert_eq!(removed.minutes, Minutes(60));
        let rec = s.get_entry(me, entry_id).await.unwrap();
        assert_eq!(numbers(&rec), ["INC0045601", "INC0045603"]);
        assert_eq!(rec.entry.description, "Ticket queue");
        let restored = s.tray_restore(me, day.items[1].id).await.unwrap();
        assert_eq!(restored.minutes, Minutes(75));

        // Empty tray: the roll-up goes away.
        for item in &restored.items {
            s.tray_remove(me, item.id).await.unwrap();
        }
        let empty = s.tray_day(me, DAY).await.unwrap();
        assert!(empty.items.is_empty());
        assert_eq!(empty.entry_id, None);
        assert!(matches!(
            s.get_entry(me, entry_id).await,
            Err(StorageError::NotFound)
        ));

        // Deleting the roll-up empties the tray; restoring brings both back.
        let day = s.tray_add(me, DAY, "INC1111 INC2222").await.unwrap();
        let entry_id = day.entry_id.unwrap();
        s.delete_entry(me, entry_id).await.unwrap();
        assert!(s.tray_day(me, DAY).await.unwrap().items.is_empty());
        let back = s.restore_entry(me, entry_id).await.unwrap();
        assert_eq!(back.entry.minutes, Minutes(30));
        assert_eq!(s.tray_day(me, DAY).await.unwrap().items.len(), 2);

        // Other days and owners are separate.
        assert!(s
            .tray_day(me, date!(2026 - 10 - 14))
            .await
            .unwrap()
            .items
            .is_empty());
        assert!(s.tray_day(other, DAY).await.unwrap().items.is_empty());
        assert_eq!(s.list_entries(me, DAY, DAY).await.unwrap().len(), 1);
    });
}

#[test]
fn ticket_copies_tick_together_or_not_at_all() {
    let dir = TestDir::new("ticketcopies");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let rec = s
            .create_entry(me, &input(DAY, 60, "Closet move"), "CHG0146615")
            .await
            .unwrap();
        let entry_ticket = rec.tickets[0].id;
        let tray = s.tray_add(me, DAY, "CHG0146615").await.unwrap().items[0].id;

        s.set_ticket_copies_done(me, &[entry_ticket], &[tray], true)
            .await
            .unwrap();
        assert!(s.get_entry(me, rec.entry.id).await.unwrap().tickets[0].done);
        assert!(s.tray_day(me, DAY).await.unwrap().items[0].done);

        // A copy that isn't found stops the lot: nothing is left half ticked.
        let missing = shiftbook_core::ids::TrayItemId::new();
        assert!(matches!(
            s.set_ticket_copies_done(me, &[entry_ticket], &[tray, missing], false)
                .await,
            Err(StorageError::NotFound)
        ));
        assert!(s.get_entry(me, rec.entry.id).await.unwrap().tickets[0].done);
        assert!(s.tray_day(me, DAY).await.unwrap().items[0].done);

        // Someone else's tickets can't be ticked.
        assert!(s
            .set_ticket_copies_done(other, &[entry_ticket], &[], false)
            .await
            .is_err());

        s.set_ticket_copies_done(me, &[entry_ticket], &[tray], false)
            .await
            .unwrap();
        assert!(!s.get_entry(me, rec.entry.id).await.unwrap().tickets[0].done);
        assert!(!s.tray_day(me, DAY).await.unwrap().items[0].done);
    });
}

#[test]
fn entry_tickets_have_a_state_and_a_notepad() {
    let dir = TestDir::new("entrytickets");
    block_on(async {
        let (s, me, _other) = setup(&dir).await;
        let rec = s
            .create_entry(me, &input(DAY, 60, "VPN issue"), "INC1001 INC1002")
            .await
            .unwrap();
        // New tickets start still open, and the bell can list them.
        assert!(rec.tickets.iter().all(|t| !t.done));
        assert_eq!(
            s.open_entry_tickets(me).await.unwrap(),
            ["INC1001", "INC1002"]
        );
        let first = rec.tickets[0].id;
        let rec = s.set_ticket_done(me, first, true).await.unwrap();
        assert!(rec.tickets[0].done && !rec.tickets[1].done);
        assert_eq!(s.open_entry_tickets(me).await.unwrap(), ["INC1002"]);
        let rec = s
            .set_ticket_note(me, first, "user can't reach the VPN from home")
            .await
            .unwrap();
        assert_eq!(
            rec.tickets[0].note.as_deref(),
            Some("user can't reach the VPN from home")
        );
        let rec = s.set_ticket_note(me, first, "  ").await.unwrap();
        assert_eq!(rec.tickets[0].note, None);
        // The entry's time is untouched.
        assert_eq!(rec.entry.minutes, Minutes(60));
        // Tray roll-up tickets aren't counted: their state is in the tray.
        s.tray_add(me, DAY, "INC2001").await.unwrap();
        assert_eq!(s.open_entry_tickets(me).await.unwrap(), ["INC1002"]);
    });
}

#[test]
fn a_jot_is_logged_as_an_entry() {
    use crate::notes::NoteInput;
    use shiftbook_core::notes::NoteKind;
    let dir = TestDir::new("logjot");
    block_on(async {
        let (s, me, _other) = setup(&dir).await;
        let p = s.create_project(me, "Network", None).await.unwrap();
        let jot = s
            .create_note(
                me,
                &NoteInput {
                    kind: NoteKind::Jot,
                    title: String::new(),
                    body: "PSUs for the Meraki switch\nasked Sam, CHG0012345".into(),
                    project: Some(p.id),
                    tags: String::new(),
                },
            )
            .await
            .unwrap();
        let rec = s.log_note(me, jot.id, DAY, 30).await.unwrap();
        assert_eq!(rec.entry.description, "PSUs for the Meraki switch");
        assert_eq!(rec.entry.date, DAY);
        assert_eq!(rec.entry.minutes, Minutes(30));
        assert_eq!(
            rec.entry.next_step.as_deref(),
            Some("asked Sam, CHG0012345")
        );
        assert_eq!(rec.entry.target, EntryTarget::Project(p.id));
        assert_eq!(numbers(&rec), ["CHG0012345"]);
        // The jot moves to the archive.
        assert!(s.get_note(me, jot.id).await.unwrap().archived_at.is_some());
    });
}

#[test]
fn added_time_lands_on_the_day_worked() {
    let dir = TestDir::new("addtime");
    block_on(async {
        let (s, me, _other) = setup(&dir).await;
        let p = s.create_project(me, "Network", None).await.unwrap();
        let mut old = input(date!(2026 - 10 - 13), 60, "Closet cleanup");
        old.project = Some(p.id);
        let old = s.create_entry(me, &old, "CHG0012345").await.unwrap();

        // +15m on last week's entry makes a copy today; the old day is untouched.
        let (copy, made) = s.add_time_on(me, old.entry.id, DAY, 15).await.unwrap();
        assert!(made);
        assert_ne!(copy.entry.id, old.entry.id);
        assert_eq!(copy.entry.date, DAY);
        assert_eq!(copy.entry.minutes, Minutes(15));
        assert_eq!(copy.entry.target, EntryTarget::Project(p.id));
        assert_eq!(numbers(&copy), ["CHG0012345"]);
        let again = s.get_entry(me, old.entry.id).await.unwrap();
        assert_eq!(again.entry.minutes, Minutes(60));

        // Again: it adds to today's copy; on the copy itself, it adds to it.
        let (more, made) = s.add_time_on(me, old.entry.id, DAY, 30).await.unwrap();
        assert!(!made);
        assert_eq!(more.entry.id, copy.entry.id);
        assert_eq!(more.entry.minutes, Minutes(45));
        let (same, _) = s.add_time_on(me, copy.entry.id, DAY, 15).await.unwrap();
        assert_eq!(same.entry.minutes, Minutes(60));

        // Back-filling the old day (Day open on it) adds to the old entry.
        let (back, made) = s
            .add_time_on(me, old.entry.id, date!(2026 - 10 - 13), 15)
            .await
            .unwrap();
        assert!(!made);
        assert_eq!(back.entry.minutes, Minutes(75));
    });
}

#[test]
fn log_work_reuses_or_creates() {
    let dir = TestDir::new("quicklog");
    block_on(async {
        let (s, me, _other) = setup(&dir).await;
        let p = s.create_project(me, "Network", None).await.unwrap();
        let list = [CaptureProject {
            id: p.id,
            name: "Network",
        }];
        let cap = |t: &str| parse_capture(t, DAY, WeekStart::default(), &list);

        // An earlier day: CHG0012345 on "Switch upgrade" in Network.
        let mut earlier = input(date!(2026 - 10 - 13), 60, "Switch upgrade");
        earlier.project = Some(p.id);
        s.create_entry(me, &earlier, "CHG0012345").await.unwrap();

        let a = s
            .quick_log(me, DAY, &cap("CHG0012345"), Minutes(30))
            .await
            .unwrap();
        assert_eq!(a.entry.description, "Switch upgrade");
        assert_eq!(a.entry.target, EntryTarget::Project(p.id));
        assert_eq!(a.entry.date, DAY);
        let b = s
            .quick_log(me, DAY, &cap("chg0012345 INC0045678"), Minutes(15))
            .await
            .unwrap();
        assert_eq!(b.entry.id, a.entry.id, "same ticket, same day");
        assert_eq!(b.entry.minutes, Minutes(45));
        assert_eq!(numbers(&b), ["CHG0012345", "INC0045678"]);

        let c = s
            .quick_log(me, DAY, &cap("printer on 2nd floor"), Minutes(60))
            .await
            .unwrap();
        assert_ne!(c.entry.id, a.entry.id);
        assert_eq!(c.entry.target, EntryTarget::None);
        assert!(matches!(
            s.quick_log(me, DAY, &cap("  "), Minutes(15)).await,
            Err(StorageError::Invalid(CoreError::NothingToLog))
        ));
        assert_eq!(s.list_entries(me, DAY, DAY).await.unwrap().len(), 2);
    });
}

#[test]
fn timesheet_codes_mappings_and_ticks() {
    let dir = TestDir::new("codes");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let code = |c: &str| CodeInput {
            area: "Operations".into(),
            phase_label: "Phase".into(),
            code: c.into(),
            sort_order: 0,
            hidden: false,
        };
        let a = s
            .add_timesheet_code(me, &code("1 - Support"))
            .await
            .unwrap();
        let b = s
            .add_timesheet_code(me, &code("2 - Maintenance"))
            .await
            .unwrap();
        assert!(b.sort_order > a.sort_order);
        assert!(s.add_timesheet_code(me, &code(" ")).await.is_err());
        let mut edit = code("2 - Maint");
        edit.sort_order = -1;
        edit.hidden = true;
        let b2 = s.update_timesheet_code(me, b.id, &edit).await.unwrap();
        assert!(b2.hidden);
        let order: Vec<String> = s
            .list_timesheet_codes(me)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.code)
            .collect();
        assert_eq!(order, ["2 - Maint", "1 - Support"]);
        assert!(matches!(
            s.update_timesheet_code(other, b.id, &edit).await,
            Err(StorageError::NotFound)
        ));

        s.set_category_code(me, "Meetings", Some(a.id))
            .await
            .unwrap();
        s.set_category_code(me, "meetings", Some(b.id))
            .await
            .unwrap();
        let maps = s.list_category_codes(me).await.unwrap();
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].timesheet_code_id, b.id);
        s.set_category_code(me, "Meetings", None).await.unwrap();
        assert!(s.list_category_codes(me).await.unwrap().is_empty());
        assert!(matches!(
            s.set_category_code(other, "Meetings", Some(a.id)).await,
            Err(StorageError::NotFound)
        ));

        let week = date!(2026 - 10 - 12);
        s.set_tick(me, week, DAY, "unassigned", true).await.unwrap();
        s.set_tick(me, week, DAY, &a.id.to_string(), true)
            .await
            .unwrap();
        s.set_tick(me, week, DAY, "unassigned", false)
            .await
            .unwrap();
        let ticks = s.list_ticks(me, week).await.unwrap();
        assert_eq!(ticks.len(), 2);
        assert_eq!(ticks.iter().filter(|t| t.done).count(), 1);
        assert!(s.list_ticks(other, week).await.unwrap().is_empty());
        assert!(s
            .list_ticks(me, date!(2026 - 10 - 19))
            .await
            .unwrap()
            .is_empty());
    });
}

#[test]
fn custom_fields_and_values() {
    let dir = TestDir::new("fields");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let jira = FieldInput {
            applies_to: AppliesTo::Ticket,
            name: "Jira".into(),
            kind: FieldKind::Link,
            options: String::new(),
            link_template: "https://example.atlassian.net/browse/{ticket}".into(),
            ticket_prefix: "PROJ-".into(),
            hidden: false,
        };
        let f = s.add_custom_field(me, &jira).await.unwrap();
        assert_eq!(f.ticket_prefix.as_deref(), Some("PROJ-"));
        assert!(matches!(
            s.add_custom_field(me, &jira).await,
            Err(StorageError::Duplicate(_))
        ));
        let mut bad = jira.clone();
        bad.name = "Bad".into();
        bad.link_template = "javascript:{ticket}".into();
        assert!(s.add_custom_field(me, &bad).await.is_err());

        let size = s
            .add_custom_field(
                me,
                &FieldInput {
                    applies_to: AppliesTo::Entry,
                    name: "Size".into(),
                    kind: FieldKind::Choice,
                    options: "Small\nLarge".into(),
                    link_template: String::new(),
                    ticket_prefix: String::new(),
                    hidden: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(size.options, ["Small", "Large"]);
        let record = *TimeEntryId::new().as_uuid();
        assert_eq!(
            s.set_custom_value(me, size.id, record, "large")
                .await
                .unwrap(),
            "Large"
        );
        assert!(s
            .set_custom_value(me, size.id, record, "Huge")
            .await
            .is_err());
        let values = s.list_custom_values(me, record).await.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].value, "Large");
        s.set_custom_value(me, size.id, record, "Small")
            .await
            .unwrap();
        assert_eq!(
            s.list_custom_values(me, record).await.unwrap()[0].value,
            "Small"
        );
        s.set_custom_value(me, size.id, record, " ").await.unwrap();
        assert!(s.list_custom_values(me, record).await.unwrap().is_empty());
        assert!(matches!(
            s.set_custom_value(other, size.id, record, "Small").await,
            Err(StorageError::NotFound)
        ));

        let mut renamed = jira.clone();
        renamed.name = "Jira link".into();
        renamed.hidden = true;
        let f2 = s.update_custom_field(me, f.id, &renamed).await.unwrap();
        assert!(f2.hidden);
        assert_eq!(s.list_custom_fields(me).await.unwrap().len(), 2);
        s.delete_custom_field(me, f.id).await.unwrap();
        assert_eq!(s.list_custom_fields(me).await.unwrap().len(), 1);
        assert!(s.list_custom_fields(other).await.unwrap().is_empty());
    });
}

#[test]
fn inbox_and_prefs() {
    let dir = TestDir::new("inbox");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let a = s.add_inbox(me, "guest wifi drops again??").await.unwrap();
        s.add_inbox(me, "Idea: auto-renew certs").await.unwrap();
        assert!(s.add_inbox(me, "   ").await.is_err());
        let list = s.list_inbox(me).await.unwrap();
        assert_eq!(list[0].text, "Idea: auto-renew certs", "newest first");
        s.set_inbox_deleted(me, a.id, true).await.unwrap();
        assert_eq!(s.list_inbox(me).await.unwrap().len(), 1);
        s.set_inbox_deleted(me, a.id, false).await.unwrap();
        assert_eq!(s.list_inbox(me).await.unwrap().len(), 2);
        assert!(matches!(
            s.set_inbox_deleted(other, a.id, true).await,
            Err(StorageError::NotFound)
        ));

        let p = s.prefs(me).await.unwrap();
        assert_eq!(p.theme, Theme::System);
        s.set_pref(me, KEY_THEME, "dark").await.unwrap();
        s.set_pref(me, KEY_MOTION, "still").await.unwrap();
        s.set_pref(me, KEY_COMMENT_SEPARATOR, " / ").await.unwrap();
        assert!(s.set_pref(me, KEY_THEME, "neon").await.is_err());
        assert!(s.set_pref(me, "seed.version", "2").await.is_err());
        let p = s.prefs(me).await.unwrap();
        assert_eq!(p.theme, Theme::Dark);
        assert_eq!(p.motion, Motion::Still);
        assert_eq!(p.comment.separator, " / ");
        assert_eq!(s.prefs(other).await.unwrap().theme, Theme::System);
    });
}

#[test]
fn next_up_list() {
    let dir = TestDir::new("nextup");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        assert!(s.next_up(me).await.unwrap().is_empty());
        let lines = |v: &[&str]| v.iter().map(|l| l.to_string()).collect::<Vec<_>>();
        let list = s
            .set_next_up(me, &lines(&[" Call the vendor ", "", "Renew certs"]))
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].text, "Call the vendor");
        assert_eq!(list[1].position, 1);

        s.tick_next_up(me, list[1].id, true).await.unwrap();
        assert!(s.next_up(me).await.unwrap()[1].done_at.is_some());
        s.tick_next_up(me, list[1].id, false).await.unwrap();
        assert!(s.next_up(me).await.unwrap()[1].done_at.is_none());
        assert!(matches!(
            s.tick_next_up(other, list[0].id, true).await,
            Err(StorageError::NotFound)
        ));
        assert!(s.next_up(other).await.unwrap().is_empty());

        // A new list replaces the old one; an empty list clears it.
        let list = s
            .set_next_up(me, &lines(&["Write the report"]))
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert!(matches!(
            s.tick_next_up(me, s.next_up(me).await.unwrap()[0].id, true)
                .await,
            Ok(())
        ));
        assert!(s.set_next_up(me, &lines(&["a"; 8])).await.is_err());
        assert_eq!(
            s.next_up(me).await.unwrap().len(),
            1,
            "a bad list changes nothing"
        );
        assert!(s.set_next_up(me, &[]).await.unwrap().is_empty());
    });
}

#[test]
fn saved_responses() {
    let dir = TestDir::new("responses");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        assert!(s.responses(me).await.unwrap().is_empty());
        let d = |name: &str, text: &str| ResponseDraft {
            name: name.into(),
            text: text.into(),
        };
        let list = s
            .set_responses(
                me,
                &[
                    d("No reply", "No reply after 3 tries.\nClosing."),
                    d("Empty", "  "),
                    d("", "Fixed and confirmed with the user."),
                ],
            )
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].text, "No reply after 3 tries.\nClosing.");
        assert_eq!(list[1].name, "Fixed and confirmed with the user.");
        assert_eq!(list[1].position, 1);
        assert!(s.responses(other).await.unwrap().is_empty());

        // A bad list changes nothing; an empty list clears it.
        let long = "x".repeat(61);
        assert!(s.set_responses(me, &[d(&long, "ok")]).await.is_err());
        assert_eq!(s.responses(me).await.unwrap().len(), 2);
        assert!(s.set_responses(me, &[]).await.unwrap().is_empty());
    });
}

#[test]
fn open_tickets_come_back_until_ticked() {
    let dir = TestDir::new("open");
    block_on(async {
        let (s, me, _) = setup(&dir).await;
        let fri = date!(2026 - 10 - 16);
        let mon = date!(2026 - 10 - 19);
        let tue = date!(2026 - 10 - 20);
        // Pasted tickets start still open; ticking INC0045678 finishes it.
        let day = s.tray_add(me, fri, "RITM0034567 INC0045678").await.unwrap();
        assert!(day.items.iter().all(|i| !i.done));
        let ritm = day.items[0].id;
        s.tray_set_done(me, day.items[1].id, true).await.unwrap();
        s.tray_set_note(me, ritm, "waiting on the user")
            .await
            .unwrap();
        let day = s.tray_day(me, fri).await.unwrap();
        assert!(!day.items[0].done);
        // Its time still counts on Friday.
        assert_eq!(day.minutes, Minutes(30));

        // Monday: it comes back, once.
        let back = s.tray_carried(me, mon).await.unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].ticket_number, "RITM0034567");
        assert!(s.tray_carried(me, fri).await.unwrap().is_empty());

        // Working on it Monday carries its notepad; it shows in Monday's tray, not twice.
        let day = s.tray_carry(me, ritm, mon).await.unwrap();
        assert_eq!(day.items.len(), 1);
        assert!(!day.items[0].done);
        assert_eq!(day.items[0].note.as_deref(), Some("waiting on the user"));
        assert!(s.tray_carried(me, mon).await.unwrap().is_empty());
        assert_eq!(s.tray_carried(me, tue).await.unwrap().len(), 1);

        // Ticking Monday's row finishes it everywhere.
        let monday_row = day.items[0].id;
        s.tray_set_done(me, monday_row, true).await.unwrap();
        assert!(s.tray_carried(me, tue).await.unwrap().is_empty());
        assert!(s.tray_day(me, fri).await.unwrap().items[0].done);

        // Pasting a ticket that is still open from before brings it into that day, still
        // open, and it no longer shows as carried there; ticking it finishes it everywhere.
        s.tray_add(me, fri, "INC0045700").await.unwrap();
        assert_eq!(s.tray_carried(me, mon).await.unwrap().len(), 1);
        let day = s.tray_add(me, mon, "inc0045700").await.unwrap();
        let again = day
            .items
            .iter()
            .find(|i| i.ticket_number.eq_ignore_ascii_case("INC0045700"))
            .unwrap();
        assert!(!again.done);
        assert!(s.tray_carried(me, mon).await.unwrap().is_empty());
        s.tray_set_done(me, again.id, true).await.unwrap();
        assert!(s.tray_carried(me, tue).await.unwrap().is_empty());
    });
}

#[test]
fn ticking_done_gives_up_the_desk_spot() {
    use shiftbook_core::canvas::{CardKind, Drawer};
    let dir = TestDir::new("done-spot");
    block_on(async {
        let (s, me, _) = setup(&dir).await;
        let day = s
            .tray_add(me, date!(2026 - 10 - 20), "INC0045678 INC0045690")
            .await
            .unwrap();
        let (a, b) = (day.items[0].id, day.items[1].id);
        for id in [a, b] {
            s.tray_set_done(me, id, false).await.unwrap();
            s.move_card(me, CardKind::Tray, *id.as_uuid(), 100, 100)
                .await
                .unwrap();
        }
        // A filed ticket stays in its drawer.
        let folder = s.save_folder(me, None, "Reference").await.unwrap();
        s.file_card(
            me,
            (CardKind::Tray, *b.as_uuid()),
            Drawer::Folder(folder.id),
        )
        .await
        .unwrap();
        s.tray_set_done(me, a, true).await.unwrap();
        s.tray_set_done(me, b, true).await.unwrap();
        let spots = s.canvas_spots(me).await.unwrap();
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0].record_id, *b.as_uuid());
    });
}

#[test]
fn tray_tickets_with_a_project_roll_up_under_it() {
    let dir = TestDir::new("trayproject");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let net = s
            .create_project(me, "Network refresh", None)
            .await
            .unwrap()
            .id;
        let day = s
            .tray_add(me, DAY, "INC1001 INC1002 INC1003")
            .await
            .unwrap();
        let plain = day.entry_id.unwrap();
        let (a, b) = (day.items[0].id, day.items[1].id);
        s.tray_set_project(me, a, Some(net)).await.unwrap();
        let day = s.tray_set_project(me, b, Some(net)).await.unwrap();
        assert_eq!(day.entry_id, Some(plain));
        assert_eq!(day.minutes, Minutes(45));

        let entries = s.list_entries(me, DAY, DAY).await.unwrap();
        assert_eq!(entries.len(), 2);
        let rollup = entries
            .iter()
            .find(|r| r.entry.target == EntryTarget::Project(net))
            .unwrap();
        assert!(rollup.is_tray);
        assert_eq!(rollup.entry.minutes, Minutes(30));
        assert_eq!(numbers(rollup), ["INC1001", "INC1002"]);
        let plain_rec = entries.iter().find(|r| r.entry.id == plain).unwrap();
        assert_eq!(numbers(plain_rec), ["INC1003"]);

        // The project roll-up keeps its project whatever the details panel sends.
        let mut edit = input(DAY, 0, "Tickets");
        edit.project = None;
        let kept = s.update_entry(me, rollup.entry.id, &edit).await.unwrap();
        assert_eq!(kept.entry.target, EntryTarget::Project(net));

        // Deleting the project's roll-up takes only its tickets; restoring brings them back.
        s.delete_entry(me, rollup.entry.id).await.unwrap();
        assert_eq!(s.tray_day(me, DAY).await.unwrap().items.len(), 1);
        s.restore_entry(me, rollup.entry.id).await.unwrap();
        assert_eq!(s.tray_day(me, DAY).await.unwrap().items.len(), 3);

        // Back to no project: the project's roll-up goes away.
        s.tray_set_project(me, a, None).await.unwrap();
        let day = s.tray_set_project(me, b, None).await.unwrap();
        assert_eq!(s.list_entries(me, DAY, DAY).await.unwrap().len(), 1);
        assert_eq!(day.minutes, Minutes(45));
        assert!(matches!(
            s.tray_set_project(other, a, Some(net)).await,
            Err(StorageError::NotFound)
        ));
    });
}

#[test]
fn brought_up_things_stay_apart_from_the_wrapup_list() {
    let dir = TestDir::new("bringup");
    block_on(async {
        let (s, me, _) = setup(&dir).await;
        let card = uuid::Uuid::now_v7();
        let item = s
            .bring_up(
                me,
                "Firewall rule cleanup",
                date!(2026 - 10 - 20),
                Some((shiftbook_core::canvas::CardKind::Entry, card)),
            )
            .await
            .unwrap();
        assert_eq!(item.show_from, Some(date!(2026 - 10 - 20)));
        assert!(s.next_up(me).await.unwrap().is_empty());
        s.set_next_up(me, &["Call the vendor".to_string()])
            .await
            .unwrap();
        assert_eq!(s.next_up_all(me).await.unwrap().len(), 2);
        // Bringing the same card up again moves it.
        s.bring_up(
            me,
            "Firewall rule cleanup",
            date!(2026 - 10 - 21),
            Some((shiftbook_core::canvas::CardKind::Entry, card)),
        )
        .await
        .unwrap();
        let all = s.next_up_all(me).await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all
            .iter()
            .any(|i| i.show_from == Some(date!(2026 - 10 - 21))));
        // Saving the wrap-up list doesn't touch it.
        s.set_next_up(me, &[]).await.unwrap();
        assert_eq!(s.next_up_all(me).await.unwrap().len(), 1);
    });
}

#[test]
fn meeting_series_are_kept_and_checked() {
    let dir = TestDir::new("meetings");
    block_on(async {
        let (s, me, other) = setup(&dir).await;
        let mon_thu = [true, false, false, true, false, false, false];
        let input = crate::meetings::SeriesInput {
            name: " Weekly ops ".into(),
            days: mon_thu,
            at: 600,
            remind_minutes: 15,
        };
        let ops = s.create_meeting_series(me, &input).await.unwrap();
        assert_eq!(ops.name, "Weekly ops");
        assert_eq!(ops.days, mon_thu);
        let early = crate::meetings::SeriesInput {
            name: "All-hands".into(),
            at: 540,
            ..input.clone()
        };
        s.create_meeting_series(me, &early).await.unwrap();
        let names: Vec<String> = s
            .meeting_series(me)
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["All-hands", "Weekly ops"]);
        assert!(s.meeting_series(other).await.unwrap().is_empty());

        let moved = crate::meetings::SeriesInput {
            at: 630,
            ..input.clone()
        };
        assert_eq!(
            s.update_meeting_series(me, ops.id, &moved)
                .await
                .unwrap()
                .at,
            630
        );
        assert!(matches!(
            s.update_meeting_series(other, ops.id, &moved).await,
            Err(StorageError::NotFound)
        ));
        let no_days = crate::meetings::SeriesInput {
            days: [false; 7],
            ..input.clone()
        };
        assert!(s.create_meeting_series(me, &no_days).await.is_err());

        s.set_meeting_series_deleted(me, ops.id, true)
            .await
            .unwrap();
        assert_eq!(s.meeting_series(me).await.unwrap().len(), 1);
        s.set_meeting_series_deleted(me, ops.id, false)
            .await
            .unwrap();
        assert_eq!(s.meeting_series(me).await.unwrap().len(), 2);
    });
}
