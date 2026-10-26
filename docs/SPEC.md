# Shiftbook — Product Spec (v1)

## 1. Goal
Replace a spreadsheet time tracker with a fast, simple desktop app that:
- logs **rough hours** per piece of work (no start/end times),
- lets you paste **any number of tickets** onto an entry,
- includes a lightweight **task manager**,
- tracks **projects** and **recurring work** (SSL certs, patching, audits) with due-date alerts,
- shows a **dashboard** and exports a weekly summary/CSV for a timesheet system.

Primary user: anyone who logs their own work (first user: an infrastructure engineer with ADHD),
not a spreadsheet power user. Speed of logging matters most. The app ships with no employer-specific
data: codes, categories and lists are the user's own.

## 2. Screens

### 2.1 Today (home screen)
- Header: today's date, **Hours today**, **Hours this week**.
- **Quick log** row: `Project / Task` picker (search-as-you-type; projects first, then recurring
  tasks, then "No project"), `Hours` input, `Category`, `What I worked on`, `+ Ticket`, **Save**.
  - Date defaults to today; a small date control allows backdating.
  - Hours accepts `1.5`, `1:30`, `90m`, `1h 30m` → stored as minutes. Quick buttons `+15m +30m +1h`.
- Today's entries listed below. Each entry has a **+ time** button (adds minutes to that entry)
  and inline edit.
- **Due soon** panel: recurring items that are Overdue / Due Soon, and tasks due today/overdue.

### 2.2 Time Log
- Week view (default current week, previous/next arrows) and a date-range filter.
- Columns: Date, Hours, Project/Task, Group, Category, Description, Tickets (chips), Status,
  Priority, Next step.
- **Entries from earlier days are visually muted** (grey) but remain editable.
- Filter by project/task, category, status, ticket text. Free-text search.
- No manual archive — older weeks are simply filtered out of the default view.

### 2.3 Tickets (behaviour inside entries)
- On any entry, click **+ Ticket** → an input appears; paste and press Enter → becomes a chip.
- Pasting multiple lines or comma-separated values creates multiple chips at once.
- Chips are removable (×) and clickable to copy. Optional per-ticket note.
- A Tickets page lists all tickets with date, project/task, entry and hours (searchable).

### 2.4 Tasks (task manager)
- Fields: title, notes, project or recurring task (optional link), status, priority, due date,
  tags (optional), checklist items (optional).
- Views: **List** (grouped by due: Overdue / Today / This week / Later / No date) and **Board**
  (columns by status). Drag to reorder/change status.
- From a task, **Log time** opens the quick log pre-filled with the task's project and title.
- Completing a task records `completed_at`.

### 2.5 Projects
- Fields: name, description, requested by, due date, status, priority, estimated hours.
- Calculated: hours logged, hours left, % of estimate used, alert (Overdue / Over budget / Blocked).
- Project detail page: entries, tickets and tasks for that project.

### 2.6 Recurring work
- Fields: name, type, repeats every (frequency), expiration/due date (optional), warn days before
  (default 30), last done before tracker (optional), **alerts enabled** (bool), notes, active.
- Calculated: last done, next due, days left, total hours, alert state (see §4).
- "Mark done" button creates/updates a time entry with status Done for today.
- Items like "General - Tickets" or "Daily meeting" are **routine**: alerts disabled, they only
  collect hours (state = Ongoing). Spreadsheet trackers often show these as OVERDUE — import them with
  `alerts_enabled = false`.

### 2.7 Dashboard
- Week selector (defaults to current week).
- Cards: hours this week, hours last week, recurring due/overdue, active projects, blocked
  projects, overdue projects, open tasks, tickets this week.
- Hours by project (selected week + all time), hours by category, hours by recurring type,
  last 8 weeks trend, status counts. Charts use inline SVG (no chart CDN).

### 2.8 Timesheet codes and timesheet export
Many timesheet systems need hours per **timesheet code** per day. Codes are hierarchical:
`Area > Phase > Code`, e.g. `Operations > Phase > 1 - Support`. The feature stays generic
("timesheet codes") and works with any system.

