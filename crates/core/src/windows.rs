//! Windows (round 12): a notepad or note popped out to float over Shiftbook. Each one is
//! open (floating), rolled up to its title bar, minimised to the bottom bar or full screen,
//! and remembers where it was. The list is kept as one preference
//! ([`crate::prefs::KEY_WINDOWS`]) in a compact text form, never as JSON in SQL.

use std::str::FromStr;

use serde::Serialize;
use uuid::Uuid;

use crate::canvas::CardKind;

/// Most windows kept at once; opening one more drops the oldest.
pub const MAX_WINDOWS: usize = 12;
/// Smallest floating window.
pub const MIN_WINDOW: (i32, i32) = (260, 160);
/// Largest floating window (a big screen).
pub const MAX_WINDOW: (i32, i32) = (2400, 1600);
/// How far off the visible area a window may be dragged (its title bar stays reachable).
pub const MAX_OFFSET: i32 = 4000;

/// How a window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    /// Floating where it was put.
    Open,
    /// Just the title bar.
    Rolled,
    /// Only its button in the bottom bar. Sent as "min", the word the UI and the stored
    /// setting use (it was "minimised", so a minimised window came back wrong after a restart).
    #[serde(rename = "min")]
    Minimised,
    /// Fills the space above the bottom bar.
    Full,
}

impl WindowMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            WindowMode::Open => "open",
            WindowMode::Rolled => "rolled",
            WindowMode::Minimised => "min",
            WindowMode::Full => "full",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "open" => Some(WindowMode::Open),
            "rolled" => Some(WindowMode::Rolled),
            "min" => Some(WindowMode::Minimised),
            "full" => Some(WindowMode::Full),
            _ => None,
        }
    }
}

/// One window: what it shows, how and where (pixels, top left of the work area).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct WindowSpot {
    /// The record it shows.
    pub kind: CardKind,
    /// Its id.
    pub id: Uuid,
    /// How it shows.
    pub mode: WindowMode,
    /// Left.
    pub x: i32,
    /// Top.
    pub y: i32,
    /// Width when floating.
    pub w: i32,
    /// Height when floating.
    pub h: i32,
}

impl WindowSpot {
    /// The same window kept within sensible bounds.
    pub fn clean(&self) -> Self {
        Self {
            x: self.x.clamp(-MAX_OFFSET, MAX_OFFSET),
            y: self.y.clamp(0, MAX_OFFSET),
            w: self.w.clamp(MIN_WINDOW.0, MAX_WINDOW.0),
            h: self.h.clamp(MIN_WINDOW.1, MAX_WINDOW.1),
            ..self.clone()
        }
    }
}

/// Read the stored list: `kind:id mode x y w h` per window, `;` between windows. Broken
/// parts are skipped, repeats of a record keep the first, and at most [`MAX_WINDOWS`].
pub fn parse_windows(text: &str) -> Vec<WindowSpot> {
    let mut out: Vec<WindowSpot> = Vec::new();
    for part in text.split(';') {
        let Some(spot) = parse_one(part) else {
            continue;
        };
        if out.len() < MAX_WINDOWS && !out.iter().any(|w| w.kind == spot.kind && w.id == spot.id) {
            out.push(spot.clean());
        }
    }
    out
}

fn parse_one(part: &str) -> Option<WindowSpot> {
    let mut bits = part.split_whitespace();
    let (kind, id) = bits.next()?.split_once(':')?;
    let kind = CardKind::from_str(kind).ok()?;
    let id = Uuid::parse_str(id).ok()?;
    let mode = WindowMode::parse(bits.next()?)?;
    let mut n = || bits.next()?.parse::<i32>().ok();
    let (x, y, w, h) = (n()?, n()?, n()?, n()?);
    Some(WindowSpot {
        kind,
        id,
        mode,
        x,
        y,
        w,
        h,
    })
}

/// Write the list in the stored form (cleaned, at most [`MAX_WINDOWS`]).
pub fn format_windows(windows: &[WindowSpot]) -> String {
    windows
        .iter()
        .take(MAX_WINDOWS)
        .map(|w| {
            let w = w.clean();
            format!(
                "{}:{} {} {} {} {} {}",
                w.kind.as_str(),
                w.id,
                w.mode.as_str(),
                w.x,
                w.y,
                w.w,
                w.h
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_round_trip_and_stay_in_bounds() {
        let id = Uuid::now_v7();
        let spot = WindowSpot {
            kind: CardKind::Entry,
            id,
            mode: WindowMode::Rolled,
            x: 120,
            y: 80,
            w: 420,
            h: 330,
        };
        let text = format_windows(std::slice::from_ref(&spot));
        assert_eq!(text, format!("entry:{id} rolled 120 80 420 330"));
        assert_eq!(parse_windows(&text), vec![spot.clone()]);

        let tiny = parse_windows(&format!("note:{id} full -9999 -5 10 99999"));
        assert_eq!(
            (tiny[0].x, tiny[0].y, tiny[0].w, tiny[0].h),
            (-MAX_OFFSET, 0, MIN_WINDOW.0, MAX_WINDOW.1)
        );
    }

    #[test]
    fn broken_parts_and_repeats_are_skipped() {
        let id = Uuid::now_v7();
        let text = format!(
            "entry:{id} open 1 2 300 200; nonsense; task:{id} open 1 2 3 4; \
             entry:{id} min 5 5 300 200; tray:{id} sideways 1 1 300 200; tray:{id} min 0 0 300 200"
        );
        let list = parse_windows(&text);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].mode, WindowMode::Open);
        assert_eq!(list[1].kind, CardKind::Tray);
        let many: String = (0..20)
            .map(|_| format!("note:{} open 0 0 300 200", Uuid::now_v7()))
            .collect::<Vec<_>>()
            .join(";");
        assert_eq!(parse_windows(&many).len(), MAX_WINDOWS);
        assert!(parse_windows("").is_empty());
    }

    #[test]
    fn a_minimised_window_goes_out_as_min() {
        // The word the UI and the stored setting use; "minimised" came back wrong.
        assert_eq!(
            serde_json::to_value(WindowMode::Minimised).unwrap(),
            serde_json::json!("min")
        );
    }
}
