//! Quick unlock with a PIN (round 15, SECURITY.md "Quick unlock with a PIN"): the rules for
//! the PIN and for the window in which it works. The key handling itself is in the storage
//! crate's vault; the PIN is never stored.

use time::Date;

/// A PIN is exactly this many digits.
pub const PIN_DIGITS: usize = 6;
/// Wrong PINs allowed in a row; the next one ends the window.
pub const PIN_TRIES: u8 = 3;
/// The window ends this long after the full unlock that started it.
pub const WINDOW_SECS: u64 = 10 * 60 * 60;
/// When the wall clock moved this much further than the app's own clock, the computer
/// slept (the app's clock stops while it sleeps), which ends the window.
pub const SLEEP_GAP_SECS: u64 = 60;

/// True for exactly six digits 0–9.
pub fn valid_pin(pin: &str) -> bool {
    pin.len() == PIN_DIGITS && pin.bytes().all(|b| b.is_ascii_digit())
}

/// True while a PIN may unlock: still the day the window started (it ends at midnight),
/// less than [`WINDOW_SECS`] since the full unlock by either clock, and no sleep in between.
/// `app_secs` is the app's own (monotonic) clock, `wall_secs` the computer's clock.
pub fn window_open(started_on: Date, today: Date, app_secs: u64, wall_secs: u64) -> bool {
    started_on == today
        && app_secs.max(wall_secs) < WINDOW_SECS
        && wall_secs.saturating_sub(app_secs) < SLEEP_GAP_SECS
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    const MON: Date = date!(2026 - 10 - 19);

    #[test]
    fn a_pin_is_six_digits() {
        assert!(valid_pin("042917"));
        for bad in ["", "12345", "1234567", "12 456", "12345a", "١٢٣٤٥٦"] {
            assert!(!valid_pin(bad), "{bad}");
        }
    }

    #[test]
    fn the_window_ends_at_midnight_after_ten_hours_and_at_sleep() {
        assert!(window_open(MON, MON, 0, 0));
        assert!(window_open(MON, MON, 4 * 3600, 4 * 3600 + 30));
        assert!(!window_open(MON, date!(2026 - 10 - 20), 60, 60));
        assert!(!window_open(MON, MON, WINDOW_SECS, WINDOW_SECS));
        assert!(!window_open(MON, MON, 3600, WINDOW_SECS + 1));
        // Twenty minutes of wall time the app didn't see: the computer slept.
        assert!(!window_open(MON, MON, 600, 600 + 20 * 60));
        assert!(!window_open(MON, MON, 600, 600 + SLEEP_GAP_SECS));
    }
}
