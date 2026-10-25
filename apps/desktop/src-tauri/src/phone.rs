//! Android-only pieces (round 15), behind one small API so the commands stay the same on
//! every platform. On Android they call `PhonePlugin` (Kotlin, in
//! `gen/android/app/src/main/java/.../PhonePlugin.kt`); elsewhere they do the desktop thing
//! or nothing. The page never calls the plugin directly.

use std::path::Path;

#[cfg(target_os = "android")]
use tauri::Manager;
use tauri::{AppHandle, Runtime, WebviewWindow};

/// The registered Kotlin plugin.
#[cfg(target_os = "android")]
pub struct Phone<R: Runtime>(tauri::plugin::PluginHandle<R>);

/// Registers the Kotlin plugin on Android; a no-op plugin elsewhere.
pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("phone")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle =
                    _api.register_android_plugin("io.github.keyxmakerx.shiftbook", "PhonePlugin")?;
                _app.manage(Phone(handle));
            }
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
fn call<R: Runtime>(
    app: &AppHandle<R>,
    command: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, ()> {
    let phone = app.try_state::<Phone<R>>().ok_or(())?;
    phone.0.run_mobile_plugin(command, args).map_err(|_| ())
}

/// Screenshot and screen-share protection: the window's content protection on a
/// computer, `FLAG_SECURE` on Android.
pub fn set_protected<R: Runtime>(window: &WebviewWindow<R>, on: bool) -> Result<(), ()> {
    #[cfg(target_os = "android")]
    {
        call(
            window.app_handle(),
            "setSecure",
            serde_json::json!({ "on": on }),
        )
        .map(|_| ())
    }
    #[cfg(not(target_os = "android"))]
    {
        window.set_content_protected(on).map_err(|_| ())
    }
}

/// Copy a file into a document the user picked (Android `content://` link).
pub fn copy_to_uri<R: Runtime>(app: &AppHandle<R>, path: &Path, uri: &str) -> Result<(), ()> {
    #[cfg(target_os = "android")]
    {
        call(
            app,
            "copyToUri",
            serde_json::json!({ "path": path.to_string_lossy(), "uri": uri }),
        )
        .map(|_| ())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, path, uri);
        Err(())
    }
}

/// Copy a document the user picked (Android `content://` link) into a file of ours.
pub fn copy_from_uri<R: Runtime>(app: &AppHandle<R>, uri: &str, path: &Path) -> Result<(), ()> {
    #[cfg(target_os = "android")]
    {
        call(
            app,
            "copyFromUri",
            serde_json::json!({ "path": path.to_string_lossy(), "uri": uri }),
        )
        .map(|_| ())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, path, uri);
        Err(())
    }
}

/// The name a picked document shows in the Files app, if Android tells us.
pub fn display_name<R: Runtime>(app: &AppHandle<R>, uri: &str) -> Option<String> {
    #[cfg(target_os = "android")]
    {
        let value = call(app, "displayName", serde_json::json!({ "uri": uri })).ok()?;
        value
            .get("name")
            .and_then(|n| n.as_str())
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, uri);
        None
    }
}
