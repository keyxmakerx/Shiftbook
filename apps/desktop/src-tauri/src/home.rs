//! Commands for round 15's home pieces: repeating things (with Mark done and Renewed),
//! reusable lists, Work / Home / Both, and "While you were away". Thin: the rules live in
//! `shiftbook_core::recurring`, `shiftbook_core::task`, `shiftbook_core::home` and
//! `shiftbook_core::bell`.

use serde::{Deserialize, Serialize};
use shiftbook_core::bell::{while_away as away_items, BellItem, DueThing};
use shiftbook_core::dates::{format_date, parse_date};
use shiftbook_core::home::UseMode;
use shiftbook_core::ids::{
    ChecklistItemId, FrequencyId, OwnerId, RecurringTaskId, TaskId, TimeEntryId, TimesheetCodeId,
};
use shiftbook_core::model::{Frequency, ListName};
use shiftbook_core::prefs::Prefs;
use shiftbook_core::recurring::{
    attention_order, due_words, last_done_for_next, summarize_recurring, Interval, RecurringState,
};
use shiftbook_core::task::list_progress;
use shiftbook_storage::lists::ListRecord;
use shiftbook_storage::recurring::{MarkedDone, RecurringInput};
use shiftbook_storage::repo::{FrequencyRepository, ListOptionRepository};
use shiftbook_storage::SqliteStore;
use tauri::State;
use time::Date;

use crate::session::{CommandError, VaultSession};

type Res<T> = Result<T, CommandError>;

fn date_arg(text: &str) -> Res<Date> {
    parse_date(text).map_err(|_| CommandError::InvalidInput)
}

fn id_arg<T: std::str::FromStr>(text: &str) -> Res<T> {
    text.parse().map_err(|_| CommandError::InvalidInput)
}

// ---------- repeating things ----------

/// One repeating thing with everything worked out.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringView {
    id: RecurringTaskId,
    name: String,
    kind: Option<String>,
    frequency_id: Option<FrequencyId>,
    /// "Every 6 months", or `None`.
    every: Option<String>,
    /// True for things that expire on a date (insurance, passport); false for things that
    /// come round again after they're done (bins, filters).
    expires: bool,
    /// The expiry date, for things that expire.
    expiration_date: Option<String>,
    next_due: Option<String>,
    days_left: Option<i64>,
    /// "tomorrow", "3 days late", …
    due_words: Option<String>,
    state: RecurringState,
    last_done: Option<String>,
    warn_days: u32,
    alerts: bool,
    notes: String,
    total_minutes: u32,
    /// The timesheet code its time gets, when set on the thing itself.
    code_id: Option<TimesheetCodeId>,
}

/// What the UI sends to save one.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringArgs {
    name: String,
    kind: Option<String>,
    frequency_id: Option<String>,
    /// True: `date` is the expiry date. False: `date` is when it next comes round.
    expires: bool,
    date: Option<String>,
    warn_days: Option<u32>,
    alerts: bool,
    notes: String,
    /// Its own timesheet code (`None` = its kind's code, then the usual rules).
    #[serde(default)]
    code_id: Option<String>,
}

