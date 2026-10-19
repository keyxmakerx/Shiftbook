//! Shiftbook desktop shell. Commands here stay thin: they call `shiftbook-core` and
//! `shiftbook-storage` and translate results for the frontend's `TauriApi`.
//!
//! Vault commands (`vault_status`, `vault_setup`, `vault_confirm_recovery_saved`,
//! `unlock`, `lock`, `change_password`, `delete_all_data`) go through [`session::VaultSession`]. Errors
//! reach the WebView only as short codes such as `"unlock_failed"`.

mod api;
mod backoff;
mod canvas;
mod notes;
mod protect;
mod session;

use serde::Serialize;
use shiftbook_storage::backup::{BackupFile, BackupSummary, BACKUP_EXTENSION};
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
    start_automatic_backup(app);
    Ok(api::UnlockOutcome { must_set_password })
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
    let _ = window.set_content_protected(true);
    state.lock().await
}

/// Set a new password straight after a recovery-key unlock (no old password).
#[tauri::command]
async fn set_new_password(
    state: State<'_, VaultSession>,
    new_password: String,
) -> Result<(), CommandError> {
    state.set_new_password(take_secret(new_password)).await
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
    state: State<'_, VaultSession>,
    current_password: String,
    new_password: String,
) -> Result<(), CommandError> {
    state
        .change_password(take_secret(current_password), take_secret(new_password))
        .await
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
    let _ = window.set_content_protected(true);
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

/// A dialog result as a path, off the main thread.
async fn dialog_path(
    work: impl FnOnce() -> Option<tauri_plugin_dialog::FilePath> + Send + 'static,
) -> Result<Option<std::path::PathBuf>, CommandError> {
    let picked = tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| CommandError::StorageError)?;
    Ok(picked.and_then(|p| p.into_path().ok()))
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
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Save a backup")
        .add_filter("Shiftbook backup", &[BACKUP_EXTENSION])
        .set_file_name(format!("Shiftbook backup {today}.{BACKUP_EXTENSION}"));
    let Some(mut path) = dialog_path(move || dialog.blocking_save_file()).await? else {
        return Ok(None);
    };
    if path.extension().is_none_or(|e| e != BACKUP_EXTENSION) {
        path.set_extension(BACKUP_EXTENSION);
    }
    let name = file_name_of(&path);
    state.backup_to(path).await?;
    Ok(Some(name))
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
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Restore from a backup")
        .add_filter("Shiftbook backup", &[BACKUP_EXTENSION]);
    let Some(path) = dialog_path(move || dialog.blocking_pick_file()).await? else {
        return Ok(None);
    };
    let name = file_name_of(&path);
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

/// Starts the desktop app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            api::add_entry_minutes,
            api::delete_entry,
            api::restore_entry,
            api::add_entry_tickets,
            api::remove_ticket,
            api::quick_log,
            api::parse_capture_line,
            api::capture_log,
            api::tray_day,
            api::tray_add,
            api::tray_set_minutes,
            api::tray_set_note,
            api::tray_set_done,
            api::tray_set_project,
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
            api::list_responses,
            api::save_responses,
            api::where_was_i,
            api::timer_offer,
            notes::list_notes,
            notes::get_note,
            notes::create_note,
            canvas::canvas,
            canvas::canvas_move,
            canvas::canvas_area,
            canvas::canvas_area_remove,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shiftbook");
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
