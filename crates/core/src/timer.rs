//! The optional "Now doing" timer (off unless turned on in Settings → Your day).
//!
//! Only minutes are ever saved: the timer counts seconds while it runs, and on Stop
//! offers to add them to the entry, rounded to [`ROUND_TO_MINUTES`]. Quiet stretches of
//! [`AWAY_AFTER_SECONDS`] or more while it ran are offered to be left out. When the app
//! locks, the running count is kept as a paused timer (an entry and counts of seconds,
//! never a start or end time).

use serde::Serialize;

use crate::ids::TimeEntryId;
use crate::minutes::Minutes;

/// The timer rounds to this many minutes (matches the 15m buttons).
pub const ROUND_TO_MINUTES: u64 = 15;
/// A quiet stretch at least this long while the timer ran counts as time away.
pub const AWAY_AFTER_SECONDS: u64 = 5 * 60;
/// Longest a timer can count (a whole day).
pub const MAX_TIMER_SECONDS: u64 = 24 * 60 * 60;

/// Round counted seconds to the nearest quarter hour. Under a minute is nothing; any
/// minute or more is at least one quarter hour.
pub fn round_timer(seconds: u64) -> Minutes {
    let seconds = seconds.min(MAX_TIMER_SECONDS);
    if seconds < 60 {
        return Minutes(0);
    }
    let step = ROUND_TO_MINUTES * 60;
    let quarters = ((seconds + step / 2) / step).max(1);
    Minutes(u32::try_from(quarters * ROUND_TO_MINUTES).unwrap_or(u32::MAX))
}

/// What Stop offers to add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerOffer {
    /// All the counted time, rounded.
    pub minutes: Minutes,
    /// The time without the stretches away, rounded; only when that makes a difference.
    pub without_away: Option<Minutes>,
    /// Time away, in whole minutes.
    pub away_minutes: Minutes,
}

/// The offer for `seconds` counted, given the quiet stretches (in seconds) noticed while
/// it ran. Only stretches of [`AWAY_AFTER_SECONDS`] or more count as away.
pub fn timer_offer(seconds: u64, quiet: &[u64]) -> TimerOffer {
    let seconds = seconds.min(MAX_TIMER_SECONDS);
    let away = quiet
        .iter()
        .filter(|q| **q >= AWAY_AFTER_SECONDS)
        .sum::<u64>()
        .min(seconds);
    let minutes = round_timer(seconds);
    let without = round_timer(seconds - away);
    TimerOffer {
        minutes,
        without_away: (away > 0 && without != minutes).then_some(without),
        away_minutes: Minutes(u32::try_from(away / 60).unwrap_or(0)),
    }
}

/// Phone (round 15): Shiftbook was in the background for `seconds` while the timer ran.
/// A short time away is simply counted; from [`AWAY_AFTER_SECONDS`] on, the timer asks
/// whether to count it, and this is the whole minutes to ask about.
pub fn away_question(seconds: u64) -> Option<Minutes> {
    let seconds = seconds.min(MAX_TIMER_SECONDS);
    (seconds >= AWAY_AFTER_SECONDS).then(|| Minutes(u32::try_from(seconds / 60).unwrap_or(0)))
}

/// A timer paused by locking: which entry, and what it had counted so far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PausedTimer {
    /// The entry the time goes to.
    pub entry_id: TimeEntryId,
    /// Seconds counted.
    pub seconds: u64,
    /// The quiet stretches noticed, in seconds.
    pub quiet: Vec<u64>,
}

/// Most quiet stretches kept with a paused timer.
pub const MAX_QUIET_STRETCHES: usize = 100;

