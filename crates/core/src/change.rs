//! MOPs on their change ticket (round 18, card r18-mop). A MOP is made from a wiki template
//! (or a page, or blank) and attached to one ticket. It has a status, ticks that keep the
//! time they were ticked (for the run log in the change record), and an optional change
//! window that shows in the bell before it starts.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::bell::{BellAbout, BellItem, BellWhen};
use crate::dates::{format_date, format_timestamp};
use crate::error::CoreError;

/// Where a MOP stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum MopStatus {
    /// Being written.
    #[default]
    Draft,
    /// Written and approved, waiting for its window.
    Ready,
    /// Being carried out.
    Running,
    /// Finished.
    Done,
    /// Rolled back.
    BackedOut,
}

impl MopStatus {
    /// The stored form.
    pub const fn as_str(self) -> &'static str {
        match self {
            MopStatus::Draft => "draft",
            MopStatus::Ready => "ready",
            MopStatus::Running => "running",
            MopStatus::Done => "done",
            MopStatus::BackedOut => "backed_out",
        }
    }

    /// The word shown, e.g. on the ticket's card.
    pub const fn label(self) -> &'static str {
        match self {
            MopStatus::Draft => "Draft",
            MopStatus::Ready => "Ready",
            MopStatus::Running => "Running",
            MopStatus::Done => "Done",
            MopStatus::BackedOut => "Backed out",
        }
    }

    /// Done or backed out: nothing more to chase.
    pub const fn finished(self) -> bool {
        matches!(self, MopStatus::Done | MopStatus::BackedOut)
    }
}

impl std::str::FromStr for MopStatus {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim() {
            "draft" => MopStatus::Draft,
            "ready" => MopStatus::Ready,
            "running" => MopStatus::Running,
            "done" => MopStatus::Done,
            "backed_out" => MopStatus::BackedOut,
            other => {
                return Err(CoreError::NotAChoice {
                    value: other.chars().take(30).collect(),
                })
            }
        })
    }
}

/// When each ticked line was ticked: (tick key, when, UTC).
pub type TickTimes = Vec<(String, OffsetDateTime)>;

/// After the ticks change to `ticks`: lines still ticked keep their time, newly ticked
/// ones get `now`, unticked ones lose theirs. In `ticks` order.
pub fn tick_times(
    before: &[(String, OffsetDateTime)],
    ticks: &[String],
    now: OffsetDateTime,
) -> TickTimes {
    ticks
        .iter()
        .map(|k| {
            let at = before
                .iter()
                .find(|(b, _)| b == k)
                .map_or(now, |(_, at)| *at);
            (k.clone(), at)
        })
        .collect()
}

