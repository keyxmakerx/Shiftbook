//! Meeting series: the repeating meetings the bell reminds about (see
//! `shiftbook_core::meetings`). Owner-scoped, bound parameters only, soft-deleted.

use shiftbook_core::ids::{MeetingSeriesId, OwnerId};
use shiftbook_core::meetings::{format_days, parse_days, validate_series};
use shiftbook_core::model::MeetingSeries;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::error::StorageError;
use crate::repo::{now_text, parse_opt_ts, parse_ts, parse_uuid, u32_from, Result};
use crate::SqliteStore;

/// The editable parts of a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesInput {
    /// Name, e.g. "Weekly ops".
    pub name: String,
    /// Weekdays it meets, Monday first.
    pub days: [bool; 7],
    /// Start in minutes after local midnight.
    pub at: u32,
    /// Minutes ahead the bell starts reminding.
    pub remind_minutes: u32,
}

fn series_from_row(row: &SqliteRow) -> Result<MeetingSeries> {
    let days: String = row.try_get("days")?;
    Ok(MeetingSeries {
        id: MeetingSeriesId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        name: row.try_get("name")?,
        days: parse_days(&days).ok_or(StorageError::Corrupt("meeting days"))?,
        at: u32_from(row.try_get("at_minute")?, "time")?,
        remind_minutes: u32_from(row.try_get("remind_minutes")?, "reminder")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

macro_rules! series_columns {
    () => {
        "id, owner_id, name, days, at_minute, remind_minutes, created_at, updated_at, deleted_at"
    };
}

impl SqliteStore {
    /// The live meeting series, by start time then name.
    pub async fn meeting_series(&self, owner: OwnerId) -> Result<Vec<MeetingSeries>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            series_columns!(),
            " FROM meeting_series WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY at_minute, lower(name)"
        ))
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(series_from_row).collect()
    }

    async fn one_series(&self, owner: OwnerId, id: MeetingSeriesId) -> Result<MeetingSeries> {
        let row = sqlx::query(concat!(
            "SELECT ",
            series_columns!(),
            " FROM meeting_series WHERE owner_id = $1 AND id = $2"
        ))
        .bind(owner.to_string())
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        series_from_row(&row.ok_or(StorageError::NotFound)?)
    }

    /// Add a meeting series.
    pub async fn create_meeting_series(
        &self,
        owner: OwnerId,
        input: &SeriesInput,
    ) -> Result<MeetingSeries> {
        let name = validate_series(&input.name, &input.days, input.at, input.remind_minutes)?;
        let id = MeetingSeriesId::new();
        sqlx::query(concat!(
            "INSERT INTO meeting_series (",
            series_columns!(),
            ") VALUES ($1, $2, $3, $4, $5, $6, $7, $7, NULL)"
        ))
        .bind(id.to_string())
        .bind(owner.to_string())
        .bind(&name)
        .bind(format_days(&input.days))
        .bind(i64::from(input.at))
        .bind(i64::from(input.remind_minutes))
        .bind(now_text())
        .execute(&self.pool)
        .await?;
        self.one_series(owner, id).await
    }

    /// Change a meeting series.
    pub async fn update_meeting_series(
        &self,
        owner: OwnerId,
        id: MeetingSeriesId,
        input: &SeriesInput,
    ) -> Result<MeetingSeries> {
        let name = validate_series(&input.name, &input.days, input.at, input.remind_minutes)?;
        let done = sqlx::query(
            "UPDATE meeting_series SET name = $1, days = $2, at_minute = $3, remind_minutes = $4, \
             updated_at = $5 WHERE id = $6 AND owner_id = $7 AND deleted_at IS NULL",
        )
        .bind(&name)
        .bind(format_days(&input.days))
        .bind(i64::from(input.at))
        .bind(i64::from(input.remind_minutes))
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(StorageError::NotFound);
        }
        self.one_series(owner, id).await
    }

    /// Remove a meeting series (soft delete), or with `deleted = false` bring it back.
    /// Its meeting notes stay.
    pub async fn set_meeting_series_deleted(
        &self,
        owner: OwnerId,
        id: MeetingSeriesId,
        deleted: bool,
    ) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE meeting_series SET deleted_at = $1, updated_at = $2 \
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
}
