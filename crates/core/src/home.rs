//! "What's Shiftbook for?" (round 15): Work, Home or Both, picked at setup and changed in
//! Settings → Look. The choice only changes words, defaults, the starter lists and which
//! views show; no record changes meaning, so switching back is always safe.
//!
//! - **Work** is Shiftbook as it was: an 8 h ring on weekdays, the Timesheet, tickets.
//! - **Home** hides the Timesheet, turns the hours ring off (daily targets of 0), lets Log
//!   work save a line with no time (a tick on the day), locks after 60 minutes instead of 15,
//!   and starts with home categories and kinds of repeating things.
//! - **Both** keeps Work's ring and Timesheet and adds the home lists and the Done button.

use serde::Serialize;

use crate::prefs::{DEFAULT_AUTO_LOCK_MINUTES, DEFAULT_DAY_TARGETS};
use crate::seed::{CATEGORIES, RECURRING_TYPES};

/// What the user said Shiftbook is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum UseMode {
    /// Work only (the default, and Shiftbook as it was before round 15).
    #[default]
    Work,
    /// Home life only.
    Home,
    /// Work and home on one Desk.
    Both,
}

impl UseMode {
    /// The stored name.
    pub const fn as_str(self) -> &'static str {
        match self {
            UseMode::Work => "work",
            UseMode::Home => "home",
            UseMode::Both => "both",
        }
    }

    /// Read a stored name.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "work" => Some(UseMode::Work),
            "home" => Some(UseMode::Home),
            "both" => Some(UseMode::Both),
            _ => None,
        }
    }

    /// True when work things (the Timesheet, the hours ring) show.
    pub const fn has_work(self) -> bool {
        !matches!(self, UseMode::Home)
    }

    /// True when home things (home lists, Done without time) show.
    pub const fn has_home(self) -> bool {
        !matches!(self, UseMode::Work)
    }
}

/// Categories a home starts with.
pub const HOME_CATEGORIES: &[&str] = &[
    "Home", "Money", "Health", "Family", "Car", "Admin", "Errands",
];

/// Kinds of repeating things a home starts with.
pub const HOME_RECURRING_TYPES: &[&str] = &[
    "Bills",
    "Home upkeep",
    "Car",
    "Health",
    "Documents",
    "Birthdays",
];

/// Auto-lock minutes for Home: a home computer is usually not shared with strangers, and
/// typing a long password every 15 minutes is a lot to ask there. Still a setting.
pub const HOME_AUTO_LOCK_MINUTES: u32 = 60;

/// The defaults a mode puts in place when it is picked. Each stays a normal setting the
/// user can change afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeDefaults {
    /// Daily targets, Monday first (all 0 turns the hours ring off).
    pub day_targets: [u32; 7],
    /// Minutes before auto-lock.
    pub auto_lock_minutes: u32,
}

/// What picking `mode` sets.
pub const fn mode_defaults(mode: UseMode) -> ModeDefaults {
    match mode {
        UseMode::Home => ModeDefaults {
            day_targets: [0; 7],
            auto_lock_minutes: HOME_AUTO_LOCK_MINUTES,
        },
        UseMode::Work | UseMode::Both => ModeDefaults {
            day_targets: DEFAULT_DAY_TARGETS,
            auto_lock_minutes: DEFAULT_AUTO_LOCK_MINUTES,
        },
    }
}

/// Which starter list values show under `mode`: `(value, shown)` for every work and home
/// starter value of one list. Values in both sets (e.g. "Admin") show whenever either set
/// does. User-added values are not in here and are never touched.
fn starter_visibility(
    mode: UseMode,
    work: impl Iterator<Item = &'static str>,
    home: &'static [&'static str],
) -> Vec<(&'static str, bool)> {
    let is_home = |v: &str| home.iter().any(|h| h.eq_ignore_ascii_case(v));
    let mut out: Vec<(&'static str, bool)> = Vec::new();
    for v in work {
        out.push((v, mode.has_work() || (mode.has_home() && is_home(v))));
    }
    for v in home {
        if !out.iter().any(|(o, _)| o.eq_ignore_ascii_case(v)) {
            out.push((v, mode.has_home()));
        }
    }
    out
}

/// Starter categories and whether each shows under `mode`.
pub fn category_visibility(mode: UseMode) -> Vec<(&'static str, bool)> {
    starter_visibility(mode, CATEGORIES.iter().map(|o| o.value), HOME_CATEGORIES)
}

/// Starter kinds of repeating things and whether each shows under `mode`.
pub fn recurring_type_visibility(mode: UseMode) -> Vec<(&'static str, bool)> {
    starter_visibility(
        mode,
        RECURRING_TYPES.iter().map(|o| o.value),
        HOME_RECURRING_TYPES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(list: &[(&str, bool)], v: &str) -> Option<bool> {
        list.iter().find(|(o, _)| *o == v).map(|(_, s)| *s)
    }

    #[test]
    fn names_round_trip() {
        for m in [UseMode::Work, UseMode::Home, UseMode::Both] {
            assert_eq!(UseMode::parse(m.as_str()), Some(m));
        }
        assert_eq!(UseMode::parse(" home "), Some(UseMode::Home));
        assert_eq!(UseMode::parse("garden"), None);
        assert_eq!(UseMode::default(), UseMode::Work);
    }

    #[test]
    fn home_turns_the_ring_off_and_locks_later() {
        let h = mode_defaults(UseMode::Home);
        assert_eq!(h.day_targets, [0; 7]);
        assert_eq!(h.auto_lock_minutes, 60);
        let w = mode_defaults(UseMode::Work);
        assert_eq!(w.day_targets, DEFAULT_DAY_TARGETS);
        assert_eq!(w.auto_lock_minutes, DEFAULT_AUTO_LOCK_MINUTES);
        assert_eq!(mode_defaults(UseMode::Both), w);
    }

    #[test]
    fn what_shows_where() {
        assert!(UseMode::Work.has_work() && !UseMode::Work.has_home());
        assert!(!UseMode::Home.has_work() && UseMode::Home.has_home());
        assert!(UseMode::Both.has_work() && UseMode::Both.has_home());
    }

    #[test]
    fn starter_lists_follow_the_mode() {
        let work = category_visibility(UseMode::Work);
        assert_eq!(shown(&work, "Troubleshooting"), Some(true));
        assert_eq!(shown(&work, "Money"), Some(false));
        let home = category_visibility(UseMode::Home);
        assert_eq!(shown(&home, "Troubleshooting"), Some(false));
        assert_eq!(shown(&home, "Money"), Some(true));
        // In both sets: listed once and shown under Home.
        assert_eq!(home.iter().filter(|(v, _)| *v == "Admin").count(), 1);
        assert_eq!(shown(&home, "Admin"), Some(true));
        let both = category_visibility(UseMode::Both);
        assert!(both.iter().all(|(_, s)| *s));
        let kinds = recurring_type_visibility(UseMode::Home);
        assert_eq!(shown(&kinds, "Patching"), Some(false));
        assert_eq!(shown(&kinds, "Bills"), Some(true));
    }
}
