//! Shiftbook core: domain models, validation and business rules.
//!
//! This crate is pure: no I/O, no storage, no Tauri imports. Every rule from
//! SPEC §4 lives here with its unit tests; the UI only displays the results.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod dates;
pub mod error;
pub mod hours;
pub mod ids;
pub mod minutes;

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
