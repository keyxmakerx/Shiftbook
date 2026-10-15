//! The "Group" label shown on each time entry (SPEC §4).

/// What a time entry's group is computed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSource<'a> {
    /// Logged against a project with this name.
    Project {
        /// The project's name.
        name: &'a str,
    },
    /// Logged against a recurring task with this type (may be blank).
    Recurring {
        /// The recurring task's type, if any.
        recurring_type: Option<&'a str>,
    },
    /// Not logged against anything.
    None,
}

/// Label used when there is no project or recurring task.
pub const NO_PROJECT: &str = "No project";
/// Type used when a recurring task's type is blank.
pub const OTHER_TYPE: &str = "Other";

/// Group = project name, or `Recurring: <type>` (blank type → `Other`), or `No project`.
pub fn group_label(source: GroupSource<'_>) -> String {
    match source {
        GroupSource::Project { name } => name.trim().to_string(),
        GroupSource::Recurring { recurring_type } => {
            let kind = recurring_type
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .unwrap_or(OTHER_TYPE);
            format!("Recurring: {kind}")
        }
        GroupSource::None => NO_PROJECT.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_labels() {
        let cases = [
            (GroupSource::Project { name: "Website" }, "Website"),
            (GroupSource::Project { name: " Website " }, "Website"),
            (
                GroupSource::Recurring {
                    recurring_type: Some("Patching"),
                },
                "Recurring: Patching",
            ),
            (
                GroupSource::Recurring {
                    recurring_type: Some(""),
                },
                "Recurring: Other",
            ),
            (
                GroupSource::Recurring {
                    recurring_type: Some("   "),
                },
                "Recurring: Other",
            ),
            (
                GroupSource::Recurring {
                    recurring_type: None,
                },
                "Recurring: Other",
            ),
            (GroupSource::None, "No project"),
        ];
        for (source, expected) in cases {
            assert_eq!(group_label(source), expected, "{source:?}");
        }
    }
}
