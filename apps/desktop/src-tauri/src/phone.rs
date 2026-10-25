//! Android-only pieces (round 15), behind one small API so the commands stay the same on
//! every platform. On Android they call `PhonePlugin` (Kotlin, in
//! `gen/android/app/src/main/java/.../PhonePlugin.kt`); elsewhere they do the desktop thing
//! or nothing. The page never calls the plugin directly.

use std::path::Path;

#[cfg(target_os = "android")]
use shiftbook_storage::ExposeSecret;
use shiftbook_storage::SecretString;
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

// ---------- launches, time away, the timer notification and nudges (round 15, ideas C–G) ----------

/// Longest shared text taken in; more is cut.
#[cfg_attr(not(target_os = "android"), allow(dead_code))] // Used on Android; tested everywhere.
pub const MAX_SHARED_CHARS: usize = 10_000;

/// Why Shiftbook was opened, when it wasn't just its icon: text shared into it, a
/// long-press shortcut, the timer tile or a nudge. Taken once.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    /// `share`, `log`, `timer`, `jot`, `due` or `wrapup`.
    pub kind: String,
    /// The shared text, for `share`.
    pub text: Option<String>,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))] // Used on Android; tested everywhere.
const LAUNCH_KINDS: [&str; 6] = ["share", "log", "timer", "jot", "due", "wrapup"];

/// Check what Android handed over: a known kind, and text only with `share`.
#[cfg_attr(not(target_os = "android"), allow(dead_code))] // Used on Android; tested everywhere.
pub fn launch_from(kind: &str, text: Option<&str>) -> Option<Launch> {
    if !LAUNCH_KINDS.contains(&kind) {
        return None;
    }
    let text = (kind == "share")
        .then(|| text.map(|t| t.chars().take(MAX_SHARED_CHARS).collect::<String>()))
        .flatten()
        .filter(|t| !t.trim().is_empty());
    if kind == "share" && text.is_none() {
        return None;
    }
    Some(Launch {
        kind: kind.to_string(),
        text,
    })
}

/// Maps to `DataApi.phoneLaunch()`.
#[tauri::command]
pub fn phone_launch<R: Runtime>(app: AppHandle<R>) -> Option<Launch> {
    #[cfg(target_os = "android")]
    {
        let v = call(&app, "takeLaunch", serde_json::json!({})).ok()?;
        launch_from(
            v.get("kind")?.as_str()?,
            v.get("text").and_then(|t| t.as_str()),
        )
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        None
    }
}

/// Maps to `DataApi.phoneAwaySeconds()`: seconds since Shiftbook last went to the
/// background, measured by Android, then forgotten. 0 when it hasn't been away.
#[tauri::command]
pub fn phone_away_seconds<R: Runtime>(app: AppHandle<R>) -> u64 {
    #[cfg(target_os = "android")]
    {
        call(&app, "takeAway", serde_json::json!({}))
            .ok()
            .and_then(|v| v.get("seconds").and_then(|s| s.as_u64()))
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        0
    }
}

/// Maps to `DataApi.phoneTimerAway()`: while Shiftbook is in the background with the timer
/// running, a quiet notification and the timer tile show it runs and for how long (never
/// what it's on). `on: false` takes them away.
#[tauri::command]
pub fn phone_timer_away<R: Runtime>(app: AppHandle<R>, on: bool, seconds: u64) {
    #[cfg(target_os = "android")]
    {
        let seconds = seconds.min(shiftbook_core::timer::MAX_TIMER_SECONDS);
        let _ = call(
            &app,
            "timerAway",
            serde_json::json!({ "on": on, "seconds": seconds }),
        );
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, on, seconds);
    }
}

/// Maps to `DataApi.phoneAllowNotifications()`: ask Android for permission to notify (needed
/// from Android 13). True when it's already allowed; otherwise Android asks the user.
#[tauri::command]
pub fn phone_allow_notifications<R: Runtime>(app: AppHandle<R>) -> bool {
    #[cfg(target_os = "android")]
    {
        call(&app, "allowNotifications", serde_json::json!({}))
            .ok()
            .and_then(|v| v.get("allowed").and_then(|a| a.as_bool()))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        false
    }
}

