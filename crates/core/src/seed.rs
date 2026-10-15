//! Default lists created for a new user. Generic on purpose: no employer
//! names or codes. No timesheet codes and no category → code mappings are
//! seeded; users add their own.

use time::OffsetDateTime;

use crate::ids::{FrequencyId, ListOptionId, OwnerId};
use crate::model::{Frequency, ListName, ListOption, STATUS_BLOCKED, STATUS_DONE};

/// A default dropdown option: value and optional colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedOption {
    /// The text value.
    pub value: &'static str,
    /// Optional `#rrggbb` colour.
    pub color: Option<&'static str>,
}

const fn plain(value: &'static str) -> SeedOption {
    SeedOption { value, color: None }
}

const fn colored(value: &'static str, color: &'static str) -> SeedOption {
    SeedOption {
        value,
        color: Some(color),
    }
}

/// Default categories (kinds of work).
pub const CATEGORIES: &[SeedOption] = &[
    plain("Troubleshooting"),
    plain("Maintenance"),
    plain("Support / Tickets"),
    plain("Meetings"),
    plain("Project work"),
    plain("Documentation"),
    plain("Training"),
    plain("Admin"),
    plain("Travel"),
];

/// Default statuses with colours. `Done` and `Blocked` drive the alert rules.
pub const STATUSES: &[SeedOption] = &[
    colored("Not started", "#6b7280"),
    colored("In progress", "#2563eb"),
    colored("Waiting", "#d97706"),
    colored(STATUS_BLOCKED, "#dc2626"),
    colored(STATUS_DONE, "#16a34a"),
];

/// Default priorities, lowest first.
pub const PRIORITIES: &[SeedOption] = &[
    colored("Low", "#6b7280"),
    colored("Medium", "#2563eb"),
    colored("High", "#d97706"),
    colored("Urgent", "#dc2626"),
];

/// Default kinds of recurring work.
pub const RECURRING_TYPES: &[SeedOption] = &[
    plain("Certificate renewal"),
    plain("Patching"),
    plain("Backup check"),
    plain("Licence renewal"),
    plain("Audit / review"),
    plain("Maintenance"),
    plain("Routine"),
];

/// A default frequency: label, months, days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedFrequency {
    /// Label shown to the user.
    pub label: &'static str,
    /// Whole months.
    pub months: u32,
    /// Extra days.
    pub days: u32,
}

const fn freq(label: &'static str, months: u32, days: u32) -> SeedFrequency {
    SeedFrequency {
        label,
        months,
        days,
    }
}

/// Default frequencies, shortest first.
pub const FREQUENCIES: &[SeedFrequency] = &[
    freq("Daily", 0, 1),
    freq("Weekly", 0, 7),
    freq("Every 2 weeks", 0, 14),
    freq("Monthly", 1, 0),
    freq("Every 2 months", 2, 0),
    freq("Quarterly", 3, 0),
    freq("Every 6 months", 6, 0),
    freq("Yearly", 12, 0),
    freq("Every 2 years", 24, 0),
    freq("Every 3 years", 36, 0),
];

/// The seed options for one list.
pub const fn seed_options(list: ListName) -> &'static [SeedOption] {
    match list {
        ListName::Category => CATEGORIES,
        ListName::Status => STATUSES,
        ListName::Priority => PRIORITIES,
        ListName::RecurringType => RECURRING_TYPES,
    }
}

fn sort_order(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}

/// All default list options as records for `owner`, timestamped `now`.
pub fn default_list_options(owner: OwnerId, now: OffsetDateTime) -> Vec<ListOption> {
    [
        ListName::Category,
        ListName::Status,
        ListName::Priority,
        ListName::RecurringType,
    ]
    .into_iter()
    .flat_map(|list| {
        seed_options(list)
            .iter()
            .enumerate()
            .map(move |(i, opt)| ListOption {
                id: ListOptionId::new(),
                owner_id: owner,
                list_name: list,
                value: opt.value.to_string(),
                sort_order: sort_order(i),
                color: opt.color.map(String::from),
                hidden: false,
                created_at: now,
                updated_at: now,
                deleted_at: None,
            })
    })
    .collect()
}

