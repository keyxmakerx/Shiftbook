//! Shiftbook core: domain models, validation and business rules.
//!
//! This crate is pure: no I/O, no storage, no Tauri imports. Rules land here from M1.

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
