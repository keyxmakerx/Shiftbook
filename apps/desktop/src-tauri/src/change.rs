//! Commands for MOPs on their change ticket (round 18, see `shiftbook_core::change`):
//! templates to pick from, attaching a MOP, its status, change window and ticks.

use serde::Serialize;
use shiftbook_app::views::{note_view, NoteView};
use shiftbook_core::change::{template_order, MopStatus};
use shiftbook_core::dates::{format_date, format_timestamp, parse_date};
use shiftbook_core::ids::{NoteId, OwnerId};
use shiftbook_core::notes::NoteKind;
use shiftbook_storage::change::{MopFrom, MopRun};
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::OffsetDateTime;

use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

fn id_arg<T: std::str::FromStr>(text: &str) -> Res<T> {
    text.parse().map_err(|_| CommandError::InvalidInput)
}

/// A template offered for a ticket.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MopTemplateView {
    id: String,
    title: String,
    /// The first line under its title, to tell templates apart.
    hint: String,
}

/// When one line was ticked.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TickTimeView {
    key: String,
    /// UTC, RFC 3339.
    at: String,
}

/// A MOP on a ticket.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MopRunView {
    mop_id: String,
    title: String,
    ticket: String,
    template_id: Option<String>,
    status: MopStatus,
    /// The change window's day, `YYYY-MM-DD`.
    window_on: Option<String>,
    /// Its start, minutes after local midnight.
    window_at: Option<u32>,
    tick_times: Vec<TickTimeView>,
    /// When it first went Running (UTC, RFC 3339).
    started_at: Option<String>,
}

fn run_view(r: MopRun) -> MopRunView {
    MopRunView {
        mop_id: r.mop.to_string(),
        title: r.title,
        ticket: r.ticket,
        template_id: r.template.map(|t| t.to_string()),
        status: r.status,
        window_on: r.window.map(|(on, _)| format_date(on)),
        window_at: r.window.map(|(_, at)| at),
        tick_times: r
            .tick_times
            .into_iter()
            .map(|(key, at)| TickTimeView {
                key,
                at: format_timestamp(at),
            })
            .collect(),
        started_at: r.started_at.map(format_timestamp),
    }
}

/// Maps to `DataApi.mopTemplates()`: the MOP templates to pick from for `ticket`, the ones
/// used most recently and the ticket's project's first.
#[tauri::command]
pub async fn mop_templates(
    s: State<'_, VaultSession>,
    ticket: String,
) -> Res<Vec<MopTemplateView>> {
    let (store, owner) = s.data().await?;
    let templates: Vec<_> = store
        .list_notes(owner, false)
        .await?
        .into_iter()
        .filter(|n| n.kind == NoteKind::Mop && n.template)
        .collect();
    let project = store
        .item_summaries(owner, Some(&[ticket]))
        .await?
        .into_iter()
        .next()
        .and_then(|i| i.project)
        .map(|p| *p.as_uuid());
    let offered: Vec<_> = templates
        .iter()
        .map(|n| {
            (
                *n.id.as_uuid(),
                n.title.as_str(),
                n.project_id.map(|p| *p.as_uuid()),
            )
        })
        .collect();
    let order = template_order(&offered, &store.templates_last_used(owner).await?, project);
    Ok(order
        .into_iter()
        .filter_map(|id| templates.iter().find(|n| *n.id.as_uuid() == id))
        .map(|n| MopTemplateView {
            id: n.id.to_string(),
            title: n.title.clone(),
            hint: n
                .body
                .lines()
                .map(|l| l.trim_start_matches('#').trim())
                .find(|l| !l.is_empty())
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
        })
        .collect())
}

/// Maps to `DataApi.attachMop()`: make a MOP for `ticket` from a template (with what's typed
/// into its blanks), from a wiki page (copied), or blank. Returns the MOP.
#[tauri::command]
pub async fn attach_mop(
    s: State<'_, VaultSession>,
    ticket: String,
    template_id: Option<String>,
    page_id: Option<String>,
    values: Vec<(String, String)>,
    title: String,
) -> Res<NoteView> {
    let (store, owner) = s.data().await?;
    let from = match (template_id, page_id) {
        (Some(t), _) => MopFrom::Template {
            id: id_arg(&t)?,
            values,
            title,
        },
        (None, Some(p)) => MopFrom::Page(id_arg(&p)?),
        (None, None) => MopFrom::Blank(title),
    };
    let note = store.attach_mop(owner, &ticket, from).await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    Ok(note_view(note, OffsetDateTime::now_utc(), after))
}

/// Maps to `DataApi.mopRuns()`: MOPs on tickets, newest first (only `ticket`'s when given).
#[tauri::command]
pub async fn mop_runs(s: State<'_, VaultSession>, ticket: Option<String>) -> Res<Vec<MopRunView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .mop_runs(owner, ticket.as_deref())
        .await?
        .into_iter()
        .map(run_view)
        .collect())
}

/// Maps to `DataApi.mopRun()`: one MOP's run, or null for a note that isn't on a ticket.
#[tauri::command]
pub async fn mop_run(s: State<'_, VaultSession>, id: String) -> Res<Option<MopRunView>> {
    let (store, owner) = s.data().await?;
    Ok(store.mop_run(owner, id_arg(&id)?).await?.map(run_view))
}

/// Maps to `DataApi.mopLink()`: put a MOP written on its own on `ticket` (made if new).
#[tauri::command]
pub async fn mop_link(s: State<'_, VaultSession>, id: String, ticket: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.link_mop(owner, id_arg(&id)?, &ticket).await?)
}

/// Maps to `DataApi.mopSetStatus()`.
#[tauri::command]
pub async fn mop_set_status(s: State<'_, VaultSession>, id: String, status: String) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_mop_status(owner, id_arg(&id)?, id_arg(&status)?)
        .await?)
}

/// Maps to `DataApi.mopSetWindow()`: the change window's day and start, or null to clear.
#[tauri::command]
pub async fn mop_set_window(
    s: State<'_, VaultSession>,
    id: String,
    on: Option<String>,
    at: Option<u32>,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let window = match (on, at) {
        (Some(on), Some(at)) => {
            Some((parse_date(&on).map_err(|_| CommandError::InvalidInput)?, at))
        }
        _ => None,
    };
    Ok(store.set_mop_window(owner, id_arg(&id)?, window).await?)
}

/// Maps to `DataApi.mopTick()`: tick lines off, keeping when each was ticked.
#[tauri::command]
pub async fn mop_tick(s: State<'_, VaultSession>, id: String, ticks: Vec<String>) -> Res<NoteView> {
    let (store, owner) = s.data().await?;
    let note = store
        .tick_mop(owner, id_arg::<NoteId>(&id)?, &ticks)
        .await?;
    let after = store.prefs(owner).await?.notes_auto_archive_days;
    Ok(note_view(note, OffsetDateTime::now_utc(), after))
}

/// The bell's lines for change windows (see `shiftbook_core::change::window_item`).
pub(crate) async fn window_items(
    store: &SqliteStore,
    owner: OwnerId,
    today: time::Date,
    now: u32,
) -> Res<Vec<shiftbook_core::bell::BellItem>> {
    Ok(store
        .mop_runs(owner, None)
        .await?
        .iter()
        .filter_map(|r| {
            let (on, at) = r.window?;
            shiftbook_core::change::window_item(
                &shiftbook_core::change::Window {
                    mop: r.mop,
                    ticket: &r.ticket,
                    title: &r.title,
                    status: r.status,
                    on,
                    at,
                },
                today,
                now,
            )
        })
        .collect())
}
