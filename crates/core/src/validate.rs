//! Validation of user input before it is saved. Returns [`CoreError`], never panics.
//!
//! Field helpers return the cleaned (trimmed) value; record helpers check a
//! whole record and return the first problem found.

use crate::error::CoreError;
use crate::minutes::{Minutes, MAX_ENTRY_MINUTES};
use crate::model::{Project, RecurringTask, Task, TimeEntry, TimesheetCode};

/// Longest name/title (projects, recurring tasks, tasks).
pub const MAX_NAME_CHARS: usize = 200;
/// Longest dropdown value or frequency label.
pub const MAX_LIST_VALUE_CHARS: usize = 60;
/// Longest one-line text (description, next step, requested by).
pub const MAX_LINE_CHARS: usize = 2_000;
/// Longest notes field.
pub const MAX_NOTES_CHARS: usize = 20_000;
/// Largest warning window, in days (10 years).
pub const MAX_WARN_DAYS: u32 = 3_650;
/// Largest frequency months part (100 years).
pub const MAX_FREQUENCY_MONTHS: u32 = 1_200;
/// Largest frequency days part (about 10 years).
pub const MAX_FREQUENCY_DAYS: u32 = 3_660;
/// Largest project estimate: 100,000 hours.
pub const MAX_ESTIMATE_MINUTES: u32 = 100_000 * 60;
/// Shortest master password, in characters (SPEC §2.9).
pub const MIN_PASSWORD_CHARS: usize = 12;
/// Longest master password, in characters (keeps key derivation input bounded).
pub const MAX_PASSWORD_CHARS: usize = 1_024;

fn check_len(field: &'static str, value: &str, max: usize) -> Result<(), CoreError> {
    if value.chars().count() > max {
        Err(CoreError::TooLong { field, max })
    } else {
        Ok(())
    }
}

/// A required single-line value: trimmed, not empty, at most `max` characters.
pub fn validate_required(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<String, CoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CoreError::Required { field });
    }
    check_len(field, trimmed, max)?;
    Ok(trimmed.to_string())
}

/// An optional value: at most `max` characters (blank is fine).
pub fn validate_optional(field: &'static str, value: &str, max: usize) -> Result<(), CoreError> {
    check_len(field, value.trim(), max)
}

/// A new master password: 12 to 1,024 characters. Spaces count and nothing is
/// trimmed, so the password is used exactly as typed. The error never contains it.
pub fn validate_new_password(password: &str) -> Result<(), CoreError> {
    let count = password.chars().count();
    if count < MIN_PASSWORD_CHARS {
        return Err(CoreError::PasswordTooShort {
            min: MIN_PASSWORD_CHARS,
        });
    }
    if count > MAX_PASSWORD_CHARS {
        return Err(CoreError::PasswordTooLong {
            max: MAX_PASSWORD_CHARS,
        });
    }
    Ok(())
}

/// A dropdown value (category, status, priority, recurring type).
pub fn validate_list_value(value: &str) -> Result<String, CoreError> {
    validate_required("Value", value, MAX_LIST_VALUE_CHARS)
}

/// A `#rrggbb` colour, returned in lower case.
pub fn validate_color(value: &str) -> Result<String, CoreError> {
    let trimmed = value.trim();
    let ok = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|c| c.is_ascii_hexdigit());
    if ok {
        Ok(trimmed.to_ascii_lowercase())
    } else {
        Err(CoreError::ColorInvalid {
            input: crate::error::preview(trimmed, 20),
        })
    }
}

/// A frequency: label required, months and days in range, not both zero.
pub fn validate_frequency(label: &str, months: u32, days: u32) -> Result<(), CoreError> {
    validate_required("Label", label, MAX_LIST_VALUE_CHARS)?;
    if months > MAX_FREQUENCY_MONTHS {
        return Err(out_of_range("Months", 0, MAX_FREQUENCY_MONTHS));
    }
    if days > MAX_FREQUENCY_DAYS {
        return Err(out_of_range("Days", 0, MAX_FREQUENCY_DAYS));
    }
    if months == 0 && days == 0 {
        return Err(CoreError::FrequencyEmpty);
    }
    Ok(())
}

fn out_of_range(field: &'static str, min: u32, max: u32) -> CoreError {
    CoreError::OutOfRange {
        field,
        min: i64::from(min),
        max: i64::from(max),
    }
}

/// Warn days: blank (means 30) or 0 to 3650.
pub fn validate_warn_days(warn: Option<u32>) -> Result<(), CoreError> {
    match warn {
        Some(w) if w > MAX_WARN_DAYS => Err(out_of_range("Warn days before", 0, MAX_WARN_DAYS)),
        _ => Ok(()),
    }
}

/// Minutes on one entry: 0 to 24 hours.
pub fn validate_entry_minutes(minutes: Minutes) -> Result<(), CoreError> {
    if minutes.get() > MAX_ENTRY_MINUTES {
        Err(CoreError::HoursTooLarge)
    } else {
        Ok(())
    }
}

