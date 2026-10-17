//! Shiftbook core: domain models, validation and business rules.
//!
//! This crate is pure: no I/O, no storage, no Tauri imports. Every rule from
//! SPEC §4 lives here with its unit tests; the UI only displays the results.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod capture;
pub mod dates;
pub mod error;
pub mod fields;
pub mod group;
pub mod hours;
pub mod ids;
pub mod minutes;
pub mod model;
pub mod nextup;
pub mod prefs;
pub mod project;
pub mod quicklog;
pub mod recurring;
pub mod resume;
pub mod ribbon;
pub mod search;
pub mod seed;
pub mod sheet;
pub mod task;
pub mod tickets;
pub mod timer;
pub mod timesheet;
pub mod tray;
pub mod validate;
pub mod week;
pub mod wipe;

pub use error::CoreError;
pub use minutes::Minutes;

/// The product name shown to the user.
pub fn app_name() -> &'static str {
    "Shiftbook"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_is_shiftbook() {
        assert_eq!(app_name(), "Shiftbook");
    }
}
