//! Find: what capture shows when you type something that isn't a log line.
//!
//! Each word of the query must match the text, ignoring case:
//!
//! 1. a **whole ticket** or whole word scores highest,
//! 2. the start of a word next,
//! 3. anywhere inside the text after that,
//! 4. and, last, its **letters in order inside one word** that starts with the same letter
//!    (`prntr` finds "printer"), so a half-remembered name still turns something up.
//!
//! A text matches only if every word does; its score is the sum. Results are best
//! first, and ties keep the order they were given in (callers pass newest first).
//!
//! Round 21: the last tier used to look for the letters anywhere across the whole text, so
//! "psu" matched any long note, and those loose hits sat above the real ones. It now stays
//! inside one word, needs three letters or more, and loose hits are dropped from a list
//! that has a real match in it.

/// What the loose tier (letters in order) scores.
const LOOSE: u32 = 10;
/// The fewest letters a query word needs before the loose tier is tried.
const LOOSE_MIN_LETTERS: usize = 3;

/// Score for one query word against a text, or `None` when it doesn't match.
fn word_score(word: &str, text: &str, words: &[&str]) -> Option<u32> {
    if words.contains(&word) {
        Some(100)
    } else if words.iter().any(|w| w.starts_with(word)) {
        Some(60)
    } else if text.contains(word) {
        Some(30)
    } else if word.chars().count() >= LOOSE_MIN_LETTERS && words.iter().any(|w| in_order(word, w)) {
        Some(LOOSE)
    } else {
        None
    }
}

/// True when `hay` starts with `needle`'s first letter and holds the rest of its letters in
/// the same order.
fn in_order(needle: &str, hay: &str) -> bool {
    if needle.chars().next() != hay.chars().next() {
        return false;
    }
    let mut rest = hay.chars();
    needle.chars().all(|c| rest.any(|h| h == c))
}

/// How well `text` matches `query` and whether any word only matched loosely, or `None`
/// when it doesn't match. A blank query matches nothing.
fn score_parts(query: &str, text: &str) -> Option<(u32, bool)> {
    let text = text.to_lowercase();
    let words: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let query = query.to_lowercase();
    let mut parts = query.split_whitespace().peekable();
    parts.peek()?;
    parts.try_fold((0, false), |(sum, loose), q| {
        word_score(q, &text, &words).map(|s| (sum + s, loose || s == LOOSE))
    })
}

/// How well `text` matches `query`, or `None` when it doesn't. A blank query matches
/// nothing.
pub fn score(query: &str, text: &str) -> Option<u32> {
    score_parts(query, text).map(|(sum, _)| sum)
}

/// The items that match, best first, at most `limit`. Ties keep their given order. Loose
/// matches (letters in order) only show when nothing in the list matched properly.
pub fn rank<T>(query: &str, items: impl IntoIterator<Item = (T, String)>, limit: usize) -> Vec<T> {
    let mut hits: Vec<(u32, bool, usize, T)> = items
        .into_iter()
        .enumerate()
        .filter_map(|(i, (item, text))| {
            score_parts(query, &text).map(|(s, loose)| (s, loose, i, item))
        })
        .collect();
    if hits.iter().any(|h| !h.1) {
        hits.retain(|h| !h.1);
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)));
    hits.into_iter()
        .take(limit)
        .map(|(_, _, _, item)| item)
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

    #[test]
    fn loose_matches_stay_inside_one_word() {
        // The letters are all there in order, but spread over the sentence: no match.
        assert_eq!(
            score("psu", "Project board: Monitoring cleanup status update"),
            None
        );
        assert_eq!(score("bgp", "Project board: Monitoring cleanup"), None);
        // Inside one word that starts the same way: still found.
        assert_eq!(score("prntr", "The printer jammed"), Some(10));
        // Not when the word starts with another letter, and not for one or two letters.
        assert_eq!(score("rntr", "The printer jammed"), None);
        assert_eq!(score("pr", "The paper"), None);
    }

    #[test]
    fn loose_matches_give_way_to_real_ones() {
        let items = vec![
            ("loose", "Passthrough module notes".to_string()),
            ("real", "Swapped failed PSU on sw-acc-03".to_string()),
        ];
        assert_eq!(rank("psu", items, 10), vec!["real"]);
        // With nothing better, the loose one still turns up.
        let only = vec![("loose", "Passthrough module notes".to_string())];
        assert_eq!(rank("psu", only, 10), vec!["loose"]);
    }
}
