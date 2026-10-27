//! The time ribbon: the last few weeks as days of coloured hours.
//!
//! The ribbon shows whole weeks ending with the week that contains today. Each
//! day has its total and its minutes per project (projects in creation order,
//! "No project" last); each week has its total. Days after today are marked as
//! future. Only live entries count. A past or present day with a daily target but
//! nothing logged is a **gap**, marked with a dot so missed days stand out.

use std::collections::BTreeMap;

use serde::Serialize;
use time::Date;

use crate::dates::{add_days, iso_date};
use crate::hours::format_hours;
use crate::ids::ProjectId;
use crate::minutes::Minutes;
use crate::model::{EntryTarget, TimeEntry};
use crate::week::{week_days, week_of, WeekStart};

/// How many weeks the ribbon shows.
pub const RIBBON_WEEKS: u32 = 4;

/// Minutes on one project on one day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct DayPart {
    /// The project (`None` = No project).
    pub project_id: Option<ProjectId>,
    /// Minutes.
    pub minutes: Minutes,
}

/// One day in the ribbon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RibbonDay {
    /// The date.
    #[serde(with = "iso_date")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub date: Date,
    /// Total minutes.
    pub minutes: Minutes,
    /// Total as decimal hours.
    pub hours: String,
    /// Minutes per project.
    pub parts: Vec<DayPart>,
    /// After today.
    pub future: bool,
    /// Nothing logged on a day that has a target (see [`mark_gaps`]).
    pub gap: bool,
}

/// One week in the ribbon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct RibbonWeek {
    /// First day.
    #[serde(with = "iso_date")]
    #[cfg_attr(feature = "ts", ts(type = "string"))]
    pub week_start: Date,
    /// Total minutes.
    pub minutes: Minutes,
    /// Total as decimal hours.
    pub hours: String,
    /// Seven days.
    pub days: Vec<RibbonDay>,
}

/// First and last date the ribbon covers.
pub fn ribbon_range(today: Date, start: WeekStart, weeks: u32) -> (Date, Date) {
    let this_week = week_of(today, start);
    let first = add_days(this_week, -7 * i64::from(weeks.max(1) - 1));
    (first, add_days(this_week, 6))
}

