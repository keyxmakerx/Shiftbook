//! Screenshot and screen-share protection (content protection).
//!
//! The window starts protected (`contentProtected` in tauri.conf.json). The user
//! can allow captures for ten minutes; a timer in the app (not the page) turns
//! protection back on, so it comes back even if the page is reloaded or locked.
//! Each allow gets a number; only the timer of the latest allow re-protects.
//! Works on Windows and Android, partly on macOS, and does nothing on Linux.
//!
//! Hiding from screenshots can be turned off in Settings → Security (round 15). The
//! window stays protected while locked, and the setting applies once prefs are loaded.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{State, WebviewWindow};

use crate::session::CommandError;

/// How long one "allow screenshots" lasts.
pub const ALLOW_SECONDS: u32 = 10 * 60;

/// Counts allows so an older timer never re-protects over a newer allow, and
/// remembers whether the user turned screenshot hiding off.
#[derive(Debug, Default, Clone)]
pub struct CaptureGuard {
    generation: Arc<AtomicU64>,
    /// The latest allow is still running.
    allowing: Arc<AtomicBool>,
    /// Hiding from screenshots is turned off in Settings.
    off: Arc<AtomicBool>,
}

impl CaptureGuard {
    /// Start a new allow; returns its number.
    pub fn begin_allow(&self) -> u64 {
        self.allowing.store(true, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Protect now: any running allow timer becomes stale.
    pub fn end_allow(&self) {
        self.allowing.store(false, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Allow number `n` ran out. True when it was still the latest, so protection returns.
    pub fn finish_allow(&self, n: u64) -> bool {
        let current = self.is_current(n);
        if current {
            self.allowing.store(false, Ordering::SeqCst);
        }
        current
    }

    /// Follow the Settings switch; returns whether the window should be hidden now.
    pub fn set_hiding(&self, on: bool) -> bool {
        self.off.store(!on, Ordering::SeqCst);
        self.should_hide()
    }

    /// Hidden unless the user turned hiding off or an allow is running.
    pub fn should_hide(&self) -> bool {
        !self.off.load(Ordering::SeqCst) && !self.allowing.load(Ordering::SeqCst)
    }

    /// True when allow number `n` is still the latest (nothing changed since).
    pub fn is_current(&self, n: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == n
    }
}

/// Allow screenshots and screen sharing for ten minutes. Returns the seconds.
/// Maps to `DataApi.allowCapture()`.
#[tauri::command]
pub fn allow_capture(
    window: WebviewWindow,
    guard: State<'_, CaptureGuard>,
) -> Result<u32, CommandError> {
    let n = guard.begin_allow();
    crate::phone::set_protected(&window, false).map_err(|_| CommandError::StorageError)?;
    let guard = guard.inner().clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(u64::from(ALLOW_SECONDS)));
        if guard.finish_allow(n) && guard.should_hide() {
            let _ = crate::phone::set_protected(&window, true);
        }
    });
    Ok(ALLOW_SECONDS)
}

/// Protect again now. Maps to `DataApi.protectCapture()`.
#[tauri::command]
pub fn protect_capture(
    window: WebviewWindow,
    guard: State<'_, CaptureGuard>,
) -> Result<(), CommandError> {
    guard.end_allow();
    crate::phone::set_protected(&window, guard.should_hide())
        .map_err(|_| CommandError::StorageError)
}

/// Apply the "Hide from screenshots" setting after prefs are loaded or saved.
pub fn apply_setting(window: &WebviewWindow, guard: &CaptureGuard, hide: bool) {
    let _ = crate::phone::set_protected(window, guard.set_hiding(hide));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_latest_allow_reprotects() {
        let g = CaptureGuard::default();
        let first = g.begin_allow();
        assert!(g.is_current(first));
        let second = g.begin_allow();
        assert!(!g.is_current(first));
        assert!(g.is_current(second));
        g.end_allow();
        assert!(!g.is_current(second));
        assert_eq!(ALLOW_SECONDS, 600);
    }

    #[test]
    fn the_setting_and_allows_decide_hiding() {
        let g = CaptureGuard::default();
        assert!(g.should_hide());
        let n = g.begin_allow();
        assert!(!g.should_hide());
        assert!(
            !g.set_hiding(true),
            "loading prefs must not end a running allow"
        );
        assert!(g.finish_allow(n));
        assert!(g.should_hide());
        assert!(!g.set_hiding(false));
        g.end_allow();
        assert!(
            !g.should_hide(),
            "Protect now keeps hiding off when it is turned off"
        );
        let old = g.begin_allow();
        g.begin_allow();
        assert!(!g.finish_allow(old), "an older timer changes nothing");
        assert!(!g.should_hide());
    }
}
