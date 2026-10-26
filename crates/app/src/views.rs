//! The small camelCase views of records that the UI shows, shared by every command that
//! returns them (desktop now, the Phase 2 server later).

use serde::Serialize;
use shiftbook_core::boards::links_in;
use shiftbook_core::dates::{format_date, format_timestamp};
use shiftbook_core::fields::{build_link, ticket_matches_prefix};
use shiftbook_core::hours::format_hours;
use shiftbook_core::ids::{
    InboxItemId, NoteId, OwnerId, ProjectId, RecurringTaskId, TimeEntryId, TimesheetCodeId,
    TrayItemId,
};
use shiftbook_core::minutes::Minutes;
use shiftbook_core::model::{CustomField, EntryTarget, Note, TrayItem};
use shiftbook_core::notes::{archive_outlook, ArchiveOutlook, NoteKind};
use shiftbook_storage::notes::facts;
use shiftbook_storage::work::EntryRecord;
use shiftbook_storage::{SqliteStore, StorageError};
use time::OffsetDateTime;

type Result<T> = std::result::Result<T, StorageError>;

/// A link a ticket can be copied as (from a custom link field).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    pub field_name: String,
    pub url: String,
}

/// A ticket chip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketView {
    pub id: String,
    pub number: String,
    pub links: Vec<LinkView>,
    /// Ticked done; false while still open (round 14).
    pub done: bool,
    /// Its notepad.
    pub note: Option<String>,
}

/// A time entry with its tickets.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: TimeEntryId,
    pub date: String,
    pub minutes: Minutes,
    pub hours: String,
    pub description: String,
    pub project_id: Option<ProjectId>,
    /// The repeating thing it belongs to (round 15), when it has no project.
    pub recurring_id: Option<RecurringTaskId>,
    pub category: Option<String>,
    pub notes: Option<String>,
    pub timesheet_code_id: Option<TimesheetCodeId>,
    pub tickets: Vec<TicketView>,
    pub is_tray: bool,
}

/// One tray ticket.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayItemView {
    pub id: TrayItemId,
    pub date: String,
    pub ticket: String,
    pub minutes: Minutes,
    pub note: Option<String>,
    pub done: bool,
    pub project_id: Option<ProjectId>,
    pub recurring_id: Option<RecurringTaskId>,
    pub links: Vec<LinkView>,
}

/// An Inbox scrap.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxView {
    pub id: InboxItemId,
    pub text: String,
    pub created_at: String,
}

/// Link fields that can turn a ticket into a link (template and prefix set).
pub fn link_fields(fields: &[CustomField]) -> Vec<CustomField> {
    fields
        .iter()
        .filter(|f| !f.hidden && f.link_template.is_some() && f.ticket_prefix.is_some())
        .cloned()
        .collect()
}

pub fn ticket_links(fields: &[CustomField], ticket: &str) -> Vec<LinkView> {
    fields
        .iter()
        .filter_map(|f| {
            let prefix = f.ticket_prefix.as_deref()?;
            let template = f.link_template.as_deref()?;
            ticket_matches_prefix(prefix, ticket).then(|| LinkView {
                field_name: f.name.clone(),
                url: build_link(template, ticket),
            })
        })
        .collect()
}

pub fn entry_view(rec: EntryRecord, fields: &[CustomField]) -> EntryView {
    let (project_id, recurring_id) = match rec.entry.target {
        EntryTarget::Project(p) => (Some(p), None),
        EntryTarget::Recurring(r) => (None, Some(r)),
        EntryTarget::None => (None, None),
    };
    EntryView {
        id: rec.entry.id,
        date: format_date(rec.entry.date),
        minutes: rec.entry.minutes,
        hours: format_hours(rec.entry.minutes),
        description: rec.entry.description,
        project_id,
        recurring_id,
        category: rec.entry.category,
        notes: rec.entry.next_step,
        timesheet_code_id: rec.entry.timesheet_code_id,
        tickets: rec
            .tickets
            .into_iter()
            .map(|t| TicketView {
                id: t.id.to_string(),
                done: t.done,
                note: t.note,
                links: ticket_links(fields, &t.ticket_number),
                number: t.ticket_number,
            })
            .collect(),
        is_tray: rec.is_tray,
    }
}

pub fn tray_item_view(item: TrayItem, fields: &[CustomField]) -> TrayItemView {
    TrayItemView {
        id: item.id,
        date: format_date(item.date),
        links: ticket_links(fields, &item.ticket_number),
        ticket: item.ticket_number,
        minutes: item.minutes,
        note: item.note,
        done: item.done,
        project_id: item.project_id,
        recurring_id: item.recurring_id,
    }
}

pub async fn links_for(store: &SqliteStore, owner: OwnerId) -> Result<Vec<CustomField>> {
    Ok(link_fields(&store.list_custom_fields(owner).await?))
}

pub fn inbox_view(i: shiftbook_core::model::InboxItem) -> InboxView {
    InboxView {
        id: i.id,
        text: i.text,
        created_at: format_timestamp(i.created_at),
    }
}

/// A note as the UI sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteView {
    pub id: NoteId,
    pub kind: NoteKind,
    pub title: String,
    pub body: String,
    pub project_id: Option<ProjectId>,
    pub tags: Vec<String>,
    pub keep: bool,
    pub archived: bool,
    /// Days until auto-archive moves it, when its warning should show.
    pub archive_in_days: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
    /// Web addresses written in it, in order.
    pub links: Vec<String>,
}

pub fn note_view(n: Note, now: OffsetDateTime, after_days: u32) -> NoteView {
    let archive_in_days = match archive_outlook(facts(&n), now, after_days) {
        ArchiveOutlook::Soon { days_left } => Some(days_left),
        ArchiveOutlook::Stays | ArchiveOutlook::Due => None,
    };
    NoteView {
        links: links_in(&n.body),
        id: n.id,
        kind: n.kind,
        title: n.title,
        body: n.body,
        project_id: n.project_id,
        tags: n.tags,
        keep: n.keep,
        archived: n.archived_at.is_some(),
        archive_in_days,
        created_at: format_timestamp(n.created_at),
        updated_at: format_timestamp(n.updated_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shiftbook_core::fields::{AppliesTo, FieldKind};
    use shiftbook_core::ids::CustomFieldId;

    fn field(prefix: Option<&str>, template: Option<&str>, hidden: bool) -> CustomField {
        let now = OffsetDateTime::now_utc();
        CustomField {
            id: CustomFieldId::new(),
            owner_id: OwnerId::new(),
            applies_to: AppliesTo::Ticket,
            name: "Jira".into(),
            kind: FieldKind::Link,
            options: vec![],
            link_template: template.map(String::from),
            ticket_prefix: prefix.map(String::from),
            sort_order: 0,
            hidden,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    #[test]
    fn tickets_get_links_from_matching_fields() {
        let t = Some("https://example.atlassian.net/browse/{ticket}");
        let fields = link_fields(&[
            field(Some("PROJ-"), t, false),
            field(Some("PROJ-"), t, true),
            field(None, t, false),
            field(Some("OPS-"), None, false),
        ]);
        assert_eq!(fields.len(), 1);
        let links = ticket_links(&fields, "proj-12");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.atlassian.net/browse/proj-12");
        assert!(ticket_links(&fields, "INC0045678").is_empty());
    }
}
