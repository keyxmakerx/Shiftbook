//! The timesheet reminder (round 15): from midday on the last work day of the week, the bell
//! says "This week's timesheet isn't entered yet" until every day of the crib sheet is
//! ticked off (stamped ENTERED on the Timesheet).

use time::Date;

use crate::sheet::WeekSheet;
use crate::week::week_days;

/// The reminder starts at midday, in minutes after local midnight.
pub const REMIND_FROM: u32 = 12 * 60;

/// The last day of the week starting `week_start` that is a work day, if any is.
pub fn last_work_day(week_start: Date, is_work_day: impl Fn(Date) -> bool) -> Option<Date> {
    week_days(week_start)
        .into_iter()
        .rev()
        .find(|d| is_work_day(*d))
}

/// True when every day of `sheet` that has lines has all of them ticked. A week with
/// nothing to enter counts as entered.
pub fn week_entered(sheet: &WeekSheet, ticked: impl Fn(Date, &str) -> bool) -> bool {
    sheet
        .days
        .iter()
        .all(|d| d.lines.iter().all(|l| ticked(d.date, &l.key)))
}

/// Entries in the week with no timesheet code yet.
pub fn uncoded_entries(sheet: &WeekSheet) -> usize {
    sheet
        .days
        .iter()
        .flat_map(|d| d.lines.iter().filter(|l| l.unassigned))
        .flat_map(|l| l.items.iter())
        .map(|i| i.entry_ids.len())
        .sum()
}

/// True when the bell should remind: the week isn't entered and it is the last work day
/// from [`REMIND_FROM`], or later in the same week.
pub fn timesheet_due(today: Date, now: u32, last_work_day: Option<Date>, entered: bool) -> bool {
    let Some(last) = last_work_day else {
        return false;
    };
    !entered && (today > last || (today == last && now >= REMIND_FROM))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TimeEntryId;
    use crate::minutes::Minutes;
    use crate::sheet::{GroupBy, SheetDay, SheetItem, SheetLine};
    use time::macros::date;

    const MON: Date = date!(2026 - 10 - 19);
    const FRI: Date = date!(2026 - 10 - 23);

    fn line(key: &str, unassigned: bool, entries: usize) -> SheetLine {
        SheetLine {
            key: key.into(),
            area: String::new(),
            label: key.into(),
            minutes: Minutes(60),
            hours: "1".into(),
            comment: String::new(),
            items: vec![SheetItem {
                title: "work".into(),
                tickets: Vec::new(),
                entry_ids: (0..entries).map(|_| TimeEntryId::new()).collect(),
                minutes: Minutes(60),
            }],
            unassigned,
            project_id: None,
            rollup_ids: Vec::new(),
        }
    }

    fn sheet(days: Vec<(Date, Vec<SheetLine>)>) -> WeekSheet {
        WeekSheet {
            week_start: MON,
            group_by: GroupBy::default(),
            days: days
                .into_iter()
                .map(|(date, lines)| SheetDay {
                    date,
                    minutes: Minutes(60),
                    hours: "1".into(),
                    lines,
                })
                .collect(),
            effort: Vec::new(),
            minutes: Minutes(0),
            hours: "0".into(),
            ticket_count: 0,
            entry_count: 0,
            warnings: Vec::new(),
            tsv: String::new(),
        }
    }

    #[test]
    fn the_last_work_day_follows_the_targets() {
        let weekdays = |d: Date| d.weekday().number_days_from_monday() < 5;
        assert_eq!(last_work_day(MON, weekdays), Some(FRI));
        let four_days = |d: Date| d.weekday().number_days_from_monday() < 4;
        assert_eq!(last_work_day(MON, four_days), Some(date!(2026 - 10 - 22)));
        assert_eq!(last_work_day(MON, |_| false), None);
    }

    #[test]
    fn entered_when_every_line_is_ticked() {
        let s = sheet(vec![
            (
                MON,
                vec![line("ops", false, 1), line("unassigned", true, 2)],
            ),
            (FRI, vec![]),
        ]);
        assert!(!week_entered(&s, |_, k| k == "ops"));
        assert!(week_entered(&s, |_, _| true));
        assert!(week_entered(&sheet(vec![(MON, vec![])]), |_, _| false));
        assert_eq!(uncoded_entries(&s), 2);
    }

    #[test]
    fn due_from_midday_on_the_last_work_day() {
        let last = Some(FRI);
        assert!(!timesheet_due(date!(2026 - 10 - 22), 16 * 60, last, false));
        assert!(!timesheet_due(FRI, REMIND_FROM - 1, last, false));
        assert!(timesheet_due(FRI, REMIND_FROM, last, false));
        assert!(timesheet_due(date!(2026 - 10 - 24), 9 * 60, last, false));
        assert!(!timesheet_due(FRI, 16 * 60, last, true));
        assert!(!timesheet_due(FRI, 16 * 60, None, false));
    }
}
