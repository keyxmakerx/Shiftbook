//! Timesheet codes and category defaults, crib sheet tick-offs, custom fields
//! and their values, the Inbox, and preferences.
//!
//! Owner-scoped, bound parameters only, soft deletes.

use shiftbook_core::canvas::CardKind;
use shiftbook_core::dates::{format_date, parse_date};
use shiftbook_core::fields::{
    join_options, parse_options, validate_field, validate_value, AppliesTo, FieldKind,
};
use shiftbook_core::ids::{
    CategoryCodeDefaultId, CustomFieldId, CustomValueId, InboxItemId, NextUpItemId, OwnerId,
    SavedResponseId, TimesheetCodeId, TimesheetTickId,
};
use shiftbook_core::model::{
    CategoryCodeDefault, CustomField, CustomValue, InboxItem, NextUpItem, SavedResponse,
    TimesheetCode, TimesheetTick,
};
use shiftbook_core::nextup::{bring_up_text, clean_next_up};
use shiftbook_core::prefs::{prefs_from_settings, validate_pref, Prefs, KEY_TICKETS_CODE};
use shiftbook_core::responses::{clean_responses, ResponseDraft};
use shiftbook_core::validate::{
    validate_list_value, validate_required, validate_timesheet_code, MAX_LINE_CHARS,
};
use sqlx::sqlite::{SqliteConnection, SqliteRow};
use sqlx::Row;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::error::StorageError;
use crate::repo::{
    i32_from, now_text, parse_opt_ts, parse_ts, parse_uuid, Result, SettingsRepository,
};
use crate::work::code_exists;
use crate::SqliteStore;

/// The editable parts of a timesheet code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeInput {
    /// Area, e.g. "Operations".
    pub area: String,
    /// Phase label.
    pub phase_label: String,
    /// The code.
    pub code: String,
    /// Position.
    pub sort_order: i32,
    /// Hidden from pickers.
    pub hidden: bool,
}

/// The editable parts of a custom field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInput {
    /// Which records it appears on.
    pub applies_to: AppliesTo,
    /// Name.
    pub name: String,
    /// Kind.
    pub kind: FieldKind,
    /// Options text (choice fields), one per line or comma separated.
    pub options: String,
    /// Link template with `{ticket}`.
    pub link_template: String,
    /// Ticket prefix, e.g. `PROJ-`.
    pub ticket_prefix: String,
    /// Hidden.
    pub hidden: bool,
}

fn corrupt_date(text: &str) -> Result<Date> {
    parse_date(text).map_err(|_| StorageError::Corrupt("date"))
}

macro_rules! code_columns {
    () => {
        "id, owner_id, area, phase_label, code, sort_order, hidden, created_at, updated_at, \
         deleted_at"
    };
}

