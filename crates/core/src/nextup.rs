//! The "next up" list: a few things to start with, written at the end of a day and shown
//! by "Where was I?" (first item, the rest as a checklist).

use crate::error::CoreError;
use crate::validate::validate_required;

/// Most things on the list.
pub const MAX_NEXT_UP: usize = 7;
/// Longest line on the list.
pub const MAX_NEXT_UP_CHARS: usize = 200;

/// Clean a typed list: trim each line, drop blank ones, check the length and count.
pub fn clean_next_up(lines: &[String]) -> Result<Vec<String>, CoreError> {
    let items = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| validate_required("A line", l, MAX_NEXT_UP_CHARS))
        .collect::<Result<Vec<_>, _>>()?;
    if items.len() > MAX_NEXT_UP {
        return Err(CoreError::TooManyItems {
            field: "the list",
            max: MAX_NEXT_UP,
        });
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn trims_and_drops_blanks() {
        assert_eq!(
            clean_next_up(&lines(&["  Call the vendor ", "", "   ", "Renew certs"])),
            Ok(lines(&["Call the vendor", "Renew certs"]))
        );
        assert_eq!(clean_next_up(&[]), Ok(vec![]));
    }

    #[test]
    fn limits_count_and_length() {
        let eight = lines(&["a", "b", "c", "d", "e", "f", "g", "h"]);
        assert!(matches!(
            clean_next_up(&eight),
            Err(CoreError::TooManyItems { max: 7, .. })
        ));
        assert!(clean_next_up(&eight[..7]).is_ok());
        let long = "x".repeat(MAX_NEXT_UP_CHARS + 1);
        assert!(matches!(
            clean_next_up(&[long]),
            Err(CoreError::TooLong { .. })
        ));
    }
}