/// Every repeating thing with its frequency, summary and the frequency list.
async fn summaries(store: &SqliteStore, owner: OwnerId, today: Date) -> Res<Vec<RecurringView>> {
    let tasks = store.recurring_all(owner).await?;
    let entries = store.recurring_entries(owner).await?;
    let frequencies = store.list_frequencies(owner).await?;
    let find = |id: Option<FrequencyId>| -> Option<&Frequency> {
        id.and_then(|id| frequencies.iter().find(|f| f.id == id))
    };
    let mut views: Vec<RecurringView> = tasks
        .into_iter()
        .map(|t| {
            let f = find(t.frequency_id);
            let sum = summarize_recurring(&t, f, &entries, today);
            RecurringView {
                id: t.id,
                name: t.name,
                kind: t.recurring_type,
                frequency_id: t.frequency_id,
                every: f.map(|f| f.label.clone()),
                expires: t.expiration_date.is_some(),
                expiration_date: t.expiration_date.map(format_date),
                next_due: sum.next_due.map(format_date),
                days_left: sum.days_left,
                due_words: sum.days_left.map(due_words),
                state: sum.state,
                last_done: sum.last_done.map(format_date),
                warn_days: sum.warn_days,
                alerts: t.alerts_enabled,
                notes: t.notes,
                total_minutes: sum.total_minutes.get(),
                code_id: t.default_timesheet_code_id,
            }
        })
        .collect();
    views.sort_by(|a, b| {
        (attention_order(a.state), a.days_left.unwrap_or(i64::MAX))
            .cmp(&(attention_order(b.state), b.days_left.unwrap_or(i64::MAX)))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(views)
}

/// The repeating things the bell shows (see `shiftbook_core::bell::with_due`).
pub async fn due_things(store: &SqliteStore, owner: OwnerId, today: Date) -> Res<Vec<DueThing>> {
    Ok(summaries(store, owner, today)
        .await?
        .into_iter()
        .filter_map(|v| {
            let next = v.next_due.as_deref().and_then(|d| parse_date(d).ok())?;
            Some((v.name, v.id.to_string(), v.state, next, v.warn_days))
        })
        .collect())
}

/// Maps to `DataApi.recurring()`.
#[tauri::command]
pub async fn recurring_list(s: State<'_, VaultSession>, today: String) -> Res<Vec<RecurringView>> {
    let (store, owner) = s.data().await?;
    summaries(&store, owner, date_arg(&today)?).await
}

/// Maps to `DataApi.saveRecurring()`: add one (`id` null) or change one. For a thing that
/// repeats, the date is when it next comes round, and needs a frequency.
#[tauri::command]
pub async fn recurring_save(
    s: State<'_, VaultSession>,
    id: Option<String>,
    input: RecurringArgs,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    let frequency_id: Option<FrequencyId> = input
        .frequency_id
        .as_deref()
        .filter(|f| !f.is_empty())
        .map(id_arg)
        .transpose()?;
    let date = input
        .date
        .as_deref()
        .filter(|d| !d.is_empty())
        .map(date_arg)
        .transpose()?;
    let (expiration_date, last_done) = if input.expires {
        (Some(date.ok_or(CommandError::InvalidInput)?), None)
    } else {
        let interval = match frequency_id {
            Some(fid) => store
                .list_frequencies(owner)
                .await?
                .into_iter()
                .find(|f| f.id == fid)
                .and_then(|f| Interval::new(f.months, f.days)),
            None => None,
        };
        match (date, interval) {
            (Some(next), Some(iv)) => (None, Some(last_done_for_next(next, iv))),
            // A repeating thing with no date yet: it shows as "Needs a date".
            (None, Some(_)) => (None, None),
            _ => return Err(CommandError::InvalidInput),
        }
    };
    let input = RecurringInput {
        name: input.name,
        recurring_type: input.kind,
        frequency_id,
        expiration_date,
        last_done_before_tracker: last_done,
        warn_days_before: input.warn_days,
        alerts_enabled: input.alerts,
        notes: input.notes,
        timesheet_code_id: input
            .code_id
            .as_deref()
            .filter(|c| !c.is_empty())
            .map(id_arg)
            .transpose()?,
    };
    match id.as_deref() {
        Some(id) => {
            store.update_recurring(owner, id_arg(id)?, &input).await?;
        }
        None => {
            store.create_recurring(owner, &input).await?;
        }
    }
    Ok(())
}

/// What Mark done did, for Undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkedView {
    entry_id: String,
    created: bool,
}

impl From<MarkedDone> for MarkedView {
    fn from(m: MarkedDone) -> Self {
        Self {
            entry_id: m.entry_id.to_string(),
            created: m.created,
        }
    }
}

/// Maps to `DataApi.recurringDone()`: Mark done on `date`.
#[tauri::command]
pub async fn recurring_done(
    s: State<'_, VaultSession>,
    id: String,
    date: String,
) -> Res<MarkedView> {
    let (store, owner) = s.data().await?;
    Ok(store
        .mark_recurring_done(owner, id_arg(&id)?, date_arg(&date)?)
        .await?
        .into())
}

/// Maps to `DataApi.recurringRenewed()`: a thing that expires was renewed on `date` and
/// now runs to `expires`.
#[tauri::command]
pub async fn recurring_renewed(
    s: State<'_, VaultSession>,
    id: String,
    expires: String,
    date: String,
) -> Res<MarkedView> {
    let (store, owner) = s.data().await?;
    let id: RecurringTaskId = id_arg(&id)?;
    let expires = date_arg(&expires)?;
    let date = date_arg(&date)?;
    if expires <= date {
        return Err(CommandError::InvalidInput);
    }
    store.set_recurring_expiry(owner, id, expires).await?;
    Ok(store.mark_recurring_done(owner, id, date).await?.into())
}

/// Maps to `DataApi.recurringUndo()`: take back a Mark done.
#[tauri::command]
pub async fn recurring_undo(s: State<'_, VaultSession>, marked: MarkedView) -> Res<()> {
    let (store, owner) = s.data().await?;
    let entry_id: TimeEntryId = id_arg(&marked.entry_id)?;
    Ok(store
        .undo_recurring_done(
            owner,
            MarkedDone {
                entry_id,
                created: marked.created,
            },
        )
        .await?)
}

/// Maps to `DataApi.setRecurringDeleted()`.
#[tauri::command]
pub async fn recurring_set_deleted(
    s: State<'_, VaultSession>,
    id: String,
    deleted: bool,
) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store
        .set_recurring_deleted(owner, id_arg(&id)?, deleted)
        .await?)
}

