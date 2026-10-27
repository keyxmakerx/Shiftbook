//! Stacks and drawers on the Canvas (round 8): cards piled together, and cards filed away
//! in a drawer (one per project, plus folders the owner makes). Only the layout changes;
//! the records stay as they are. Owner-scoped, bound parameters only.

use shiftbook_core::canvas::{clamp_spot, clean_folder_name, clean_pile_name, CardKind, Drawer};
use shiftbook_core::ids::OwnerId;
use sqlx::sqlite::{SqliteConnection, SqliteRow};
use sqlx::Row;
use uuid::Uuid;

use crate::canvas::CardRef;
use crate::error::StorageError;
use crate::repo::{now_text, Result};
use crate::SqliteStore;

/// How far apart cards land when a pile comes apart.
pub const SPREAD: i32 = 28;

/// A pile on the Desk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasStack {
    /// Id.
    pub id: Uuid,
    /// Its name; `None` shows the count.
    pub name: Option<String>,
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
}

/// A folder in the cabinet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasFolder {
    /// Id.
    pub id: Uuid,
    /// Its name.
    pub name: String,
}

fn uuid_of(row: &SqliteRow, col: &str) -> Result<Uuid> {
    let text: String = row.try_get(col)?;
    Uuid::parse_str(&text).map_err(|_| StorageError::Corrupt("canvas id"))
}

fn int_of(row: &SqliteRow, col: &str) -> Result<i32> {
    i32::try_from(row.try_get::<i64, _>(col)?).map_err(|_| StorageError::Corrupt("canvas spot"))
}

async fn top_z(conn: &mut SqliteConnection, owner: OwnerId) -> Result<i64> {
    let top: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(z) FROM canvas_cards WHERE owner_id = $1 AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .fetch_one(&mut *conn)
    .await?;
    Ok(top.unwrap_or(0).saturating_add(1))
}