/// A project estimate: blank or up to 100,000 hours.
pub fn validate_estimate(estimate: Option<Minutes>) -> Result<(), CoreError> {
    match estimate {
        Some(m) if m.get() > MAX_ESTIMATE_MINUTES => Err(out_of_range(
            "Estimated hours",
            0,
            MAX_ESTIMATE_MINUTES / 60,
        )),
        _ => Ok(()),
    }
}

fn validate_opt_list(field: &'static str, value: Option<&str>) -> Result<(), CoreError> {
    value.map_or(Ok(()), |v| {
        validate_optional(field, v, MAX_LIST_VALUE_CHARS)
    })
}

/// Check a project before saving.
pub fn validate_project(p: &Project) -> Result<(), CoreError> {
    validate_required("Name", &p.name, MAX_NAME_CHARS)?;
    validate_optional("Description", &p.description, MAX_NOTES_CHARS)?;
    if let Some(r) = &p.requested_by {
        validate_optional("Requested by", r, MAX_LINE_CHARS)?;
    }
    validate_opt_list("Status", p.status.as_deref())?;
    validate_opt_list("Priority", p.priority.as_deref())?;
    validate_estimate(p.estimated_minutes)
}

/// Check a recurring task before saving.
pub fn validate_recurring_task(r: &RecurringTask) -> Result<(), CoreError> {
    validate_required("Name", &r.name, MAX_NAME_CHARS)?;
    validate_opt_list("Type", r.recurring_type.as_deref())?;
    validate_warn_days(r.warn_days_before)?;
    validate_optional("Notes", &r.notes, MAX_NOTES_CHARS)
}

/// Clean an entry's notepad: trimmed at the end, blank means none, at most
/// [`MAX_NOTES_CHARS`] characters.
pub fn clean_notes(notes: &str) -> Result<Option<String>, CoreError> {
    let kept = notes.trim_end();
    if kept.trim().is_empty() {
        return Ok(None);
    }
    validate_optional("Notes", kept, MAX_NOTES_CHARS)?;
    Ok(Some(kept.to_string()))
}

/// Check a time entry before saving.
pub fn validate_time_entry(e: &TimeEntry) -> Result<(), CoreError> {
    validate_entry_minutes(e.minutes)?;
    validate_optional("What I worked on", &e.description, MAX_LINE_CHARS)?;
    validate_opt_list("Category", e.category.as_deref())?;
    validate_opt_list("Status", e.status.as_deref())?;
    validate_opt_list("Priority", e.priority.as_deref())?;
    if let Some(n) = &e.next_step {
        // The entry's notepad (round 11), so as long as any note.
        validate_optional("Notes", n, MAX_NOTES_CHARS)?;
    }
    Ok(())
}

/// Check a task before saving.
pub fn validate_task(t: &Task) -> Result<(), CoreError> {
    validate_required("Title", &t.title, MAX_NAME_CHARS)?;
    validate_optional("Notes", &t.notes, MAX_NOTES_CHARS)?;
    validate_opt_list("Status", t.status.as_deref())?;
    validate_opt_list("Priority", t.priority.as_deref())
}

