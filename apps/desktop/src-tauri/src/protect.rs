//! Screenshot and screen-share protection (content protection).
//!
//! The window starts protected (`contentProtected` in tauri.conf.json). The user
//! can allow captures for ten minutes; a timer in the app (not the page) turns
//! protection back on, so it comes back even if the page is reloaded or locked.
//! Each allow gets a number; only the timer of the latest allow re-protects.
//! Works on Windows, partly on macOS, and does nothing on Linux.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{State, WebviewWindow};

use crate::session::CommandError;

/// How long one "allow screenshots" lasts.
pub const ALLOW_SECONDS: u32 = 10 * 60;

/// Counts allows so an older timer never re-protects over a newer allow.
#[derive(Debug, Default, Clone)]
pub struct CaptureGuard {
    generation: Arc<AtomicU64>,
}

impl CaptureGuard {
    /// Start a new allow; returns its number.
    pub fn begin_allow(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Protect now: any running allow timer becomes stale.
    pub fn end_allow(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
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
    window
        .set_content_protected(false)
        .map_err(|_| CommandError::StorageError)?;
    let guard = guard.inner().clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(u64::from(ALLOW_SECONDS)));
        if guard.is_current(n) {
            let _ = window.set_content_protected(true);
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
    window
        .set_content_protected(true)
        .map_err(|_| CommandError::StorageError)
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
}
