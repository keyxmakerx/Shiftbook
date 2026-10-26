//! Shiftbook desktop shell. Commands here stay thin: they call `shiftbook-core` and
//! `shiftbook-storage` and translate results for the frontend's `TauriApi`.
//!
//! Vault commands (`vault_status`, `vault_setup`, `vault_confirm_recovery_saved`,
//! `unlock`, `lock`, `change_password`, `delete_all_data` and Quick unlock's
//! `unlock_with_pin` and friends) go through [`session::VaultSession`]. Errors
//! reach the WebView only as short codes such as `"unlock_failed"`.

mod api;
mod backoff;
mod boards;
mod canvas;
mod home;
mod meetings;
mod notes;
mod phone;
mod pins;
mod protect;
mod session;
#[cfg(test)]
mod test_support;

use serde::Serialize;
use shiftbook_storage::backup::{BackupFile, BackupSummary, BACKUP_EXTENSION};
use shiftbook_storage::sync::{SyncReport, SYNC_EXTENSION};
use shiftbook_storage::{ExposeSecret, SecretString};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use session::{reveal_once, take_secret, CommandError, SecretKind, VaultSession, VaultStatus};

/// Basic information about the running app, shown in the UI footer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

/// Builds the `AppInfo` for this build.
fn current_app_info() -> AppInfo {
    AppInfo {
        name: shiftbook_core::app_name().to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// Returns the app name and version. Maps to `DataApi.appInfo()`.
#[tauri::command]
fn app_info() -> AppInfo {
    current_app_info()
}

/// `needs_setup`, `locked` or `unlocked`. Maps to `DataApi.vaultStatus()`.
#[tauri::command]
async fn vault_status(state: State<'_, VaultSession>) -> Result<VaultStatus, CommandError> {
    state.status().await
}

/// First-run setup with a new master password. Returns the recovery key text,
/// the only time it ever leaves the vault code. Maps to `DataApi.vaultSetup()`.
#[tauri::command]
async fn vault_setup(
    state: State<'_, VaultSession>,
    password: String,
) -> Result<String, CommandError> {
    let key = state.setup(take_secret(password)).await?;
    Ok(reveal_once(key))
}

/// The user confirmed they saved the recovery key.
#[tauri::command]
async fn vault_confirm_recovery_saved(state: State<'_, VaultSession>) -> Result<(), CommandError> {
    state.confirm_recovery_saved().await
}

/// Unlock with `kind` = `"password"` or `"recovery_key"`. Tells the UI whether
/// to ask for a new password now (after a recovery-key unlock).
#[tauri::command]
async fn unlock(
    app: AppHandle,
    state: State<'_, VaultSession>,
    secret: String,
    kind: String,
) -> Result<api::UnlockOutcome, CommandError> {
    let secret = take_secret(secret);
    let kind = SecretKind::parse(&kind).ok_or(CommandError::UnlockFailed)?;
    let must_set_password = state.unlock(secret, kind).await?;
    if kind == SecretKind::Password {
        phone::password_typed(&app);
    }
    start_automatic_backup(app);
    Ok(api::UnlockOutcome { must_set_password })
}

/// Phone: can this phone unlock with a fingerprint, and is it on and ready?
#[tauri::command]
fn fingerprint_status(app: AppHandle) -> phone::FingerprintStatus {
    phone::fingerprint_status(&app)
}

/// Phone: turn fingerprint unlock on. Checks the password first, then Android asks for a
/// fingerprint and keeps the password encrypted in the phone's secure hardware.
#[tauri::command]
async fn fingerprint_on(
    app: AppHandle,
    state: State<'_, VaultSession>,
    password: String,
) -> Result<(), CommandError> {
    let password = take_secret(password);
    let check = SecretString::from(password.expose_secret());
    state.check_password(check).await?;
    tauri::async_runtime::spawn_blocking(move || phone::fingerprint_on(&app, &password))
        .await
        .map_err(|_| CommandError::StorageError)?
        .map_err(|_| CommandError::FingerprintFailed)
}

/// Phone: turn fingerprint unlock off.
#[tauri::command]
fn fingerprint_off(app: AppHandle) {
    phone::fingerprint_off(&app);
}

/// Phone: unlock with a fingerprint. The stored password goes from Android straight to the
/// vault, never through the page. If it no longer unlocks (changed elsewhere), fingerprint
/// unlock turns off and the password is asked for.
#[tauri::command]
async fn fingerprint_unlock(
    app: AppHandle,
    state: State<'_, VaultSession>,
) -> Result<api::UnlockOutcome, CommandError> {
    let asking = app.clone();
    let password =
        tauri::async_runtime::spawn_blocking(move || phone::fingerprint_password(&asking))
            .await
            .map_err(|_| CommandError::StorageError)?
            .ok_or(CommandError::FingerprintFailed)?;
    match state.unlock(password, SecretKind::Password).await {
        Ok(must_set_password) => {
            start_automatic_backup(app);
            Ok(api::UnlockOutcome { must_set_password })
        }
        Err(CommandError::UnlockFailed) => {
            phone::fingerprint_off(&app);
            Err(CommandError::FingerprintFailed)
        }
        Err(other) => Err(other),
    }
}

/// Close the database and wipe the key, and turn screenshot protection back on.
/// The frontend reloads the WebView next.
#[tauri::command]
async fn lock(
    state: State<'_, VaultSession>,
    window: tauri::WebviewWindow,
    guard: State<'_, protect::CaptureGuard>,
) -> Result<(), CommandError> {
    guard.end_allow();
    let _ = phone::set_protected(&window, true);
    state.lock().await
}

/// Unlock with today's PIN (Quick unlock, round 15). A wrong PIN answers with the tries left
/// rather than an error. Maps to `DataApi.unlockWithPin()`.
#[tauri::command]
async fn unlock_with_pin(
    app: AppHandle,
    state: State<'_, VaultSession>,
    pin: String,
    today: String,
) -> Result<session::PinOutcome, CommandError> {
    let pin = take_secret(pin);
    let outcome = state.unlock_with_pin(pin, api::date_arg(&today)?).await?;
    if outcome.unlocked {
        start_automatic_backup(app);
    }
    Ok(outcome)
}

/// True when a PIN can open the app now. Maps to `DataApi.quickUnlockReady()`.
#[tauri::command]
async fn quick_unlock_ready(
    state: State<'_, VaultSession>,
    today: String,
) -> Result<bool, CommandError> {
    state.quick_unlock_ready(api::date_arg(&today)?).await
}

/// Set today's PIN after a password unlock. Maps to `DataApi.startQuickUnlock()`.
#[tauri::command]
async fn start_quick_unlock(
    state: State<'_, VaultSession>,
    pin: String,
    today: String,
) -> Result<(), CommandError> {
    state
        .start_quick_unlock(take_secret(pin), api::date_arg(&today)?)
        .await
}

/// Turn Quick unlock on in Settings → Security (checks the password, sets today's PIN).
/// Maps to `DataApi.enableQuickUnlock()`.
#[tauri::command]
async fn enable_quick_unlock(
    state: State<'_, VaultSession>,
    password: String,
    pin: String,
    today: String,
) -> Result<(), CommandError> {
    state
        .enable_quick_unlock(
            take_secret(password),
            take_secret(pin),
            api::date_arg(&today)?,
        )
        .await?;
    let (store, owner) = state.data().await?;
    store
        .set_pref(owner, shiftbook_core::prefs::KEY_QUICK_UNLOCK, "true")
        .await?;
    Ok(())
}

/// End the PIN window now (wipes the wrapped key). Maps to `DataApi.endQuickUnlock()`.
#[tauri::command]
async fn end_quick_unlock(state: State<'_, VaultSession>) -> Result<(), CommandError> {
    state.end_quick_unlock().await
}

/// Set a new password straight after a recovery-key unlock (no old password).
#[tauri::command]
async fn set_new_password(
    app: AppHandle,
    state: State<'_, VaultSession>,
    new_password: String,
) -> Result<(), CommandError> {
    state.set_new_password(take_secret(new_password)).await?;
    // A stored fingerprint password would be the old one.
    phone::fingerprint_off(&app);
    Ok(())
}

/// Make a new recovery key (needs the password). Returns it once.
#[tauri::command]
async fn regenerate_recovery_key(
    state: State<'_, VaultSession>,
    password: String,
) -> Result<String, CommandError> {
    let key = state.regenerate_recovery_key(take_secret(password)).await?;
    Ok(reveal_once(key))
}

/// Change the master password (re-wraps the key; the data isn't re-encrypted).
#[tauri::command]
async fn change_password(
    app: AppHandle,
    state: State<'_, VaultSession>,
    current_password: String,
    new_password: String,
) -> Result<(), CommandError> {
    state
        .change_password(take_secret(current_password), take_secret(new_password))
        .await?;
    // A stored fingerprint password would be the old one: turn it on again with the new one.
    phone::fingerprint_off(&app);
    Ok(())
}

/// Delete all local data (database and vault) so the app starts again at setup.
/// Needs the typed confirmation word; works while locked. Turns screenshot
/// protection back on like `lock`. The frontend reloads the WebView next.
#[tauri::command]
async fn delete_all_data(
    state: State<'_, VaultSession>,
    window: tauri::WebviewWindow,
    guard: State<'_, protect::CaptureGuard>,
    confirm: String,
) -> Result<(), CommandError> {
    guard.end_allow();
    let _ = phone::set_protected(&window, true);
    state.delete_all_data(&confirm).await
}

// ---------- backups ----------

/// Make the daily automatic backup in the background, if one is due. A failure
/// is not shown: the next unlock tries again.
fn start_automatic_backup(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _ = app.state::<VaultSession>().automatic_backup().await;
    });
}

