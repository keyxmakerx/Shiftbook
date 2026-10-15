//! Shiftbook desktop shell. Commands here stay thin: they call `shiftbook-core` and
//! `shiftbook-storage` and translate results for the frontend's `TauriApi`.

use serde::Serialize;

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

/// Starts the desktop app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
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
