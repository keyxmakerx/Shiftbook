//! Notes: write, edit, archive, keep and auto-archive (rules in `shiftbook_core::notes`).
//!
//! Owner-scoped, bound parameters only, soft deletes. Any change bumps `updated_at`,
//! which restarts the auto-archive clock, so a note brought back from the archive
//! doesn't move straight back.

use shiftbook_core::dates::format_timestamp;
use shiftbook_core::ids::{NoteId, OwnerId, ProjectId};
use shiftbook_core::model::Note;
use shiftbook_core::mop::{
    clean_ticks, clean_values, fill_blanks, filled_title, join_ticks, parse_ticks,
};
use shiftbook_core::notes::{
    archive_outlook, join_tags, note_title, parse_tags, validate_body, ArchiveFacts,
    ArchiveOutlook, NoteKind,
};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use time::OffsetDateTime;

use crate::error::StorageError;
use crate::repo::{now_text, parse_opt_ts, parse_ts, parse_uuid, Result};
use crate::work::project_exists;
use crate::SqliteStore;

/// What a note is made of when it's written or changed.
#[derive(Debug, Clone)]
pub struct NoteInput {
    /// Jot, Daily, Meeting, Page or MOP.
    pub kind: NoteKind,
    /// Title; blank uses the body's first line.
    pub title: String,
    /// What's written in it.
    pub body: String,
    /// The project it belongs to, if any.
    pub project: Option<ProjectId>,
    /// Tags as typed, comma-separated.
    pub tags: String,
    /// Written as a MOP template (only when created; see [`SqliteStore::set_note_template`]).
    pub template: bool,
}

macro_rules! note_columns {
    () => {
        "id, owner_id, kind, title, body, project_id, tags, keep, archived_at, \
         created_at, updated_at, deleted_at, template, ticks"
    };
}

