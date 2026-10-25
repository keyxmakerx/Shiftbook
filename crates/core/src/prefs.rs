//! The user's preferences, stored as key/value settings.
//!
//! Every key has a default; a stored value that is unknown or broken falls back
//! to the default instead of failing. Saving checks the value first.

use serde::{Serialize, Serializer};
use time::Weekday;

use crate::error::{preview, CoreError};
use crate::home::UseMode;
use crate::notes::{AUTO_ARCHIVE_CHOICES, DEFAULT_AUTO_ARCHIVE_DAYS};
use crate::sheet::{CommentFormat, GroupBy};
use crate::timer::PausedTimer;
use crate::validate::validate_color;
use crate::week::WeekStart;

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
/// First day of the week: `monday` … `sunday`.
pub const KEY_WEEK_START: &str = "calendar.week_start";
/// Minutes without activity before the app locks (1 to 240).
pub const KEY_AUTO_LOCK: &str = "security.auto_lock_minutes";
/// Phone: minutes away from the app before it locks, one of [`LEAVE_LOCK_CHOICES`].
pub const KEY_LEAVE_LOCK: &str = "security.leave_lock_minutes";

/// Daily target per weekday for the Desk ring: seven minute counts, Monday first,
/// comma-separated (e.g. `480,480,480,480,240,0,0`).
pub const KEY_DAY_TARGETS: &str = "day.targets";
/// Text size in percent: `90`, `100`, `115` or `130`.
pub const KEY_TEXT_SIZE: &str = "ui.text_size";
/// End-of-day wrap-up time `HH:MM` (24-hour), or blank when it's off.
pub const KEY_WRAPUP_AT: &str = "day.wrapup_at";
/// The optional "Now doing" timer: `true` or `false` (off by default).
pub const KEY_TIMER: &str = "day.timer";
/// Phone: the quiet due reminder's time `HH:MM` on work days, or blank when it's off
/// (off by default). See [`crate::nudge`].
pub const KEY_REMIND_AT: &str = "phone.remind_at";
/// Phone: a wrap-up nudge at the wrap-up time, `true` or `false` (off by default).
pub const KEY_WRAPUP_NUDGE: &str = "phone.wrapup_nudge";
/// A timer paused by locking (see [`PausedTimer`]), or blank.
pub const KEY_TIMER_PAUSED: &str = "timer.paused";
/// Open windows and where they are (see [`crate::windows`]), or blank.
pub const KEY_WINDOWS: &str = "ui.windows";
/// Days without a change before a note moves to the archive, one of
/// [`AUTO_ARCHIVE_CHOICES`] (`0` = off).
pub const KEY_NOTES_AUTO_ARCHIVE: &str = "notes.auto_archive_days";
/// Where the Desk's "Done today" pile sits, `x y` in canvas units, or blank for automatic.
pub const KEY_DONE_PILE: &str = "desk.done_pile";
/// What Shiftbook is for: `work`, `home` or `both` (see [`crate::home`]).
pub const KEY_USE_MODE: &str = "use.mode";
/// The last day Shiftbook was open (`YYYY-MM-DD`), for "While you were away".
pub const KEY_LAST_OPEN: &str = "app.last_open";
/// The two headings of "Copy for stand-up", `done|next` (e.g. `Yesterday|Today`).
pub const KEY_STANDUP_WORDS: &str = "day.standup_words";
/// Longest stand-up heading.
pub const MAX_STANDUP_WORD_CHARS: usize = 20;
/// Quick unlock with a PIN is turned on (round 15). Only a flag: the PIN is never stored.
pub const KEY_QUICK_UNLOCK: &str = "security.quick_unlock";
/// Your own time buttons (round 15): minutes, comma-separated, or blank for the defaults.
pub const KEY_TIME_BUTTONS: &str = "day.time_buttons";
/// The time buttons everyone starts with.
pub const DEFAULT_TIME_BUTTONS: [u32; 5] = [15, 30, 45, 60, 120];
/// Most time buttons.
pub const MAX_TIME_BUTTONS: usize = 5;
/// Longest time a button can add: 12 hours.
pub const MAX_TIME_BUTTON: u32 = 12 * 60;

