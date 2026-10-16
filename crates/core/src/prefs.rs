//! The user's preferences, stored as key/value settings.
//!
//! Every key has a default; a stored value that is unknown or broken falls back
//! to the default instead of failing. Saving checks the value first.

use serde::Serialize;

use crate::error::{preview, CoreError};
use crate::sheet::{CommentFormat, GroupBy};
use crate::validate::validate_color;

/// Theme: `system`, `light` or `dark`.
pub const KEY_THEME: &str = "ui.theme";
/// Motion: `still`, `calm` or `lively`.
pub const KEY_MOTION: &str = "ui.motion";
/// Accent colour `#rrggbb`, or blank for the built-in one.
pub const KEY_ACCENT: &str = "ui.accent";
/// Crib sheet grouping: `code` or `project`.
pub const KEY_GROUP_BY: &str = "timesheet.group_by";
/// Tickets in comments: `true` or `false`.
pub const KEY_COMMENT_TICKETS: &str = "timesheet.comment_tickets";
/// Text between comment items (1 to 5 characters, e.g. `; `).
pub const KEY_COMMENT_SEPARATOR: &str = "timesheet.comment_separator";
/// Minutes without activity before the app locks (1 to 240).
pub const KEY_AUTO_LOCK: &str = "security.auto_lock_minutes";

/// Default minutes before auto-lock (SPEC §2.9).
pub const DEFAULT_AUTO_LOCK_MINUTES: u32 = 15;
/// Largest auto-lock setting (4 hours).
pub const MAX_AUTO_LOCK_MINUTES: u32 = 240;
/// Longest comment separator.
pub const MAX_SEPARATOR_CHARS: usize = 5;

/// Colours offered for projects, in the order new projects get them.
pub const PROJECT_PALETTE: [&str; 10] = [
    "#3d5bb0", "#148c79", "#de9930", "#dd5536", "#8a4c7b", "#5c9145", "#2b8fc9", "#c2417a",
    "#7a6a55", "#5a5f68",
];

/// The colour for a new project: the first palette colour no live project uses,
/// or, when all are used, the palette colour at the number of projects.
pub fn next_project_color<S: AsRef<str>>(used: &[Option<S>]) -> String {
    let taken = |c: &str| {
        used.iter()
            .flatten()
            .any(|u| u.as_ref().eq_ignore_ascii_case(c))
    };
    PROJECT_PALETTE
        .iter()
        .find(|c| !taken(c))
        .copied()
        .unwrap_or(PROJECT_PALETTE[used.len() % PROJECT_PALETTE.len()])
        .to_string()
}

/// Light, dark or follow the system.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Follow the operating system (default).
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/// How much things move.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    /// No animation.
    Still,
    /// Gentle (default).
    #[default]
    Calm,
    /// More bounce and travel.
    Lively,
}

/// All preferences with their current values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    /// Theme.
    pub theme: Theme,
    /// Motion.
    pub motion: Motion,
    /// Accent colour, `None` for the built-in one.
    pub accent: Option<String>,
    /// Crib sheet grouping.
    pub group_by: GroupBy,
    /// Comment style.
    pub comment: CommentFormat,
    /// Minutes before auto-lock.
    pub auto_lock_minutes: u32,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            motion: Motion::default(),
            accent: None,
            group_by: GroupBy::default(),
            comment: CommentFormat::default(),
            auto_lock_minutes: DEFAULT_AUTO_LOCK_MINUTES,
        }
    }
}

fn invalid(value: &str) -> CoreError {
    CoreError::SettingInvalid {
        value: preview(value, 30),
    }
}

/// Check a preference before saving. Returns the value to store. Unknown keys
/// are refused.
pub fn validate_pref(key: &str, value: &str) -> Result<String, CoreError> {
    let v = value.trim();
    let ok = |allowed: &[&str]| {
        if allowed.contains(&v) {
            Ok(v.to_string())
        } else {
            Err(invalid(value))
        }
    };
    match key {
        KEY_THEME => ok(&["system", "light", "dark"]),
        KEY_MOTION => ok(&["still", "calm", "lively"]),
        KEY_GROUP_BY => ok(&["code", "project"]),
        KEY_COMMENT_TICKETS => ok(&["true", "false"]),
        KEY_ACCENT => {
            if v.is_empty() {
                Ok(String::new())
            } else {
                validate_color(v)
            }
        }
        KEY_COMMENT_SEPARATOR => {
            // Spaces matter here ("; "), so the raw value is kept.
            let n = value.chars().count();
            if (1..=MAX_SEPARATOR_CHARS).contains(&n) && !value.chars().any(|c| c.is_control()) {
                Ok(value.to_string())
            } else {
                Err(invalid(value))
            }
        }
        KEY_AUTO_LOCK => match v.parse::<u32>() {
            Ok(n) if (1..=MAX_AUTO_LOCK_MINUTES).contains(&n) => Ok(n.to_string()),
            _ => Err(CoreError::OutOfRange {
                field: "Lock after",
                min: 1,
                max: i64::from(MAX_AUTO_LOCK_MINUTES),
            }),
        },
        _ => Err(invalid(key)),
    }
}