/// One frequency for the "every…" picker.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyView {
    id: FrequencyId,
    label: String,
}

/// The pickers on the Repeating view's form.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoicesView {
    frequencies: Vec<FrequencyView>,
    /// Kinds of repeating things that show (Settings lists, by mode).
    kinds: Vec<String>,
}

/// Maps to `DataApi.recurringChoices()`.
#[tauri::command]
pub async fn recurring_choices(s: State<'_, VaultSession>) -> Res<ChoicesView> {
    let (store, owner) = s.data().await?;
    let frequencies = store
        .list_frequencies(owner)
        .await?
        .into_iter()
        .map(|f| FrequencyView {
            id: f.id,
            label: f.label,
        })
        .collect();
    let kinds = store
        .list_options(owner, ListName::RecurringType, false)
        .await?
        .into_iter()
        .map(|o| o.value)
        .collect();
    Ok(ChoicesView { frequencies, kinds })
}

// ---------- lists ----------

/// One list item.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListItemView {
    id: ChecklistItemId,
    text: String,
    done: bool,
}

/// One list with its items.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListView {
    id: TaskId,
    title: String,
    items: Vec<ListItemView>,
    done: usize,
    total: usize,
}

fn list_view(r: ListRecord) -> ListView {
    let (done, total) = list_progress(&r.items);
    ListView {
        id: r.list.id,
        title: r.list.title,
        items: r
            .items
            .into_iter()
            .map(|i| ListItemView {
                id: i.id,
                text: i.text,
                done: i.done,
            })
            .collect(),
        done,
        total,
    }
}

/// Maps to `DataApi.lists()`.
#[tauri::command]
pub async fn lists(s: State<'_, VaultSession>) -> Res<Vec<ListView>> {
    let (store, owner) = s.data().await?;
    Ok(store
        .lists(owner)
        .await?
        .into_iter()
        .map(list_view)
        .collect())
}

/// Maps to `DataApi.createList()`.
#[tauri::command]
pub async fn list_create(
    s: State<'_, VaultSession>,
    title: String,
    items: String,
) -> Res<ListView> {
    let (store, owner) = s.data().await?;
    Ok(list_view(store.create_list(owner, &title, &items).await?))
}

