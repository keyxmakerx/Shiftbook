//! Shiftbook desktop shell. Commands here stay thin: they call `shiftbook-core` and
//! `shiftbook-storage` and translate results for the frontend's `TauriApi`.
//!
//! Vault commands (`vault_status`, `vault_setup`, `vault_confirm_recovery_saved`,
//! `unlock`, `lock`, `change_password`) go through [`session::VaultSession`]. Errors
//! reach the WebView only as short codes such as `"unlock_failed"`.

mod api;
mod backoff;
mod protect;
mod session;

use serde::Serialize;
use tauri::{Manager, State};

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
    state: State<'_, VaultSession>,
    secret: String,
    kind: String,
) -> Result<api::UnlockOutcome, CommandError> {
    let secret = take_secret(secret);
    let kind = SecretKind::parse(&kind).ok_or(CommandError::UnlockFailed)?;
    let must_set_password = state.unlock(secret, kind).await?;
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

/// Starts the desktop app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            api::add_inbox,
            api::set_inbox_deleted,
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