**Codes:** none are shipped. The user adds their own in Settings (area, phase label, code, order),
or imports them. The first-run tour shows a generic example.

**How an entry gets its code** (first match wins; shown on the entry and changeable there):
1. Code chosen on the entry itself (override).
2. Default code on its **project** or **recurring task**.
3. Default code for its **category** (mapping in Settings).
4. For a day's **Tickets** entry (rolled up from the Tickets tray): the **tickets code**
   (Customize → Timesheet → Default codes).
5. Otherwise **Unassigned** — highlighted in amber in the Time Log and listed at the top of the
   timesheet view so nothing is missed.

**Category → code defaults:** none are shipped. The user maps categories to codes in Settings;
unmapped categories fall through to Unassigned.

**Holiday / time off:** a one-click "Holiday" entry (default 8 h, editable) that uses
the code the user picked as their holiday code in Settings, and needs no project.

**Timesheet view / export:**
- Pick a week → grid of **code × day** (Mon–Sun) with daily and weekly totals — laid out in the
  same order you enter it in your timesheet system. Click a cell to see the entries behind it.
- Second section: per day, the entries with descriptions and ticket numbers (for comments).
- Warnings: unassigned hours, days with 0 h on weekdays, days over 12 h.
- **Copy** (tab-separated, pastes cleanly), **Export .xlsx**, **Export .csv**.
- Optionally mark a week as **Submitted** (records date) so you can see what's done.

### 2.9 First-run setup, password and lock (REQUIRED from the first release)
- **First launch** shows a setup screen: choose where to store data, then **create a master
  password** (min 12 characters, strength meter, confirm field). The database is created
  encrypted with a key derived from this password. There is no unencrypted mode.
- Setup then shows a one-time **Recovery Key** (e.g. 24 words / 32-char code) with
  **Save as file** and **Copy**, and requires a checkbox "I saved my recovery key" to continue.
  Explain plainly: *if you lose both the password and the recovery key, your data cannot be
  recovered — not even by the developer.*
- **Unlock screen** on every launch (password or recovery key).
- **Auto-lock** after N minutes idle (default 15, configurable) and on OS sleep/lock; manual
  **Lock now** button and shortcut.
- **Change password** (re-wraps the key; requires the current password).
- Rate-limit failed unlock attempts (increasing delay); never log or display the password.

### 2.10 Backup (secure) and Export (readable)
**Backups are always encrypted. Exports are readable and clearly labelled as unprotected.**

- **Automatic backups:** daily + on app close, keep the last 14 (configurable), stored in
  `backups/` next to the data file. Encrypted with the same master key.
- **Backup now / Restore:** creates a single `.shiftbak` file —
  see SECURITY.md "Backup file format". Restore verifies integrity before replacing anything and
  makes a safety backup of the current data first.
- Backup files can be restored on any OS and on a new computer using the password or recovery key.
- **Export (unencrypted)** — shows a warning: "This file is not password-protected. Store it
  somewhere safe and delete it when done."
  - **Excel (.xlsx):** one workbook with sheets Time Entries, Tickets, Tasks, Projects,
    Recurring, and a Summary sheet; real dates/numbers (hours as decimal), header row frozen,
    filters on, sensible column widths.
  - **CSV:** per entity (time entries, tickets, tasks, projects, recurring) as UTF-8 with BOM
    (opens correctly in Excel), or one zip containing all CSVs.
  - **Timesheet export:** selected week as .xlsx or .csv (see §2.8).
  - **JSON:** full data in the documented import format (for migration to the hosted version).
- Exports can be filtered by date range and project.

### 2.11 Settings
- Manage dropdown lists: categories, statuses (with colour), priorities, recurring types,
  frequencies (label + months + days). Reorder, rename, hide (soft-delete; never break history).
- Data: show data file location, **Change location**, **Backup now**, **Restore**,
  **Export** (Excel / CSV / JSON), **Import JSON**, **Import from Excel export** (see `IMPORT_FORMAT.md`).
