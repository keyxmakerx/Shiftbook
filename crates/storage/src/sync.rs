//! Swapping changes between two copies of Shiftbook, such as a phone and a computer (round 15).
//!
//! A sync file is a backup (the same `.shiftbak` format and encryption) under its own name.
//! Bringing one in *merges* instead of replacing: rows match by `id` and the newer
//! `updated_at` wins. Deletes are soft, so they travel as changes and can be brought back.
//! Settings stay on each device. Both copies must come from the same start (the phone set up by
//! restoring a backup from the computer), which shows as the same owner.
//!
//! A row that clashes with a different row here on a unique rule (both logged the same day's
//! Tickets tray, say), or that points at one left out, is left out too and counted. Each row
//! goes in under its own savepoint so one clash never stops the rest.
//!
//! The same pass is the preview: [`merge_backup`] with `apply = false` does everything in a
//! transaction and rolls it back, so the counts are exactly what bringing it in would do.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;
use sqlx::Acquire;
use time::OffsetDateTime;

use crate::backup::{
    bind_values, cell, quote_ident, table_columns, table_names_sql, BackupError, OpenedBackup,
};
use crate::fsutil;
use crate::repo::{parse_ts, SqliteStore};

/// File extension of a sync file.
pub const SYNC_EXTENSION: &str = "shiftsync";

/// Tables that never travel: who the owner is, and each device's own settings.
const LOCAL_ONLY: [&str; 2] = ["users", "settings"];

/// File in the data folder holding when this device last swapped (UTC, RFC 3339). A time only;
/// kept outside the database so it never travels in a sync file.
const LAST_SWAP_FILE: &str = "last-swap";

/// When this device last made or brought in a sync file, if ever.
pub fn last_swap(data_dir: &Path) -> Option<OffsetDateTime> {
    let text = std::fs::read_to_string(data_dir.join(LAST_SWAP_FILE)).ok()?;
    parse_ts(text.trim()).ok()
}

/// Remember a swap.
pub fn note_swap(data_dir: &Path, at: OffsetDateTime) -> io::Result<()> {
    fsutil::write_atomic(
        &data_dir.join(LAST_SWAP_FILE),
        shiftbook_core::dates::format_timestamp(at).as_bytes(),
    )
}

/// Most clashes listed by name.
pub const MAX_CLASHES_SHOWN: usize = 5;

/// Which side's edit won a clash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Winner {
    /// The edit on this device was newer and stays.
    Here,
    /// The edit in the sync file was newer and comes in.
    There,
}

/// Something edited on both devices since the last swap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    /// The entry's description, the note's or project's name, or what it is.
    pub title: String,
    /// Whose edit wins (the newer one).
    pub winner: Winner,
    /// When the winning edit was made (UTC, RFC 3339).
    pub at: String,
}

/// What bringing a sync file in does (or did).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// When the sync file was made (UTC, RFC 3339).
    pub created_at: String,
    /// Entries new to this device.
    pub new_entries: u64,
    /// Entries edited there more recently.
    pub changed_entries: u64,
    /// Notes new to this device.
    pub new_notes: u64,
    /// Notes edited there more recently.
    pub changed_notes: u64,
    /// Projects new to this device.
    pub new_projects: u64,
    /// Projects edited there more recently.
    pub changed_projects: u64,
    /// Everything else new or changed (tickets, the tray, scraps, codes…).
    pub other: u64,
    /// Things deleted there that are still here (they can be brought back).
    pub deleted: u64,
    /// Things edited more recently here, which stay as they are.
    pub kept_here: u64,
    /// Things left out because they clash with something here.
    pub left_out: u64,
    /// Edited on both devices since the last swap, up to [`MAX_CLASHES_SHOWN`].
    pub clashes: Vec<Clash>,
    /// How many clashes there were in all.
    pub clash_count: u64,
}

impl SyncReport {
    fn count_new(&mut self, table: &str) {
        match table {
            "time_entries" => self.new_entries += 1,
            "notes" => self.new_notes += 1,
            "projects" => self.new_projects += 1,
            _ => self.other += 1,
        }
    }

