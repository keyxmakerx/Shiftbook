//! Typed record ids.
//!
//! Every record id is a UUID v7 (time-ordered) wrapped in its own type, so a
//! project id can never be passed where a task id is expected. They serialize
//! as the plain hyphenated UUID string.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{preview, CoreError};

macro_rules! typed_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a new, time-ordered (v7) id.
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wrap an existing UUID, e.g. one read from storage.
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// The underlying UUID.
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl FromStr for $name {
            type Err = CoreError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s.trim()).map(Self).map_err(|_| CoreError::IdInvalid {
                    input: preview(s, 40),
                })
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

typed_id!(
    /// The person who owns a record. Single local user in Phase 1.
    OwnerId
);
typed_id!(
    /// A project.
    ProjectId
);
typed_id!(
    /// A recurring task (certificate renewal, patching, a routine bucket...).
    RecurringTaskId
);
typed_id!(
    /// A time log entry.
    TimeEntryId
);
typed_id!(
    /// A ticket attached to a time entry.
    TicketId
);
typed_id!(
    /// A task in the task manager.
    TaskId
);
typed_id!(
    /// A checklist item inside a task.
    ChecklistItemId
);
typed_id!(
    /// A note.
    NoteId
);
typed_id!(
    /// An option in a user-editable list (category, status, priority, recurring type).
    ListOptionId
);
typed_id!(
    /// A repeat frequency (label + months + days).
    FrequencyId
);
typed_id!(
    /// A user-defined timesheet code.
    TimesheetCodeId
);
typed_id!(
    /// A category to timesheet code mapping.
    CategoryCodeDefaultId
);
typed_id!(
    /// A timesheet week record (submitted or not).
    TimesheetWeekId
);
typed_id!(
    /// A ticket in the "Done tickets" tray.
    TrayItemId
);
typed_id!(
    /// An item (round 15): one ticket, task or chore, however many days and entries it's on.
    ItemId
);
typed_id!(
    /// A user-defined custom field.
    CustomFieldId
);
typed_id!(
    /// A custom field's value on one record.
    CustomValueId
);
typed_id!(
    /// A crib sheet tick-off.
    TimesheetTickId
);
typed_id!(
    /// A scrap in the Inbox.
    InboxItemId
);
typed_id!(
    /// One thing on the "next up" list.
    NextUpItemId
);
typed_id!(
    /// A saved response (a message kept for pasting into tickets).
    SavedResponseId
);
typed_id!(
    /// A meeting series (a repeating meeting the bell reminds about).
    MeetingSeriesId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_are_v7_and_unique() {
        let a = ProjectId::new();
        let b = ProjectId::new();
        assert_ne!(a, b);
        assert_eq!(a.as_uuid().get_version_num(), 7);
    }

    #[test]
    fn ids_are_time_ordered() {
        let ids: Vec<TaskId> = (0..50).map(|_| TaskId::new()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn display_and_parse_round_trip() {
        let id = TimeEntryId::new();
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text.parse::<TimeEntryId>(), Ok(id));
        assert_eq!(format!(" {text} ").parse::<TimeEntryId>(), Ok(id));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(matches!(
            "not-an-id".parse::<OwnerId>(),
            Err(CoreError::IdInvalid { .. })
        ));
        assert!("".parse::<OwnerId>().is_err());
    }
}