/// Build the ribbon from the entries in [`ribbon_range`].
pub fn build_ribbon(
    today: Date,
    start: WeekStart,
    weeks: u32,
    entries: &[TimeEntry],
) -> Vec<RibbonWeek> {
    let (first, _) = ribbon_range(today, start, weeks);
    let mut per_day: BTreeMap<Date, BTreeMap<(u8, Option<ProjectId>), Minutes>> = BTreeMap::new();
    for e in entries.iter().filter(|e| e.is_live()) {
        let project = match e.target {
            EntryTarget::Project(id) => Some(id),
            _ => None,
        };
        let rank = u8::from(project.is_none());
        let slot = per_day
            .entry(e.date)
            .or_default()
            .entry((rank, project))
            .or_default();
        *slot = *slot + e.minutes;
    }
    (0..weeks.max(1))
        .map(|w| {
            let week_start = add_days(first, 7 * i64::from(w));
            let days: Vec<RibbonDay> = week_days(week_start)
                .into_iter()
                .map(|date| {
                    let parts: Vec<DayPart> = per_day
                        .get(&date)
                        .map(|m| {
                            m.iter()
                                .filter(|(_, minutes)| !minutes.is_zero())
                                .map(|((_, project_id), minutes)| DayPart {
                                    project_id: *project_id,
                                    minutes: *minutes,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let minutes: Minutes = parts.iter().map(|p| p.minutes).sum();
                    RibbonDay {
                        date,
                        minutes,
                        hours: format_hours(minutes),
                        parts,
                        future: date > today,
                        gap: false,
                    }
                })
                .collect();
            let minutes: Minutes = days.iter().map(|d| d.minutes).sum();
            RibbonWeek {
                week_start,
                minutes,
                hours: format_hours(minutes),
                days,
            }
        })
        .collect()
}

/// Mark gaps: days up to today with nothing logged whose weekday has a target
/// (`targets` in minutes, Monday first). Today counts, so an empty Desk shows it.
pub fn mark_gaps(weeks: &mut [RibbonWeek], targets: &[u32; 7]) {
    for day in weeks.iter_mut().flat_map(|w| w.days.iter_mut()) {
        let target = targets[usize::from(day.date.weekday().number_days_from_monday())];
        day.gap = !day.future && day.minutes.is_zero() && target > 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{OwnerId, TimeEntryId};
    use time::macros::{date, datetime};

    fn entry(date: Date, minutes: u32, project: Option<ProjectId>) -> TimeEntry {
        TimeEntry {
            id: TimeEntryId::new(),
            owner_id: OwnerId::new(),
            date,
            minutes: Minutes(minutes),
            target: project.map_or(EntryTarget::None, EntryTarget::Project),
            category: None,
            description: String::new(),
            status: None,
            priority: None,
            next_step: None,
            timesheet_code_id: None,
            legacy_ticket_text: None,
            created_at: datetime!(2026-10-12 09:00 UTC),
            updated_at: datetime!(2026-10-12 09:00 UTC),
            deleted_at: None,
        }
    }

    #[test]
    fn range_is_whole_weeks_ending_this_week() {
        assert_eq!(
            ribbon_range(date!(2026 - 10 - 15), WeekStart::default(), 4),
            (date!(2026 - 09 - 21), date!(2026 - 10 - 18))
        );
        assert_eq!(
            ribbon_range(date!(2026 - 10 - 15), WeekStart::default(), 0),
            (date!(2026 - 10 - 12), date!(2026 - 10 - 18))
        );
    }

    #[test]
    fn days_weeks_and_parts() {
        let a = ProjectId::new();
        let b = ProjectId::new();
        let mut gone = entry(date!(2026 - 10 - 14), 600, Some(a));
        gone.deleted_at = Some(datetime!(2026-10-14 10:00 UTC));
        let entries = vec![
            entry(date!(2026 - 10 - 14), 60, None),
            entry(date!(2026 - 10 - 14), 90, Some(b)),
            entry(date!(2026 - 10 - 14), 30, Some(a)),
            entry(date!(2026 - 10 - 14), 30, Some(a)),
            entry(date!(2026 - 10 - 05), 120, Some(a)),
            gone,
        ];
        let weeks = build_ribbon(date!(2026 - 10 - 15), WeekStart::default(), 4, &entries);
        assert_eq!(weeks.len(), 4);
        assert_eq!(weeks[0].week_start, date!(2026 - 09 - 21));
        assert_eq!(weeks[3].week_start, date!(2026 - 10 - 12));
        assert!(weeks.iter().all(|w| w.days.len() == 7));

        let wed = &weeks[3].days[2];
        assert_eq!(wed.date, date!(2026 - 10 - 14));
        assert_eq!(wed.minutes, Minutes(210));
        assert_eq!(wed.hours, "3.5");
        let parts: Vec<(Option<ProjectId>, u32)> = wed
            .parts
            .iter()
            .map(|p| (p.project_id, p.minutes.get()))
            .collect();
        assert_eq!(parts, vec![(Some(a), 60), (Some(b), 90), (None, 60)]);
        assert!(!wed.future);
        assert!(!weeks[3].days[3].future, "today is not future");
        assert!(weeks[3].days[4].future);
        assert_eq!(weeks[3].minutes, Minutes(210));
        assert_eq!(weeks[2].minutes, Minutes(120));
        assert_eq!(weeks[2].days[0].parts.len(), 1);
        assert_eq!(weeks[0].minutes, Minutes::ZERO);
    }

    #[test]
    fn gaps_are_empty_working_days_up_to_today() {
        let entries = vec![entry(date!(2026 - 10 - 13), 60, None)];
        let mut weeks = build_ribbon(date!(2026 - 10 - 15), WeekStart::default(), 1, &entries);
        assert!(weeks[0].days.iter().all(|d| !d.gap));
        mark_gaps(&mut weeks, &[480, 480, 480, 480, 0, 0, 0]);
        let gaps: Vec<bool> = weeks[0].days.iter().map(|d| d.gap).collect();
        // Mon empty, Tue logged, Wed empty, Thu (today) empty, Fri future, weekend no target.
        assert_eq!(gaps, vec![true, false, true, true, false, false, false]);
        let json = serde_json::to_value(&weeks[0].days[0]).unwrap();
        assert_eq!(json["gap"], true);
    }
}