    fn count_changed(&mut self, table: &str) {
        match table {
            "time_entries" => self.changed_entries += 1,
            "notes" => self.changed_notes += 1,
            "projects" => self.changed_projects += 1,
            _ => self.other += 1,
        }
    }

    fn clash(&mut self, title: String, winner: Winner, at: &str) {
        self.clash_count += 1;
        if self.clashes.len() < MAX_CLASHES_SHOWN {
            self.clashes.push(Clash {
                title,
                winner,
                at: at.to_string(),
            });
        }
    }

    /// True when the file holds nothing new for this device.
    pub fn nothing_new(&self) -> bool {
        self.new_entries
            + self.changed_entries
            + self.new_notes
            + self.changed_notes
            + self.new_projects
            + self.changed_projects
            + self.other
            == 0
    }
}

/// How to name a record in a clash.
fn title_of(table: &str, columns: &[String], row: &[Value]) -> String {
    let column = match table {
        "time_entries" => "description",
        "notes" => "title",
        "projects" | "custom_fields" => "name",
        "inbox_items" => "text",
        "ticket_tray" => "ticket_number",
        "tickets" => "number",
        _ => "",
    };
    let named = columns
        .iter()
        .position(|c| c == column)
        .and_then(|i| row[i].as_str())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(60).collect::<String>());
    named.unwrap_or_else(|| match table {
        "time_entries" => "An entry".into(),
        "notes" => "A note".into(),
        _ => "A setting or list item".into(),
    })
}

/// Tables in an order where every table comes after the tables it points at.
async fn ordered_tables(
    conn: &mut sqlx::SqliteConnection,
    names: &[String],
) -> Result<Vec<String>, BackupError> {
    let mut deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for name in names {
        let parents: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT \"table\" FROM pragma_foreign_key_list($1)")
                .bind(name)
                .fetch_all(&mut *conn)
                .await?;
        deps.insert(
            name.clone(),
            parents.into_iter().filter(|p| p != name).collect(),
        );
    }
    let mut out: Vec<String> = Vec::new();
    while out.len() < names.len() {
        let ready: Vec<String> = deps
            .iter()
            .filter(|(n, ps)| !out.contains(n) && ps.iter().all(|p| out.contains(p)))
            .map(|(n, _)| n.clone())
            .collect();
        if ready.is_empty() {
            // A loop (none today): take the rest as they are.
            out.extend(
                deps.keys()
                    .filter(|n| !out.contains(n))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
            break;
        }
        out.extend(ready);
    }
    Ok(out)
}

