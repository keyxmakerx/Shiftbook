//! Deleting all local data: the word the user must type to confirm it.

/// The word typed to confirm deleting everything. Shown to the user in capitals.
pub const DELETE_CONFIRM_WORD: &str = "DELETE";

/// True when `typed` confirms deleting all data: the word, ignoring case and
/// spaces around it.
pub fn confirms_delete(typed: &str) -> bool {
    typed.trim().eq_ignore_ascii_case(DELETE_CONFIRM_WORD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_word_confirms() {
        assert!(confirms_delete("DELETE"));
        assert!(confirms_delete("delete"));
        assert!(confirms_delete("  Delete \n"));
        assert!(!confirms_delete(""));
        assert!(!confirms_delete("   "));
        assert!(!confirms_delete("DELET"));
        assert!(!confirms_delete("DELETE!"));
        assert!(!confirms_delete("delete everything"));
        assert!(!confirms_delete("yes"));
    }
}