/// The stored form: one `when<TAB>key` per line (keys never hold a newline).
pub fn join_tick_times(times: &[(String, OffsetDateTime)]) -> String {
    times
        .iter()
        .map(|(k, at)| format!("{}\t{k}", format_timestamp(*at)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read the stored form back, skipping anything unreadable.
pub fn parse_tick_times(text: &str) -> TickTimes {
    text.lines()
        .filter_map(|line| {
            let (at, key) = line.split_once('\t')?;
            let at =
                OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).ok()?;
            Some((key.to_string(), at))
        })
        .collect()
}

/// A template offered when attaching a MOP: id, title, its project.
pub type Offered<'a> = (Uuid, &'a str, Option<Uuid>);

/// The order templates are offered in for a ticket: the ones used most recently first,
/// then ones for the ticket's project, then the rest, each by title.
pub fn template_order(
    templates: &[Offered<'_>],
    last_used: &HashMap<Uuid, OffsetDateTime>,
    project: Option<Uuid>,
) -> Vec<Uuid> {
    let mut list: Vec<&Offered<'_>> = templates.iter().collect();
    list.sort_by(|a, b| {
        let used = |t: &Offered<'_>| last_used.get(&t.0).copied();
        let mine = |t: &Offered<'_>| project.is_some() && t.2 == project;
        used(b)
            .cmp(&used(a))
            .then(mine(b).cmp(&mine(a)))
            .then(a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    list.into_iter().map(|t| t.0).collect()
}

/// How long before a change window the bell says "Now".
pub const SOON_MINUTES: u32 = 60;

/// A MOP's change window for the bell: its MOP, ticket, title, day and start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window<'a> {
    /// The MOP's note id.
    pub mop: Uuid,
    /// The ticket number.
    pub ticket: &'a str,
    /// The MOP's title.
    pub title: &'a str,
    /// Where it stands.
    pub status: MopStatus,
    /// The window's day (local).
    pub on: Date,
    /// Its start, minutes after local midnight.
    pub at: u32,
}

fn clock(at: u32) -> String {
    format!("{:02}:{:02}", (at / 60) % 24, at % 60)
}

/// The bell's line for a change window at `now` (minutes after local midnight on `today`):
/// "Now" from an hour before it starts and while it runs, "Today" earlier in its day.
/// A finished MOP, a window on another day (unless still running), or none: nothing.
pub fn window_item(w: &Window<'_>, today: Date, now: u32) -> Option<BellItem> {
    if w.status.finished() || w.on > today {
        return None;
    }
    let (when, title) = if w.on < today || now >= w.at {
        if w.on < today && w.status != MopStatus::Running {
            return None;
        }
        (
            BellWhen::Now,
            if w.status == MopStatus::Running {
                format!("Change running: {} · {}", w.ticket, w.title)
            } else {
                format!("Change window now: {} · {}", w.ticket, w.title)
            },
        )
    } else if w.at - now <= SOON_MINUTES {
        (
            BellWhen::Now,
            format!(
                "Change window in {} min: {} · {}",
                w.at - now,
                w.ticket,
                w.title
            ),
        )
    } else {
        (
            BellWhen::Today,
            format!(
                "Change window at {}: {} · {}",
                clock(w.at),
                w.ticket,
                w.title
            ),
        )
    };
    Some(BellItem {
        when,
        title,
        about: BellAbout::ChangeWindow {
            mop: w.mop.to_string(),
            ticket: w.ticket.to_string(),
            on: format_date(w.on),
            at: w.at,
            status: w.status,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{date, datetime};

    #[test]
    fn statuses_round_trip() {
        for s in [
            MopStatus::Draft,
            MopStatus::Ready,
            MopStatus::Running,
            MopStatus::Done,
            MopStatus::BackedOut,
        ] {
            assert_eq!(s.as_str().parse::<MopStatus>().unwrap(), s);
        }
        assert!("later".parse::<MopStatus>().is_err());
        assert!(MopStatus::BackedOut.finished() && !MopStatus::Running.finished());
    }

    #[test]
    fn ticks_keep_their_times() {
        let t1 = datetime!(2026-11-04 22:02 UTC);
        let t2 = datetime!(2026-11-04 22:11 UTC);
        let before = vec![("1|a".to_string(), t1)];
        let after = tick_times(&before, &["1|a".into(), "1|b".into()], t2);
        assert_eq!(after, [("1|a".to_string(), t1), ("1|b".to_string(), t2)]);
        // Unticked lines lose their time.
        assert!(tick_times(&after, &[], t2).is_empty());
        let stored = join_tick_times(&after);
        assert_eq!(parse_tick_times(&stored), after);
        assert!(parse_tick_times("junk\nalso junk").is_empty());
    }

    #[test]
    fn templates_used_lately_come_first() {
        let (a, b, c, d) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let p = Uuid::now_v7();
        let list = [
            (a, "Alpha", None),
            (b, "beta", Some(p)),
            (c, "Change", None),
            (d, "Delta", None),
        ];
        let used = HashMap::from([
            (c, datetime!(2026-11-01 10:00 UTC)),
            (d, datetime!(2026-11-03 10:00 UTC)),
        ]);
        assert_eq!(template_order(&list, &used, Some(p)), [d, c, b, a]);
        assert_eq!(template_order(&list, &HashMap::new(), None), [a, b, c, d]);
    }

    #[test]
    fn the_window_shows_on_its_day() {
        let mop = Uuid::now_v7();
        let day = date!(2026 - 11 - 04);
        let w = |status, on, at| Window {
            mop,
            ticket: "CHG0146615",
            title: "Core switch swap",
            status,
            on,
            at,
        };
        let later = window_item(&w(MopStatus::Ready, day, 22 * 60), day, 9 * 60).unwrap();
        assert_eq!(later.when, BellWhen::Today);
        assert!(later.title.starts_with("Change window at 22:00"));
        let soon = window_item(&w(MopStatus::Ready, day, 22 * 60), day, 21 * 60 + 20).unwrap();
        assert_eq!(soon.when, BellWhen::Now);
        assert!(soon.title.starts_with("Change window in 40 min"));
        let now = window_item(&w(MopStatus::Ready, day, 22 * 60), day, 22 * 60 + 5).unwrap();
        assert!(now.title.starts_with("Change window now"));
        assert!(window_item(&w(MopStatus::Done, day, 22 * 60), day, 22 * 60).is_none());
        assert!(window_item(&w(MopStatus::Ready, date!(2026 - 11 - 05), 60), day, 0).is_none());
        // A window from yesterday only shows while it's still running.
        assert!(window_item(&w(MopStatus::Ready, date!(2026 - 11 - 03), 60), day, 0).is_none());
        let run = window_item(
            &w(MopStatus::Running, date!(2026 - 11 - 03), 23 * 60),
            day,
            30,
        )
        .unwrap();
        assert!(run.title.starts_with("Change running"));
    }
}