/// Text sizes offered, in percent of normal.
pub const TEXT_SIZES: [u32; 4] = [90, 100, 115, 130];
/// Normal text size.
pub const DEFAULT_TEXT_SIZE: u32 = 100;
/// Default daily target: 8 hours on weekdays, nothing at the weekend.
pub const DEFAULT_DAY_TARGETS: [u32; 7] = [480, 480, 480, 480, 480, 0, 0];
/// Largest daily target (a whole day).
pub const MAX_DAY_TARGET: u32 = 24 * 60;

/// Default minutes before auto-lock (SPEC §2.9).
pub const DEFAULT_AUTO_LOCK_MINUTES: u32 = 15;
/// Largest auto-lock setting (4 hours).
pub const MAX_AUTO_LOCK_MINUTES: u32 = 240;
/// Phone: lock straight away, or after 1, 5 or 15 minutes away from the app.
pub const LEAVE_LOCK_CHOICES: [u32; 4] = [0, 1, 5, 15];
/// Phone: lock after a minute away (phones get lost more than computers).
pub const DEFAULT_LEAVE_LOCK_MINUTES: u32 = 1;
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

/// Week start days by their stored names, Monday first.
const WEEKDAY_NAMES: [(&str, Weekday); 7] = [
    ("monday", Weekday::Monday),
    ("tuesday", Weekday::Tuesday),
    ("wednesday", Weekday::Wednesday),
    ("thursday", Weekday::Thursday),
    ("friday", Weekday::Friday),
    ("saturday", Weekday::Saturday),
    ("sunday", Weekday::Sunday),
];

fn weekday_from_name(name: &str) -> Option<Weekday> {
    WEEKDAY_NAMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, d)| *d)
}

fn weekday_name(day: Weekday) -> &'static str {
    WEEKDAY_NAMES
        .iter()
        .find(|(_, d)| *d == day)
        .map_or("monday", |(n, _)| n)
}

fn serialize_week_start<S: Serializer>(start: &WeekStart, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(weekday_name(start.0))
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
    /// First day of the week.
    #[serde(serialize_with = "serialize_week_start")]
    pub week_start: WeekStart,
    /// Minutes before auto-lock.
    pub auto_lock_minutes: u32,
    /// Phone: minutes away from the app before it locks.
    pub leave_lock_minutes: u32,
    /// Daily target in minutes, Monday first.
    pub day_targets: [u32; 7],
    /// Text size in percent.
    pub text_size: u32,
    /// Wrap-up time `HH:MM`, `None` when off.
    pub wrapup_at: Option<String>,
    /// The "Now doing" timer is turned on.
    pub timer: bool,
    /// Phone: due reminder time `HH:MM`, `None` when off.
    pub remind_at: Option<String>,
    /// Phone: a wrap-up nudge at the wrap-up time.
    pub wrapup_nudge: bool,
    /// Quick unlock with a PIN is turned on.
    pub quick_unlock: bool,
    /// A timer paused by locking, to pick up after unlock.
    pub paused_timer: Option<PausedTimer>,
    /// Days without a change before a note moves to the archive (0 = off).
    pub notes_auto_archive_days: u32,
    /// Open windows and where they are.
    pub windows: Vec<crate::windows::WindowSpot>,
    /// Where the Desk's "Done today" pile sits; `None` places it automatically.
    pub done_pile: Option<(i32, i32)>,
    /// What Shiftbook is for (round 15).
    pub use_mode: UseMode,
    /// The headings of "Copy for stand-up": what was done, what's next.
    pub standup_words: (String, String),
    /// The time buttons in Log work, jots and tickets, shortest first.
    pub time_buttons: Vec<u32>,
}