- Security: change password, view/regenerate recovery key (requires password), auto-lock minutes.
- Week starts Monday (configurable).

## 3. Data model (all tables)
Common columns: `id UUID PK`, `owner_id UUID`, `created_at`, `updated_at`, `deleted_at NULL`.

| Entity | Key fields |
|---|---|
| users | display_name (Phase 2: email, oidc_subject, role) |
| list_options | list_name (category/status/priority/recurring_type), value, sort_order, color, hidden |
| frequencies | label, months INT, days INT, sort_order |
| timesheet_codes | area, phase_label, code, sort_order, hidden |
| category_code_defaults | category (list value), timesheet_code_id |
| projects | name, description, requested_by, due_date, status, priority, estimated_minutes, default_timesheet_code_id |
| recurring_tasks | name, type, frequency_id, expiration_date, warn_days_before, last_done_before_tracker, alerts_enabled, active, notes, default_timesheet_code_id |
| time_entries | date, minutes, target_type (project/recurring/none), target_id, category, description, status, priority, next_step, timesheet_code_id (override, nullable), legacy_ticket_text |
| timesheet_weeks | week_start, submitted_at |
| items | kind (ticket/task/chore), number (unique per owner, ignoring case), title, done, note, project_id |
| tickets | time_entry_id, ticket_number, note, done, item_id (copies its item's done and note) |
| ticket_tray | date, ticket_number, minutes, project_id, note, done, item_id (as tickets) |
| tasks | title, notes, project_id, recurring_task_id, status, priority, due_date, completed_at, sort_order |
| task_checklist_items | task_id, text, done, sort_order |
| settings | key, value (per owner) |
| audit_log (Phase 2) | actor_id, action, entity, entity_id, at, details |

Status/category/priority are stored as their text value (simple, portable); renames in Settings
update existing rows in a transaction.

## 4. Business rules (implement in `crates/core`, unit-test each)
- **Week of** = Monday on or before the date.
- **Group** = project name, or `Recurring: <type>` (type blank → `Other`), or `No project`.
- **Project:** hours_logged = Σ minutes of entries targeting it; hours_left = estimate − logged;
  pct_used = logged / estimate. Alert priority: `Overdue` (due_date < today and status ≠ Done) →
  `Over budget` (pct_used > 100%) → `Blocked` (status = Blocked) → none.
- **Recurring:**
  - last_done = max(last_done_before_tracker, max date of its entries with status Done).
  - warn = warn_days_before or 30.
  - interval = frequency months + days.
  - next_due:
    - if expiration_date set: if last_done ≥ expiration_date − warn and a frequency exists →
      expiration_date + interval (estimate), else expiration_date.
    - else: last_done + interval (if both exist), else none.
  - state, in order: inactive → `Inactive`; alerts disabled → `Ongoing`; no expiration and no
    frequency → `Ongoing`; next_due none → `Needs a date`; days_left < 0 → `Overdue`;
    days_left ≤ warn → `Due soon`; expiration set and last_done ≥ expiration − warn →
    `Renewed – update expiration date`; else `OK`.
- **Tasks:** overdue = due_date < today and status ≠ Done.
- **Timesheet code resolution:** entry override → project/recurring default → category default →
  tickets code (Tickets roll-up entries only) → Unassigned (see §2.8). Timesheet totals = Σ minutes grouped by (resolved code, date).
- Port the Excel behaviour exactly where these rules came from; add tests for each state.

## 5. Non-functional
- Startup < 1 s on a normal laptop; logging an entry takes ≤ 3 interactions.
- Works fully offline. No network calls (Phase 1).
- Data in one **encrypted** SQLite file in the OS app-data dir by default; location changeable.
- Automatic encrypted backups (daily + on close, keep last 14) into `backups/` next to the data file.
- Nothing readable is ever written to disk except files the user explicitly exports.
- Keyboard: `N` new entry, `/` search, `Ctrl+Enter` save.
- Light and dark theme following the OS.
