//! Task rules (SPEC §4).

use time::Date;

use crate::model::{is_done, Task};

/// Overdue = due date before today and status is not Done.
pub fn is_overdue(due_date: Option<Date>, status: Option<&str>, today: Date) -> bool {
    due_date.is_some_and(|due| due < today) && !is_done(status)
}

/// [`is_overdue`] for a task record.
pub fn task_is_overdue(task: &Task, today: Date) -> bool {
    is_overdue(task.due_date, task.status.as_deref(), today)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

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