impl Prefs {
    /// The target for a date's weekday.
    pub fn day_target(&self, date: time::Date) -> u32 {
        self.day_targets[usize::from(date.weekday().number_days_from_monday())]
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            motion: Motion::default(),
            accent: None,
            group_by: GroupBy::default(),
            comment: CommentFormat::default(),
            week_start: WeekStart::default(),
            auto_lock_minutes: DEFAULT_AUTO_LOCK_MINUTES,
            leave_lock_minutes: DEFAULT_LEAVE_LOCK_MINUTES,
            day_targets: DEFAULT_DAY_TARGETS,
            text_size: DEFAULT_TEXT_SIZE,
            wrapup_at: None,
            remind_at: None,
            wrapup_nudge: false,
            timer: false,
            quick_unlock: false,
            paused_timer: None,
            notes_auto_archive_days: DEFAULT_AUTO_ARCHIVE_DAYS,
            windows: Vec::new(),
            done_pile: None,
            use_mode: UseMode::Work,
            standup_words: (
                crate::standup::DEFAULT_WORDS.0.to_string(),
                crate::standup::DEFAULT_WORDS.1.to_string(),
            ),
            time_buttons: DEFAULT_TIME_BUTTONS.to_vec(),
        }
    }
}

fn invalid(value: &str) -> CoreError {
    CoreError::SettingInvalid {
        value: preview(value, 30),
    }
}

/// Read a `day.targets` value: seven whole minute counts from 0 to a day.
pub fn parse_day_targets(value: &str) -> Option<[u32; 7]> {
    let parts: Vec<u32> = value
        .split(',')
        .map(|p| {
            p.trim()
                .parse::<u32>()
                .ok()
                .filter(|n| *n <= MAX_DAY_TARGET)
        })
        .collect::<Option<_>>()?;
    parts.try_into().ok()
}

/// Read a wrap-up time `H:MM` or `HH:MM` and store it as `HH:MM`.
fn parse_clock(value: &str) -> Option<String> {
    let (h, m) = value.split_once(':')?;
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (h < 24 && m < 60 && value.len() <= 5).then(|| format!("{h:02}:{m:02}"))
}

/// Read a `desk.done_pile` value `x y`, kept on the Canvas.
fn parse_spot(value: &str) -> Option<(i32, i32)> {
    let (x, y) = value.split_once(' ')?;
    Some(crate::canvas::clamp_spot(
        x.trim().parse().ok()?,
        y.trim().parse().ok()?,
    ))
}

/// Read a `day.standup_words` value: two short headings split by `|`.
fn parse_standup_words(value: &str) -> Option<(String, String)> {
    let (a, b) = value.split_once('|')?;
    let ok = |w: &str| {
        let w = w.trim();
        let n = w.chars().count();
        ((1..=MAX_STANDUP_WORD_CHARS).contains(&n)
            && !w.chars().any(|c| c.is_control() || c == '|'))
        .then(|| w.to_string())
    };
    Some((ok(a)?, ok(b)?))
}

