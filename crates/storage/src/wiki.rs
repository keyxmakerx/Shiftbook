//! The wiki (see `shiftbook_core::wiki`): spaces, where each page sits, and pages' earlier
//! versions. Owner-scoped, bound parameters only, soft deletes.

use shiftbook_core::ids::{NoteId, OwnerId};
use shiftbook_core::model::Note;
use shiftbook_core::mop::old_blanks_to_new;
use shiftbook_core::notes::NoteKind;
use shiftbook_core::wiki::{check_move, space_name, version_due, Placed, MAX_SPACES, MAX_VERSIONS};
use shiftbook_core::CoreError;
use sqlx::{Row, SqliteConnection};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::StorageError;
use crate::repo::{now_text, parse_ts, parse_uuid, upsert_setting, Result};
use crate::work::opt_uuid;
use crate::SqliteStore;

/// Set once the MOPs from before round 18 have moved into the wiki.
const MOVED_IN_KEY: &str = "wiki.moved_in";
/// The space old MOPs move into.
const MOPS_SPACE: &str = "MOPs";

/// A space in the wiki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiSpace {
    /// Id.
    pub id: Uuid,
    /// Its name, e.g. "Network".
    pub name: String,
    /// A colour for its dot, if chosen.
    pub color: Option<String>,
    /// Its place in the list of spaces.
    pub position: i64,
}

/// Where a page sits: its space (none: "Not in a space"), the page it's under, its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WikiPlace {
    /// The page (its note id).
    pub page: Uuid,
    /// Its space; `None` is "Not in a space".
    pub space: Option<Uuid>,
    /// The page it's under, if any.
    pub parent: Option<Uuid>,
    /// Its place among its siblings.
    pub position: i64,
}

/// An earlier version of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteVersion {
    /// Id.
    pub id: Uuid,
    /// The title it had.
    pub title: String,
    /// What it said.
    pub body: String,
    /// When it was kept (UTC).
    pub at: OffsetDateTime,
}

/// True for the notes the wiki shows: pages, and MOP templates.
pub fn in_wiki(note: &Note) -> bool {
    note.kind == NoteKind::Page || (note.kind == NoteKind::Mop && note.template)
}

async fn space_exists(conn: &mut SqliteConnection, owner: OwnerId, id: Uuid) -> Result<()> {
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM wiki_spaces WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?;
    found.map(|_| ()).ok_or(StorageError::NotFound)
}

async fn places(conn: &mut SqliteConnection, owner: OwnerId) -> Result<Vec<WikiPlace>> {
    let rows = sqlx::query(
        "SELECT p.id, p.space_id, p.parent_id, p.position FROM wiki_places p \
         JOIN notes n ON n.id = p.id \
         WHERE p.owner_id = $1 AND p.deleted_at IS NULL AND n.deleted_at IS NULL \
         ORDER BY p.position, p.id",
    )
    .bind(owner.to_string())
    .fetch_all(&mut *conn)
    .await?;
    rows.iter()
        .map(|row| {
            Ok(WikiPlace {
                page: parse_uuid(row.try_get("id")?)?,
                space: opt_uuid(row.try_get("space_id")?)?,
                parent: opt_uuid(row.try_get("parent_id")?)?,
                position: row.try_get("position")?,
            })
        })
        .collect()
}

