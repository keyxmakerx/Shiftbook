//! The Canvas layout: where the owner put each card (see `shiftbook_core::canvas`).
//!
//! Owner-scoped, bound parameters only. Moving a card that has no spot yet creates one.

use shiftbook_core::canvas::{clamp_spot, CardKind};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

use crate::error::StorageError;
use crate::repo::{now_text, Result};
use crate::SqliteStore;
use shiftbook_core::ids::OwnerId;

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
    })
}

impl SqliteStore {
    /// Every moved card's spot.
    pub async fn canvas_spots(&self, owner: OwnerId) -> Result<Vec<CanvasSpot>> {
        let rows = sqlx::query(
            "SELECT record_kind, record_id, x, y, z FROM canvas_cards \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY z",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(spot_from_row).collect()
    }

    /// Put a card at `x`, `y` and on top of the others.
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
        tx.commit().await?;
        Ok(CanvasSpot {
            kind,
            record_id,
            x,
            y,
            z,
        })
    }
}