fn code_from_row(row: &SqliteRow) -> Result<TimesheetCode> {
    Ok(TimesheetCode {
        id: TimesheetCodeId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        area: row.try_get("area")?,
        phase_label: row.try_get("phase_label")?,
        code: row.try_get("code")?,
        sort_order: i32_from(row.try_get("sort_order")?)?,
        hidden: row.try_get("hidden")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn checked_code(owner: OwnerId, input: &CodeInput) -> Result<TimesheetCode> {
    let now = OffsetDateTime::now_utc();
    let code = TimesheetCode {
        id: TimesheetCodeId::new(),
        owner_id: owner,
        area: input.area.trim().to_string(),
        phase_label: input.phase_label.trim().to_string(),
        code: input.code.trim().to_string(),
        sort_order: input.sort_order,
        hidden: input.hidden,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    validate_timesheet_code(&code)?;
    Ok(code)
}

async fn fetch_code(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: TimesheetCodeId,
) -> Result<TimesheetCode> {
    let row = sqlx::query(concat!(
        "SELECT ",
        code_columns!(),
        " FROM timesheet_codes WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    code_from_row(&row)
}

fn mapping_from_row(row: &SqliteRow) -> Result<CategoryCodeDefault> {
    Ok(CategoryCodeDefault {
        id: CategoryCodeDefaultId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        category: row.try_get("category")?,
        timesheet_code_id: TimesheetCodeId::from_uuid(parse_uuid(
            row.try_get("timesheet_code_id")?,
        )?),
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn tick_from_row(row: &SqliteRow) -> Result<TimesheetTick> {
    Ok(TimesheetTick {
        id: TimesheetTickId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        week_start: corrupt_date(row.try_get("week_start")?)?,
        day: corrupt_date(row.try_get("day")?)?,
        line_key: row.try_get("line_key")?,
        done: row.try_get("done")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

macro_rules! field_columns {
    () => {
        "id, owner_id, applies_to, name, kind, options, link_template, ticket_prefix, sort_order, \
         hidden, created_at, updated_at, deleted_at"
    };
}

fn field_from_row(row: &SqliteRow) -> Result<CustomField> {
    let options: String = row.try_get("options")?;
    Ok(CustomField {
        id: CustomFieldId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        applies_to: AppliesTo::parse(row.try_get("applies_to")?)
            .ok_or(StorageError::Corrupt("applies to"))?,
        name: row.try_get("name")?,
        kind: FieldKind::parse(row.try_get("kind")?).ok_or(StorageError::Corrupt("kind"))?,
        options: parse_options(&options),
        link_template: row.try_get("link_template")?,
        ticket_prefix: row.try_get("ticket_prefix")?,
        sort_order: i32_from(row.try_get("sort_order")?)?,
        hidden: row.try_get("hidden")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

async fn fetch_field(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    id: CustomFieldId,
) -> Result<CustomField> {
    let row = sqlx::query(concat!(
        "SELECT ",
        field_columns!(),
        " FROM custom_fields WHERE id = $1 AND owner_id = $2 AND deleted_at IS NULL"
    ))
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    field_from_row(&row)
}

async fn ensure_unique_field(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    applies_to: AppliesTo,
    name: &str,
    except: Option<CustomFieldId>,
) -> Result<()> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM custom_fields WHERE owner_id = $1 AND applies_to = $2 \
         AND deleted_at IS NULL",
    )
    .bind(owner.to_string())
    .bind(applies_to.as_str())
    .fetch_all(&mut *conn)
    .await?;
    let wanted = name.to_lowercase();
    let except = except.map(|id| id.to_string());
    if rows
        .iter()
        .any(|(id, n)| Some(id) != except.as_ref() && n.trim().to_lowercase() == wanted)
    {
        return Err(StorageError::Duplicate(name.to_string()));
    }
    Ok(())
}

fn value_from_row(row: &SqliteRow) -> Result<CustomValue> {
    Ok(CustomValue {
        id: CustomValueId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        field_id: CustomFieldId::from_uuid(parse_uuid(row.try_get("field_id")?)?),
        record_id: parse_uuid(row.try_get("record_id")?)?,
        value: row.try_get("value")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn inbox_from_row(row: &SqliteRow) -> Result<InboxItem> {
    Ok(InboxItem {
        id: InboxItemId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        text: row.try_get("text")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn next_up_from_row(row: &SqliteRow) -> Result<NextUpItem> {
    Ok(NextUpItem {
        id: NextUpItemId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        position: u32::try_from(row.try_get::<i64, _>("position")?).unwrap_or(0),
        text: row.try_get("text")?,
        show_from: row
            .try_get::<Option<String>, _>("show_from")?
            .as_deref()
            .map(|d| parse_date(d).map_err(|_| StorageError::Corrupt("date")))
            .transpose()?,
        about: match (
            row.try_get::<Option<String>, _>("about_kind")?,
            row.try_get::<Option<String>, _>("about_id")?,
        ) {
            (Some(kind), Some(id)) => Some((
                kind.parse()
                    .map_err(|_| StorageError::Corrupt("card kind"))?,
                parse_uuid(&id)?,
            )),
            _ => None,
        },
        done_at: parse_opt_ts(row.try_get("done_at")?)?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn response_from_row(row: &SqliteRow) -> Result<SavedResponse> {
    Ok(SavedResponse {
        id: SavedResponseId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        position: u32::try_from(row.try_get::<i64, _>("position")?).unwrap_or(0),
        name: row.try_get("name")?,
        text: row.try_get("text")?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn not_found_if_zero(done: sqlx::sqlite::SqliteQueryResult) -> Result<()> {
    if done.rows_affected() == 0 {
        Err(StorageError::NotFound)
    } else {
        Ok(())
    }
}

/// The two name → code tables: categories, and kinds of repeating thing.
#[derive(Clone, Copy)]
enum NamedCodes {
    Category,
    Kind,
}

impl NamedCodes {
    /// Its queries: find by name, change the code, remove, add.
    fn sql(self) -> [&'static str; 4] {
        macro_rules! queries {
            ($table:literal, $column:literal) => {
                [
                    concat!(
                        "SELECT id FROM ",
                        $table,
                        " WHERE owner_id = $1 AND lower(",
                        $column,
                        ") = lower($2) AND deleted_at IS NULL"
                    ),
                    concat!(
                        "UPDATE ",
                        $table,
                        " SET timesheet_code_id = $1, updated_at = $2 ",
                        "WHERE id = $3 AND owner_id = $4"
                    ),
                    concat!(
                        "UPDATE ",
                        $table,
                        " SET deleted_at = $1, updated_at = $1 ",
                        "WHERE id = $2 AND owner_id = $3"
                    ),
                    concat!(
                        "INSERT INTO ",
                        $table,
                        " (id, owner_id, ",
                        $column,
                        ", timesheet_code_id, ",
                        "created_at, updated_at, deleted_at) VALUES ($1, $2, $3, $4, $5, $5, NULL)"
                    ),
                ]
            };
        }
        match self {
            NamedCodes::Category => queries!("category_code_defaults", "category"),
            NamedCodes::Kind => queries!("kind_code_defaults", "kind"),
        }
    }
}

impl SqliteStore {
    // ---------- timesheet codes ----------

    /// Live codes (hidden ones too) in timesheet order.
    pub async fn list_timesheet_codes(&self, owner: OwnerId) -> Result<Vec<TimesheetCode>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            code_columns!(),
            " FROM timesheet_codes WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY sort_order, lower(area), lower(code), id"
        ))
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(code_from_row).collect()
    }

    /// Add a code. Its order is after the last one.
    pub async fn add_timesheet_code(
        &self,
        owner: OwnerId,
        input: &CodeInput,
    ) -> Result<TimesheetCode> {
        let mut code = checked_code(owner, input)?;
        let mut tx = self.pool.begin().await?;
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM timesheet_codes WHERE owner_id = $1",
        )
        .bind(owner.to_string())
        .fetch_one(&mut *tx)
        .await?;
        code.sort_order = i32_from(next)?;
        let now = now_text();
        sqlx::query(concat!(
            "INSERT INTO timesheet_codes (",
            code_columns!(),
            ") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8, NULL)"
        ))
        .bind(code.id.to_string())
        .bind(owner.to_string())
        .bind(&code.area)
        .bind(&code.phase_label)
        .bind(&code.code)
        .bind(i64::from(code.sort_order))
        .bind(code.hidden)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        let saved = fetch_code(&mut tx, owner, code.id).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Change a code (area, phase, code, order, hidden).
    pub async fn update_timesheet_code(
        &self,
        owner: OwnerId,
        id: TimesheetCodeId,
        input: &CodeInput,
    ) -> Result<TimesheetCode> {
        let code = checked_code(owner, input)?;
        let mut tx = self.pool.begin().await?;
        not_found_if_zero(
            sqlx::query(
                "UPDATE timesheet_codes SET area = $1, phase_label = $2, code = $3, sort_order = $4, \
                 hidden = $5, updated_at = $6 WHERE id = $7 AND owner_id = $8 AND deleted_at IS NULL",
            )
            .bind(&code.area)
            .bind(&code.phase_label)
            .bind(&code.code)
            .bind(i64::from(code.sort_order))
            .bind(code.hidden)
            .bind(now_text())
            .bind(id.to_string())
            .bind(owner.to_string())
            .execute(&mut *tx)
            .await?,
        )?;
        let saved = fetch_code(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Live category → code mappings.
    pub async fn list_category_codes(&self, owner: OwnerId) -> Result<Vec<CategoryCodeDefault>> {
        let rows = sqlx::query(
            "SELECT id, owner_id, category, timesheet_code_id, created_at, updated_at, deleted_at \
             FROM category_code_defaults WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY lower(category)",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(mapping_from_row).collect()
    }

    /// Map a category to a code, or with `None` remove its mapping.
    pub async fn set_category_code(
        &self,
        owner: OwnerId,
        category: &str,
        code: Option<TimesheetCodeId>,
    ) -> Result<()> {
        self.set_named_code(NamedCodes::Category, owner, category, code)
            .await
    }

    /// Live kind → code mappings (a kind of repeating thing, e.g. "Certificates"), by kind.
    pub async fn list_kind_codes(&self, owner: OwnerId) -> Result<Vec<(String, TimesheetCodeId)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT kind, timesheet_code_id FROM kind_code_defaults \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY lower(kind)",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(kind, code)| Ok((kind, TimesheetCodeId::from_uuid(parse_uuid(&code)?))))
            .collect()
    }

    /// Map a kind of repeating thing to a code, or with `None` remove its mapping.
    pub async fn set_kind_code(
        &self,
        owner: OwnerId,
        kind: &str,
        code: Option<TimesheetCodeId>,
    ) -> Result<()> {
        self.set_named_code(NamedCodes::Kind, owner, kind, code)
            .await
    }

    async fn set_named_code(
        &self,
        table: NamedCodes,
        owner: OwnerId,
        name: &str,
        code: Option<TimesheetCodeId>,
    ) -> Result<()> {
        let [find, change, remove, add] = table.sql();
        let name = validate_list_value(name)?;
        let mut tx = self.pool.begin().await?;
        let now = now_text();
        let existing: Option<String> = sqlx::query_scalar(find)
            .bind(owner.to_string())
            .bind(&name)
            .fetch_optional(&mut *tx)
            .await?;
        match (existing, code) {
            (Some(id), Some(code)) => {
                code_exists(&mut tx, owner, code).await?;
                sqlx::query(change)
                    .bind(code.to_string())
                    .bind(&now)
                    .bind(id)
                    .bind(owner.to_string())
                    .execute(&mut *tx)
                    .await?;
            }
            (Some(id), None) => {
                sqlx::query(remove)
                    .bind(&now)
                    .bind(id)
                    .bind(owner.to_string())
                    .execute(&mut *tx)
                    .await?;
            }
            (None, Some(code)) => {
                code_exists(&mut tx, owner, code).await?;
                sqlx::query(add)
                    .bind(CategoryCodeDefaultId::new().to_string())
                    .bind(owner.to_string())
                    .bind(&name)
                    .bind(code.to_string())
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
            }
            (None, None) => {}
        }
        tx.commit().await?;
        Ok(())
    }

    // ---------- crib sheet tick-offs ----------

    /// Tick-offs recorded for a week.
    pub async fn list_ticks(&self, owner: OwnerId, week_start: Date) -> Result<Vec<TimesheetTick>> {
        let rows = sqlx::query(
            "SELECT id, owner_id, week_start, day, line_key, done, created_at, updated_at, \
             deleted_at FROM timesheet_ticks WHERE owner_id = $1 AND week_start = $2 \
             AND deleted_at IS NULL ORDER BY day, line_key",
        )
        .bind(owner.to_string())
        .bind(format_date(week_start))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(tick_from_row).collect()
    }

    /// Tick or untick one line of one day.
    pub async fn set_tick(
        &self,
        owner: OwnerId,
        week_start: Date,
        day: Date,
        line_key: &str,
        done: bool,
    ) -> Result<()> {
        let key = validate_required("Line", line_key, 100)?;
        let mut tx = self.pool.begin().await?;
        let now = now_text();
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM timesheet_ticks WHERE owner_id = $1 AND day = $2 AND line_key = $3 \
             AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(format_date(day))
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?;
        match existing {
            Some(id) => {
                sqlx::query(
                    "UPDATE timesheet_ticks SET done = $1, week_start = $2, updated_at = $3 \
                     WHERE id = $4 AND owner_id = $5",
                )
                .bind(done)
                .bind(format_date(week_start))
                .bind(&now)
                .bind(id)
                .bind(owner.to_string())
                .execute(&mut *tx)
                .await?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO timesheet_ticks (id, owner_id, week_start, day, line_key, done, \
                     created_at, updated_at, deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $7, NULL)",
                )
                .bind(TimesheetTickId::new().to_string())
                .bind(owner.to_string())
                .bind(format_date(week_start))
                .bind(format_date(day))
                .bind(&key)
                .bind(done)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    // ---------- custom fields ----------

    /// Live custom fields (hidden ones too), in order.
    pub async fn list_custom_fields(&self, owner: OwnerId) -> Result<Vec<CustomField>> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            field_columns!(),
            " FROM custom_fields WHERE owner_id = $1 AND deleted_at IS NULL \
             ORDER BY sort_order, lower(name), id"
        ))
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(field_from_row).collect()
    }

    /// Add a custom field at the end.
    pub async fn add_custom_field(
        &self,
        owner: OwnerId,
        input: &FieldInput,
    ) -> Result<CustomField> {
        let clean = validate_field(
            &input.name,
            input.kind,
            &input.options,
            &input.link_template,
            &input.ticket_prefix,
        )?;
        let mut tx = self.pool.begin().await?;
        ensure_unique_field(&mut tx, owner, input.applies_to, &clean.name, None).await?;
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM custom_fields WHERE owner_id = $1",
        )
        .bind(owner.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let id = CustomFieldId::new();
        sqlx::query(concat!(
            "INSERT INTO custom_fields (",
            field_columns!(),
            ") VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11, NULL)"
        ))
        .bind(id.to_string())
        .bind(owner.to_string())
        .bind(input.applies_to.as_str())
        .bind(&clean.name)
        .bind(input.kind.as_str())
        .bind(join_options(&clean.options))
        .bind(clean.link_template.as_deref())
        .bind(clean.ticket_prefix.as_deref())
        .bind(next)
        .bind(input.hidden)
        .bind(now_text())
        .execute(&mut *tx)
        .await?;
        let saved = fetch_field(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Change a custom field. Existing values are kept as they are.
    pub async fn update_custom_field(
        &self,
        owner: OwnerId,
        id: CustomFieldId,
        input: &FieldInput,
    ) -> Result<CustomField> {
        let clean = validate_field(
            &input.name,
            input.kind,
            &input.options,
            &input.link_template,
            &input.ticket_prefix,
        )?;
        let mut tx = self.pool.begin().await?;
        fetch_field(&mut tx, owner, id).await?;
        ensure_unique_field(&mut tx, owner, input.applies_to, &clean.name, Some(id)).await?;
        sqlx::query(
            "UPDATE custom_fields SET applies_to = $1, name = $2, kind = $3, options = $4, \
             link_template = $5, ticket_prefix = $6, hidden = $7, updated_at = $8 \
             WHERE id = $9 AND owner_id = $10",
        )
        .bind(input.applies_to.as_str())
        .bind(&clean.name)
        .bind(input.kind.as_str())
        .bind(join_options(&clean.options))
        .bind(clean.link_template.as_deref())
        .bind(clean.ticket_prefix.as_deref())
        .bind(input.hidden)
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let saved = fetch_field(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(saved)
    }

    /// Remove a custom field (soft delete); its values stay but are no longer shown.
    pub async fn delete_custom_field(&self, owner: OwnerId, id: CustomFieldId) -> Result<()> {
        not_found_if_zero(
            sqlx::query(
                "UPDATE custom_fields SET deleted_at = $1, updated_at = $1 \
                 WHERE id = $2 AND owner_id = $3 AND deleted_at IS NULL",
            )
            .bind(now_text())
            .bind(id.to_string())
            .bind(owner.to_string())
            .execute(&self.pool)
            .await?,
        )
    }

    /// Live values on one record.
    pub async fn list_custom_values(
        &self,
        owner: OwnerId,
        record: Uuid,
    ) -> Result<Vec<CustomValue>> {
        let rows = sqlx::query(
            "SELECT v.id, v.owner_id, v.field_id, v.record_id, v.value, v.created_at, v.updated_at, \
             v.deleted_at FROM custom_values v JOIN custom_fields f ON f.id = v.field_id \
             WHERE v.owner_id = $1 AND v.record_id = $2 AND v.deleted_at IS NULL \
             AND f.deleted_at IS NULL ORDER BY f.sort_order, v.id",
        )
        .bind(owner.to_string())
        .bind(record.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(value_from_row).collect()
    }

    /// Set a field's value on a record (checked against the field's kind);
    /// blank clears it. Returns the stored value.
    pub async fn set_custom_value(
        &self,
        owner: OwnerId,
        field: CustomFieldId,
        record: Uuid,
        value: &str,
    ) -> Result<String> {
        let mut tx = self.pool.begin().await?;
        let def = fetch_field(&mut tx, owner, field).await?;
        let value = validate_value(def.kind, &def.options, value)?;
        let now = now_text();
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM custom_values WHERE owner_id = $1 AND field_id = $2 AND record_id = $3 \
             AND deleted_at IS NULL",
        )
        .bind(owner.to_string())
        .bind(field.to_string())
        .bind(record.to_string())
        .fetch_optional(&mut *tx)
        .await?;
        match (existing, value.is_empty()) {
            (Some(id), true) => {
                sqlx::query(
                    "UPDATE custom_values SET deleted_at = $1, updated_at = $1 WHERE id = $2 \
                     AND owner_id = $3",
                )
                .bind(&now)
                .bind(id)
                .bind(owner.to_string())
                .execute(&mut *tx)
                .await?;
            }
            (Some(id), false) => {
                sqlx::query(
                    "UPDATE custom_values SET value = $1, updated_at = $2 WHERE id = $3 \
                     AND owner_id = $4",
                )
                .bind(&value)
                .bind(&now)
                .bind(id)
                .bind(owner.to_string())
                .execute(&mut *tx)
                .await?;
            }
            (None, false) => {
                sqlx::query(
                    "INSERT INTO custom_values (id, owner_id, field_id, record_id, value, created_at, \
                     updated_at, deleted_at) VALUES ($1, $2, $3, $4, $5, $6, $6, NULL)",
                )
                .bind(CustomValueId::new().to_string())
                .bind(owner.to_string())
                .bind(field.to_string())
                .bind(record.to_string())
                .bind(&value)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
            }
            (None, true) => {}
        }
        tx.commit().await?;
        Ok(value)
    }

    // ---------- inbox ----------

    /// Live Inbox scraps, newest first.
    pub async fn list_inbox(&self, owner: OwnerId) -> Result<Vec<InboxItem>> {
        let rows = sqlx::query(
            "SELECT id, owner_id, text, created_at, updated_at, deleted_at FROM inbox_items \
             WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY id DESC",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(inbox_from_row).collect()
    }

    /// Save a scrap to the Inbox.
    pub async fn add_inbox(&self, owner: OwnerId, text: &str) -> Result<InboxItem> {
        let text = validate_required("Text", text, MAX_LINE_CHARS)?;
        let now = parse_ts(&now_text())?;
        let item = InboxItem {
            id: InboxItemId::new(),
            owner_id: owner,
            text,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        sqlx::query(
            "INSERT INTO inbox_items (id, owner_id, text, created_at, updated_at, deleted_at) \
             VALUES ($1, $2, $3, $4, $4, NULL)",
        )
        .bind(item.id.to_string())
        .bind(owner.to_string())
        .bind(&item.text)
        .bind(now_text())
        .execute(&self.pool)
        .await?;
        Ok(item)
    }

    /// Remove a scrap (soft delete), or with `restore` bring it back.
    pub async fn set_inbox_deleted(
        &self,
        owner: OwnerId,
        id: InboxItemId,
        delete: bool,
    ) -> Result<()> {
        let now = now_text();
        not_found_if_zero(
            sqlx::query(
                "UPDATE inbox_items SET deleted_at = $1, updated_at = $2 \
                 WHERE id = $3 AND owner_id = $4 AND ((deleted_at IS NULL) = $5)",
            )
            .bind(if delete { Some(now.as_str()) } else { None })
            .bind(&now)
            .bind(id.to_string())
            .bind(owner.to_string())
            .bind(delete)
            .execute(&self.pool)
            .await?,
        )
    }

    // ---------- next up ----------

    /// The live wrap-up list, in order (without the things brought up from cards).
    pub async fn next_up(&self, owner: OwnerId) -> Result<Vec<NextUpItem>> {
        Ok(self
            .next_up_all(owner)
            .await?
            .into_iter()
            .filter(|i| i.show_from.is_none())
            .collect())
    }

    /// Everything live on the list: the wrap-up's and the things brought up from cards
    /// (see `shiftbook_core::resume::resume_start` for which show when).
    pub async fn next_up_all(&self, owner: OwnerId) -> Result<Vec<NextUpItem>> {
        let rows = sqlx::query(
            "SELECT id, owner_id, position, text, show_from, about_kind, about_id, done_at, \
             created_at, updated_at, deleted_at \
             FROM next_up_items WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY position, id",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(next_up_from_row).collect()
    }

    /// Bring a card up on `show_from` ("Bring up tomorrow"): one line from its title.
    /// Bringing the same card up again moves it to the new day instead of adding a twin.
    pub async fn bring_up(
        &self,
        owner: OwnerId,
        title: &str,
        show_from: Date,
        about: Option<(CardKind, Uuid)>,
    ) -> Result<NextUpItem> {
        let text = bring_up_text(title)?;
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        if let Some((kind, id)) = about {
            sqlx::query(
                "UPDATE next_up_items SET deleted_at = $1, updated_at = $1 WHERE owner_id = $2 \
                 AND about_kind = $3 AND about_id = $4 AND done_at IS NULL AND deleted_at IS NULL",
            )
            .bind(&now)
            .bind(owner.to_string())
            .bind(kind.as_str())
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        }
        let id = NextUpItemId::new();
        sqlx::query(
            "INSERT INTO next_up_items (id, owner_id, position, text, show_from, about_kind, \
             about_id, done_at, created_at, updated_at, deleted_at) \
             VALUES ($1, $2, 0, $3, $4, $5, $6, NULL, $7, $7, NULL)",
        )
        .bind(id.to_string())
        .bind(owner.to_string())
        .bind(&text)
        .bind(format_date(show_from))
        .bind(about.map(|(k, _)| k.as_str()))
        .bind(about.map(|(_, u)| u.to_string()))
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.next_up_all(owner)
            .await?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or(StorageError::NotFound)
    }

    /// Replace the "next up" list (blank lines dropped). An empty list clears it.
    pub async fn set_next_up(&self, owner: OwnerId, lines: &[String]) -> Result<Vec<NextUpItem>> {
        let items = clean_next_up(lines)?;
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE next_up_items SET deleted_at = $1, updated_at = $1 \
             WHERE owner_id = $2 AND deleted_at IS NULL AND show_from IS NULL",
        )
        .bind(&now)
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        for (position, text) in items.iter().enumerate() {
            sqlx::query(
                "INSERT INTO next_up_items \
                 (id, owner_id, position, text, done_at, created_at, updated_at, deleted_at) \
                 VALUES ($1, $2, $3, $4, NULL, $5, $5, NULL)",
            )
            .bind(NextUpItemId::new().to_string())
            .bind(owner.to_string())
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(text)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.next_up(owner).await
    }

    /// Tick one thing off the list, or with `done = false` untick it.
    pub async fn tick_next_up(&self, owner: OwnerId, id: NextUpItemId, done: bool) -> Result<()> {
        let now = now_text();
        not_found_if_zero(
            sqlx::query(
                "UPDATE next_up_items SET done_at = $1, updated_at = $2 \
                 WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
            )
            .bind(if done { Some(now.as_str()) } else { None })
            .bind(&now)
            .bind(id.to_string())
            .bind(owner.to_string())
            .execute(&self.pool)
            .await?,
        )
    }

    // ---------- saved responses ----------

    /// The saved responses, in order.
    pub async fn responses(&self, owner: OwnerId) -> Result<Vec<SavedResponse>> {
        let rows = sqlx::query(
            "SELECT id, owner_id, position, name, text, created_at, updated_at, deleted_at \
             FROM saved_responses WHERE owner_id = $1 AND deleted_at IS NULL ORDER BY position",
        )
        .bind(owner.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(response_from_row).collect()
    }

    /// Replace the saved responses (rows with no message are dropped). An empty list clears them.
    pub async fn set_responses(
        &self,
        owner: OwnerId,
        drafts: &[ResponseDraft],
    ) -> Result<Vec<SavedResponse>> {
        let items = clean_responses(drafts)?;
        let now = now_text();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE saved_responses SET deleted_at = $1, updated_at = $1 \
             WHERE owner_id = $2 AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        for (position, item) in items.iter().enumerate() {
            sqlx::query(
                "INSERT INTO saved_responses \
                 (id, owner_id, position, name, text, created_at, updated_at, deleted_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $6, NULL)",
            )
            .bind(SavedResponseId::new().to_string())
            .bind(owner.to_string())
            .bind(i64::try_from(position).unwrap_or(i64::MAX))
            .bind(&item.name)
            .bind(&item.text)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.responses(owner).await
    }

    // ---------- preferences ----------

    /// The user's preferences (defaults for anything unset).
    pub async fn prefs(&self, owner: OwnerId) -> Result<Prefs> {
        Ok(prefs_from_settings(&self.list_settings(owner).await?))
    }

    /// Save one preference after checking it.
    pub async fn set_pref(&self, owner: OwnerId, key: &str, value: &str) -> Result<()> {
        let value = validate_pref(key, value)?;
        if key == KEY_TICKETS_CODE && !value.is_empty() {
            let code: TimesheetCodeId = value.parse()?;
            let mut conn = self.pool.acquire().await?;
            code_exists(&mut conn, owner, code).await?;
        }
        self.set_setting(owner, key, &value).await
    }
}