/// Read a `day.time_buttons` value: one to five minute counts, each from 1 minute to 12
/// hours, kept shortest first without repeats.
fn parse_time_buttons(value: &str) -> Option<Vec<u32>> {
    let mut list: Vec<u32> = value
        .split(',')
        .map(|p| {
            p.trim()
                .parse::<u32>()
                .ok()
                .filter(|n| (1..=MAX_TIME_BUTTON).contains(n))
        })
        .collect::<Option<_>>()?;
    list.sort_unstable();
    list.dedup();
    (1..=MAX_TIME_BUTTONS).contains(&list.len()).then_some(list)
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
        KEY_COMMENT_TICKETS | KEY_TIMER | KEY_WRAPUP_NUDGE | KEY_QUICK_UNLOCK => {
            ok(&["true", "false"])
        }
        KEY_WEEK_START => ok(&WEEKDAY_NAMES.map(|(n, _)| n)),
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
        KEY_LEAVE_LOCK => match v.parse::<u32>() {
            Ok(n) if LEAVE_LOCK_CHOICES.contains(&n) => Ok(n.to_string()),
            _ => Err(invalid(value)),
        },
        KEY_DAY_TARGETS => parse_day_targets(v)
            .map(|t| t.map(|n| n.to_string()).join(","))
            .ok_or_else(|| invalid(value)),
        KEY_TEXT_SIZE => match v.parse::<u32>() {
            Ok(n) if TEXT_SIZES.contains(&n) => Ok(n.to_string()),
            _ => Err(invalid(value)),
        },
        KEY_NOTES_AUTO_ARCHIVE => match v.parse::<u32>() {
            Ok(n) if AUTO_ARCHIVE_CHOICES.contains(&n) => Ok(n.to_string()),
            _ => Err(invalid(value)),
        },
        KEY_WRAPUP_AT | KEY_REMIND_AT => {
            if v.is_empty() {
                Ok(String::new())
            } else {
                parse_clock(v).ok_or_else(|| invalid(value))
            }
        }
        KEY_WINDOWS => Ok(crate::windows::format_windows(
            &crate::windows::parse_windows(v),
        )),
        KEY_DONE_PILE => {
            if v.is_empty() {
                Ok(String::new())
            } else {
                parse_spot(v)
                    .map(|(x, y)| format!("{x} {y}"))
                    .ok_or_else(|| invalid(value))
            }
        }
        KEY_USE_MODE => UseMode::parse(v)
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| invalid(value)),
        KEY_LAST_OPEN => crate::dates::parse_date(v)
            .map(crate::dates::format_date)
            .map_err(|_| invalid(value)),
        KEY_STANDUP_WORDS => parse_standup_words(v)
            .map(|(a, b)| format!("{a}|{b}"))
            .ok_or_else(|| invalid(value)),
        KEY_TIME_BUTTONS if v.is_empty() => Ok(String::new()),
        KEY_TIME_BUTTONS => parse_time_buttons(v)
            .map(|l| l.iter().map(u32::to_string).collect::<Vec<_>>().join(","))
            .ok_or_else(|| invalid(value)),
        KEY_TIMER_PAUSED => {
            if v.is_empty() {
                Ok(String::new())
            } else {
                PausedTimer::parse(v)
                    .map(|p| p.format())
                    .ok_or_else(|| invalid(value))
            }
        }
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
            KEY_WEEK_START => {
                prefs.week_start = WeekStart(weekday_from_name(&v).unwrap_or(Weekday::Monday))
            }
            KEY_AUTO_LOCK => {
                prefs.auto_lock_minutes = v.parse().unwrap_or(DEFAULT_AUTO_LOCK_MINUTES)
            }
            KEY_LEAVE_LOCK => {
                prefs.leave_lock_minutes = v.parse().unwrap_or(DEFAULT_LEAVE_LOCK_MINUTES)
            }
            KEY_DAY_TARGETS => {
                prefs.day_targets = parse_day_targets(&v).unwrap_or(DEFAULT_DAY_TARGETS)
            }
            KEY_TEXT_SIZE => prefs.text_size = v.parse().unwrap_or(DEFAULT_TEXT_SIZE),
            KEY_WRAPUP_AT => prefs.wrapup_at = Some(v).filter(|t| !t.is_empty()),
            KEY_TIMER => prefs.timer = v == "true",
            KEY_REMIND_AT => prefs.remind_at = parse_clock(&v),
            KEY_WRAPUP_NUDGE => prefs.wrapup_nudge = v == "true",
            KEY_QUICK_UNLOCK => prefs.quick_unlock = v == "true",
            KEY_TIMER_PAUSED => prefs.paused_timer = PausedTimer::parse(&v),
            KEY_NOTES_AUTO_ARCHIVE => {
                prefs.notes_auto_archive_days = v.parse().unwrap_or(DEFAULT_AUTO_ARCHIVE_DAYS)
            }
            KEY_WINDOWS => prefs.windows = crate::windows::parse_windows(&v),
            KEY_DONE_PILE => prefs.done_pile = parse_spot(&v),
            KEY_USE_MODE => prefs.use_mode = UseMode::parse(&v).unwrap_or_default(),
            KEY_STANDUP_WORDS => {
                if let Some(words) = parse_standup_words(&v) {
                    prefs.standup_words = words;
                }
            }
            KEY_TIME_BUTTONS => {
                if let Some(list) = parse_time_buttons(&v) {
                    prefs.time_buttons = list;
                }
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
    fn use_mode_and_last_open() {
        assert_eq!(validate_pref(KEY_USE_MODE, " home "), Ok("home".into()));
        assert!(validate_pref(KEY_USE_MODE, "garden").is_err());
        let p = prefs_from_settings(&[(KEY_USE_MODE, "both")]);
        assert_eq!(p.use_mode, UseMode::Both);
        assert_eq!(Prefs::default().use_mode, UseMode::Work);
        assert_eq!(
            validate_pref(KEY_LAST_OPEN, "2026-10-25"),
            Ok("2026-10-25".into())
        );
        assert!(validate_pref(KEY_LAST_OPEN, "yesterday").is_err());
    }

    #[test]
    fn the_done_pile_spot_is_kept_on_the_canvas() {
        assert_eq!(
            validate_pref(KEY_DONE_PILE, " 40  -12 "),
            Ok("40 -12".into())
        );
        assert_eq!(validate_pref(KEY_DONE_PILE, ""), Ok(String::new()));
        assert!(validate_pref(KEY_DONE_PILE, "left").is_err());
        let far = validate_pref(KEY_DONE_PILE, "99999999 0").unwrap();
        let p = prefs_from_settings(&[(KEY_DONE_PILE, far.as_str())]);
        assert!(p.done_pile.unwrap().0 < 99_999_999);
        assert_eq!(Prefs::default().done_pile, None);
    }

    #[test]
    fn stand_up_words_are_two_short_headings() {
        assert_eq!(
            validate_pref(KEY_STANDUP_WORDS, " Done | Next "),
            Ok("Done|Next".into())
        );
        assert!(validate_pref(KEY_STANDUP_WORDS, "Done").is_err());
        assert!(validate_pref(KEY_STANDUP_WORDS, "|Next").is_err());
        assert!(validate_pref(KEY_STANDUP_WORDS, &format!("{}|x", "a".repeat(21))).is_err());
        let p = prefs_from_settings(&[(KEY_STANDUP_WORDS, "Done|Next")]);
        assert_eq!(p.standup_words, ("Done".into(), "Next".into()));
        assert_eq!(Prefs::default().standup_words.0, "Yesterday");
    }

    #[test]
    fn time_buttons_are_one_to_five_short_times() {
        assert_eq!(
            validate_pref(KEY_TIME_BUTTONS, " 60, 10,20 ,10"),
            Ok("10,20,60".into())
        );
        assert_eq!(validate_pref(KEY_TIME_BUTTONS, ""), Ok(String::new()));
        assert!(validate_pref(KEY_TIME_BUTTONS, "0").is_err());
        assert!(validate_pref(KEY_TIME_BUTTONS, "721").is_err());
        assert!(validate_pref(KEY_TIME_BUTTONS, "1,2,3,4,5,6").is_err());
        assert!(validate_pref(KEY_TIME_BUTTONS, "ten").is_err());
        let p = prefs_from_settings(&[(KEY_TIME_BUTTONS, "10,20,60,180")]);
        assert_eq!(p.time_buttons, vec![10, 20, 60, 180]);
        assert_eq!(
            prefs_from_settings(&[(KEY_TIME_BUTTONS, "")]).time_buttons,
            DEFAULT_TIME_BUTTONS.to_vec()
        );
    }

    #[test]
    fn windows_are_kept_cleaned() {
        let id = uuid::Uuid::now_v7();
        let stored =
            validate_pref(KEY_WINDOWS, &format!("note:{id} open 10 20 30 40;junk")).unwrap();
        assert_eq!(stored, format!("note:{id} open 10 20 260 160"));
        let p = prefs_from_settings(&[(KEY_WINDOWS, stored.as_str())]);
        assert_eq!(p.windows.len(), 1);
        assert_eq!(validate_pref(KEY_WINDOWS, ""), Ok(String::new()));
    }

    #[test]
    fn defaults() {
        let p = prefs_from_settings::<&str, &str>(&[]);
        assert_eq!(p, Prefs::default());
        assert_eq!(p.theme, Theme::System);
        assert_eq!(p.motion, Motion::Calm);
        assert_eq!(p.auto_lock_minutes, 15);
        assert!(p.comment.include_tickets);
        assert!(!p.timer, "the timer is off unless turned on");
        assert_eq!(p.paused_timer, None);
        assert_eq!(p.notes_auto_archive_days, 90);
        assert_eq!(p.comment.separator, "; ");
        assert_eq!(p.group_by, GroupBy::Code);
        assert_eq!(p.week_start, WeekStart(Weekday::Monday));
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
            ("calendar.week_start", "sunday"),
            ("seed.version", "1"),
        ]);
        assert_eq!(p.theme, Theme::Dark);
        assert_eq!(p.motion, Motion::Lively);
        assert_eq!(p.accent.as_deref(), Some("#aabbcc"));
        assert_eq!(p.group_by, GroupBy::Project);
        assert!(!p.comment.include_tickets);
        assert_eq!(p.comment.separator, " / ");
        assert_eq!(p.auto_lock_minutes, 30);
        assert_eq!(p.week_start, WeekStart(Weekday::Sunday));

        let p = prefs_from_settings(&[
            ("ui.theme", "purple"),
            ("calendar.week_start", "Funday"),
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
        assert_eq!(validate_pref(KEY_LEAVE_LOCK, " 5 "), Ok("5".into()));
        assert_eq!(validate_pref(KEY_LEAVE_LOCK, "0"), Ok("0".into()));
        assert!(validate_pref(KEY_LEAVE_LOCK, "2").is_err());
        assert_eq!(Prefs::default().leave_lock_minutes, 1);
        assert_eq!(
            prefs_from_settings(&[(KEY_LEAVE_LOCK, "15")]).leave_lock_minutes,
            15
        );
        assert!(validate_pref(KEY_COMMENT_SEPARATOR, "").is_err());
        assert!(validate_pref(KEY_COMMENT_SEPARATOR, "------").is_err());
        assert!(validate_pref(KEY_COMMENT_SEPARATOR, "\n").is_err());
        assert_eq!(
            validate_pref(KEY_WEEK_START, " saturday "),
            Ok("saturday".into())
        );
        assert!(validate_pref(KEY_WEEK_START, "Saturday").is_err());
        assert!(validate_pref(KEY_WEEK_START, "sun").is_err());
        assert!(validate_pref("security.password", "x").is_err());
    }

    #[test]
    fn every_week_start_round_trips() {
        for (name, day) in WEEKDAY_NAMES {
            let p = prefs_from_settings(&[(KEY_WEEK_START, name)]);
            assert_eq!(p.week_start, WeekStart(day), "{name}");
            assert_eq!(weekday_name(day), name);
        }
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
        assert_eq!(json["dayTargets"][0], 480);
        assert_eq!(json["dayTargets"][6], 0);
        assert_eq!(json["textSize"], 100);
        assert!(json["wrapupAt"].is_null());
        assert_eq!(json["weekStart"], "monday");
        let sunday = Prefs {
            week_start: WeekStart(Weekday::Sunday),
            ..Prefs::default()
        };
        assert_eq!(serde_json::to_value(sunday).unwrap()["weekStart"], "sunday");
    }

    #[test]
    fn day_targets() {
        assert_eq!(
            validate_pref(KEY_DAY_TARGETS, " 480, 480,480,480,240,0,0 "),
            Ok("480,480,480,480,240,0,0".into())
        );
        assert!(validate_pref(KEY_DAY_TARGETS, "480,480,480,480,480,0").is_err());
        assert!(validate_pref(KEY_DAY_TARGETS, "480,480,480,480,480,0,0,0").is_err());
        assert!(validate_pref(KEY_DAY_TARGETS, "480,480,480,480,1441,0,0").is_err());
        assert!(validate_pref(KEY_DAY_TARGETS, "8h,480,480,480,480,0,0").is_err());

        let p = prefs_from_settings(&[("day.targets", "420,420,420,420,240,0,0")]);
        assert_eq!(p.day_target(time::macros::date!(2026 - 10 - 16)), 240); // Friday
        assert_eq!(p.day_target(time::macros::date!(2026 - 10 - 12)), 420); // Monday
        assert_eq!(p.day_target(time::macros::date!(2026 - 10 - 18)), 0); // Sunday
        let broken = prefs_from_settings(&[("day.targets", "lots")]);
        assert_eq!(broken.day_targets, DEFAULT_DAY_TARGETS);
    }

    #[test]
    fn notes_auto_archive() {
        let p = prefs_from_settings(&[("notes.auto_archive_days", "0")]);
        assert_eq!(p.notes_auto_archive_days, 0, "off");
        let p = prefs_from_settings(&[("notes.auto_archive_days", "45")]);
        assert_eq!(p.notes_auto_archive_days, 90, "not a choice: default");
        assert_eq!(
            validate_pref(KEY_NOTES_AUTO_ARCHIVE, " 180 ").unwrap(),
            "180"
        );
        assert!(validate_pref(KEY_NOTES_AUTO_ARCHIVE, "7").is_err());
    }

    #[test]
    fn text_size_and_wrapup() {
        assert_eq!(validate_pref(KEY_TEXT_SIZE, "115"), Ok("115".into()));
        assert!(validate_pref(KEY_TEXT_SIZE, "120").is_err());
        assert_eq!(validate_pref(KEY_WRAPUP_AT, "9:05"), Ok("09:05".into()));
        assert_eq!(validate_pref(KEY_WRAPUP_AT, "17:00"), Ok("17:00".into()));
        assert_eq!(validate_pref(KEY_WRAPUP_AT, ""), Ok(String::new()));
        assert!(validate_pref(KEY_WRAPUP_AT, "24:00").is_err());
        assert!(validate_pref(KEY_WRAPUP_AT, "5pm").is_err());
        let p = prefs_from_settings(&[("ui.text_size", "130"), ("day.wrapup_at", "17:30")]);
        assert_eq!(p.text_size, 130);
        assert_eq!(p.wrapup_at.as_deref(), Some("17:30"));
    }

    #[test]
    fn timer_on_and_paused() {
        assert_eq!(validate_pref(KEY_TIMER, "true"), Ok("true".into()));
        assert!(validate_pref(KEY_TIMER, "yes").is_err());
        assert_eq!(validate_pref(KEY_REMIND_AT, "8:00"), Ok("08:00".into()));
        assert_eq!(validate_pref(KEY_REMIND_AT, ""), Ok(String::new()));
        assert!(validate_pref(KEY_REMIND_AT, "8am").is_err());
        assert_eq!(validate_pref(KEY_WRAPUP_NUDGE, "true"), Ok("true".into()));
        let p = prefs_from_settings(&[(KEY_REMIND_AT, "08:00"), (KEY_WRAPUP_NUDGE, "true")]);
        assert_eq!(p.remind_at.as_deref(), Some("08:00"));
        assert!(p.wrapup_nudge);
        assert_eq!(Prefs::default().remind_at, None);
        assert!(!Prefs::default().wrapup_nudge);
        let id = crate::ids::TimeEntryId::new();
        let stored = format!("{id} 3700 400,90");
        assert_eq!(validate_pref(KEY_TIMER_PAUSED, &stored), Ok(stored.clone()));
        assert_eq!(validate_pref(KEY_TIMER_PAUSED, ""), Ok(String::new()));
        assert!(validate_pref(KEY_TIMER_PAUSED, "12:00 start").is_err());
        let p = prefs_from_settings(&[("day.timer", "true"), ("timer.paused", stored.as_str())]);
        assert!(p.timer);
        assert_eq!(p.paused_timer.map(|t| t.seconds), Some(3700));
    }
}
