//! Task rules (SPEC §4), and reusable lists (round 15): a list is a task whose checklist
//! is the point (shopping, packing, "before we go away"). Ticking items off and **Reset for
//! next time** unticks them all and keeps the items.

use time::Date;

use crate::error::CoreError;
use crate::model::{is_done, ChecklistItem, Task};
use crate::validate::{validate_required, MAX_LINE_CHARS, MAX_NAME_CHARS};

/// Most items on one list.
pub const MAX_LIST_ITEMS: usize = 300;

/// Overdue = due date before today and status is not Done.
pub fn is_overdue(due_date: Option<Date>, status: Option<&str>, today: Date) -> bool {
    due_date.is_some_and(|due| due < today) && !is_done(status)
}

/// [`is_overdue`] for a task record.
pub fn task_is_overdue(task: &Task, today: Date) -> bool {
    is_overdue(task.due_date, task.status.as_deref(), today)
}

/// A list's name, trimmed and required.
pub fn validate_list_title(title: &str) -> Result<String, CoreError> {
    validate_required("List name", title, MAX_NAME_CHARS)
}

/// One list item, trimmed and required.
pub fn validate_list_item(text: &str) -> Result<String, CoreError> {
    validate_required("Item", text, MAX_LINE_CHARS)
}

/// Room for `adding` more items on a list that has `count` live ones.
pub fn check_list_room(count: usize, adding: usize) -> Result<(), CoreError> {
    if count + adding > MAX_LIST_ITEMS {
        Err(CoreError::TooManyItems {
            field: "a list",
            max: MAX_LIST_ITEMS,
        })
    } else {
        Ok(())
    }
}

/// Items typed or pasted at once: one per line, blank lines and list marks (`-`, `*`,
/// `[ ]`, `[x]`) dropped, so a shopping list pasted from a note comes in item by item.
pub fn split_list_items(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| {
            let l = l.trim();
            let l = l
                .strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| l.strip_prefix("• "))
                .unwrap_or(l)
                .trim_start();
            let l = ["[ ]", "[x]", "[X]"]
                .iter()
                .find_map(|m| l.strip_prefix(m))
                .unwrap_or(l);
            l.trim().to_string()
        })
        .filter(|l| !l.is_empty() && !matches!(l.as_str(), "-" | "*" | "•"))
        .collect()
}

/// Ticked and total live items.
pub fn list_progress(items: &[ChecklistItem]) -> (usize, usize) {
    let live = items.iter().filter(|i| i.deleted_at.is_none());
    let (mut done, mut total) = (0, 0);
    for i in live {
        total += 1;
        if i.done {
            done += 1;
        }
    }
    (done, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn pasted_lists_split_into_items() {
        let text = "- milk\n\n* bread\n[x] eggs\n  [ ] butter \n• tea\nplain";
        assert_eq!(
            split_list_items(text),
            ["milk", "bread", "eggs", "butter", "tea", "plain"]
        );
        assert!(split_list_items(" \n - \n").is_empty());
    }

    #[test]
    fn list_names_and_room() {
        assert_eq!(validate_list_title("  Shopping "), Ok("Shopping".into()));
        assert!(validate_list_title("  ").is_err());
        assert!(validate_list_item("").is_err());
        assert!(check_list_room(299, 1).is_ok());
        assert!(check_list_room(299, 2).is_err());
    }

    #[test]
    fn overdue_rule() {
        let today = date!(2026 - 10 - 15);
        let yesterday = Some(date!(2026 - 10 - 14));
        let cases = [
            ("past due, open", yesterday, Some("In progress"), true),
            ("past due, no status", yesterday, None, true),
            ("past due, done", yesterday, Some("Done"), false),
            ("past due, done lower case", yesterday, Some("done"), false),
            ("due today", Some(today), Some("In progress"), false),
            ("due tomorrow", Some(date!(2026 - 10 - 16)), None, false),
            ("no due date", None, Some("In progress"), false),
            (
                "long ago",
                Some(date!(2020 - 01 - 01)),
                Some("Blocked"),
                true,
            ),
        ];
        for (name, due, status, expected) in cases {
            assert_eq!(is_overdue(due, status, today), expected, "{name}");
        }
    }
}
