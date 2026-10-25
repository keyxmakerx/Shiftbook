//! Phone nudges (round 15): the quiet due reminder and the wrap-up nudge. Both are off by
//! default, come on work days only (days with a target in Settings → Your day), and never
//! say what is due: the lock screen shows a count and a time, never a title, project or
//! ticket. The phone shows the next one; the app plans it again every time it's opened.

use serde::Serialize;
use time::{Date, Duration};

use crate::prefs::Prefs;

/// Which nudge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NudgeKind {
    /// "N things are due today", at the reminder time.
    Due,
    /// "Time to wrap up the day?", at the wrap-up time.
    WrapUp,
}

/// One nudge for the phone to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Nudge {
    /// Which nudge.
    pub kind: NudgeKind,
    /// The local day it shows on.
    pub date: Date,
    /// Minutes after local midnight.
    pub at: u32,
    /// The notification's first line.
    pub title: String,
    /// The notification's second line.
    pub body: String,
}

/// Minutes after midnight for a stored `HH:MM`.
pub fn clock_minutes(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.split_once(':')?;
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// The first work day at `at` that is still ahead of `now` on `today`: today when it's a
/// work day and `at` hasn't passed, otherwise the next work day within a week. `None`
/// when no weekday has a target.
pub fn next_work_day(prefs: &Prefs, today: Date, now: u32, at: u32) -> Option<Date> {
    (0..=7)
        .map(|n| today + Duration::days(n))
        .filter(|d| *d != today || now < at)
        .find(|d| prefs.day_target(*d) > 0)
}

/// When the next due reminder shows, if it's on. The count is filled in later
/// ([`due_nudge`]), from the bell for that day.
pub fn due_reminder_at(prefs: &Prefs, today: Date, now: u32) -> Option<(Date, u32)> {
    let at = clock_minutes(prefs.remind_at.as_deref()?)?;
    Some((next_work_day(prefs, today, now, at)?, at))
}

/// The due reminder for `count` things due; nothing when nothing is due.
pub fn due_nudge(date: Date, at: u32, count: usize) -> Option<Nudge> {
    let title = match count {
        0 => return None,
        1 => "1 thing is due today".to_string(),
        n => format!("{n} things are due today"),
    };
    Some(Nudge {
        kind: NudgeKind::Due,
        date,
        at,
        title,
        body: "Open Shiftbook to see them.".into(),
    })
}

/// The next wrap-up nudge, if it's on and a wrap-up time is set.
pub fn wrapup_nudge(prefs: &Prefs, today: Date, now: u32) -> Option<Nudge> {
    if !prefs.wrapup_nudge {
        return None;
    }
    let at = clock_minutes(prefs.wrapup_at.as_deref()?)?;
    Some(Nudge {
        kind: NudgeKind::WrapUp,
        date: next_work_day(prefs, today, now, at)?,
        at,
        title: "Time to wrap up the day?".into(),
        body: "Takes a minute: check today's hours and tickets.".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    // 27 Oct 2026 is a Tuesday.
    const TUE: Date = date!(2026 - 10 - 27);

    fn prefs() -> Prefs {
        Prefs {
            remind_at: Some("08:00".into()),
            wrapup_at: Some("16:45".into()),
            wrapup_nudge: true,
            ..Prefs::default()
        }
    }

    #[test]
    fn clock() {
        assert_eq!(clock_minutes("08:00"), Some(480));
        assert_eq!(clock_minutes("23:59"), Some(1439));
        assert_eq!(clock_minutes("24:00"), None);
        assert_eq!(clock_minutes("8"), None);
    }

    #[test]
    fn work_days_only() {
        let p = prefs();
        // Before 8:00 on a Tuesday: today. After: Wednesday.
        assert_eq!(due_reminder_at(&p, TUE, 7 * 60), Some((TUE, 480)));
        assert_eq!(
            due_reminder_at(&p, TUE, 480),
            Some((date!(2026 - 10 - 28), 480))
        );
        // Friday evening: skips the weekend (no target) to Monday.
        assert_eq!(
            due_reminder_at(&p, date!(2026 - 10 - 30), 20 * 60),
            Some((date!(2026 - 11 - 02), 480))
        );
        // No work days at all: never.
        let none = Prefs {
            day_targets: [0; 7],
            ..prefs()
        };
        assert_eq!(due_reminder_at(&none, TUE, 0), None);
        // Only Tuesday, already past: next Tuesday.
        let tue_only = Prefs {
            day_targets: [0, 480, 0, 0, 0, 0, 0],
            ..prefs()
        };
        assert_eq!(
            next_work_day(&tue_only, TUE, 600, 480),
            Some(date!(2026 - 11 - 03))
        );
    }

    #[test]
    fn off_by_default() {
        assert_eq!(due_reminder_at(&Prefs::default(), TUE, 0), None);
        assert_eq!(wrapup_nudge(&Prefs::default(), TUE, 0), None);
        // The wrap-up nudge needs a wrap-up time too.
        let no_time = Prefs {
            wrapup_at: None,
            ..prefs()
        };
        assert_eq!(wrapup_nudge(&no_time, TUE, 0), None);
    }

    #[test]
    fn counts_never_titles() {
        assert_eq!(due_nudge(TUE, 480, 0), None);
        assert_eq!(
            due_nudge(TUE, 480, 1).map(|n| n.title),
            Some("1 thing is due today".into())
        );
        let n = due_nudge(TUE, 480, 3).unwrap();
        assert_eq!(n.title, "3 things are due today");
        assert_eq!(n.body, "Open Shiftbook to see them.");
        let w = wrapup_nudge(&prefs(), TUE, 600).unwrap();
        assert_eq!(
            (w.kind, w.date, w.at),
            (NudgeKind::WrapUp, TUE, 16 * 60 + 45)
        );
    }
}