/// Bring a sync file's changes in (`apply`), or only work out what that would do. Rows edited
/// on both sides after `last_swap` are reported as clashes; the newer edit wins either way.
pub async fn merge_backup(
    store: &SqliteStore,
    file: &OpenedBackup,
    last_swap: Option<OffsetDateTime>,
    apply: bool,
) -> Result<SyncReport, BackupError> {
    let mut report = SyncReport {
        created_at: file.created_at.clone(),
        ..SyncReport::default()
    };
    let mut tx = store.pool().begin().await?;

    // Same start: every owner in the file is an owner here.
    let here: Vec<String> = sqlx::query_scalar("SELECT id FROM users")
        .fetch_all(&mut *tx)
        .await?;
    if let Some(users) = file.contents.tables.get("users") {
        let id = users.columns.iter().position(|c| c == "id");
        let theirs = users
            .rows
            .iter()
            .filter_map(|r| id.and_then(|i| r[i].as_str()));
        for owner in theirs {
            if !here.iter().any(|h| h == owner) {
                return Err(BackupError::NotPaired);
            }
        }
    }

    let names: Vec<String> = sqlx::query_scalar(table_names_sql())
        .fetch_all(&mut *tx)
        .await?;
    let mut current = BTreeMap::new();
    for name in &names {
        current.insert(name.clone(), table_columns(&mut tx, name).await?);
    }
    for (name, table) in &file.contents.tables {
        let cols = current.get(name).ok_or(BackupError::TooNew)?;
        if table.columns.iter().any(|c| !cols.contains(c)) {
            return Err(BackupError::TooNew);
        }
    }

    for name in ordered_tables(&mut tx, &names).await? {
        if LOCAL_ONLY.contains(&name.as_str()) {
            continue;
        }
        let Some(table) = file.contents.tables.get(&name) else {
            continue;
        };
        let at = |col: &str| table.columns.iter().position(|c| c == col);
        let (Some(id_at), Some(updated_at), Some(deleted_at)) =
            (at("id"), at("updated_at"), at("deleted_at"))
        else {
            continue;
        };
        let list = table
            .columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let params = (1..=table.columns.len())
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sets = table
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{} = ${}", quote_ident(c), i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let table_sql = quote_ident(&name);
        let select = format!("SELECT {list} FROM {table_sql} WHERE id = $1");
        let insert = format!("INSERT INTO {table_sql} ({list}) VALUES ({params})");
        let update = format!(
            "UPDATE {table_sql} SET {sets} WHERE id = ${}",
            table.columns.len() + 1
        );

        for row in &table.rows {
            let Some(id) = row[id_at].as_str() else {
                return Err(BackupError::Invalid);
            };
            let found = sqlx::query(sqlx::AssertSqlSafe(select.as_str()))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
            let Some(found) = found else {
                if put(
                    &mut tx,
                    bind_values(sqlx::query(sqlx::AssertSqlSafe(insert.as_str())), row)?,
                )
                .await?
                {
                    if row[deleted_at].is_null() {
                        report.count_new(&name);
                    }
                } else {
                    report.left_out += 1;
                }
                continue;
            };
            let mine: Vec<Value> = (0..table.columns.len())
                .map(|i| cell(&found, i))
                .collect::<Result<_, _>>()?;
            if &mine == row {
                continue;
            }
            let stamp = |v: &Value| v.as_str().and_then(|t| parse_ts(t).ok());
            let (Some(theirs_at), Some(mine_at)) =
                (stamp(&row[updated_at]), stamp(&mine[updated_at]))
            else {
                return Err(BackupError::Invalid);
            };
            let both = last_swap.is_some_and(|t| theirs_at > t && mine_at > t);
            if theirs_at > mine_at {
                let q =
                    bind_values(sqlx::query(sqlx::AssertSqlSafe(update.as_str())), row)?.bind(id);
                if put(&mut tx, q).await? {
                    if !row[deleted_at].is_null() && mine[deleted_at].is_null() {
                        report.deleted += 1;
                    } else {
                        report.count_changed(&name);
                    }
                    if both {
                        let at = row[updated_at].as_str().unwrap_or_default();
                        report.clash(title_of(&name, &table.columns, row), Winner::There, at);
                    }
                } else {
                    report.left_out += 1;
                }
            } else {
                report.kept_here += 1;
                if both {
                    let at = mine[updated_at].as_str().unwrap_or_default();
                    report.clash(title_of(&name, &table.columns, &mine), Winner::Here, at);
                }
            }
        }
    }

    if apply {
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }
    Ok(report)
}