/// Check a timesheet code before saving: the code is required.
pub fn validate_timesheet_code(c: &TimesheetCode) -> Result<(), CoreError> {
    validate_optional("Area", &c.area, MAX_NAME_CHARS)?;
    validate_optional("Phase", &c.phase_label, MAX_NAME_CHARS)?;
    validate_required("Code", &c.code, MAX_NAME_CHARS).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{OwnerId, ProjectId, TaskId, TimeEntryId, TimesheetCodeId};
    use crate::model::EntryTarget;
    use time::macros::{date, datetime};

    #[test]
    fn new_password_length() {
        assert_eq!(
            validate_new_password("short"),
            Err(CoreError::PasswordTooShort { min: 12 })
        );
        assert_eq!(
            validate_new_password("elevenchars"),
            Err(CoreError::PasswordTooShort { min: 12 })
        );
        assert_eq!(validate_new_password("twelve chars"), Ok(()));
        // Characters, not bytes: 12 accented letters are enough, 11 are not.
        assert_eq!(validate_new_password(&"é".repeat(12)), Ok(()));
        assert!(validate_new_password(&"é".repeat(11)).is_err());
        // Nothing is trimmed: spaces count.
        assert_eq!(validate_new_password("            "), Ok(()));
        assert_eq!(validate_new_password(&"x".repeat(1_024)), Ok(()));
        assert_eq!(
            validate_new_password(&"x".repeat(1_025)),
            Err(CoreError::PasswordTooLong { max: 1_024 })
        );
        let message = validate_new_password("secret-pw").unwrap_err().to_string();
        assert!(!message.contains("secret-pw"));
    }

    #[test]
    fn required_fields() {
        assert_eq!(validate_required("Name", "  Web  ", 10), Ok("Web".into()));
        assert_eq!(
            validate_required("Name", "   ", 10),
            Err(CoreError::Required { field: "Name" })
        );
        assert_eq!(validate_required("Name", "éééé", 4), Ok("éééé".into()));
        assert_eq!(
            validate_required("Name", "ééééé", 4),
            Err(CoreError::TooLong {
                field: "Name",
                max: 4
            })
        );
        assert_eq!(validate_optional("Notes", "", 4), Ok(()));
        assert!(validate_optional("Notes", "12345", 4).is_err());
    }

    #[test]
    fn colors() {
        assert_eq!(validate_color("#2563EB"), Ok("#2563eb".into()));
        assert_eq!(validate_color(" #abcdef "), Ok("#abcdef".into()));
        for bad in ["", "2563eb", "#25", "#2563eg", "#2563ebff", "red", "#ééé"] {
            assert!(validate_color(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn frequencies() {
        assert_eq!(validate_frequency("Monthly", 1, 0), Ok(()));
        assert_eq!(validate_frequency("Daily", 0, 1), Ok(()));
        assert_eq!(
            validate_frequency("Never", 0, 0),
            Err(CoreError::FrequencyEmpty)
        );
        assert!(validate_frequency("", 1, 0).is_err());
        assert!(validate_frequency("Too long", 1201, 0).is_err());
        assert!(validate_frequency("Too long", 0, 3661).is_err());
        assert_eq!(validate_frequency("Max", 1200, 3660), Ok(()));
    }

    #[test]
    fn numbers() {
        assert_eq!(validate_warn_days(None), Ok(()));
        assert_eq!(validate_warn_days(Some(0)), Ok(()));
        assert_eq!(validate_warn_days(Some(3650)), Ok(()));
        assert!(validate_warn_days(Some(3651)).is_err());
        assert_eq!(validate_entry_minutes(Minutes(0)), Ok(()));
        assert_eq!(validate_entry_minutes(Minutes(1440)), Ok(()));
        assert_eq!(
            validate_entry_minutes(Minutes(1441)),
            Err(CoreError::HoursTooLarge)
        );
        assert_eq!(validate_estimate(None), Ok(()));
        assert_eq!(
            validate_estimate(Some(Minutes(MAX_ESTIMATE_MINUTES))),
            Ok(())
        );
        assert!(validate_estimate(Some(Minutes(MAX_ESTIMATE_MINUTES + 1))).is_err());
    }

    #[test]
    fn records() {
        let now = datetime!(2026-10-15 09:00 UTC);
        let mut project = Project {
            id: ProjectId::new(),
            owner_id: OwnerId::new(),
            name: "Website".into(),
            description: String::new(),
            requested_by: Some("Sam".into()),
            due_date: Some(date!(2026 - 12 - 01)),
            status: Some("In progress".into()),
            priority: None,
            estimated_minutes: Some(Minutes(600)),
            default_timesheet_code_id: None,
            color: None,
            archived: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        assert_eq!(validate_project(&project), Ok(()));
        project.name = " ".into();
        assert_eq!(
            validate_project(&project),
            Err(CoreError::Required { field: "Name" })
        );

        let mut entry = TimeEntry {
            id: TimeEntryId::new(),
            owner_id: OwnerId::new(),
            date: date!(2026 - 10 - 15),
            minutes: Minutes(90),
            target: EntryTarget::None,
            category: Some("Meetings".into()),
            description: "Weekly sync".into(),
            status: None,
            priority: None,
            next_step: None,
            timesheet_code_id: None,
            legacy_ticket_text: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        assert_eq!(validate_time_entry(&entry), Ok(()));
        assert_eq!(clean_notes("  \n "), Ok(None));
        assert_eq!(
            clean_notes("  - ask the board\n"),
            Ok(Some("  - ask the board".into()))
        );
        // The notepad holds a full note, not one line.
        entry.next_step = Some("x".repeat(MAX_NOTES_CHARS));
        assert_eq!(validate_time_entry(&entry), Ok(()));
        entry.next_step = Some("x".repeat(MAX_NOTES_CHARS + 1));
        assert!(validate_time_entry(&entry).is_err());
        entry.next_step = None;
        entry.minutes = Minutes(2000);
        assert_eq!(validate_time_entry(&entry), Err(CoreError::HoursTooLarge));

        let mut task = Task {
            id: TaskId::new(),
            owner_id: OwnerId::new(),
            title: "Call vendor".into(),
            notes: String::new(),
            project_id: None,
            recurring_task_id: None,
            status: None,
            priority: Some("High".into()),
            due_date: None,
            completed_at: None,
            sort_order: 0,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        assert_eq!(validate_task(&task), Ok(()));
        task.title.clear();
        assert_eq!(
            validate_task(&task),
            Err(CoreError::Required { field: "Title" })
        );

        let code = TimesheetCode {
            id: TimesheetCodeId::new(),
            owner_id: OwnerId::new(),
            area: "Operations".into(),
            phase_label: "Phase".into(),
            code: String::new(),
            sort_order: 0,
            hidden: false,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        assert_eq!(
            validate_timesheet_code(&code),
            Err(CoreError::Required { field: "Code" })
        );
    }
}
