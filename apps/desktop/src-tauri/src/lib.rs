//! Shiftbook desktop shell. Commands here stay thin: they call `shiftbook-core` and
//! `shiftbook-storage` and translate results for the frontend's `TauriApi`.
//!
//! Vault commands (`vault_status`, `vault_setup`, `vault_confirm_recovery_saved`,
//! `unlock`, `lock`, `change_password`) go through [`session::VaultSession`]. Errors
//! reach the WebView only as short codes such as `"unlock_failed"`.

mod backoff;
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

/// Unlock with `kind` = `"password"` or `"recovery_key"`. Nothing is returned.
#[tauri::command]
async fn unlock(
    state: State<'_, VaultSession>,
    secret: String,
    kind: String,
) -> Result<(), CommandError> {
    let secret = take_secret(secret);
    let kind = SecretKind::parse(&kind).ok_or(CommandError::UnlockFailed)?;
    state.unlock(secret, kind).await
}

/// Close the database and wipe the key. The frontend reloads the WebView next.
#[tauri::command]
async fn lock(state: State<'_, VaultSession>) -> Result<(), CommandError> {
    state.lock().await
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            vault_status,
            vault_setup,
            vault_confirm_recovery_saved,
            unlock,
            lock,
            change_password
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