/// Run one insert or update under a savepoint: true if it went in, false if it broke a rule
/// (a unique index, or a row it points at isn't here) and was undone.
async fn put(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    q: sqlx::query::Query<'_, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
) -> Result<bool, BackupError> {
    let mut savepoint = tx.begin().await?;
    match q.execute(&mut *savepoint).await {
        Ok(_) => {
            savepoint.commit().await?;
            Ok(true)
        }
        Err(sqlx::Error::Database(e))
            if e.is_unique_violation() || e.is_foreign_key_violation() =>
        {
            savepoint.rollback().await?;
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::{
        create_backup, current_schema_version, open_backup, restore_backup, BackupKey,
    };
    use crate::db::{database_path, open_encrypted};
    use crate::fsutil::test_dir::TestDir;
    use crate::repo::SettingsRepository;
    use crate::test_rt::block_on;
    use crate::vault::{SecretKind, VaultDir};
    use crate::work::EntryInput;
    use secrecy::SecretString;
    use shiftbook_core::ids::OwnerId;
    use shiftbook_core::minutes::Minutes;
    use time::macros::{date, datetime};

    const PW: &str = "first password here";

    struct Device {
        _dir: TestDir,
        vault_dir: VaultDir,
        store: SqliteStore,
        vault: crate::Vault,
        me: OwnerId,
    }

    async fn device(label: &str) -> Device {
        let dir = TestDir::new(label);
        let vault_dir = VaultDir::new_for_tests(dir.path());
        let (vault, _) = vault_dir.create(&SecretString::from(PW)).unwrap();
        let pool = open_encrypted(&database_path(dir.path()), vault.dek())
            .await
            .unwrap();
        let store = SqliteStore::new(pool);
        let me = store.initialize_local_owner("Me").await.unwrap().id;
        Device {
            _dir: dir,
            vault_dir,
            store,
            vault,
            me,
        }
    }

    /// A sync file made by `from`, opened on `to` with the password (different keys).
    async fn swap(from: &Device, to: &Device) -> OpenedBackup {
        let bytes = create_backup(
            &from.store,
            &from.vault_dir,
            from.vault.dek(),
            "0.3.0",
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
        let schema = current_schema_version(&to.store).await.unwrap();
        let pw = SecretString::from(PW);
        open_backup(
            &bytes,
            BackupKey::Secret(&pw, SecretKind::Password),
            &to.vault_dir,
            schema,
        )
        .unwrap()
    }

    fn entry(description: &str) -> EntryInput {
        EntryInput {
            date: date!(2026 - 10 - 27),
            minutes: Minutes(30),
            description: description.into(),
            project: None,
            category: None,
            next_step: None,
            timesheet_code_id: None,
        }
    }

    async fn touch(store: &SqliteStore, table: &str, id: &str, at: &str) {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE {table} SET updated_at = $1 WHERE id = $2"
        )))
        .bind(at)
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
    }

    async fn descriptions(d: &Device) -> Vec<String> {
        let mut list: Vec<String> = d
            .store
            .list_entries(d.me, date!(2026 - 10 - 01), date!(2026 - 10 - 31))
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.entry.description)
            .collect();
        list.sort();
        list
    }

    /// The phone starts from the computer's backup, then both change things.
    async fn paired() -> (Device, Device, String) {
        let computer = device("sync-pc").await;
        let shared = computer
            .store
            .create_project(computer.me, "Laptop rollout", None)
            .await
            .unwrap();
        computer
            .store
            .create_entry(computer.me, &entry("Weekly ops meeting"), "")
            .await
            .unwrap();
        let phone = device("sync-phone").await;
        let first = swap(&computer, &phone).await;
        restore_backup(&phone.store, &first).await.unwrap();
        let phone = Device {
            me: phone.store.initialize_local_owner("Me").await.unwrap().id,
            ..phone
        };
        assert_eq!(phone.me, computer.me);
        (computer, phone, shared.id.to_string())
    }

    #[test]
    fn brings_in_new_and_newer_and_keeps_newer_here() {
        block_on(async {
            let (computer, phone, project) = paired().await;
            let last_swap = datetime!(2026-10-27 09:00 UTC);
            phone
                .store
                .create_entry(phone.me, &entry("Fixed VPN drop-outs"), "INC0045678")
                .await
                .unwrap();
            computer
                .store
                .create_entry(computer.me, &entry("Printer queue stuck"), "")
                .await
                .unwrap();
            // The project is renamed on both; the phone's edit is newer.
            sqlx::query("UPDATE projects SET name = 'Rollout (phone)' WHERE id = $1")
                .bind(&project)
                .execute(phone.store.pool())
                .await
                .unwrap();
            touch(&phone.store, "projects", &project, "2026-10-27T10:00:00Z").await;
            sqlx::query("UPDATE projects SET name = 'Rollout (pc)' WHERE id = $1")
                .bind(&project)
                .execute(computer.store.pool())
                .await
                .unwrap();
            touch(
                &computer.store,
                "projects",
                &project,
                "2026-10-27T11:00:00Z",
            )
            .await;

            let file = swap(&phone, &computer).await;
            let preview = merge_backup(&computer.store, &file, Some(last_swap), false)
                .await
                .unwrap();
            assert_eq!(preview.new_entries, 1);
            assert_eq!(preview.kept_here, 1);
            assert!(preview.other >= 1, "the phone's ticket");
            assert_eq!(preview.clash_count, 1);
            assert_eq!(preview.clashes[0].winner, Winner::Here);
            assert_eq!(preview.clashes[0].title, "Rollout (pc)");
            // The preview changed nothing.
            assert_eq!(descriptions(&computer).await.len(), 2);

            let done = merge_backup(&computer.store, &file, Some(last_swap), true)
                .await
                .unwrap();
            assert_eq!(done, preview);
            assert_eq!(
                descriptions(&computer).await,
                [
                    "Fixed VPN drop-outs",
                    "Printer queue stuck",
                    "Weekly ops meeting"
                ]
            );
            let name: String = sqlx::query_scalar("SELECT name FROM projects WHERE id = $1")
                .bind(&project)
                .fetch_one(computer.store.pool())
                .await
                .unwrap();
            assert_eq!(name, "Rollout (pc)");

            // Bringing the same file in again changes nothing.
            let again = merge_backup(&computer.store, &file, Some(last_swap), true)
                .await
                .unwrap();
            assert!(again.nothing_new());
        });
    }

    #[test]
    fn deletes_travel_and_settings_stay() {
        block_on(async {
            let (computer, phone, _) = paired().await;
            let gone = phone
                .store
                .list_entries(phone.me, date!(2026 - 10 - 01), date!(2026 - 10 - 31))
                .await
                .unwrap()
                .remove(0);
            phone
                .store
                .delete_entry(phone.me, gone.entry.id)
                .await
                .unwrap();
            phone
                .store
                .set_setting(phone.me, "ui.theme", "dark")
                .await
                .unwrap();
            computer
                .store
                .set_setting(computer.me, "ui.theme", "light")
                .await
                .unwrap();
            let file = swap(&phone, &computer).await;
            let done = merge_backup(&computer.store, &file, None, true)
                .await
                .unwrap();
            assert_eq!(done.deleted, 1);
            assert!(descriptions(&computer).await.is_empty());
            assert_eq!(
                computer
                    .store
                    .get_setting(computer.me, "ui.theme")
                    .await
                    .unwrap()
                    .as_deref(),
                Some("light")
            );
        });
    }

    #[test]
    fn a_clash_on_a_unique_rule_is_left_out_with_what_points_at_it() {
        block_on(async {
            let (computer, phone, _) = paired().await;
            // Both add to the same day's Tickets tray: two roll-up entries for one day.
            phone
                .store
                .tray_add(phone.me, date!(2026 - 10 - 27), "REQ2210")
                .await
                .unwrap();
            computer
                .store
                .tray_add(computer.me, date!(2026 - 10 - 27), "REQ2214")
                .await
                .unwrap();
            let file = swap(&phone, &computer).await;
            let done = merge_backup(&computer.store, &file, None, true)
                .await
                .unwrap();
            assert!(done.left_out >= 1);
            // Still one roll-up entry for the day, and nothing broken.
            let day = computer
                .store
                .tray_day(computer.me, date!(2026 - 10 - 27))
                .await
                .unwrap();
            assert!(day.items.iter().any(|i| i.ticket_number == "REQ2214"));
        });
    }

    #[test]
    fn last_swap_round_trips() {
        let dir = TestDir::new("sync-last");
        assert_eq!(last_swap(dir.path()), None);
        let at = datetime!(2026-10-27 17:10 UTC);
        note_swap(dir.path(), at).unwrap();
        assert_eq!(last_swap(dir.path()), Some(at));
    }

    #[test]
    fn refuses_a_copy_set_up_separately() {
        block_on(async {
            let a = device("sync-a").await;
            let b = device("sync-b").await;
            let file = swap(&a, &b).await;
            assert!(matches!(
                merge_backup(&b.store, &file, None, false).await,
                Err(BackupError::NotPaired)
            ));
        });
    }
}
