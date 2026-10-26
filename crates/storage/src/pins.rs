//! Pins (see `shiftbook_core::pins`): records kept in view on the bottom bar, each with an
//! optional chase date. Owner-scoped, bound parameters only.

use shiftbook_core::canvas::CardKind;
use shiftbook_core::dates::{format_date, parse_date};
use shiftbook_core::ids::OwnerId;
use shiftbook_core::pins::{check_pin, Pin};
use sqlx::Row;
use time::Date;
use uuid::Uuid;

use crate::error::StorageError;
use crate::repo::{now_text, parse_ts, parse_uuid, Result};
use crate::SqliteStore;

impl SqliteStore {
    /// Every pin, in the order they were pinned.
    pub async fn pins(&self, owner: OwnerId) -> Result<Vec<Pin>> {
        let rows = sqlx::query(
            "SELECT record_kind, record_id, chase_on, created_at FROM pins \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY created_at, id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let kind: String = row.try_get("record_kind")?;
                let chase: Option<String> = row.try_get("chase_on")?;
                Ok(Pin {
                    kind: kind.parse().map_err(|_| StorageError::Corrupt("pin"))?,
                    record_id: parse_uuid(&row.try_get::<String, _>("record_id")?)?,
                    chase_on: chase
                        .map(|d| parse_date(&d).map_err(|_| StorageError::Corrupt("date")))
                        .transpose()?,
                    created_at: parse_ts(&row.try_get::<String, _>("created_at")?)?,
                })
            })
            .collect()
    }

    /// Pin a record, or change the chase date of one already pinned (`None` clears it).
    pub async fn pin(
        &self,
        owner: OwnerId,
        kind: CardKind,
        record_id: Uuid,
        chase_on: Option<Date>,
    ) -> Result<()> {
        let now = now_text();
        let chase = chase_on.map(format_date);
        let mut tx = self.pool.begin().await?;
        // Check before writing anything: a refused pin must not leave a write lock behind.
        let pinned: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pins WHERE owner_id = $1 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let already: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pins WHERE owner_id = $1 AND record_kind = $2 \
             AND record_id = $3 AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(kind.as_str())
        .bind(record_id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        check_pin(kind, already, usize::try_from(pinned).unwrap_or(usize::MAX))?;
        if already {
            sqlx::query(
                "UPDATE pins SET chase_on = $1, updated_at = $2 WHERE owner_id = $3 \
                 AND record_kind = $4 AND record_id = $5 AND deleted_at IS NULL",
            )
            .bind(&chase)
            .bind(&now)
            .bind(owner.to_string())
            .bind(kind.as_str())
            .bind(record_id.to_string())
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO pins (id, owner_id, record_kind, record_id, chase_on, created_at, \
                 updated_at, deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $6, NULL)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(owner.to_string())
            .bind(kind.as_str())
            .bind(record_id.to_string())
            .bind(&chase)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Take a record's pin off the bar (nothing happens if it isn't pinned).
    pub async fn unpin(&self, owner: OwnerId, kind: CardKind, record_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE pins SET deleted_at = $1, updated_at = $1 WHERE owner_id = $2 \
             AND record_kind = $3 AND record_id = $4 AND deleted_at IS NULL",
        )
        .bind(now_text())
        .bind(owner.to_string())
        .bind(kind.as_str())
        .bind(record_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Follow up (round 15, card r15-things): the day each card is to be followed up, from
    /// a thing brought up for a day or a pin's chase day, the earlier when it has both.
    pub async fn follow_ups(&self, owner: OwnerId) -> Result<Vec<(CardKind, Uuid, Date)>> {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT about_kind, about_id, show_from FROM next_up_items WHERE owner_id = $1 \
             AND show_from IS NOT NULL AND about_kind IS NOT NULL AND about_id IS NOT NULL \
             AND done_at IS NULL AND deleted_at IS NULL \
             UNION ALL \
             SELECT record_kind, record_id, chase_on FROM pins WHERE owner_id = $1 \
             AND chase_on IS NOT NULL AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        let mut out: Vec<(CardKind, Uuid, Date)> = Vec::new();
        for (kind, id, on) in rows {
            let kind: CardKind = kind
                .parse()
                .map_err(|_| StorageError::Corrupt("follow-up"))?;
            let id = parse_uuid(&id)?;
            let on = parse_date(&on).map_err(|_| StorageError::Corrupt("date"))?;
            match out.iter_mut().find(|(k, i, _)| *k == kind && *i == id) {
                Some(f) => f.2 = f.2.min(on),
                None => out.push((kind, id, on)),
            }
        }
        Ok(out)
    }

    /// "No follow-up": take a card's follow-up day away, whether brought up or a chase day.
    pub async fn clear_follow_up(&self, owner: OwnerId, kind: CardKind, id: Uuid) -> Result<()> {
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE next_up_items SET deleted_at = $1, updated_at = $1 WHERE owner_id = $2 \
             AND about_kind = $3 AND about_id = $4 AND show_from IS NOT NULL \
             AND done_at IS NULL AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(owner.to_string())
        .bind(kind.as_str())
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE pins SET chase_on = NULL, updated_at = $1 WHERE owner_id = $2 \
             AND record_kind = $3 AND record_id = $4 AND chase_on IS NOT NULL",
        )
        .bind(&now)
        .bind(owner.to_string())
        .bind(kind.as_str())
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{database_path, open_encrypted, test_dek};
    use crate::fsutil::test_dir::TestDir;
    use crate::test_rt::block_on;
    use shiftbook_core::pins::MAX_PINS;
    use time::macros::date;

    #[test]
    fn pin_chase_and_unpin() {
        let dir = TestDir::new("pins");
        block_on(async {
            let s = SqliteStore::new(
                open_encrypted(&database_path(dir.path()), &test_dek(13))
                    .await
                    .unwrap(),
            );
            let me = s.initialize_local_owner("Me").await.unwrap().id;
            let note = Uuid::now_v7();
            s.pin(me, CardKind::Note, note, None).await.unwrap();
            // Pinning again only changes the chase date.
            s.pin(me, CardKind::Note, note, Some(date!(2026 - 10 - 21)))
                .await
                .unwrap();
            let pins = s.pins(me).await.unwrap();
            assert_eq!(pins.len(), 1);
            assert_eq!(pins[0].chase_on, Some(date!(2026 - 10 - 21)));
            s.pin(me, CardKind::Note, note, None).await.unwrap();
            assert_eq!(s.pins(me).await.unwrap()[0].chase_on, None);

            // Scraps can't be pinned; there's a limit.
            assert!(s
                .pin(me, CardKind::Inbox, Uuid::now_v7(), None)
                .await
                .is_err());
            for _ in 1..MAX_PINS {
                s.pin(me, CardKind::Tray, Uuid::now_v7(), None)
                    .await
                    .unwrap();
            }
            assert!(s
                .pin(me, CardKind::Entry, Uuid::now_v7(), None)
                .await
                .is_err());
            // A full bar still lets a pin's chase date change, and unpinning makes room.
            s.pin(me, CardKind::Note, note, Some(date!(2026 - 10 - 22)))
                .await
                .unwrap();
            s.unpin(me, CardKind::Note, note).await.unwrap();
            assert_eq!(s.pins(me).await.unwrap().len(), MAX_PINS - 1);
            s.pin(me, CardKind::Note, note, None).await.unwrap();
            assert_eq!(s.pins(me).await.unwrap().len(), MAX_PINS);
        });
    }
}
