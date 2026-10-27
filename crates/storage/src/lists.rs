//! Reusable lists (round 15): a task with `is_list` set, and its checklist items. Tick items
//! off, then **Reset for next time** unticks them all and keeps the items. Rules (names,
//! splitting pasted lines, room) live in `shiftbook_core::task`. Owner-scoped, bound
//! parameters only, soft-deleted.

use shiftbook_core::ids::{ChecklistItemId, OwnerId, TaskId};
use shiftbook_core::model::{ChecklistItem, Task};
use shiftbook_core::task::{
    check_list_room, split_list_items, validate_list_item, validate_list_title,
};
use sqlx::sqlite::{SqliteConnection, SqliteRow};
use sqlx::Row;

use crate::error::StorageError;
use crate::repo::{i32_from, now_text, parse_opt_ts, parse_ts, parse_uuid, Result};
use crate::SqliteStore;

/// A list with its live items, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRecord {
    /// The list (a task).
    pub list: Task,
    /// Its items.
    pub items: Vec<ChecklistItem>,
}

fn list_from_row(row: &SqliteRow) -> Result<Task> {
    Ok(Task {
        id: TaskId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        title: row.try_get("title")?,
        notes: row.try_get("notes")?,
        project_id: None,
        recurring_task_id: None,
        status: None,
        priority: None,
        due_date: None,
        completed_at: None,
        sort_order: i32_from(row.try_get("sort_order")?)?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

fn item_from_row(row: &SqliteRow) -> Result<ChecklistItem> {
    Ok(ChecklistItem {
        id: ChecklistItemId::from_uuid(parse_uuid(row.try_get("id")?)?),
        owner_id: OwnerId::from_uuid(parse_uuid(row.try_get("owner_id")?)?),
        task_id: TaskId::from_uuid(parse_uuid(row.try_get("task_id")?)?),
        text: row.try_get("text")?,
        done: row.try_get("done")?,
        sort_order: i32_from(row.try_get("sort_order")?)?,
        created_at: parse_ts(row.try_get("created_at")?)?,
        updated_at: parse_ts(row.try_get("updated_at")?)?,
        deleted_at: parse_opt_ts(row.try_get("deleted_at")?)?,
    })
}

async fn live_list(conn: &mut SqliteConnection, owner: OwnerId, id: TaskId) -> Result<Task> {
    let row = sqlx::query(
        "SELECT id, owner_id, title, notes, sort_order, created_at, updated_at, deleted_at \
         FROM tasks WHERE id = $1 AND owner_id = $2 AND is_list = TRUE AND deleted_at IS NULL",
    )
    .bind(id.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(StorageError::NotFound)?;
    list_from_row(&row)
}

async fn items_of(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    list: TaskId,
) -> Result<Vec<ChecklistItem>> {
    let rows = sqlx::query(
        "SELECT id, owner_id, task_id, text, done, sort_order, created_at, updated_at, \
         deleted_at FROM task_checklist_items WHERE task_id = $1 AND owner_id = $2 \
         AND deleted_at IS NULL ORDER BY sort_order, id",
    )
    .bind(list.to_string())
    .bind(owner.to_string())
    .fetch_all(&mut *conn)
    .await?;
    rows.iter().map(item_from_row).collect()
}

async fn record(conn: &mut SqliteConnection, owner: OwnerId, id: TaskId) -> Result<ListRecord> {
    let list = live_list(conn, owner, id).await?;
    let items = items_of(conn, owner, id).await?;
    Ok(ListRecord { list, items })
}

/// The list an item is on (live item, live list).
async fn list_of_item(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    item: ChecklistItemId,
) -> Result<TaskId> {
    let task: Option<String> = sqlx::query_scalar(
        "SELECT i.task_id FROM task_checklist_items i JOIN tasks t ON t.id = i.task_id \
         WHERE i.id = $1 AND i.owner_id = $2 AND t.owner_id = $2 AND t.is_list = TRUE \
         AND t.deleted_at IS NULL",
    )
    .bind(item.to_string())
    .bind(owner.to_string())
    .fetch_optional(&mut *conn)
    .await?;
    Ok(TaskId::from_uuid(parse_uuid(
        &task.ok_or(StorageError::NotFound)?,
    )?))
}

impl SqliteStore {
    /// Every live list with its items, in the order they were made.
    pub async fn lists(&self, owner: OwnerId) -> Result<Vec<ListRecord>> {
        let mut conn = self.pool.acquire().await?;
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM tasks WHERE owner_id = $1 AND is_list = TRUE AND deleted_at IS NULL \
             ORDER BY sort_order, id",
        )
        .bind(owner.to_string())
        .fetch_all(&mut *conn)
        .await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            out.push(record(&mut conn, owner, TaskId::from_uuid(parse_uuid(&id)?)).await?);
        }
        Ok(out)
    }

    /// Make a list, with items typed or pasted one per line.
    pub async fn create_list(
        &self,
        owner: OwnerId,
        title: &str,
        items: &str,
    ) -> Result<ListRecord> {
        let title = validate_list_title(title)?;
        let lines = split_list_items(items);
        check_list_room(0, lines.len())?;
        let mut tx = self.write_tx().await?;
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tasks WHERE owner_id = $1 AND is_list = TRUE",
        )
        .bind(owner.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let id = TaskId::new();
        let now = now_text();
        sqlx::query(
            "INSERT INTO tasks (id, owner_id, title, notes, sort_order, created_at, updated_at, \
             is_list) VALUES ($1, $2, $3, '', $4, $5, $5, TRUE)",
        )
        .bind(id.to_string())
        .bind(owner.to_string())
        .bind(&title)
        .bind(next)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        insert_items(&mut tx, owner, id, &lines, 0).await?;
        let rec = record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Rename a list.
    pub async fn rename_list(&self, owner: OwnerId, id: TaskId, title: &str) -> Result<ListRecord> {
        let title = validate_list_title(title)?;
        let mut tx = self.write_tx().await?;
        live_list(&mut tx, owner, id).await?;
        sqlx::query("UPDATE tasks SET title = $1, updated_at = $2 WHERE id = $3 AND owner_id = $4")
            .bind(&title)
            .bind(now_text())
            .bind(id.to_string())
            .bind(owner.to_string())
            .execute(&mut *tx)
            .await?;
        let rec = record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Remove a list (soft delete), or with `deleted = false` bring it back (Undo).
    pub async fn set_list_deleted(&self, owner: OwnerId, id: TaskId, deleted: bool) -> Result<()> {
        let now = now_text();
        let done = sqlx::query(
            "UPDATE tasks SET deleted_at = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND is_list = TRUE",
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

    /// Add items (one per line) at the end of a list.
    pub async fn add_list_items(
        &self,
        owner: OwnerId,
        id: TaskId,
        text: &str,
    ) -> Result<ListRecord> {
        let lines = split_list_items(text);
        if lines.is_empty() {
            validate_list_item("")?;
        }
        let mut tx = self.write_tx().await?;
        let current = record(&mut tx, owner, id).await?;
        check_list_room(current.items.len(), lines.len())?;
        let next = current
            .items
            .iter()
            .map(|i| i.sort_order)
            .max()
            .map_or(0, |m| m + 1);
        insert_items(&mut tx, owner, id, &lines, next).await?;
        let rec = record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Tick or untick one item.
    pub async fn set_list_item_done(
        &self,
        owner: OwnerId,
        item: ChecklistItemId,
        done: bool,
    ) -> Result<ListRecord> {
        let mut tx = self.write_tx().await?;
        let list = list_of_item(&mut tx, owner, item).await?;
        sqlx::query(
            "UPDATE task_checklist_items SET done = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(done)
        .bind(now_text())
        .bind(item.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = record(&mut tx, owner, list).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Change one item's words.
    pub async fn set_list_item_text(
        &self,
        owner: OwnerId,
        item: ChecklistItemId,
        text: &str,
    ) -> Result<ListRecord> {
        let text = validate_list_item(text)?;
        let mut tx = self.write_tx().await?;
        let list = list_of_item(&mut tx, owner, item).await?;
        sqlx::query(
            "UPDATE task_checklist_items SET text = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4 AND deleted_at IS NULL",
        )
        .bind(&text)
        .bind(now_text())
        .bind(item.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = record(&mut tx, owner, list).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Remove one item (soft delete), or with `deleted = false` bring it back (Undo).
    pub async fn set_list_item_deleted(
        &self,
        owner: OwnerId,
        item: ChecklistItemId,
        deleted: bool,
    ) -> Result<ListRecord> {
        let mut tx = self.write_tx().await?;
        let list = list_of_item(&mut tx, owner, item).await?;
        let now = now_text();
        sqlx::query(
            "UPDATE task_checklist_items SET deleted_at = $1, updated_at = $2 \
             WHERE id = $3 AND owner_id = $4",
        )
        .bind(deleted.then_some(now.as_str()))
        .bind(&now)
        .bind(item.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = record(&mut tx, owner, list).await?;
        tx.commit().await?;
        Ok(rec)
    }

    /// Reset for next time: untick every item, keep them all.
    pub async fn reset_list(&self, owner: OwnerId, id: TaskId) -> Result<ListRecord> {
        let mut tx = self.write_tx().await?;
        live_list(&mut tx, owner, id).await?;
        sqlx::query(
            "UPDATE task_checklist_items SET done = FALSE, updated_at = $1 \
             WHERE task_id = $2 AND owner_id = $3 AND deleted_at IS NULL AND done = TRUE",
        )
        .bind(now_text())
        .bind(id.to_string())
        .bind(owner.to_string())
        .execute(&mut *tx)
        .await?;
        let rec = record(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(rec)
    }
}

async fn insert_items(
    conn: &mut SqliteConnection,
    owner: OwnerId,
    list: TaskId,
    lines: &[String],
    first: i32,
) -> Result<()> {
    let now = now_text();
    for (n, line) in lines.iter().enumerate() {
        let text = validate_list_item(line)?;
        let order = first.saturating_add(i32::try_from(n).unwrap_or(i32::MAX));
        sqlx::query(
            "INSERT INTO task_checklist_items (id, owner_id, task_id, text, done, sort_order, \
             created_at, updated_at) VALUES ($1, $2, $3, $4, FALSE, $5, $6, $6)",
        )
        .bind(ChecklistItemId::new().to_string())
        .bind(owner.to_string())
        .bind(list.to_string())
        .bind(&text)
        .bind(i64::from(order))
        .bind(&now)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