impl PausedTimer {
    /// Read the stored form `entry-id seconds quiet,quiet,…` (the list may be empty).
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split_whitespace();
        let entry_id = parts.next()?.parse().ok()?;
        let seconds = parts.next()?.parse::<u64>().ok()?;
        let quiet = match parts.next() {
            None => Vec::new(),
            Some(list) => list
                .split(',')
                .map(|q| q.parse::<u64>().ok().filter(|q| *q <= MAX_TIMER_SECONDS))
                .collect::<Option<Vec<_>>>()?,
        };
        let ok = parts.next().is_none()
            && seconds <= MAX_TIMER_SECONDS
            && quiet.len() <= MAX_QUIET_STRETCHES;
        ok.then_some(Self {
            entry_id,
            seconds,
            quiet,
        })
    }

    /// The stored form.
    pub fn format(&self) -> String {
        let quiet: Vec<String> = self.quiet.iter().map(u64::to_string).collect();
        format!("{} {} {}", self.entry_id, self.seconds, quiet.join(","))
            .trim_end()
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60;

    #[test]
    fn away_asks_only_when_long() {
        assert_eq!(away_question(0), None);
        assert_eq!(away_question(4 * MIN + 59), None);
        assert_eq!(away_question(5 * MIN), Some(Minutes(5)));
        assert_eq!(away_question(25 * MIN + 30), Some(Minutes(25)));
        assert_eq!(away_question(u64::MAX), Some(Minutes(24 * 60)));
    }

    #[test]
    fn rounds_to_the_nearest_quarter_hour() {
        assert_eq!(round_timer(0), Minutes(0));
        assert_eq!(round_timer(59), Minutes(0));
        assert_eq!(round_timer(60), Minutes(15));
        assert_eq!(round_timer(22 * MIN), Minutes(15));
        assert_eq!(round_timer(23 * MIN), Minutes(30));
        assert_eq!(round_timer(67 * MIN), Minutes(60));
        assert_eq!(round_timer(68 * MIN), Minutes(75));
        assert_eq!(round_timer(40 * 60 * MIN), Minutes(1440));
    }

    #[test]
    fn offers_to_leave_out_time_away() {
        let offer = timer_offer(105 * MIN, &[2 * MIN, 30 * MIN]);
        assert_eq!(offer.minutes, Minutes(105));
        assert_eq!(offer.without_away, Some(Minutes(75)));
        assert_eq!(offer.away_minutes, Minutes(30));

        // Short pauses don't count, and away that rounds the same isn't offered.
        let offer = timer_offer(60 * MIN, &[4 * MIN, 4 * MIN]);
        assert_eq!(offer.without_away, None);
        assert_eq!(offer.away_minutes, Minutes(0));
        let offer = timer_offer(60 * MIN, &[6 * MIN]);
        assert_eq!(offer.minutes, Minutes(60));
        assert_eq!(offer.without_away, None);
        assert_eq!(offer.away_minutes, Minutes(6));
    }

    #[test]
    fn away_never_exceeds_the_time_counted() {
        let offer = timer_offer(20 * MIN, &[50 * MIN]);
        assert_eq!(offer.minutes, Minutes(15));
        assert_eq!(offer.without_away, Some(Minutes(0)));
    }

    #[test]
    fn paused_timer_round_trips_and_refuses_junk() {
        let id = TimeEntryId::new();
        let paused = PausedTimer {
            entry_id: id,
            seconds: 3700,
            quiet: vec![400, 90],
        };
        assert_eq!(PausedTimer::parse(&paused.format()), Some(paused));
        let bare = PausedTimer {
            entry_id: id,
            seconds: 12,
            quiet: vec![],
        };
        assert_eq!(bare.format(), format!("{id} 12"));
        assert_eq!(PausedTimer::parse(&bare.format()), Some(bare));
        assert_eq!(PausedTimer::parse(""), None);
        assert_eq!(PausedTimer::parse("not-an-id 12"), None);
        assert_eq!(PausedTimer::parse(&format!("{id} 999999")), None);
        assert_eq!(PausedTimer::parse(&format!("{id} 12 a,b")), None);
        assert_eq!(PausedTimer::parse(&format!("{id} 12 1 extra")), None);
    }
}