/// Read preferences from stored settings; anything missing or broken uses the default.
pub fn prefs_from_settings<K: AsRef<str>, V: AsRef<str>>(settings: &[(K, V)]) -> Prefs {
    let mut prefs = Prefs::default();
    for (key, value) in settings {
        let key = key.as_ref();
        let Ok(v) = validate_pref(key, value.as_ref()) else {
            continue;
        };
        match key {
            KEY_THEME => {
                prefs.theme = match v.as_str() {
                    "light" => Theme::Light,
                    "dark" => Theme::Dark,
                    _ => Theme::System,
                }
            }
            KEY_MOTION => {
                prefs.motion = match v.as_str() {
                    "still" => Motion::Still,
                    "lively" => Motion::Lively,
                    _ => Motion::Calm,
                }
            }
            KEY_ACCENT => prefs.accent = Some(v).filter(|a| !a.is_empty()),
            KEY_GROUP_BY => {
                prefs.group_by = if v == "project" {
                    GroupBy::Project
                } else {
                    GroupBy::Code
                }
            }
            KEY_COMMENT_TICKETS => prefs.comment.include_tickets = v == "true",
            KEY_COMMENT_SEPARATOR => prefs.comment.separator = v,
            KEY_AUTO_LOCK => {
                prefs.auto_lock_minutes = v.parse().unwrap_or(DEFAULT_AUTO_LOCK_MINUTES)
            }
            _ => {}
        }
    }
    prefs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let p = prefs_from_settings::<&str, &str>(&[]);
        assert_eq!(p, Prefs::default());
        assert_eq!(p.theme, Theme::System);
        assert_eq!(p.motion, Motion::Calm);
        assert_eq!(p.auto_lock_minutes, 15);
        assert!(p.comment.include_tickets);
        assert_eq!(p.comment.separator, "; ");
        assert_eq!(p.group_by, GroupBy::Code);
    }

    #[test]
    fn reads_stored_values_and_ignores_broken_ones() {
        let p = prefs_from_settings(&[
            ("ui.theme", "dark"),
            ("ui.motion", "lively"),
            ("ui.accent", "#AABBCC"),
            ("timesheet.group_by", "project"),
            ("timesheet.comment_tickets", "false"),
            ("timesheet.comment_separator", " / "),
            ("security.auto_lock_minutes", "30"),
            ("seed.version", "1"),
        ]);
        assert_eq!(p.theme, Theme::Dark);
        assert_eq!(p.motion, Motion::Lively);
        assert_eq!(p.accent.as_deref(), Some("#aabbcc"));
        assert_eq!(p.group_by, GroupBy::Project);
        assert!(!p.comment.include_tickets);
        assert_eq!(p.comment.separator, " / ");
        assert_eq!(p.auto_lock_minutes, 30);

        let p = prefs_from_settings(&[
            ("ui.theme", "purple"),
            ("security.auto_lock_minutes", "0"),
            ("ui.accent", "blue"),
        ]);
        assert_eq!(p, Prefs::default());
    }

    #[test]
    fn validation() {
        assert_eq!(validate_pref(KEY_THEME, " light "), Ok("light".into()));
        assert!(validate_pref(KEY_THEME, "Light").is_err());
        assert_eq!(validate_pref(KEY_ACCENT, ""), Ok(String::new()));
        assert!(validate_pref(KEY_ACCENT, "#12345").is_err());
        assert_eq!(validate_pref(KEY_AUTO_LOCK, "240"), Ok("240".into()));
        assert!(validate_pref(KEY_AUTO_LOCK, "241").is_err());
        assert!(validate_pref(KEY_AUTO_LOCK, "x").is_err());
        assert!(validate_pref(KEY_COMMENT_SEPARATOR, "").is_err());
        assert!(validate_pref(KEY_COMMENT_SEPARATOR, "------").is_err());
        assert!(validate_pref(KEY_COMMENT_SEPARATOR, "\n").is_err());
        assert!(validate_pref("security.password", "x").is_err());
    }

    #[test]
    fn project_colours() {
        assert_eq!(next_project_color::<&str>(&[]), "#3d5bb0");
        assert_eq!(next_project_color(&[Some("#3D5BB0"), None]), "#148c79");
        let all: Vec<Option<&str>> = PROJECT_PALETTE.iter().map(|c| Some(*c)).collect();
        assert_eq!(next_project_color(&all), PROJECT_PALETTE[0]);
        let mut eleven = all.clone();
        eleven.push(None);
        assert_eq!(next_project_color(&eleven), PROJECT_PALETTE[1]);
    }

    #[test]
    fn serializes_for_the_ui() {
        let json = serde_json::to_value(Prefs::default()).unwrap();
        assert_eq!(json["theme"], "system");
        assert_eq!(json["motion"], "calm");
        assert_eq!(json["groupBy"], "code");
        assert_eq!(json["comment"]["includeTickets"], true);
        assert_eq!(json["autoLockMinutes"], 15);
    }
}
