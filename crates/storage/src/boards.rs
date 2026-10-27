//! Where the owner put each card on a board's own canvas (see `shiftbook_core::boards`). The
//! Desk keeps its spots in `canvas_cards`; every other board keeps them here, by its key, so
//! the same card can sit in different places on different boards.
//!
//! Owner-scoped, bound parameters only. Moving a card that has no spot yet creates one.

use shiftbook_core::boards::Board;
use shiftbook_core::canvas::{clamp_spot, CardKind};
use sqlx::Row;
use uuid::Uuid;

use crate::error::StorageError;
use crate::repo::{now_text, Result};
use crate::SqliteStore;
use shiftbook_core::ids::OwnerId;

/// One moved card's spot on a board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardSpot {
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

/// Only boards with a canvas of their own keep spots; the Desk keeps its own elsewhere.
fn board_key(board: Board) -> Result<String> {
    if board.has_canvas() {
        Ok(board.key())
    } else {
        Err(StorageError::NotFound)
    }
}

impl SqliteStore {
    /// Every moved card's spot on `board`, bottom first.
    pub async fn board_spots(&self, owner: OwnerId, board: Board) -> Result<Vec<BoardSpot>> {
        let rows = sqlx::query(
            "SELECT record_kind, record_id, x, y, z FROM board_cards \
             WHERE owner_id = $1 AND board = $2 AND deleted_at IS NULL ORDER BY z",
        )
        .bind(owner.to_string())
        .bind(board_key(board)?)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let corrupt = || StorageError::Corrupt("board card");
                let kind: String = row.try_get("record_kind")?;
                let id: String = row.try_get("record_id")?;
                Ok(BoardSpot {
                    kind: kind.parse().map_err(|_| corrupt())?,
                    record_id: Uuid::parse_str(&id).map_err(|_| corrupt())?,
                    x: i32::try_from(row.try_get::<i64, _>("x")?).map_err(|_| corrupt())?,
                    y: i32::try_from(row.try_get::<i64, _>("y")?).map_err(|_| corrupt())?,
                    z: row.try_get("z")?,
                })
            })
            .collect()
    }

    /// Put a card at `x`, `y` on `board`, on top of the others there.
    pub async fn move_on_board(
        &self,
        owner: OwnerId,
        board: Board,
        kind: CardKind,
        record_id: Uuid,
        x: i32,
        y: i32,
    ) -> Result<BoardSpot> {
        let key = board_key(board)?;
        let (x, y) = clamp_spot(x, y);
        let now = now_text();
        let mut tx = self.write_tx().await?;
        let top: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(z) FROM board_cards \
             WHERE owner_id = $1 AND board = $2 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(&key)
        .fetch_one(&mut *tx)
        .await?;
        let z = top.unwrap_or(0).saturating_add(1);
        let updated = sqlx::query(
            "UPDATE board_cards SET x = $1, y = $2, z = $3, updated_at = $4 \
             WHERE owner_id = $5 AND board = $6 AND record_kind = $7 AND record_id = $8 \
             AND deleted_at IS NULL",
        )
        .bind(i64::from(x))
        .bind(i64::from(y))
        .bind(z)
        .bind(&now)
        .bind(owner.to_string())
        .bind(&key)
        .bind(kind.as_str())
        .bind(record_id.to_string())
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO board_cards \
                 (id, owner_id, board, record_kind, record_id, x, y, z, created_at, updated_at, \
                 deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9, NULL)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(owner.to_string())
            .bind(&key)
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
        Ok(BoardSpot {
            kind,
            record_id,
            x,
            y,
            z,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{database_path, open_encrypted, test_dek};
    use crate::fsutil::test_dir::TestDir;
    use crate::test_rt::block_on;

    #[test]
    fn each_board_keeps_its_own_spots() {
        let dir = TestDir::new("boards");
        block_on(async {
            let s = SqliteStore::new(
                open_encrypted(&database_path(dir.path()), &test_dek(12))
                    .await
                    .unwrap(),
            );
            let me = s.initialize_local_owner("Me").await.unwrap().id;
            let project = Board::Project(Uuid::now_v7());
            let note = Uuid::now_v7();

            // The same card sits in different places on different boards, and not on the Desk.
            s.move_on_board(me, project, CardKind::Note, note, 10, 20)
                .await
                .unwrap();
            s.move_on_board(me, Board::Daily, CardKind::Note, note, 300, 40)
                .await
                .unwrap();
            assert!(s.canvas_spots(me).await.unwrap().is_empty());
            let spots = s.board_spots(me, project).await.unwrap();
            assert_eq!(spots.len(), 1);
            assert_eq!((spots[0].x, spots[0].y), (10, 20));

            // Moving again updates it and puts it on top.
            let other = Uuid::now_v7();
            let below = s
                .move_on_board(me, project, CardKind::Entry, other, 0, 0)
                .await
                .unwrap();
            let again = s
                .move_on_board(me, project, CardKind::Note, note, 50, 60)
                .await
                .unwrap();
            assert!(again.z > below.z);
            let spots = s.board_spots(me, project).await.unwrap();
            assert_eq!(spots.len(), 2);
            assert_eq!(spots.last().unwrap().record_id, note);
            assert_eq!(s.board_spots(me, Board::Daily).await.unwrap()[0].x, 300);

            // Lists of boards have no canvas.
            assert!(s.board_spots(me, Board::Projects).await.is_err());
            assert!(s
                .move_on_board(me, Board::Desk, CardKind::Note, note, 0, 0)
                .await
                .is_err());
        });
    }
}