async fn set_place(conn: &mut SqliteConnection, owner: OwnerId, place: WikiPlace) -> Result<()> {
    sqlx::query(
        "INSERT INTO wiki_places (id, owner_id, space_id, parent_id, position, created_at, \
         updated_at, deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $6, NULL) \
         ON CONFLICT (id) DO UPDATE SET space_id = excluded.space_id, \
         parent_id = excluded.parent_id, position = excluded.position, \
         updated_at = excluded.updated_at, deleted_at = NULL",
    )
    .bind(place.page.to_string())
    .bind(owner.to_string())
    .bind(place.space.map(|s| s.to_string()))
    .bind(place.parent.map(|p| p.to_string()))
    .bind(place.position)
    .bind(now_text())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

impl SqliteStore {
    /// Every space, in their order.
    pub async fn wiki_spaces(&self, owner: OwnerId) -> Result<Vec<WikiSpace>> {
        let rows = sqlx::query(
            "SELECT id, name, color, position FROM wiki_spaces \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY position, name, id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(WikiSpace {
                    id: parse_uuid(row.try_get("id")?)?,
                    name: row.try_get("name")?,
                    color: row.try_get("color")?,
                    position: row.try_get("position")?,
                })
            })
            .collect()
    }

    /// Make a space (`id` None, added last) or rename and recolour one.
    pub async fn save_wiki_space(
        &self,
        owner: OwnerId,
        id: Option<Uuid>,
        name: &str,
        color: Option<&str>,
    ) -> Result<WikiSpace> {
        let name = space_name(name)?;
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        let id = match id {
            Some(id) => {
                space_exists(&mut tx, owner, id).await?;
                sqlx::query(
                    "UPDATE wiki_spaces SET name = $1, color = $2, updated_at = $3 \
                     WHERE id = $4 AND owner_id = $5",
                )
                .bind(&name)
                .bind(color)
                .bind(&now)
                .bind(id.to_string())
                .bind(owner.to_string())
                .execute(&mut *tx)
                .await?;
                id
            }
            None => {
                let (count, last): (i64, Option<i64>) = sqlx::query_as(
                    "SELECT COUNT(*), MAX(position) FROM wiki_spaces \
                     WHERE owner_id = $1 AND deleted_at IS NULL",
                )
                .bind(owner.to_string())
                .fetch_one(&mut *tx)
                .await?;
                if usize::try_from(count).unwrap_or(usize::MAX) >= MAX_SPACES {
                    return Err(CoreError::TooManyItems {
                        field: "spaces",
                        max: MAX_SPACES,
                    }
                    .into());
                }
                let id = Uuid::now_v7();
                sqlx::query(
                    "INSERT INTO wiki_spaces (id, owner_id, name, color, position, created_at, \
                     updated_at, deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $6, NULL)",
                )
                .bind(id.to_string())
                .bind(owner.to_string())
                .bind(&name)
                .bind(color)
                .bind(last.map_or(0, |l| l + 1))
                .bind(&now)
                .execute(&mut *tx)
                .await?;
                id
            }
        };
        tx.commit().await?;
        self.wiki_spaces(owner)
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(StorageError::NotFound)
    }

    /// Remove a space. Its pages stay, under "Not in a space", keeping who's under whom.
    pub async fn delete_wiki_space(&self, owner: OwnerId, id: Uuid) -> Result<()> {
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        space_exists(&mut tx, owner, id).await?;
        sqlx::query(
            "UPDATE wiki_places SET space_id = NULL, updated_at = $1 \
             WHERE owner_id = $2 AND space_id = $3",
        )
        .bind(&now)
        .bind(owner.to_string())
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE wiki_spaces SET deleted_at = $1, updated_at = $1 WHERE id = $2 AND owner_id = $3",
        )
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Where every page sits (pages with no row are "Not in a space").
    pub async fn wiki_places(&self, owner: OwnerId) -> Result<Vec<WikiPlace>> {
        let mut conn = self.pool.acquire().await?;
        places(&mut conn, owner).await
    }

    /// Put a page in a space (None: "Not in a space"), under another page of that space or
    /// at the top, at `position` among its siblings (None: last). Pages under it go with it.
    pub async fn place_wiki_page(
        &self,
        owner: OwnerId,
        page: NoteId,
        space: Option<Uuid>,
        parent: Option<NoteId>,
        position: Option<i64>,
    ) -> Result<()> {
        let page_id = *page.as_uuid();
        let parent_id = parent.map(|p| *p.as_uuid());
        let note = self.get_note(owner, page).await?;
        if !in_wiki(&note) {
            return Err(CoreError::NotAChoice {
                value: note.kind.as_str().into(),
            }
            .into());
        }
        let mut tx = self.pool.begin().await?;
        if let Some(s) = space {
            space_exists(&mut tx, owner, s).await?;
        }
        let all = places(&mut tx, owner).await?;
        if let Some(p) = parent_id {
            // The page above must be a wiki page in the same space.
            let above = all.iter().find(|w| w.page == p);
            if above.is_none_or(|w| w.space != space) {
                return Err(StorageError::NotFound);
            }
        }
        let placed: Vec<Placed> = all
            .iter()
            .map(|w| Placed {
                id: w.page,
                parent: w.parent,
                position: w.position,
            })
            .collect();
        check_move(&placed, page_id, parent_id)?;
        let position = match position {
            Some(p) => p,
            None => all
                .iter()
                .filter(|w| w.space == space && w.parent == parent_id && w.page != page_id)
                .map(|w| w.position + 1)
                .max()
                .unwrap_or(0),
        };
        // Everything under the page moves space with it.
        let mut below = vec![page_id];
        let mut i = 0;
        while i < below.len() {
            let at = below[i];
            below.extend(all.iter().filter(|w| w.parent == Some(at)).map(|w| w.page));
            i += 1;
        }
        for w in all.iter().filter(|w| below[1..].contains(&w.page)) {
            set_place(&mut tx, owner, WikiPlace { space, ..*w }).await?;
        }
        set_place(
            &mut tx,
            owner,
            WikiPlace {
                page: page_id,
                space,
                parent: parent_id,
                position,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// A page's earlier versions, newest first.
    pub async fn note_versions(&self, owner: OwnerId, note: NoteId) -> Result<Vec<NoteVersion>> {
        let rows = sqlx::query(
            "SELECT id, title, body, created_at FROM note_versions \
             WHERE owner_id = $1 AND note_id = $2 AND deleted_at IS NULL \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(owner.to_string())
        .bind(note.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(NoteVersion {
                    id: parse_uuid(row.try_get("id")?)?,
                    title: row.try_get("title")?,
                    body: row.try_get("body")?,
                    at: parse_ts(row.try_get("created_at")?)?,
                })
            })
            .collect()
    }
}

/// Before a wiki page is saved with new text: keep what it said as an earlier version when
/// one is due (see `shiftbook_core::wiki::version_due`), and only the newest
/// [`MAX_VERSIONS`].
pub(crate) async fn keep_version(
    conn: &mut SqliteConnection,
    before: &Note,
    title: &str,
    body: &str,
) -> Result<()> {
    if !in_wiki(before) {
        return Ok(());
    }
    let owner = before.owner_id.to_string();
    let note = before.id.to_string();
    let newest: Option<String> = sqlx::query_scalar(
        "SELECT MAX(created_at) FROM note_versions \
         WHERE owner_id = $1 AND note_id = $2 AND deleted_at IS NULL",
    )
    .bind(&owner)
    .bind(&note)
    .fetch_one(&mut *conn)
    .await?;
    let newest = newest.as_deref().map(parse_ts).transpose()?;
    let changed = before.title != title || before.body != body;
    let now = now_text();
    if !version_due(newest, parse_ts(&now)?, changed) {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO note_versions (id, owner_id, note_id, title, body, created_at, updated_at, \
         deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $6, NULL)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&owner)
    .bind(&note)
    .bind(&before.title)
    .bind(&before.body)
    .bind(&now)
    .execute(&mut *conn)
    .await?;
    let keep = i64::try_from(MAX_VERSIONS).unwrap_or(i64::MAX);
    sqlx::query(
        "UPDATE note_versions SET deleted_at = $1, updated_at = $1 \
         WHERE owner_id = $2 AND note_id = $3 AND deleted_at IS NULL AND id NOT IN \
         (SELECT id FROM note_versions WHERE owner_id = $2 AND note_id = $3 \
          AND deleted_at IS NULL ORDER BY created_at DESC, id DESC LIMIT $4)",
    )
    .bind(&now)
    .bind(&owner)
    .bind(&note)
    .bind(keep)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Once, on the first unlock with the wiki (round 18): templates' `[[blanks]]` become
/// `{{blanks}}`, and the other MOPs become pages in a MOPs space.
pub(crate) async fn move_in(conn: &mut SqliteConnection, owner: OwnerId) -> Result<()> {
    let done: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE owner_id = $1 AND key = $2 AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .bind(MOVED_IN_KEY)
    .fetch_optional(&mut *conn)
    .await?;
    if done.is_some() {
        return Ok(());
    }
    let now = now_text();
    let templates: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, title, body FROM notes WHERE owner_id = $1 AND kind = 'mop' AND template \
         AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .fetch_all(&mut *conn)
    .await?;
    for (id, title, body) in templates {
        sqlx::query("UPDATE notes SET title = $1, body = $2, updated_at = $3 WHERE id = $4")
            .bind(old_blanks_to_new(&title))
            .bind(old_blanks_to_new(&body))
            .bind(&now)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    let mops: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM notes WHERE owner_id = $1 AND kind = 'mop' AND NOT template \
         AND deleted_at IS NULL ORDER BY title, id",
    )
    .bind(owner.to_string())
    .fetch_all(&mut *conn)
    .await?;
    if !mops.is_empty() {
        let last: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(position) FROM wiki_spaces WHERE owner_id = $1 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .fetch_one(&mut *conn)
        .await?;
        let space = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO wiki_spaces (id, owner_id, name, color, position, created_at, \
             updated_at, deleted_at) VALUES ($1, $2, $3, NULL, $4, $5, $5, NULL)",
        )
        .bind(space.to_string())
        .bind(owner.to_string())
        .bind(MOPS_SPACE)
        .bind(last.map_or(0, |l| l + 1))
        .bind(&now)
        .execute(&mut *conn)
        .await?;
        for (position, id) in (0i64..).zip(&mops) {
            sqlx::query("UPDATE notes SET kind = 'page', updated_at = $1 WHERE id = $2")
                .bind(&now)
                .bind(id)
                .execute(&mut *conn)
                .await?;
            set_place(
                conn,
                owner,
                WikiPlace {
                    page: parse_uuid(id)?,
                    space: Some(space),
                    parent: None,
                    position,
                },
            )
            .await?;
        }
    }
    upsert_setting(conn, owner, MOVED_IN_KEY, "1").await
}
