//! Find: what capture shows when you type something that isn't a log line.
//!
//! Each word of the query must match the text, ignoring case:
//!
//! 1. a **whole ticket** or whole word scores highest,
//! 2. the start of a word next,
//! 3. anywhere inside the text after that,
//! 4. and, last, its **letters in order** (`prntr` finds "printer"), so a half-remembered
//!    name still turns something up.
//!
//! A text matches only if every word does; its score is the sum. Results are best
//! first, and ties keep the order they were given in (callers pass newest first).

/// Score for one query word against a text, or `None` when it doesn't match.
fn word_score(word: &str, text: &str, words: &[&str]) -> Option<u32> {
    if words.contains(&word) {
        Some(100)
    } else if words.iter().any(|w| w.starts_with(word)) {
        Some(60)
    } else if text.contains(word) {
        Some(30)
    } else if in_order(word, text) {
        Some(10)
    } else {
        None
    }
}

/// True when every letter of `needle` appears in `hay` in the same order.
fn in_order(needle: &str, hay: &str) -> bool {
    let mut rest = hay.chars();
    needle.chars().all(|c| rest.any(|h| h == c))
}

/// How well `text` matches `query`, or `None` when it doesn't. A blank query matches
/// nothing.
pub fn score(query: &str, text: &str) -> Option<u32> {
    let text = text.to_lowercase();
    let words: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let query = query.to_lowercase();
    let mut parts = query.split_whitespace().peekable();
    parts.peek()?;
    parts.try_fold(0, |sum, q| word_score(q, &text, &words).map(|s| sum + s))
}

/// The items that match, best first, at most `limit`. Ties keep their given order.
pub fn rank<T>(query: &str, items: impl IntoIterator<Item = (T, String)>, limit: usize) -> Vec<T> {
    let mut hits: Vec<(u32, usize, T)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, (item, text))| score(query, &text).map(|s| (s, i, item)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter()
        .take(limit)
        .map(|(_, _, item)| item)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers() {
        assert_eq!(score("inc0045678", "Printer INC0045678"), Some(100));
        assert_eq!(score("print", "Printer on 2nd floor"), Some(60));
        assert_eq!(score("0045", "INC0045678"), Some(30));
        assert_eq!(score("prntr", "Printer on 2nd floor"), Some(10));
        assert_eq!(score("xyz", "Printer"), None);
    }

    #[test]
    fn every_word_must_match() {
        assert_eq!(score("printer floor", "Printer on 2nd floor"), Some(200));
        assert_eq!(score("printer vpn", "Printer on 2nd floor"), None);
        assert_eq!(score("   ", "anything"), None);
        assert_eq!(score("", "anything"), None);
    }

    #[test]
    fn ranks_best_first_and_keeps_order_on_ties() {
        let items = vec![
            ("a", "VPN for new starter".to_string()),
            ("b", "Printer on 2nd floor".to_string()),
            ("c", "Reprint badges".to_string()),
            ("d", "Printer toner".to_string()),
        ];
        assert_eq!(rank("printer", items.clone(), 10), vec!["b", "d"]);
        assert_eq!(rank("print", items.clone(), 10), vec!["b", "d", "c"]);
        assert_eq!(rank("print", items, 1), vec!["b"]);
    }
}
