//! The Canvas layout: where the owner put each card (see `shiftbook_core::canvas`).
//!
//! Owner-scoped, bound parameters only. Moving a card that has no spot yet creates one.

use shiftbook_core::canvas::{clamp_spot, clean_label, Area, CardKind, Drawer};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

use crate::error::StorageError;
use crate::repo::{now_text, Result};
use crate::SqliteStore;
use shiftbook_core::ids::{OwnerId, ProjectId};

/// One moved card's spot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasSpot {
    /// What the card shows.
    pub kind: CardKind,
    /// The record's id.
    pub record_id: Uuid,
    /// Left edge, in canvas units.
    pub x: i32,
    /// Top edge, in canvas units.
    pub y: i32,
    /// Stacking order: higher is on top.
    pub z: i64,
    /// The pile it's in, if any (it shows at the pile's spot).
    pub stack: Option<Uuid>,
    /// The drawer it's filed in, if any (it's off the Desk).
    pub drawer: Option<Drawer>,
}

fn spot_from_row(row: &SqliteRow) -> Result<CanvasSpot> {
    let kind: String = row.try_get("record_kind")?;
    let id: String = row.try_get("record_id")?;
    let corrupt = |()| StorageError::Corrupt("canvas card");
    Ok(CanvasSpot {
        kind: kind.parse().map_err(|_| corrupt(()))?,
        record_id: Uuid::parse_str(&id).map_err(|_| corrupt(()))?,
        x: i32::try_from(row.try_get::<i64, _>("x")?).map_err(|_| corrupt(()))?,
        y: i32::try_from(row.try_get::<i64, _>("y")?).map_err(|_| corrupt(()))?,
        z: row.try_get("z")?,
        stack: row
            .try_get::<Option<String>, _>("stack_id")?
            .map(|s| Uuid::parse_str(&s).map_err(|_| corrupt(())))
            .transpose()?,
        drawer: row
            .try_get::<Option<String>, _>("drawer")?
            .and_then(|d| Drawer::parse(&d)),
    })
}