/// A dialog result, off the main thread.
async fn dialog_pick(
    work: impl FnOnce() -> Option<tauri_plugin_dialog::FilePath> + Send + 'static,
) -> Result<Option<tauri_plugin_dialog::FilePath>, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| CommandError::StorageError)
}

/// A file of our own for passing a backup to or from a picked Android document.
fn phone_temp(app: &AppHandle, name: &str) -> Result<std::path::PathBuf, CommandError> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|_| CommandError::StorageError)?;
    shiftbook_storage::create_private_dir(&dir).map_err(|_| CommandError::StorageError)?;
    Ok(dir.join(name))
}

fn file_name_of(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Save a backup where the user chooses; `today` (the local date) names the
/// file. Returns the file name, or `None` if the dialog was cancelled. Maps to
/// `DataApi.backupNow()`.
#[tauri::command]
async fn backup_now(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultSession>,
    today: String,
) -> Result<Option<String>, CommandError> {
    let today = shiftbook_core::dates::parse_date(&today)
        .map(shiftbook_core::dates::format_date)
        .map_err(|_| CommandError::InvalidInput)?;
    let dialog = app.dialog().file();
    // Phones have one window and no parent dialogs.
    #[cfg(desktop)]
    let dialog = dialog.set_parent(&window);
    #[cfg(mobile)]
    let _ = window;
    let file_name = format!("Shiftbook backup {today}.{BACKUP_EXTENSION}");
    let dialog = dialog.set_title("Save a backup").set_file_name(&file_name);
    // Android picks by file type, and knows no type for .shiftbak.
    #[cfg(desktop)]
    let dialog = dialog.add_filter("Shiftbook backup", &[BACKUP_EXTENSION]);
    match dialog_pick(move || dialog.blocking_save_file()).await? {
        None => Ok(None),
        Some(tauri_plugin_dialog::FilePath::Path(mut path)) => {
            if path.extension().is_none_or(|e| e != BACKUP_EXTENSION) {
                path.set_extension(BACKUP_EXTENSION);
            }
            let name = file_name_of(&path);
            state.backup_to(path).await?;
            Ok(Some(name))
        }
        // Android: a document link. Make the backup in our cache, copy it there, remove ours.
        Some(tauri_plugin_dialog::FilePath::Url(url)) => {
            let temp = phone_temp(&app, "outgoing.shiftbak")?;
            let made = state.backup_to(temp.clone()).await;
            let copied = made.and_then(|()| {
                phone::copy_to_uri(&app, &temp, url.as_str())
                    .map_err(|()| CommandError::StorageError)
            });
            let _ = std::fs::remove_file(&temp);
            copied?;
            Ok(Some(
                phone::display_name(&app, url.as_str()).unwrap_or(file_name),
            ))
        }
    }
}

/// The automatic and safety backups. Maps to `DataApi.listBackups()`.
#[tauri::command]
async fn list_backups(state: State<'_, VaultSession>) -> Result<Vec<BackupFile>, CommandError> {
    state.list_backups().await
}

/// Pick a backup file to restore. Returns its name, or `None` if cancelled; the
/// file is then the `"picked"` source. Maps to `DataApi.pickBackupFile()`.
#[tauri::command]
async fn pick_backup_file(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultSession>,
) -> Result<Option<String>, CommandError> {
    let dialog = app.dialog().file();
    // Phones have one window and no parent dialogs.
    #[cfg(desktop)]
    let dialog = dialog.set_parent(&window);
    #[cfg(mobile)]
    let _ = window;
    let dialog = dialog.set_title("Restore from a backup");
    #[cfg(desktop)]
    let dialog = dialog.add_filter("Shiftbook backup", &[BACKUP_EXTENSION]);
    let (path, name) = match dialog_pick(move || dialog.blocking_pick_file()).await? {
        None => return Ok(None),
        Some(tauri_plugin_dialog::FilePath::Path(path)) => {
            let name = file_name_of(&path);
            (path, name)
        }
        // Android: copy the picked document into our cache; restore reads it from there.
        Some(tauri_plugin_dialog::FilePath::Url(url)) => {
            let temp = phone_temp(&app, "incoming.shiftbak")?;
            phone::copy_from_uri(&app, url.as_str(), &temp)
                .map_err(|()| CommandError::StorageError)?;
            let name =
                phone::display_name(&app, url.as_str()).unwrap_or_else(|| "the backup".to_owned());
            (temp, name)
        }
    };
    state.set_picked_backup(path).await?;
    Ok(Some(name))
}

fn backup_secret(
    secret: Option<String>,
    kind: Option<String>,
) -> Result<Option<(shiftbook_storage::SecretString, SecretKind)>, CommandError> {
    match secret {
        None => Ok(None),
        Some(text) => {
            let secret = take_secret(text);
            let kind = kind
                .as_deref()
                .and_then(SecretKind::parse)
                .ok_or(CommandError::InvalidInput)?;
            Ok(Some((secret, kind)))
        }
    }
}

/// Check a backup (`source` is a name from `list_backups` or `"picked"`) and
/// say what it holds. `secret` is only needed for a backup from another
/// computer or an older vault. Maps to `DataApi.checkBackup()`.
#[tauri::command]
async fn check_backup(
    state: State<'_, VaultSession>,
    source: String,
    secret: Option<String>,
    kind: Option<String>,
) -> Result<BackupSummary, CommandError> {
    let secret = backup_secret(secret, kind)?;
    state.check_backup(&source, secret).await
}

/// Replace all data with a backup, after saving a safety backup of the current
/// data. The frontend reloads everything next. Maps to `DataApi.restoreBackup()`.
#[tauri::command]
async fn restore_backup(
    state: State<'_, VaultSession>,
    source: String,
    secret: Option<String>,
    kind: Option<String>,
) -> Result<BackupSummary, CommandError> {
    let secret = backup_secret(secret, kind)?;
    state.restore_backup(&source, secret).await
}

// ---------- sync files (round 15): swapping changes between a phone and a computer ----------

/// Maps to `DataApi.lastSwap()`: when this device last made or brought in a sync file.
#[tauri::command]
async fn last_swap(state: State<'_, VaultSession>) -> Result<Option<String>, CommandError> {
    state.last_swap().await
}

/// Make a sync file where the user picks (a save dialog, or Android's file picker). Returns
/// its name, or `None` if cancelled. Maps to `DataApi.makeSyncFile()`.
#[tauri::command]
async fn make_sync_file(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultSession>,
    today: String,
) -> Result<Option<String>, CommandError> {
    let today = shiftbook_core::dates::parse_date(&today)
        .map(shiftbook_core::dates::format_date)
        .map_err(|_| CommandError::InvalidInput)?;
    let dialog = app.dialog().file();
    #[cfg(desktop)]
    let dialog = dialog.set_parent(&window);
    #[cfg(mobile)]
    let _ = window;
    let file_name = format!("Shiftbook sync {today}.{SYNC_EXTENSION}");
    let dialog = dialog
        .set_title("Make a sync file")
        .set_file_name(&file_name);
    #[cfg(desktop)]
    let dialog = dialog.add_filter("Shiftbook sync file", &[SYNC_EXTENSION]);
    match dialog_pick(move || dialog.blocking_save_file()).await? {
        None => Ok(None),
        Some(tauri_plugin_dialog::FilePath::Path(mut path)) => {
            if path.extension().is_none_or(|e| e != SYNC_EXTENSION) {
                path.set_extension(SYNC_EXTENSION);
            }
            let name = file_name_of(&path);
            state.sync_file_to(path).await?;
            Ok(Some(name))
        }
        Some(tauri_plugin_dialog::FilePath::Url(url)) => {
            let temp = phone_temp(&app, "outgoing.shiftsync")?;
            let made = state.sync_file_to(temp.clone()).await;
            let copied = made.and_then(|()| {
                phone::copy_to_uri(&app, &temp, url.as_str())
                    .map_err(|()| CommandError::StorageError)
            });
            let _ = std::fs::remove_file(&temp);
            copied?;
            Ok(Some(
                phone::display_name(&app, url.as_str()).unwrap_or(file_name),
            ))
        }
    }
}

/// Pick a sync file made by the other device. Returns its name, or `None` if cancelled.
/// Maps to `DataApi.pickSyncFile()`.
#[tauri::command]
async fn pick_sync_file(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, VaultSession>,
) -> Result<Option<String>, CommandError> {
    let dialog = app.dialog().file();
    #[cfg(desktop)]
    let dialog = dialog.set_parent(&window);
    #[cfg(mobile)]
    let _ = window;
    let dialog = dialog.set_title("Open a sync file");
    #[cfg(desktop)]
    let dialog = dialog.add_filter("Shiftbook sync file", &[SYNC_EXTENSION]);
    let (path, name) = match dialog_pick(move || dialog.blocking_pick_file()).await? {
        None => return Ok(None),
        Some(tauri_plugin_dialog::FilePath::Path(path)) => {
            let name = file_name_of(&path);
            (path, name)
        }
        Some(tauri_plugin_dialog::FilePath::Url(url)) => {
            let temp = phone_temp(&app, "incoming.shiftsync")?;
            phone::copy_from_uri(&app, url.as_str(), &temp)
                .map_err(|()| CommandError::StorageError)?;
            let name = phone::display_name(&app, url.as_str())
                .unwrap_or_else(|| "the sync file".to_owned());
            (temp, name)
        }
    };
    state.set_picked_sync(path).await?;
    Ok(Some(name))
}

/// What bringing the picked sync file in would do; changes nothing. A file made with another
/// key needs that device's password or recovery key. Maps to `DataApi.checkSyncFile()`.
#[tauri::command]
async fn check_sync_file(
    state: State<'_, VaultSession>,
    secret: Option<String>,
    kind: Option<String>,
) -> Result<SyncReport, CommandError> {
    let secret = backup_secret(secret, kind)?;
    state.merge_sync(secret, false).await
}

/// Bring the picked sync file's changes in, after a safety backup. The frontend reloads
/// everything next. Maps to `DataApi.bringInSyncFile()`.
#[tauri::command]
async fn bring_in_sync_file(
    state: State<'_, VaultSession>,
    secret: Option<String>,
    kind: Option<String>,
) -> Result<SyncReport, CommandError> {
    let secret = backup_secret(secret, kind)?;
    state.merge_sync(secret, true).await
}

/// Starts the desktop app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(phone::init())
        .setup(|app| {
            // Tauri's per-user app data folder; never a hard-coded path.
            let data_dir = app
                .path()
                .app_data_dir()
                .ok()
                .filter(|dir| shiftbook_storage::create_private_dir(dir).is_ok());
            app.manage(VaultSession::new(data_dir));
            app.manage(protect::CaptureGuard::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            vault_status,
            vault_setup,
            vault_confirm_recovery_saved,
            unlock,
            lock,
            change_password,
            unlock_with_pin,
            quick_unlock_ready,
            start_quick_unlock,
            enable_quick_unlock,
            end_quick_unlock,
            delete_all_data,
            set_new_password,
            regenerate_recovery_key,
            protect::allow_capture,
            protect::protect_capture,
            api::load_prefs,
            api::save_pref,
            api::list_projects,
            api::create_project,
            api::update_project,
            api::list_entries,
            api::create_entry,
            api::update_entry,
            api::set_entry_notes,
            api::set_entry_codes,
            api::usual_code,
            api::get_entry,
            api::tray_item,
            meetings::meeting_series,
            meetings::save_meeting_series,
            meetings::set_meeting_series_deleted,
            meetings::meeting_note,
            meetings::log_meeting,
            meetings::skip_meeting,
            meetings::bell,
            api::add_entry_minutes,
            api::add_entry_time,
            api::log_note,
            api::delete_entry,
            api::restore_entry,
            api::add_entry_tickets,
            api::remove_ticket,
            api::ticket_set_done,
            api::ticket_set_note,
            api::quick_log,
            api::parse_capture_line,
            api::capture_log,
            api::tray_day,
            api::tray_add,
            api::tray_set_minutes,
            api::tray_set_note,
            api::tray_set_done,
            api::tray_set_project,
            api::tray_set_recurring,
            api::tray_carry,
            api::tray_remove,
            api::tray_restore,
            api::ribbon,
            api::week_sheet,
            api::list_ticks,
            api::set_tick,
            api::list_timesheet_codes,
            api::save_timesheet_code,
            api::category_codes,
            api::set_category_code,
            api::list_custom_fields,
            api::save_custom_field,
            api::delete_custom_field,
            api::list_custom_values,
            api::set_custom_value,
            api::list_inbox,
            api::search,
            api::add_inbox,
            api::set_inbox_deleted,
            api::next_up,
            api::set_next_up,
            api::tick_next_up,
            api::bring_up,
            api::bring_up_days,
            api::standup,
            api::pasted_tickets,
            api::find_day,
            api::list_responses,
            api::save_responses,
            api::where_was_i,
            api::timer_offer,
            api::timer_away_question,
            phone::phone_launch,
            phone::phone_away_seconds,
            phone::phone_timer_away,
            phone::phone_allow_notifications,
            phone::plan_nudges,
            fingerprint_status,
            fingerprint_on,
            fingerprint_off,
            fingerprint_unlock,
            last_swap,
            make_sync_file,
            pick_sync_file,
            check_sync_file,
            bring_in_sync_file,
            notes::list_notes,
            notes::get_note,
            notes::create_note,
            canvas::canvas,
            canvas::canvas_move,
            canvas::canvas_area,
            canvas::canvas_area_remove,
            canvas::canvas_area_collapse,
            canvas::canvas_label_save,
            canvas::canvas_label_remove,
            canvas::canvas_stack,
            canvas::canvas_unstack,
            canvas::canvas_stack_move,
            canvas::canvas_stack_rename,
            canvas::canvas_stack_dissolve,
            canvas::canvas_file,
            canvas::canvas_file_stack,
            canvas::canvas_unfile,
            canvas::canvas_drawer,
            canvas::canvas_folder_save,
            canvas::canvas_folder_remove,
            canvas::canvas_tidy,
            boards::board,
            boards::board_move,
            pins::pins,
            pins::pin,
            pins::unpin,
            canvas::canvas_link,
            canvas::canvas_link_reset,
            notes::daily_note,
            notes::save_daily_note,
            notes::update_note,
            notes::set_note_archived,
            notes::set_note_keep,
            notes::set_note_deleted,
            backup_now,
            list_backups,
            pick_backup_file,
            check_backup,
            restore_backup,
            home::recurring_list,
            home::recurring_save,
            home::recurring_done,
            home::recurring_renewed,
            home::recurring_undo,
            home::recurring_set_deleted,
            home::recurring_choices,
            home::lists,
            home::list_create,
            home::list_rename,
            home::list_set_deleted,
            home::list_add_items,
            home::list_item_done,
            home::list_item_text,
            home::list_item_deleted,
            home::list_reset,
            home::set_use_mode,
            home::while_away,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Shiftbook")
        .run(|app, event| {
            // Close the database before the process ends, so no connection is still
            // closing on its own thread while the encryption library shuts down.
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<VaultSession>() {
                    let _ = tauri::async_runtime::block_on(state.lock());
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_has_name_and_version() {
        let info = current_app_info();
        assert_eq!(info.name, "Shiftbook");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn app_info_serializes_with_expected_keys() {
        let json = serde_json::to_value(current_app_info()).unwrap();
        assert_eq!(json["name"], "Shiftbook");
        assert!(json["version"].is_string());
    }
}
