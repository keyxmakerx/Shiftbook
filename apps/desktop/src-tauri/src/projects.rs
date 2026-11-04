//! The Projects page (round 18): each project at a glance, with its week, its total, the last
//! day it was worked and its tickets still open. The sums are `shiftbook_core::project::glance`.

use serde::Serialize;
use shiftbook_core::hours::format_hours;
use shiftbook_core::project::glance;
use shiftbook_core::week::week_of;
use tauri::State;
use time::Date;

use crate::api::date_arg;
use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

/// One project on the page.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ProjectGlanceView {
    project_id: String,
    name: String,
    color: Option<String>,
    archived: bool,
    /// Hours this week, e.g. "4.5".
    hours_week: String,
    /// Every hour logged against it.
    hours_all: String,
    /// The last day it was worked on (`YYYY-MM-DD`).
    last_worked: Option<String>,
    /// Its tickets still open.
    open_tickets: u32,
}

/// Maps to `DataApi.projectGlances()`: every project, archived ones too (the page hides them
/// behind a switch), in the order they're listed in Settings.
#[tauri::command]
pub async fn project_glances(
    s: State<'_, VaultSession>,
    today: String,
) -> Res<Vec<ProjectGlanceView>> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    let week_start = week_of(today, store.prefs(owner).await?.week_start);
    let since = Date::from_calendar_date(2000, time::Month::January, 1)
        .map_err(|_| CommandError::InvalidInput)?;
    let entries: Vec<_> = store
        .list_entries(owner, since, today)
        .await?
        .into_iter()
        .map(|r| r.entry)
        .collect();
    let items = store.item_summaries(owner, None).await?;
    Ok(store
        .list_projects(owner)
        .await?
        .into_iter()
        .map(|p| {
            let g = glance(p.id, &entries, week_start, today);
            let open = items
                .iter()
                .filter(|i| !i.done && i.project == Some(p.id))
                .count();
            ProjectGlanceView {
                project_id: p.id.to_string(),
                name: p.name,
                color: p.color,
                archived: p.archived,
                hours_week: format_hours(g.minutes_week),
                hours_all: format_hours(g.minutes_all),
                last_worked: g.last_worked.map(shiftbook_core::dates::format_date),
                open_tickets: u32::try_from(open).unwrap_or(u32::MAX),
            }
        })
        .collect())
}