impl SqliteStore {
    /// Every moved card's spot.
    pub async fn canvas_spots(&self, owner: OwnerId) -> Result<Vec<CanvasSpot>> {
        let rows = sqlx::query(
            "SELECT record_kind, record_id, x, y, z, stack_id, drawer FROM canvas_cards \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY z",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(spot_from_row).collect()
    }

    /// Put a card at `x`, `y` and on top of the others. A card moved on its own leaves its
    /// pile.
    pub async fn move_card(
        &self,
        owner: OwnerId,
        kind: CardKind,
        record_id: Uuid,
        x: i32,
        y: i32,
    ) -> Result<CanvasSpot> {
        let (x, y) = clamp_spot(x, y);
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        let top: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(z) FROM canvas_cards WHERE owner_id = $1 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let z = top.unwrap_or(0).saturating_add(1);
        let updated = sqlx::query(
            "UPDATE canvas_cards SET x = $1, y = $2, z = $3, updated_at = $4 \
             WHERE owner_id = $5 AND record_kind = $6 AND record_id = $7 AND deleted_at IS NULL",
        )
        .bind(i64::from(x))
        .bind(i64::from(y))
        .bind(z)
        .bind(&now)
        .bind(owner.to_string())
        .bind(kind.as_str())
        .bind(record_id.to_string())
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO canvas_cards \
                 (id, owner_id, record_kind, record_id, x, y, z, created_at, updated_at, deleted_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, NULL)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(owner.to_string())
            .bind(kind.as_str())
            .bind(record_id.to_string())
            .bind(i64::from(x))
            .bind(i64::from(y))
            .bind(z)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        crate::piles::loosen(&mut tx, owner, (kind, record_id)).await?;
        tx.commit().await?;
        Ok(CanvasSpot {
            kind,
            record_id,
            x,
            y,
            z,
            stack: None,
            drawer: None,
        })
    }

    // ---------- project areas ----------

    /// Every project area.
    pub async fn canvas_areas(&self, owner: OwnerId) -> Result<Vec<CanvasArea>> {
        let rows = sqlx::query(
            "SELECT project_id, x, y, w, h, collapsed FROM canvas_areas \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY updated_at, id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(area_from_row).collect()
    }

    /// Put a project's area here (making it the first time). Sizes are kept sane.
    pub async fn set_area(
        &self,
        owner: OwnerId,
        project: ProjectId,
        area: Area,
    ) -> Result<CanvasArea> {
        let area = area.clean();
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        crate::work::project_exists(&mut tx, owner, project).await?;
        let updated = sqlx::query(
            "UPDATE canvas_areas SET x = $1, y = $2, w = $3, h = $4, updated_at = $5 \
             WHERE owner_id = $6 AND project_id = $7 AND deleted_at IS NULL",
        )
        .bind(i64::from(area.x))
        .bind(i64::from(area.y))
        .bind(i64::from(area.w))
        .bind(i64::from(area.h))
        .bind(&now)
        .bind(owner.to_string())
        .bind(project.to_string())
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO canvas_areas \
                 (id, owner_id, project_id, x, y, w, h, created_at, updated_at, deleted_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, NULL)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(owner.to_string())
            .bind(project.to_string())
            .bind(i64::from(area.x))
            .bind(i64::from(area.y))
            .bind(i64::from(area.w))
            .bind(i64::from(area.h))
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        let collapsed: bool = sqlx::query_scalar(
            "SELECT collapsed FROM canvas_areas \
             WHERE owner_id = $1 AND project_id = $2 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(project.to_string())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(CanvasArea {
            project,
            area,
            collapsed,
        })
    }

    /// Fold a project's area up to its name and a count, or open it out again.
    pub async fn set_area_collapsed(
        &self,
        owner: OwnerId,
        project: ProjectId,
        collapsed: bool,
    ) -> Result<()> {
        let done = sqlx::query(
            "UPDATE canvas_areas SET collapsed = $1, updated_at = $2 \
             WHERE owner_id = $3 AND project_id = $4 AND deleted_at IS NULL",
        )
        .bind(collapsed)
        .bind(now_text())
        .bind(owner.to_string())
        .bind(project.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            Err(StorageError::NotFound)
        } else {
            Ok(())
        }
    }

    // ---------- labelled boxes ----------

    /// Every plain labelled box.
    pub async fn canvas_labels(&self, owner: OwnerId) -> Result<Vec<CanvasLabel>> {
        let rows = sqlx::query(
            "SELECT id, label, x, y, w, h, collapsed FROM canvas_labels \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY created_at, id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(label_from_row).collect()
    }

    /// Make a labelled box (`id` None) or change one. Sizes are kept sane.
    pub async fn save_label(
        &self,
        owner: OwnerId,
        id: Option<Uuid>,
        label: &str,
        area: Area,
        collapsed: bool,
    ) -> Result<CanvasLabel> {
        let label = clean_label(label)?;
        let area = area.clean();
        let now = now_text();
        let id = match id {
            Some(id) => {
                let done = sqlx::query(
                    "UPDATE canvas_labels SET label = $1, x = $2, y = $3, w = $4, h = $5, \
                     collapsed = $6, updated_at = $7 \
                     WHERE id = $8 AND owner_id = $9 AND deleted_at IS NULL",
                )
                .bind(&label)
                .bind(i64::from(area.x))
                .bind(i64::from(area.y))
                .bind(i64::from(area.w))
                .bind(i64::from(area.h))
                .bind(collapsed)
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
                    "INSERT INTO canvas_labels \
                     (id, owner_id, label, x, y, w, h, collapsed, created_at, updated_at, deleted_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, NULL)",
                )
                .bind(id.to_string())
                .bind(owner.to_string())
                .bind(&label)
                .bind(i64::from(area.x))
                .bind(i64::from(area.y))
                .bind(i64::from(area.w))
                .bind(i64::from(area.h))
                .bind(collapsed)
                .bind(&now)
                .execute(&self.pool)
                .await?;
                id
            }
        };
        Ok(CanvasLabel {
            id,
            label,
            area,
            collapsed,
        })
    }

    /// Take a labelled box off the Desk (its cards stay), or with `deleted` false put it back.
    pub async fn set_label_deleted(&self, owner: OwnerId, id: Uuid, deleted: bool) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE canvas_labels SET deleted_at = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4",
        )
        .bind(deleted.then_some(now.as_str()))
        .bind(&now)
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            Err(StorageError::NotFound)
        } else {
            Ok(())
        }
    }

    /// Take a project's area off the Desk (the project and its cards stay).
    pub async fn remove_area(&self, owner: OwnerId, project: ProjectId) -> Result<()> {
        let now = now_text();
        sqlx::query(
            "UPDATE canvas_areas SET deleted_at = $1, updated_at = $1 \
             WHERE owner_id = $2 AND project_id = $3 AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(owner.to_string())
        .bind(project.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---------- links ----------

    /// The links the owner drew (`drawn`) or hid (not `drawn`).
    pub async fn canvas_links(&self, owner: OwnerId) -> Result<Vec<CanvasLink>> {
        let rows = sqlx::query(
            "SELECT a_kind, a_id, b_kind, b_id, drawn FROM canvas_links \
             WHERE owner_id = $1 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(link_from_row).collect()
    }

    /// Draw a link between two cards (`drawn`), or hide an automatic one (not `drawn`).
    pub async fn set_link(
        &self,
        owner: OwnerId,
        a: CardRef,
        b: CardRef,
        drawn: bool,
    ) -> Result<()> {
        if a == b {
            return Err(StorageError::Invalid(
                shiftbook_core::CoreError::NotAChoice {
                    value: "a card linked to itself".into(),
                },
            ));
        }
        let (a, b) = pair(a, b);
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE canvas_links SET drawn = $1, updated_at = $2 WHERE owner_id = $3 \
             AND a_kind = $4 AND a_id = $5 AND b_kind = $6 AND b_id = $7 AND deleted_at IS NULL",
        )
        .bind(i64::from(drawn))
        .bind(&now)
        .bind(owner.to_string())
        .bind(a.0.as_str())
        .bind(a.1.to_string())
        .bind(b.0.as_str())
        .bind(b.1.to_string())
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO canvas_links (id, owner_id, a_kind, a_id, b_kind, b_id, drawn, \
                 created_at, updated_at, deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, NULL)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(owner.to_string())
            .bind(a.0.as_str())
            .bind(a.1.to_string())
            .bind(b.0.as_str())
            .bind(b.1.to_string())
            .bind(i64::from(drawn))
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Forget the owner's change to a link: a drawn one goes, a hidden one shows again.
    pub async fn reset_link(&self, owner: OwnerId, a: CardRef, b: CardRef) -> Result<()> {
        let (a, b) = pair(a, b);
        sqlx::query(
            "UPDATE canvas_links SET deleted_at = $1, updated_at = $1 WHERE owner_id = $2 \
             AND a_kind = $3 AND a_id = $4 AND b_kind = $5 AND b_id = $6 AND deleted_at IS NULL",
        )
        .bind(now_text())
        .bind(owner.to_string())
        .bind(a.0.as_str())
        .bind(a.1.to_string())
        .bind(b.0.as_str())
        .bind(b.1.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// A card, by what it shows and the record's id.
pub type CardRef = (CardKind, Uuid);

/// A pair of cards in the order they're stored, so (a, b) and (b, a) are the same link.
pub fn pair(a: CardRef, b: CardRef) -> (CardRef, CardRef) {
    let key = |c: &CardRef| (c.0.as_str(), c.1);
    if key(&a) <= key(&b) {
        (a, b)
    } else {
        (b, a)
    }
}

/// A project's area on the Desk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasArea {
    /// The project.
    pub project: ProjectId,
    /// Where it is.
    pub area: Area,
    /// Folded up to its name and a count.
    pub collapsed: bool,
}

/// A plain labelled box on the Desk: it only groups cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasLabel {
    /// Id.
    pub id: Uuid,
    /// Its name.
    pub label: String,
    /// Where it is.
    pub area: Area,
    /// Folded up to its name and a count.
    pub collapsed: bool,
}

/// A link the owner changed: drawn by hand, or an automatic one hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanvasLink {
    /// One end (the lower of the pair).
    pub a: CardRef,
    /// The other end.
    pub b: CardRef,
    /// Drawn by hand (true) or hidden (false).
    pub drawn: bool,
}

fn i32_col(row: &SqliteRow, col: &str) -> Result<i32> {
    i32::try_from(row.try_get::<i64, _>(col)?).map_err(|_| StorageError::Corrupt("canvas number"))
}

fn uuid_col(row: &SqliteRow, col: &str) -> Result<Uuid> {
    let text: String = row.try_get(col)?;
    Uuid::parse_str(&text).map_err(|_| StorageError::Corrupt("canvas id"))
}

fn kind_col(row: &SqliteRow, col: &str) -> Result<CardKind> {
    let text: String = row.try_get(col)?;
    text.parse()
        .map_err(|_| StorageError::Corrupt("canvas card kind"))
}

fn area_of(row: &SqliteRow) -> Result<Area> {
    Ok(Area {
        x: i32_col(row, "x")?,
        y: i32_col(row, "y")?,
        w: i32_col(row, "w")?,
        h: i32_col(row, "h")?,
    })
}

fn area_from_row(row: &SqliteRow) -> Result<CanvasArea> {
    Ok(CanvasArea {
        project: ProjectId::from_uuid(uuid_col(row, "project_id")?),
        area: area_of(row)?,
        collapsed: row.try_get("collapsed")?,
    })
}

fn label_from_row(row: &SqliteRow) -> Result<CanvasLabel> {
    Ok(CanvasLabel {
        id: uuid_col(row, "id")?,
        label: row.try_get("label")?,
        area: area_of(row)?,
        collapsed: row.try_get("collapsed")?,
    })
}

fn link_from_row(row: &SqliteRow) -> Result<CanvasLink> {
    Ok(CanvasLink {
        a: (kind_col(row, "a_kind")?, uuid_col(row, "a_id")?),
        b: (kind_col(row, "b_kind")?, uuid_col(row, "b_id")?),
        drawn: row.try_get::<i64, _>("drawn")? != 0,
    })
}
