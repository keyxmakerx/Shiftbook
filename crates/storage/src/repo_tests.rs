//! Repository tests against a real encrypted database in a temp folder.

use shiftbook_core::ids::{FrequencyId, ListOptionId, OwnerId};
use shiftbook_core::model::ListName;
use shiftbook_core::seed::{CATEGORIES, FREQUENCIES, PRIORITIES, RECURRING_TYPES, STATUSES};
use shiftbook_core::CoreError;

use crate::db::{database_path, open_encrypted, test_dek};
use crate::fsutil::test_dir::TestDir;
use crate::repo::{
    FrequencyRepository, ListOptionRepository, SettingsRepository, UserRepository, SEED_VERSION,
    SEED_VERSION_KEY,
};
use crate::test_rt::block_on;
use crate::{SqliteStore, StorageError};

async fn store(dir: &TestDir) -> SqliteStore {
    SqliteStore::new(
        open_encrypted(&database_path(dir.path()), &test_dek(9))
            .await
            .unwrap(),
    )
}

async fn count(store: &SqliteStore, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

#[test]
fn seeds_once_and_idempotently() {
    let dir = TestDir::new("seed");
    block_on(async {
        let s = store(&dir).await;
        assert!(s.local_user().await.unwrap().is_none());
        let user = s.initialize_local_owner("Me").await.unwrap();
        assert_eq!(s.local_user().await.unwrap(), Some(user.clone()));

        let options = CATEGORIES.len() + STATUSES.len() + PRIORITIES.len() + RECURRING_TYPES.len();
        let check = |s: SqliteStore| async move {
            assert_eq!(count(&s, "SELECT count(*) FROM users").await, 1);
            assert_eq!(
                count(&s, "SELECT count(*) FROM list_options").await,
                options as i64
            );
            assert_eq!(
                count(&s, "SELECT count(*) FROM frequencies").await,
                FREQUENCIES.len() as i64
            );
        };
        check(s.clone()).await;
        assert_eq!(
            s.get_setting(user.id, SEED_VERSION_KEY)
                .await
                .unwrap()
                .as_deref(),
            Some(SEED_VERSION)
        );

        // Again, and again after reopening: same user, nothing duplicated.
        let again = s.initialize_local_owner("Someone else").await.unwrap();
        assert_eq!(again.id, user.id);
        assert_eq!(again.display_name, "Me");
        check(s.clone()).await;
        s.pool().close().await;
        let s = store(&dir).await;
        assert_eq!(s.initialize_local_owner("Me").await.unwrap().id, user.id);
        check(s.clone()).await;

        // A user who deliberately hid a default doesn't get it back.
        let statuses = s
            .list_options(user.id, ListName::Status, false)
            .await
            .unwrap();
        let waiting = statuses.iter().find(|o| o.value == "Waiting").unwrap();
        s.set_list_option_hidden(user.id, waiting.id, true)
            .await
            .unwrap();
        s.initialize_local_owner("Me").await.unwrap();
        let statuses = s
            .list_options(user.id, ListName::Status, false)
            .await
            .unwrap();
        assert!(statuses.iter().all(|o| o.value != "Waiting"));
        check(s.clone()).await;

        // Seeded order matches core.
        let all = s
            .list_options(user.id, ListName::Status, true)
            .await
            .unwrap();
        let values: Vec<&str> = all.iter().map(|o| o.value.as_str()).collect();
        let expected: Vec<&str> = STATUSES.iter().map(|o| o.value).collect();
        assert_eq!(values, expected);
        let freqs = s.list_frequencies(user.id).await.unwrap();
        assert_eq!(freqs[0].label, FREQUENCIES[0].label);
        assert_eq!(freqs.len(), FREQUENCIES.len());
        s.pool().close().await;
    });
}

#[test]
fn seeding_is_one_transaction() {
    let dir = TestDir::new("seed-tx");
    block_on(async {
        let s = store(&dir).await;
        // Make the seed fail half-way: a clashing frequency label for any owner
        // can't be pre-inserted (FK on owner), so break the table instead.
        sqlx::query("CREATE TRIGGER no_freq BEFORE INSERT ON frequencies BEGIN SELECT RAISE(ABORT, 'nope'); END")
            .execute(s.pool())
            .await
            .unwrap();
        assert!(s.initialize_local_owner("Me").await.is_err());
        assert_eq!(count(&s, "SELECT count(*) FROM users").await, 0);
        assert_eq!(count(&s, "SELECT count(*) FROM list_options").await, 0);
        assert_eq!(count(&s, "SELECT count(*) FROM settings").await, 0);
        s.pool().close().await;
    });
}

#[test]
fn settings_are_per_owner() {
    let dir = TestDir::new("settings");
    block_on(async {
        let s = store(&dir).await;
        let user = s.initialize_local_owner("Me").await.unwrap();
        assert_eq!(s.get_setting(user.id, "theme").await.unwrap(), None);
        s.set_setting(user.id, "theme", "dark").await.unwrap();
        s.set_setting(user.id, "theme", "light").await.unwrap();
        assert_eq!(
            s.get_setting(user.id, "theme").await.unwrap().as_deref(),
            Some("light")
        );
        let all = s.list_settings(user.id).await.unwrap();
        assert!(all.contains(&("theme".to_string(), "light".to_string())));
        // Another owner sees nothing (and can't write without a user row).
        let stranger = OwnerId::new();
        assert_eq!(s.get_setting(stranger, "theme").await.unwrap(), None);
        assert!(s.list_settings(stranger).await.unwrap().is_empty());
        assert!(s.set_setting(stranger, "theme", "x").await.is_err());
        // Bad keys.
        assert!(matches!(
            s.set_setting(user.id, "  ", "x").await,
            Err(StorageError::Invalid(CoreError::Required { .. }))
        ));
        assert!(s.set_setting(user.id, " padded ", "x").await.is_err());
        assert!(s.set_setting(user.id, &"k".repeat(101), "x").await.is_err());
        s.pool().close().await;
    });
}

#[test]
fn done_and_blocked_are_protected() {
    let dir = TestDir::new("protected");
    block_on(async {
        let s = store(&dir).await;
        let user = s.initialize_local_owner("Me").await.unwrap();
        let statuses = s
            .list_options(user.id, ListName::Status, true)
            .await
            .unwrap();
        for name in ["Done", "Blocked"] {
            let option = statuses.iter().find(|o| o.value == name).unwrap();
            let err = s
                .rename_list_option(user.id, option.id, "Finished")
                .await
                .unwrap_err();
            assert!(matches!(&err, StorageError::Protected(v) if v == name));
            assert!(err.to_string().contains("can't be renamed or hidden"));
            assert!(matches!(
                s.set_list_option_hidden(user.id, option.id, true).await,
                Err(StorageError::Protected(_))
            ));
            // Showing (un-hiding) and colours are fine.
            s.set_list_option_hidden(user.id, option.id, false)
                .await
                .unwrap();
            s.set_list_option_color(user.id, option.id, Some("#000000"))
                .await
                .unwrap();
        }
        // Can't sneak another "done" in, by adding or renaming.
        assert!(matches!(
            s.add_list_option(user.id, ListName::Status, " done ", None)
                .await,
            Err(StorageError::Duplicate(_))
        ));
        let waiting = statuses.iter().find(|o| o.value == "Waiting").unwrap();
        assert!(matches!(
            s.rename_list_option(user.id, waiting.id, "BLOCKED").await,
            Err(StorageError::Duplicate(_))
        ));
        // A category called "Done" is not protected: only statuses drive rules.
        let cat = s
            .add_list_option(user.id, ListName::Category, "Done", None)
            .await
            .unwrap();
        s.rename_list_option(user.id, cat.id, "Finished work")
            .await
            .unwrap();
        s.pool().close().await;
    });
}

#[test]
fn rename_updates_records_in_one_go() {
    let dir = TestDir::new("rename");
    block_on(async {
        let s = store(&dir).await;
        let user = s.initialize_local_owner("Me").await.unwrap();
        let owner = user.id.to_string();
        sqlx::query(
            "INSERT INTO projects (id, owner_id, name, status, priority, created_at, updated_at) \
             VALUES ('p1', $1, 'Web', 'Waiting', 'High', 't', 't')",
        )
        .bind(&owner)
        .execute(s.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO time_entries (id, owner_id, date, minutes, category, status, created_at, updated_at) \
             VALUES ('e1', $1, '2026-10-15', 60, 'Meetings', 'Waiting', 't', 't')",
        )
        .bind(&owner)
        .execute(s.pool())
        .await
        .unwrap();

        let statuses = s
            .list_options(user.id, ListName::Status, true)
            .await
            .unwrap();
        let waiting = statuses.iter().find(|o| o.value == "Waiting").unwrap();
        let renamed = s
            .rename_list_option(user.id, waiting.id, "  On hold ")
            .await
            .unwrap();
        assert_eq!(renamed.value, "On hold");
        let project_status: String =
            sqlx::query_scalar("SELECT status FROM projects WHERE id = 'p1'")
                .fetch_one(s.pool())
                .await
                .unwrap();
        assert_eq!(project_status, "On hold");
        let entry_status: String =
            sqlx::query_scalar("SELECT status FROM time_entries WHERE id = 'e1'")
                .fetch_one(s.pool())
                .await
                .unwrap();
        assert_eq!(entry_status, "On hold");

        let cats = s
            .list_options(user.id, ListName::Category, true)
            .await
            .unwrap();
        let meetings = cats.iter().find(|o| o.value == "Meetings").unwrap();
        s.rename_list_option(user.id, meetings.id, "Calls")
            .await
            .unwrap();
        let entry_cat: String =
            sqlx::query_scalar("SELECT category FROM time_entries WHERE id = 'e1'")
                .fetch_one(s.pool())
                .await
                .unwrap();
        assert_eq!(entry_cat, "Calls");
        // Priority untouched by a status rename.
        let prio: String = sqlx::query_scalar("SELECT priority FROM projects WHERE id = 'p1'")
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert_eq!(prio, "High");
        s.pool().close().await;
    });
}

#[test]
fn list_options_crud_and_owner_scoping() {
    let dir = TestDir::new("options");
    block_on(async {
        let s = store(&dir).await;
        let user = s.initialize_local_owner("Me").await.unwrap();
        let added = s
            .add_list_option(user.id, ListName::Priority, "Someday", Some("#ABCDEF"))
            .await
            .unwrap();
        assert_eq!(added.color.as_deref(), Some("#abcdef"));
        assert_eq!(added.sort_order, PRIORITIES.len() as i32);
        assert!(matches!(
            s.add_list_option(user.id, ListName::Priority, "  ", None)
                .await,
            Err(StorageError::Invalid(_))
        ));
        assert!(matches!(
            s.add_list_option(user.id, ListName::Priority, "X", Some("red"))
                .await,
            Err(StorageError::Invalid(_))
        ));

        s.set_list_option_hidden(user.id, added.id, true)
            .await
            .unwrap();
        let visible = s
            .list_options(user.id, ListName::Priority, false)
            .await
            .unwrap();
        assert!(visible.iter().all(|o| o.id != added.id));
        let all = s
            .list_options(user.id, ListName::Priority, true)
            .await
            .unwrap();
        assert!(all.iter().any(|o| o.id == added.id && o.hidden));

        // Reorder: Someday first.
        let mut ids: Vec<ListOptionId> = all.iter().map(|o| o.id).collect();
        ids.retain(|id| *id != added.id);
        ids.insert(0, added.id);
        s.reorder_list_options(user.id, ListName::Priority, &ids)
            .await
            .unwrap();
        let all = s
            .list_options(user.id, ListName::Priority, true)
            .await
            .unwrap();
        assert_eq!(all[0].id, added.id);
        // An id from another list is refused and nothing changes.
        let cat = s
            .list_options(user.id, ListName::Category, true)
            .await
            .unwrap()[0]
            .id;
        assert!(matches!(
            s.reorder_list_options(user.id, ListName::Priority, &[ids[1], cat])
                .await,
            Err(StorageError::NotFound)
        ));
        let again = s
            .list_options(user.id, ListName::Priority, true)
            .await
            .unwrap();
        assert_eq!(again[0].id, added.id);

        // Another owner can't see or touch these rows.
        let stranger = OwnerId::new();
        assert!(s
            .list_options(stranger, ListName::Priority, true)
            .await
            .unwrap()
            .is_empty());
        assert!(matches!(
            s.rename_list_option(stranger, added.id, "Mine now").await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            s.set_list_option_hidden(stranger, added.id, false).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            s.set_list_option_color(stranger, added.id, None).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            s.rename_list_option(user.id, ListOptionId::new(), "Ghost")
                .await,
            Err(StorageError::NotFound)
        ));
        s.pool().close().await;
    });
}

#[test]
fn frequencies_crud() {
    let dir = TestDir::new("freq");
    block_on(async {
        let s = store(&dir).await;
        let user = s.initialize_local_owner("Me").await.unwrap();
        let f = s
            .add_frequency(user.id, " Every 9 days ", 0, 9)
            .await
            .unwrap();
        assert_eq!(f.label, "Every 9 days");
        assert_eq!(f.sort_order, FREQUENCIES.len() as i32);
        assert!(matches!(
            s.add_frequency(user.id, "never", 0, 0).await,
            Err(StorageError::Invalid(CoreError::FrequencyEmpty))
        ));
        assert!(matches!(
            s.add_frequency(user.id, "weekly", 0, 7).await,
            Err(StorageError::Duplicate(_))
        ));
        let updated = s
            .update_frequency(user.id, f.id, "Every 10 days", 0, 10)
            .await
            .unwrap();
        assert_eq!(
            (updated.label.as_str(), updated.days),
            ("Every 10 days", 10)
        );
        assert!(matches!(
            s.update_frequency(user.id, f.id, "Monthly", 1, 0).await,
            Err(StorageError::Duplicate(_))
        ));
        s.hide_frequency(user.id, f.id).await.unwrap();
        assert!(s
            .list_frequencies(user.id)
            .await
            .unwrap()
            .iter()
            .all(|x| x.id != f.id));
        assert!(matches!(
            s.hide_frequency(user.id, f.id).await,
            Err(StorageError::NotFound)
        ));
        // The label is free again once hidden.
        s.add_frequency(user.id, "Every 10 days", 0, 10)
            .await
            .unwrap();
        // Owner scoping.
        let stranger = OwnerId::new();
        assert!(s.list_frequencies(stranger).await.unwrap().is_empty());
        assert!(matches!(
            s.update_frequency(stranger, updated.id, "x", 0, 1).await,
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            s.hide_frequency(user.id, FrequencyId::new()).await,
            Err(StorageError::NotFound)
        ));
        s.pool().close().await;
    });
}

/// What broke the Android run: one connection holds the write lock (a failed transaction
/// still being rolled back) while another, in a plain `BEGIN`, reads and then writes.
/// SQLite refuses that at once with "database is locked". `write_tx` takes the lock first,
/// so it waits for the other connection instead.
#[test]
fn a_write_waits_for_another_writer_instead_of_failing() {
    let dir = TestDir::new("write-lock");
    block_on(async {
        let s = store(&dir).await;
        s.initialize_local_owner("Me").await.unwrap();
        const TOUCH: &str = "UPDATE users SET display_name = display_name";

        let mut holder = s.write_tx().await.unwrap();
        sqlx::query(TOUCH).execute(&mut *holder).await.unwrap();

        // The old way: read, then write, while the lock is held elsewhere.
        let mut deferred = s.pool().begin().await.unwrap();
        sqlx::query("SELECT count(*) FROM users")
            .execute(&mut *deferred)
            .await
            .unwrap();
        let err = sqlx::query(TOUCH)
            .execute(&mut *deferred)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("locked"), "{err}");
        deferred.rollback().await.unwrap();

        // The new way waits for the holder, then goes ahead.
        let s2 = s.clone();
        let waiter = tokio::spawn(async move {
            let mut tx = s2.write_tx().await.unwrap();
            sqlx::query(TOUCH).execute(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();
        });
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        holder.commit().await.unwrap();
        waiter.await.unwrap();
    });
}