/// All default frequencies as records for `owner`, timestamped `now`.
pub fn default_frequencies(owner: OwnerId, now: OffsetDateTime) -> Vec<Frequency> {
    FREQUENCIES
        .iter()
        .enumerate()
        .map(|(i, f)| Frequency {
            id: FrequencyId::new(),
            owner_id: owner,
            label: f.label.to_string(),
            months: f.months,
            days: f.days,
            sort_order: sort_order(i),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::{validate_color, validate_frequency, validate_list_value};
    use std::collections::HashSet;
    use time::macros::datetime;

    #[test]
    fn statuses_include_the_rule_values() {
        let values: Vec<&str> = STATUSES.iter().map(|s| s.value).collect();
        assert!(values.contains(&"Done"));
        assert!(values.contains(&"Blocked"));
        assert!(STATUSES.iter().all(|s| s.color.is_some()));
    }

    #[test]
    fn priorities_low_to_urgent() {
        let values: Vec<&str> = PRIORITIES.iter().map(|s| s.value).collect();
        assert_eq!(values, ["Low", "Medium", "High", "Urgent"]);
    }

    #[test]
    fn every_seed_is_valid_and_unique() {
        for list in [
            ListName::Category,
            ListName::Status,
            ListName::Priority,
            ListName::RecurringType,
        ] {
            let options = seed_options(list);
            assert!(!options.is_empty());
            let mut seen = HashSet::new();
            for opt in options {
                assert_eq!(validate_list_value(opt.value), Ok(opt.value.to_string()));
                assert!(
                    seen.insert(opt.value.to_lowercase()),
                    "duplicate {}",
                    opt.value
                );
                if let Some(c) = opt.color {
                    assert!(validate_color(c).is_ok(), "{c}");
                }
            }
        }
        let mut labels = HashSet::new();
        for f in FREQUENCIES {
            assert!(
                validate_frequency(f.label, f.months, f.days).is_ok(),
                "{}",
                f.label
            );
            assert!(labels.insert(f.label));
        }
    }

    #[test]
    fn frequencies_cover_the_common_cases() {
        let find = |label: &str| FREQUENCIES.iter().find(|f| f.label == label).copied();
        assert_eq!(find("Daily"), Some(freq("Daily", 0, 1)));
        assert_eq!(find("Weekly"), Some(freq("Weekly", 0, 7)));
        assert_eq!(find("Monthly"), Some(freq("Monthly", 1, 0)));
        assert_eq!(find("Quarterly"), Some(freq("Quarterly", 3, 0)));
        assert_eq!(find("Every 6 months"), Some(freq("Every 6 months", 6, 0)));
        assert_eq!(find("Yearly"), Some(freq("Yearly", 12, 0)));
        assert_eq!(find("Every 2 years"), Some(freq("Every 2 years", 24, 0)));
    }

    #[test]
    fn records_are_built_for_the_owner() {
        let owner = OwnerId::new();
        let now = datetime!(2026-10-15 09:00 UTC);
        let options = default_list_options(owner, now);
        let expected = CATEGORIES.len() + STATUSES.len() + PRIORITIES.len() + RECURRING_TYPES.len();
        assert_eq!(options.len(), expected);
        assert!(options
            .iter()
            .all(|o| o.owner_id == owner && o.created_at == now));
        let statuses: Vec<&ListOption> = options
            .iter()
            .filter(|o| o.list_name == ListName::Status)
            .collect();
        assert_eq!(statuses[0].value, "Not started");
        assert_eq!(statuses[0].sort_order, 0);
        assert_eq!(statuses[4].sort_order, 4);
        let ids: HashSet<_> = options.iter().map(|o| o.id).collect();
        assert_eq!(ids.len(), options.len());

        let freqs = default_frequencies(owner, now);
        assert_eq!(freqs.len(), FREQUENCIES.len());
        assert_eq!(freqs[3].label, "Monthly");
        assert_eq!(freqs[3].months, 1);
        assert_eq!(freqs[3].sort_order, 3);
    }
}