/// Maps to `DataApi.renameList()`.
#[tauri::command]
pub async fn list_rename(s: State<'_, VaultSession>, id: String, title: String) -> Res<ListView> {
    let (store, owner) = s.data().await?;
    Ok(list_view(
        store.rename_list(owner, id_arg(&id)?, &title).await?,
    ))
}

/// Maps to `DataApi.setListDeleted()`.
#[tauri::command]
pub async fn list_set_deleted(s: State<'_, VaultSession>, id: String, deleted: bool) -> Res<()> {
    let (store, owner) = s.data().await?;
    Ok(store.set_list_deleted(owner, id_arg(&id)?, deleted).await?)
}

/// Maps to `DataApi.addListItems()`.
#[tauri::command]
pub async fn list_add_items(s: State<'_, VaultSession>, id: String, text: String) -> Res<ListView> {
    let (store, owner) = s.data().await?;
    Ok(list_view(
        store.add_list_items(owner, id_arg(&id)?, &text).await?,
    ))
}

/// Maps to `DataApi.listItemDone()`.
#[tauri::command]
pub async fn list_item_done(s: State<'_, VaultSession>, id: String, done: bool) -> Res<ListView> {
    let (store, owner) = s.data().await?;
    Ok(list_view(
        store.set_list_item_done(owner, id_arg(&id)?, done).await?,
    ))
}

/// Maps to `DataApi.listItemText()`.
#[tauri::command]
pub async fn list_item_text(s: State<'_, VaultSession>, id: String, text: String) -> Res<ListView> {
    let (store, owner) = s.data().await?;
    Ok(list_view(
        store.set_list_item_text(owner, id_arg(&id)?, &text).await?,
    ))
}

/// Maps to `DataApi.listItemDeleted()`.
#[tauri::command]
pub async fn list_item_deleted(
    s: State<'_, VaultSession>,
    id: String,
    deleted: bool,
) -> Res<ListView> {
    let (store, owner) = s.data().await?;
    Ok(list_view(
        store
            .set_list_item_deleted(owner, id_arg(&id)?, deleted)
            .await?,
    ))
}

/// Maps to `DataApi.resetList()`.
#[tauri::command]
pub async fn list_reset(s: State<'_, VaultSession>, id: String) -> Res<ListView> {
    let (store, owner) = s.data().await?;
    Ok(list_view(store.reset_list(owner, id_arg(&id)?).await?))
}

// ---------- Work / Home / Both, and While you were away ----------

/// Maps to `DataApi.setUseMode()`: pick what Shiftbook is for. Returns the preferences.
#[tauri::command]
pub async fn set_use_mode(s: State<'_, VaultSession>, mode: String) -> Res<Prefs> {
    let (store, owner) = s.data().await?;
    let mode = UseMode::parse(&mode).ok_or(CommandError::InvalidInput)?;
    store.set_use_mode(owner, mode).await?;
    Ok(store.prefs(owner).await?)
}

/// What came due while Shiftbook was closed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwayView {
    /// The last day it was open, `YYYY-MM-DD`.
    since: Option<String>,
    items: Vec<BellItem>,
}

/// Maps to `DataApi.whileAway()`: called once after unlock and when the day changes. Lists
/// what came due since the last day Shiftbook was open, then remembers today.
#[tauri::command]
pub async fn while_away(s: State<'_, VaultSession>, today: String, now: u32) -> Res<AwayView> {
    let (store, owner) = s.data().await?;
    let today = date_arg(&today)?;
    let last = store.last_open(owner).await?;
    let all = crate::meetings::bell_for(&store, owner, today, now).await?;
    let items = away_items(&all, last, today);
    if last.is_none_or(|d| d < today) {
        store.set_last_open(owner, today).await?;
    }
    Ok(AwayView {
        since: last.filter(|_| !items.is_empty()).map(format_date),
        items,
    })
}