/// Make sure a card has a layout row (on top, at `x`, `y` when it's new) and set where it
/// lives: in a pile, in a drawer, or loose (both `None`).
async fn place(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    card: CardRef,
    at: Option<(i32, i32)>,
    stack: Option<Uuid>,
    drawer: Option<Drawer>,
) -> Result<()> {
    let z = top_z(conn, owner).await?;
    let now = now_text();
    let (kind, id) = card;
    let updated = sqlx::query(
        "UPDATE canvas_cards SET x = COALESCE($1, x), y = COALESCE($2, y), z = $3, \
         stack_id = $4, drawer = $5, updated_at = $6 \
         WHERE owner_id = $7 AND record_kind = $8 AND record_id = $9 AND deleted_at IS NULL",
    )
    .bind(at.map(|a| i64::from(a.0)))
    .bind(at.map(|a| i64::from(a.1)))
    .bind(z)
    .bind(stack.map(|s| s.to_string()))
    .bind(drawer.map(|d| d.key()))
    .bind(&now)
    .bind(owner.to_string())
    .bind(kind.as_str())
    .bind(id.to_string())
    .execute(&mut *conn)
    .await?;
    if updated.rows_affected() == 0 {
        let (x, y) = at.unwrap_or((0, 0));
        sqlx::query(
            "INSERT INTO canvas_cards (id, owner_id, record_kind, record_id, x, y, z, stack_id, \
             drawer, created_at, updated_at, deleted_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10, NULL)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(owner.to_string())
        .bind(kind.as_str())
        .bind(id.to_string())
        .bind(i64::from(x))
        .bind(i64::from(y))
        .bind(z)
        .bind(stack.map(|s| s.to_string()))
        .bind(drawer.map(|d| d.key()))
        .bind(&now)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

async fn stack_of(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    card: CardRef,
) -> Result<Option<Uuid>> {
    let id: Option<Option<String>> = sqlx::query_scalar(
        "SELECT stack_id FROM canvas_cards WHERE owner_id = $1 AND record_kind = $2 \
         AND record_id = $3 AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .bind(card.0.as_str())
    .bind(card.1.to_string())
    .fetch_optional(&mut *conn)
    .await?;
    id.flatten()
        .map(|t| Uuid::parse_str(&t).map_err(|_| StorageError::Corrupt("canvas stack")))
        .transpose()
}

async fn stack_spot(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    stack: Uuid,
) -> Result<(i32, i32)> {
    let row = sqlx::query(
        "SELECT x, y FROM canvas_stacks WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL",
    )
    .bind(stack.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    Ok((int_of(&row, "x")?, int_of(&row, "y")?))
}

async fn members(conn: &mut SqliteConnection, owner: OwnerId, stack: Uuid) -> Result<Vec<CardRef>> {
    let rows = sqlx::query(
        "SELECT record_kind, record_id FROM canvas_cards WHERE owner_id = $1 AND stack_id = $2 \
         AND drawer IS NULL AND deleted_at IS NULL ORDER BY z",
    )
    .bind(owner.to_string())
    .bind(stack.to_string())
    .fetch_all(&mut *conn)
    .await?;
    rows.iter()
        .map(|r| {
            let kind: String = r.try_get("record_kind")?;
            Ok((
                kind.parse::<CardKind>()
                    .map_err(|_| StorageError::Corrupt("canvas card"))?,
                uuid_of(r, "record_id")?,
            ))
        })
        .collect()
}

/// Lay a pile's cards out in a short cascade from its spot and remove the pile.
async fn spill(conn: &mut SqliteConnection, owner: OwnerId, stack: Uuid) -> Result<()> {
    let (x, y) = stack_spot(conn, owner, stack).await?;
    for (i, card) in (0i32..).zip(members(conn, owner, stack).await?) {
        place(
            conn,
            owner,
            card,
            Some(clamp_spot(x + i * SPREAD, y + i * SPREAD)),
            None,
            None,
        )
        .await?;
    }
    let now = now_text();
    sqlx::query(
        "UPDATE canvas_stacks SET deleted_at = $1, updated_at = $1 WHERE id = $2 AND owner_id = $3",
    )
    .bind(&now)
    .bind(stack.to_string())
    .bind(owner.to_string())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// A pile with one card left is no pile: the card stays where the pile was.
async fn settle(conn: &mut SqliteConnection, owner: OwnerId, stack: Option<Uuid>) -> Result<()> {
    if let Some(stack) = stack {
        if members(conn, owner, stack).await?.len() < 2 {
            spill(conn, owner, stack).await?;
        }
    }
    Ok(())
}

/// A card moved on its own leaves its pile (and a pile left with one card comes apart).
pub(crate) async fn loosen(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    card: CardRef,
) -> Result<()> {
    let Some(left) = stack_of(conn, owner, card).await? else {
        return Ok(());
    };
    sqlx::query(
        "UPDATE canvas_cards SET stack_id = NULL WHERE owner_id = $1 AND record_kind = $2 \
         AND record_id = $3 AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .bind(card.0.as_str())
    .bind(card.1.to_string())
    .execute(&mut *conn)
    .await?;
    settle(conn, owner, Some(left)).await
}

impl SqliteStore {
    /// Every pile on the Desk.
    pub async fn canvas_stacks(&self, owner: OwnerId) -> Result<Vec<CanvasStack>> {
        let rows = sqlx::query(
            "SELECT id, name, x, y FROM canvas_stacks WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY created_at, id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(CanvasStack {
                    id: uuid_of(r, "id")?,
                    name: r.try_get("name")?,
                    x: int_of(r, "x")?,
                    y: int_of(r, "y")?,
                })
            })
            .collect()
    }

    /// Drop `moving` on `onto`: they become a pile at `onto_spot`, or `moving` joins the pile
    /// `onto` is in. A pile `moving` leaves behind with one card comes apart. Returns the pile.
    pub async fn stack_cards(
        &self,
        owner: OwnerId,
        moving: CardRef,
        onto: CardRef,
        onto_spot: (i32, i32),
    ) -> Result<Uuid> {
        if moving == onto {
            return Err(StorageError::Invalid(
                shiftbook_core::CoreError::NotAChoice {
                    value: "the same card".into(),
                },
            ));
        }
        let mut tx = self.pool.begin().await?;
        let left = stack_of(&mut tx, owner, moving).await?;
        let stack = match stack_of(&mut tx, owner, onto).await? {
            Some(s) => s,
            None => {
                let id = Uuid::now_v7();
                let (x, y) = clamp_spot(onto_spot.0, onto_spot.1);
                sqlx::query(
                    "INSERT INTO canvas_stacks (id, owner_id, name, x, y, created_at, updated_at, \
                     deleted_at) VALUES ($1, $2, NULL, $3, $4, $5, $5, NULL)",
                )
                .bind(id.to_string())
                .bind(owner.to_string())
                .bind(i64::from(x))
                .bind(i64::from(y))
                .bind(now_text())
                .execute(&mut *tx)
                .await?;
                place(&mut tx, owner, onto, Some((x, y)), Some(id), None).await?;
                id
            }
        };
        let spot = stack_spot(&mut tx, owner, stack).await?;
        place(&mut tx, owner, moving, Some(spot), Some(stack), None).await?;
        if left != Some(stack) {
            settle(&mut tx, owner, left).await?;
        }
        tx.commit().await?;
        Ok(stack)
    }

    /// Move a pile.
    pub async fn move_stack(&self, owner: OwnerId, id: Uuid, x: i32, y: i32) -> Result<()> {
        let (x, y) = clamp_spot(x, y);
        let done = sqlx::query(
            "UPDATE canvas_stacks SET x = $1, y = $2, updated_at = $3 \
             WHERE id = $4 AND owner_id = $5 AND deleted_at IS NULL",
        )
        .bind(i64::from(x))
        .bind(i64::from(y))
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

    /// Name a pile (blank goes back to showing its count).
    pub async fn rename_stack(&self, owner: OwnerId, id: Uuid, name: &str) -> Result<()> {
        let name = clean_pile_name(name)?;
        let done = sqlx::query(
            "UPDATE canvas_stacks SET name = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(name)
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

    /// Take a pile apart: its cards fan out from where it was.
    pub async fn dissolve_stack(&self, owner: OwnerId, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        spill(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(())
    }

    /// File a card in a drawer: it leaves the Desk (and its pile).
    pub async fn file_card(&self, owner: OwnerId, card: CardRef, drawer: Drawer) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let left = stack_of(&mut tx, owner, card).await?;
        place(&mut tx, owner, card, None, None, Some(drawer)).await?;
        settle(&mut tx, owner, left).await?;
        tx.commit().await?;
        Ok(())
    }

    /// File several cards in one drawer at once (round 15: Tidy up puts them in the Archive).
    /// All or nothing.
    pub async fn file_cards(
        &self,
        owner: OwnerId,
        cards: &[CardRef],
        drawer: Drawer,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for &card in cards {
            let left = stack_of(&mut tx, owner, card).await?;
            place(&mut tx, owner, card, None, None, Some(drawer)).await?;
            settle(&mut tx, owner, left).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// File a whole pile: its cards go in the drawer and the pile goes away.
    pub async fn file_stack(
        &self,
        owner: OwnerId,
        id: Uuid,
        drawer: Drawer,
    ) -> Result<Vec<CardRef>> {
        let mut tx = self.pool.begin().await?;
        let (x, y) = stack_spot(&mut tx, owner, id).await?;
        let cards = members(&mut tx, owner, id).await?;
        for card in &cards {
            place(&mut tx, owner, *card, Some((x, y)), None, Some(drawer)).await?;
        }
        sqlx::query("UPDATE canvas_stacks SET deleted_at = $1, updated_at = $1 WHERE id = $2 AND owner_id = $3")
            .bind(now_text())
            .bind(id.to_string())
            .bind(owner.to_string())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(cards)
    }

    /// Take a card out of its drawer and put it back on the Desk at `x`, `y`.
    pub async fn unfile_card(&self, owner: OwnerId, card: CardRef, x: i32, y: i32) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        place(&mut tx, owner, card, Some(clamp_spot(x, y)), None, None).await?;
        tx.commit().await?;
        Ok(())
    }

    /// The folders in the cabinet, oldest first.
    pub async fn canvas_folders(&self, owner: OwnerId) -> Result<Vec<CanvasFolder>> {
        let rows = sqlx::query(
            "SELECT id, name FROM canvas_folders WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY created_at, id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(CanvasFolder {
                    id: uuid_of(r, "id")?,
                    name: r.try_get("name")?,
                })
            })
            .collect()
    }

    /// Make a folder (`id` None) or rename one.
    pub async fn save_folder(
        &self,
        owner: OwnerId,
        id: Option<Uuid>,
        name: &str,
    ) -> Result<CanvasFolder> {
        let name = clean_folder_name(name)?;
        let now = now_text();
        let id = match id {
            Some(id) => {
                let done = sqlx::query(
                    "UPDATE canvas_folders SET name = $1, updated_at = $2 \
                     WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
                )
                .bind(&name)
                .bind(&now)
                .bind(id.to_string())
                .bind(owner.to_string())
                .execute(&self.pool)
                .await?;
                if done.rows_affected() == 0 {
                    return Err(StorageError::NotFound);
                }
                id
            }
            None => {
                let id = Uuid::now_v7();
                sqlx::query(
                    "INSERT INTO canvas_folders (id, owner_id, name, created_at, updated_at, \
                     deleted_at) VALUES ($1, $2, $3, $4, $4, NULL)",
                )
                .bind(id.to_string())
                .bind(owner.to_string())
                .bind(&name)
                .bind(&now)
                .execute(&self.pool)
                .await?;
                id
            }
        };
        Ok(CanvasFolder { id, name })
    }

    /// Remove a folder, or with `deleted` false bring it back. While it's gone its cards show
    /// on the Desk again; bringing it back files them again.
    pub async fn set_folder_deleted(&self, owner: OwnerId, id: Uuid, deleted: bool) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE canvas_folders SET deleted_at = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4",
        )
        .bind(deleted.then_some(now.as_str()))
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
}