fn note_from_row(row: &SqliteRow) -> Result<Note> {
    let kind: String = row.try_get("kind")?;
    Ok(Note {
        id: NoteId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        kind: kind
            .parse()
            .map_err(|_| StorageError::Corrupt("note kind"))?,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        project_id: row
            .try_get::<Option<String>, _>("project_id")?
            .map(|p| parse_uuid(&p).map(ProjectId::from_uuid))
            .transpose()?,
        tags: parse_tags(&row.try_get::<String, _>("tags")?).unwrap_or_default(),
        keep: row.try_get("keep")?,
        template: row.try_get("template")?,
        ticks: parse_ticks(&row.try_get::<String, _>("ticks")?),
        archived_at: parse_opt_ts(row.try_get("archived_at")?)?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

/// The checked columns for a note: title, body and stored tags.
fn checked(input: &NoteInput) -> Result<(String, String)> {
    validate_body(&input.body)?;
    let title = note_title(&input.title, &input.body)?;
    let tags = join_tags(&parse_tags(&input.tags)?);
    Ok((title, tags))
}

impl SqliteStore {
    /// Live notes, most recently changed first. Archived ones only with `include_archived`.
    pub async fn list_notes(&self, owner: OwnerId, include_archived: bool) -> Result<Vec<Note>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            note_columns!(),
            " FROM notes WHERE owner_id = $1 AND deleted_at IS NULL \
             AND ($2 OR archived_at IS NULL) ORDER BY updated_at DESC, id DESC"
        ))
        .bind(owner.to_string())
        .bind(include_archived)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(note_from_row).collect()
    }

    /// One live note (archived or not).
    pub async fn get_note(&self, owner: OwnerId, id: NoteId) -> Result<Note> {
        let row = sqlx::query(concat!(
            "SELECT ",
            note_columns!(),
            " FROM notes WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .bind(owner.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::NotFound)?;
        note_from_row(&row)
    }

    /// Write a new note.
    pub async fn create_note(&self, owner: OwnerId, input: &NoteInput) -> Result<Note> {
        let (title, tags) = checked(input)?;
        let mut conn = self.pool.acquire().await?;
        if let Some(p) = input.project {
            project_exists(&mut conn, owner, p).await?;
        }
        let id = NoteId::new();
        sqlx::query(concat!(
            "INSERT INTO notes (",
            note_columns!(),
            ") VALUES ($1, $2, $3, $4, $5, $6, $7, FALSE, NULL, $8, $8, NULL, $9, '')"
        ))
        .bind(id.to_string())
        .bind(owner.to_string())
        .bind(input.kind.as_str())
        .bind(&title)
        .bind(&input.body)
        .bind(input.project.map(|p| p.to_string()))
        .bind(&tags)
        .bind(now_text())
        .bind(input.template)
        .execute(&mut *conn)
        .await?;
        drop(conn);
        self.get_note(owner, id).await
    }

    /// Change a note's kind, title, body, project or tags.
    pub async fn update_note(&self, owner: OwnerId, id: NoteId, input: &NoteInput) -> Result<Note> {
        let (title, tags) = checked(input)?;
        let mut conn = self.pool.acquire().await?;
        if let Some(p) = input.project {
            project_exists(&mut conn, owner, p).await?;
        }
        let done = sqlx::query(
            "UPDATE notes SET kind = $1, title = $2, body = $3, project_id = $4, tags = $5, \
             updated_at = $6 WHERE id = $7 AND owner_id = $8 AND deleted_at IS NULL",
        )
        .bind(input.kind.as_str())
        .bind(&title)
        .bind(&input.body)
        .bind(input.project.map(|p| p.to_string()))
        .bind(&tags)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *conn)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        drop(conn);
        self.get_note(owner, id).await
    }

    /// Move a note to the archive, or bring it back.
    pub async fn set_note_archived(
        &self,
        owner: OwnerId,
        id: NoteId,
        archived: bool,
    ) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE notes SET archived_at = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(if archived { Some(now.as_str()) } else { None })
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Keep a note out of auto-archive, or let it move again.
    pub async fn set_note_keep(&self, owner: OwnerId, id: NoteId, keep: bool) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE notes SET keep = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(keep)
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Delete a note (soft delete), or with `deleted = false` undo that.
    pub async fn set_note_deleted(&self, owner: OwnerId, id: NoteId, deleted: bool) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE notes SET deleted_at = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND ((deleted_at IS NULL) = $5)",
        )
        .bind(if deleted { Some(now.as_str()) } else { None })
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .bind(deleted)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Mark a note as a MOP template, or back to a plain MOP.
    pub async fn set_note_template(
        &self,
        owner: OwnerId,
        id: NoteId,
        template: bool,
    ) -> Result<()> {
        let done = sqlx::query(
            "UPDATE notes SET template = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(template)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Keep what's ticked off on a note in Checklist mode (keys that no longer match a
    /// line are dropped). An empty list clears the ticks for a fresh run.
    pub async fn set_note_ticks(
        &self,
        owner: OwnerId,
        id: NoteId,
        ticks: &[String],
    ) -> Result<Note> {
        let note = self.get_note(owner, id).await?;
        let ticks = join_ticks(&clean_ticks(&note.body, ticks));
        sqlx::query(
            "UPDATE notes SET ticks = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(&ticks)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        self.get_note(owner, id).await
    }

    /// A new MOP from a template: its blanks filled with `values` (blanks left empty stay
    /// as `[[name]]`), the same project and tags, nothing ticked. A blank `title` uses the
    /// template's title with its blanks filled.
    pub async fn note_from_template(
        &self,
        owner: OwnerId,
        template: NoteId,
        title: &str,
        values: &[(String, String)],
    ) -> Result<Note> {
        let values = clean_values(values)?;
        let from = self.get_note(owner, template).await?;
        let input = NoteInput {
            kind: NoteKind::Mop,
            title: filled_title(title, &from.title, &values),
            body: fill_blanks(&from.body, &values),
            project: from.project_id,
            tags: join_tags(&from.tags),
            template: false,
        };
        self.create_note(owner, &input).await
    }

    /// Move every note that auto-archive says is due (see
    /// `shiftbook_core::notes::archive_outlook`). Returns how many moved. The move
    /// doesn't count as a change, so `updated_at` stays as it was.
    pub async fn auto_archive_notes(
        &self,
        owner: OwnerId,
        now: OffsetDateTime,
        after_days: u32,
    ) -> Result<usize> {
        if after_days == 0 {
            return Ok(0);
        }
        let due: Vec<NoteId> = self
            .list_notes(owner, false)
            .await?
            .into_iter()
            .filter(|n| archive_outlook(facts(n), now, after_days) == ArchiveOutlook::Due)
            .map(|n| n.id)
            .collect();
        let stamp = format_timestamp(now);
        let mut tx = self.write_tx().await?;
        for id in &due {
            sqlx::query(
                "UPDATE notes SET archived_at = $1 WHERE id = $2 AND owner_id = $3 \
                 AND deleted_at IS NULL AND archived_at IS NULL",
            )
            .bind(&stamp)
            .bind(id.to_string())
            .bind(owner.to_string())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(due.len())
    }
}

/// The facts auto-archive needs about a note.
pub fn facts(note: &Note) -> ArchiveFacts {
    ArchiveFacts {
        kind: note.kind,
        keep: note.keep,
        archived: note.archived_at.is_some(),
        updated_at: note.updated_at,
    }
}