/// Maps to `DataApi.planNudges()`: work out the next due reminder and wrap-up nudge
/// (`shiftbook_core::nudge`) and hand them to Android, replacing any planned before. The
/// due count is the bell's, for that day at the reminder time.
#[tauri::command]
pub async fn plan_nudges<R: Runtime>(
    app: AppHandle<R>,
    s: tauri::State<'_, crate::session::VaultSession>,
    today: String,
    now: u32,
) -> Result<(), crate::session::CommandError> {
    use shiftbook_core::nudge;
    let (store, owner) = s.data().await?;
    let today = shiftbook_core::dates::parse_date(&today)
        .map_err(|_| crate::session::CommandError::InvalidInput)?;
    let prefs = store.prefs(owner).await?;
    let mut nudges = Vec::new();
    if let Some((date, at)) = nudge::due_reminder_at(&prefs, today, now) {
        let count = crate::meetings::bell_for(&store, owner, date, at)
            .await?
            .len();
        nudges.extend(nudge::due_nudge(date, at, count));
    }
    nudges.extend(nudge::wrapup_nudge(&prefs, today, now));
    #[cfg(target_os = "android")]
    {
        let list: Vec<serde_json::Value> = nudges
            .iter()
            .map(|n| {
                serde_json::json!({
                    "kind": n.kind,
                    "date": shiftbook_core::dates::format_date(n.date),
                    "at": n.at,
                    "title": n.title,
                    "body": n.body,
                })
            })
            .collect();
        let _ = call(&app, "planNudges", serde_json::json!({ "nudges": list }));
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, nudges);
    }
    Ok(())
}

// ---------- fingerprint unlock (round 15), off until turned on ----------

/// What the unlock screen and Security need to know.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintStatus {
    /// This phone can do it (Android 11 or newer with a strong fingerprint sensor set up).
    pub available: bool,
    /// Turned on in Security.
    pub on: bool,
    /// On, and the password was typed since the phone last started.
    pub ready: bool,
}

pub fn fingerprint_status<R: Runtime>(app: &AppHandle<R>) -> FingerprintStatus {
    #[cfg(target_os = "android")]
    {
        let Ok(v) = call(app, "fingerprintStatus", serde_json::json!({})) else {
            return FingerprintStatus::default();
        };
        let flag = |k: &str| v.get(k).and_then(|b| b.as_bool()).unwrap_or(false);
        FingerprintStatus {
            available: flag("available"),
            on: flag("on"),
            ready: flag("ready"),
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        FingerprintStatus::default()
    }
}

/// Ask for a fingerprint and store the password, encrypted by the phone's secure hardware.
/// Blocks until the user answers, so call it off the async runtime.
pub fn fingerprint_on<R: Runtime>(app: &AppHandle<R>, password: &SecretString) -> Result<(), ()> {
    #[cfg(target_os = "android")]
    {
        call(
            app,
            "fingerprintOn",
            serde_json::json!({ "password": password.expose_secret() }),
        )
        .map(|_| ())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, password);
        Err(())
    }
}

/// Forget the stored password and its key. Quiet when it was off.
pub fn fingerprint_off<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "android")]
    {
        let _ = call(app, "fingerprintOff", serde_json::json!({}));
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
    }
}

/// Ask for a fingerprint; the stored password, which never goes to the page. Blocks until
/// the user answers.
pub fn fingerprint_password<R: Runtime>(app: &AppHandle<R>) -> Option<SecretString> {
    #[cfg(target_os = "android")]
    {
        let v = call(app, "fingerprintUnlock", serde_json::json!({})).ok()?;
        let text = v.get("password")?.as_str()?;
        Some(SecretString::from(text))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        None
    }
}

/// The password was typed: fingerprint unlock works again until the phone restarts.
pub fn password_typed<R: Runtime>(app: &AppHandle<R>) {
    #[cfg(target_os = "android")]
    {
        let _ = call(app, "noteBoot", serde_json::json!({}));
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launches_are_checked() {
        assert_eq!(
            launch_from("log", Some("ignored")),
            Some(Launch {
                kind: "log".into(),
                text: None
            })
        );
        assert_eq!(launch_from("other", None), None);
        assert_eq!(launch_from("share", None), None);
        assert_eq!(launch_from("share", Some("  ")), None);
        let long = "x".repeat(MAX_SHARED_CHARS + 5);
        let got = launch_from("share", Some(&long)).unwrap();
        assert_eq!(got.text.unwrap().chars().count(), MAX_SHARED_CHARS);
    }
}
