# Architecture

## Layers
```
┌──────────────────────────────────────────┐
│ web/ (Svelte SPA)                        │  identical in desktop + hosted
│   └─ lib/api/DataApi.ts  (interface)     │
│        ├─ TauriApi.ts   (Phase 1)        │  invoke('cmd', args)
│        └─ HttpApi.ts    (Phase 2)        │  fetch('/api/...')
├──────────────────────────────────────────┤
│ apps/desktop/src-tauri   │ apps/server   │  thin adapters: auth, transport, config
│  (Tauri commands)        │  (Axum, P2)   │
├──────────────────────────────────────────┤
│ crates/core   — models, rules, validation (pure, no I/O)
├──────────────────────────────────────────┤
│ crates/storage — Repository traits + sqlx impl (SQLite now, Postgres later)
└──────────────────────────────────────────┘
```

## Key decisions
- **One `DataApi` contract.** Each method maps 1:1 to a Tauri command *and* a future REST
  endpoint (e.g. `listTimeEntries(range)` ↔ `GET /api/time-entries?from=&to=`). Define request and
  response types once in Rust with `serde` and generate TS types (e.g. `ts-rs` — ask before adding)
  so desktop and server share the same shapes.
- **Owner scoping everywhere.** Every repository method takes an `OwnerId`. Desktop passes the
  single local user; the server passes the authenticated user. This is what makes multi-user a
  configuration change, not a rewrite.
- **Rules in core.** Dashboard numbers and alert states are computed in `crates/core` and returned
  ready to display, so desktop and server always agree.
- **IDs:** UUID v7 (time-ordered) generated in Rust. Safe to merge desktop data into a server.
- **Migrations:** plain SQL compatible with SQLite and Postgres; `sqlx migrate` at startup.
- **Time:** minutes (INT). Dates as `TEXT` `YYYY-MM-DD` in SQLite / `DATE` in Postgres via sqlx types.

## Desktop specifics (Phase 1)
- Data path: Tauri `app_data_dir()` (created `0700` on Unix), holding `vault.json`,
  `vault.json.prev` and `shiftbook.db` (files `0600` on Unix). Relocating via Settings (stored in
  a small config file in the app config dir) comes later. On Linux the WebView (WebKitGTK) also
  keeps its own cache folders there; Shiftbook stores nothing in WebView storage.
- Backups: copy DB with SQLite online backup API (`VACUUM INTO`) to `backups/YYYY-MM-DD.db`.
- Plugins allowed: `dialog` (pick backup/import file), `fs` scoped to app data + user-picked paths.
  Not allowed: `shell`, `http`, `opener` for remote URLs, updater (until signing is set up).

## Server specifics (Phase 2 — plan only)
- Axum + tower; same `core`/`storage` crates; Postgres (or SQLite for tiny installs).
- Auth: OIDC (Authorization Code + PKCE) against the organisation's IdP; session cookie
  (`HttpOnly; Secure; SameSite=Lax`). Optional local accounts with Argon2id for small installs.
- RBAC: `user`, `admin`. Admin panel: users, roles, shared list options, audit log, team reports.
- Reverse proxy: `BASE_PATH` env, honour `X-Forwarded-Proto/Host/For` only from `TRUSTED_PROXIES`,
  `/healthz` (liveness) and `/readyz` (DB check).
- Docker: multi-stage build → distroless/non-root image; config via env; data via volume or
  external Postgres; migrations on start.
- Import: users upload their desktop JSON export; UUIDs make it a clean merge.
- Optional Phase 3: desktop ↔ server sync using `updated_at` + soft deletes.

## Core rules
`crates/core` is pure Rust (no I/O, no SQL, no Tauri, `forbid(unsafe_code)`). Modules:

| Module | What it holds |
|---|---|
| `ids` | Typed UUID v7 ids (`OwnerId`, `ProjectId`, `TimeEntryId`, ...) |
| `minutes` | `Minutes(u32)`; sums saturate instead of overflowing |
| `dates` | `YYYY-MM-DD` dates, RFC 3339 UTC timestamps, serde helpers, month arithmetic |
| `hours` | Hours box parsing (`parse_hours`) and display (`format_hours`, `format_hours_minutes`) |
| `week` | Week of, with a configurable start day (Monday by default) |
| `group` | Group label |
| `model` | Plain serde records from SPEC §3 |
| `project` | Hours logged/left, % used, project alert |
| `recurring` | Last done, next due, days left, alert state |
| `task` | Task overdue rule |
| `timesheet` | Timesheet code resolution and code × day totals |
| `tickets` | Splitting pasted ticket text |
| `seed` | Generic default lists and frequencies (no timesheet codes) |
| `validate` | Input checks returning `CoreError` |
| `error` | `CoreError` with plain-English messages |
| `capture` | One-line capture parser (`parse_capture`): hours, tickets, `#project`, day words, description |
| `quicklog` | Log work: reuse the same day's entry for a known ticket, or plan a new entry |
| `tray` | Done tickets tray: minutes per ticket, paste splitting, per-day roll-up plan |
| `sheet` | Timesheet crib sheet (`build_week_sheet`): lines per day by code or project, comments, effort, warnings, tab-separated week |
| `ribbon` | The last four weeks as days of minutes per project, with week totals |
| `fields` | Custom field kinds, option lists, value checks, ticket prefixes and link building |
| `prefs` | Preferences (theme, motion, accent, grouping, comment format, week start day, auto-lock) with defaults; project colour palette |

Interpretations made where the spec is silent:
- **Hours rounding:** decimal hours round to the nearest whole minute, halves up (`0.01` → 1 min,
  `1.333` → 80 min); at most 6 decimals. Minutes must be whole (`1.5m` is rejected).
- **Bare numbers:** a whole number up to 24 is hours (`8` → 8 h); above 24 it is minutes (`45` → 45 min).
- **Minutes part:** in `1:30` and `1h 30m` the minutes must be under 60; `1:30` needs two minute digits.
- **Entry size:** 0 to 24 h. Zero is allowed (e.g. a "done" marker with no time).
- **Month arithmetic:** intervals add months first, then days; a day that does not exist in the
  target month clamps to the month end (31 Jan + 1 month = 28/29 Feb), like Excel `EDATE`.
- **Done / Blocked:** status text is compared ignoring case and surrounding spaces (like Excel `=`).
- **Warn days:** blank means 30; an explicit 0 stays 0 (warn only on the day).
- **Zero frequency:** 0 months and 0 days counts as "no frequency" (and validation rejects it).
- **Zero estimate:** an estimate of 0 is treated as "no estimate": no hours left, no % used,
  never Over budget.
- **% used:** whole percent, rounded to nearest; Over budget compares exact minutes (logged > estimate).
- **Soft-deleted rows** are ignored in every total and in last done.
- **Tickets:** repeats are matched ignoring case (`INC1` = `inc1`); at most 64 characters per
  ticket and 100 tickets per entry; control characters are rejected.
- **Category → code lookup** ignores case and surrounding spaces.
- **Capture:** a plain whole number counts as hours only as the first word and only up to 24
  (`45 errors` stays text); decimals, `1:30`, `90m`, `2 hours`, `1h 30m` count anywhere; the first
  time wins. Tickets are 2–10 letters, an optional dash, 2+ digits; lower-case letters need 4+
  digits (`inc0045678` yes, `win10` no); tickets are upper-cased. `#name` matches a project by
  whole name, name prefix, name-without-spaces prefix, then word prefix. Day words are today,
  yesterday and weekdays of the current week; the first one wins.
- **Log work** reuses an entry on the same day that has one of the pasted tickets when no new
  words were typed (or the words equal its title); the tray's roll-up entry is never reused.
  A new entry takes its title and project from the latest earlier entry with that ticket.
- **Tray:** 15 minutes per ticket by default, steps of 5, 0 to 480; the roll-up is capped at
  24 h. Pasting text that contains ticket-shaped words keeps only those.
- **Crib sheet:** items with the same title in a line merge (tickets combined); Unassigned is
  listed first; the ticket count counts each number once; weekend days without entries are
  hidden in the UI; "nothing logged" warns only for Monday–Friday before today.
- **Link templates** must start with `http://` or `https://` and contain `{ticket}`; the ticket
  is percent-encoded. Links are only displayed and copied.

## Storage, vault and unlock (M1)
`crates/storage` modules:

| Module | What it holds |
|---|---|
| `vault` | `VaultDir` (create, unlock with password or recovery key, change password, regenerate recovery key), `Vault`/`Dek` (the unlocked key, wiped on drop), `VaultError` |
| `recovery` | `RecoveryKey`: 256 random bits shown as 13 groups of 4 Crockford base32 characters; forgiving parser |
| `db` | `open_encrypted(path, dek)`: SQLCipher pool with the key pragma first, then migrations |
| `repo` | Repository traits + `SqliteStore` (local user, settings, list options, frequencies) |
| `work` | Projects, time entries, tickets, the Done tickets tray and its roll-up, Log work |
| `extras` | Timesheet codes and category defaults, crib sheet tick-offs, custom fields and values, Inbox, preferences |
| `fsutil` | Private folders/files and atomic replace (temp file → fsync → rename → fsync folder) |
| `hex` | Tiny hex helpers |

- **Vault file** (`vault.json`): `{version, kdf, kdf_params, salts, nonces, wrapped_dek_password,
  wrapped_dek_recovery}`, bytes as lowercase hex. Argon2id parameters are calibrated at setup to
  about 0.5 s (floor 64 MiB / t=3 / p=1, cap 1 GiB / t=10 / p=4) and anything outside that range
  in the file is refused. Each wrap is XChaCha20-Poly1305 with a fresh nonce; the AAD binds the
  version, KDF name and parameters, salt and purpose (`pw` / `recovery`). Any unlock problem is
  the same `UnlockFailed`.
- **Saving the vault:** the current file is copied to `vault.json.prev`, the new file replaces
  `vault.json` atomically and is read back, then `.prev` is updated to match, so `.prev` is a
  safety copy of the current vault and never keeps an old password working. There is no automatic
  fallback to `.prev`; it is for manual repair.
- **Database:** sqlx 0.9 with `libsqlite3-sys` 0.37 built as SQLCipher 4 (vendored OpenSSL).
  Every connection runs `PRAGMA key = "x'…'"` (the raw DEK) first, then
  `cipher_compatibility = 4`, `foreign_keys = ON`, `cipher_memory_security = ON`; journal mode
  DELETE; statement logging off. Migrations are embedded from `crates/storage/migrations/`
  (portable SQL: TEXT ids/dates/timestamps, INTEGER minutes, BOOLEAN flags, partial unique
  indexes, no AUTOINCREMENT or JSON). The `users` table has no `owner_id`: a user is its own owner.
- **Seeding:** on every unlock `initialize_local_owner` creates the single local user if needed
  and, once, the default lists and frequencies from `shiftbook_core::seed`, all in one
  transaction (`settings.seed.version` marks it done).
- **List rules in storage:** "Done" and "Blocked" statuses can't be renamed or hidden (the alert
  rules match on them). Duplicates are refused ignoring case. Renaming a list value updates the
  records that store that text in the same transaction.
- **Tests** use tiny Argon2 parameters through a `cfg(test)`-only path (`VaultDir::new_for_tests`),
  which doesn't exist in release builds; one test runs the real calibration. Argon2 is compiled
  with `opt-level = 3` even in debug builds so `tauri dev` unlocks at release speed.

Unlock flow (desktop):
```
WebView                          src-tauri (VaultSession, one lock)          crates/storage
vault_status ─────────────────▶  needs_setup | locked | unlocked
vault_setup(password) ────────▶  VaultDir::create (blocking thread) ──────▶ vault.json
                                 open_encrypted + initialize_local_owner ─▶ shiftbook.db
          ◀── recovery key (once)
vault_confirm_recovery_saved ─▶  settings.security.recovery_key_saved_at
unlock(secret, kind) ─────────▶  unlock_with_password / _recovery_key ─────▶ open_encrypted
          ◀── ok | "unlock_failed" (after 5 failures: wait 1 s, 2 s … 60 s)
lock ─────────────────────────▶  close pool, drop Vault (key wiped) ── WebView reloads
change_password(cur, new) ────▶  VaultDir::change_password (re-wrap only)
```
Errors reach the WebView only as codes (`unlock_failed`, `weak_password`, `busy`,
`already_set_up`, `not_set_up`, `locked`, `storage_error`). While a vault command runs (including
the back-off wait) the others answer `busy`. The frontend locks on `Lock now`, `Ctrl+L`, 15 minutes
without keyboard/mouse activity, and when its timer notices a gap of a minute or more (the
computer slept); after locking it reloads the page.

## Alpha: tables, commands and UI

### Migration 0002 (`crates/storage/migrations/0002_alpha.sql`)
- `projects.color` (`#rrggbb`) and `projects.archived`.
- `time_entries.source`: `NULL` for entries the user logged, `'tray'` for a day's roll-up of the
  Done tickets tray, with a partial unique index so there is at most one live roll-up per day.
- `ticket_tray` (date, ticket_number, minutes 0–480, note). **Why a table of its own:** the
  existing `tickets` table requires a time entry, and SQLite can't drop a `NOT NULL` without a
  table rebuild (not portable). Tray tickets exist before any entry, so they live in their own
  table and every tray change re-plans the day's roll-up entry in the same transaction
  (`shiftbook_core::tray::plan_rollup`): create it, update its minutes and tickets, or remove
  it when the tray is empty. Its title, project and notes stay editable; deleting it empties the
  tray and restoring brings both back.
- `custom_fields` (applies_to entry/ticket/project, name, kind text/number/choice/link,
  options one per line, link_template, ticket_prefix, sort_order, hidden) and `custom_values`
  (field_id, record_id, value; one live value per field and record).
- `timesheet_ticks` (week_start, day, line_key, done): crib sheet tick-offs.
- `inbox_items` (text): scraps saved from capture.

Repositories for the alpha are inherent async methods on `SqliteStore` (`work.rs`, `extras.rs`);
like the M1 ones they take the `OwnerId`, bind every value and soft-delete. They become traits
when the Postgres implementation arrives.

### Desktop commands (`apps/desktop/src-tauri/src/api.rs`, `protect.rs`, `lib.rs`)
Each command has its own `allow-<command>` permission (declared in `build.rs`, granted in
`capabilities/default.json`); no plugins are used.

| Area | Commands |
|---|---|
| Vault | `vault_status`, `vault_setup`, `vault_confirm_recovery_saved`, `unlock` (returns `mustSetPassword`), `lock` (also re-protects), `change_password`, `set_new_password` (only straight after a recovery-key unlock), `regenerate_recovery_key` (needs the password; returns the key once) |
| Protection | `allow_capture` (content protection off for 10 minutes; a timer in the app turns it back on, only the latest allow counts), `protect_capture` |
| Preferences | `load_prefs`, `save_pref` |
| Projects | `list_projects`, `create_project`, `update_project` |
| Entries | `list_entries`, `create_entry`, `update_entry`, `add_entry_minutes`, `delete_entry`, `restore_entry`, `add_entry_tickets`, `remove_ticket`, `quick_log` |
| Capture | `parse_capture_line`, `capture_log` |
| Tray | `tray_day`, `tray_add`, `tray_set_minutes`, `tray_remove`, `tray_restore` |
| Ribbon, crib sheet | `ribbon`, `week_sheet`, `list_ticks`, `set_tick` |
| Timesheet codes | `list_timesheet_codes`, `save_timesheet_code`, `category_codes`, `set_category_code` |
| Custom fields | `list_custom_fields`, `save_custom_field`, `delete_custom_field`, `list_custom_values`, `set_custom_value` |
| Inbox | `list_inbox`, `add_inbox`, `set_inbox_deleted` |

New error codes: `invalid_input`, `not_found`, `duplicate`, `tray_managed`, `not_allowed`.
The UI passes the user's local date (`today`) because the app computes week-of and day words
from it. Ticket chips carry ready-made links from link fields whose prefix matches.

### Web UI (`web/src/`)
- `App.svelte` — setup / recovery key / unlock / new password / app; idle lock uses the
  auto-lock preference.
- `pages/Shell.svelte` — ribbon, the current view (sliding left or right by view order), the
  overlays, keyboard shortcuts (`lib/keys.ts`) and the copy guard (`lib/protection.svelte.ts`).
- `components/` — `Ribbon`, `LogWork`, `EntryRow`, `TicketChip`, `TrayCard`, `InboxCard`,
  `Drawer`, `Capture`, `PaperMenu`, `Toast`, `Icon` (inline SVG paths in `lib/icons.ts`).
- `views/` — `Desk`, `Timesheet`, `Customize` with `customize/Look|Projects|TimesheetSettings|
  Fields|FieldEditor|Security`.
- `lib/store.svelte.ts` — shared state filled only from `DataApi`; `lib/menu.svelte.ts` and
  `lib/menuPlacement.ts` (pure, tested) for the paper menu; `lib/motion.ts` scales animation by
  the Motion setting and turns it off for Still or the system's reduced-motion setting.
- **CSP:** styles are the compiled stylesheet plus CSSOM changes (Svelte `style:` directives,
  `element.style`), never inline `style` attributes, so `style-src 'self'` holds. Animations use
  CSS and the Web Animations API.
- **Paper menu:** the source element is hidden and a copy of it is drawn above everything with
  a lift; the sheet sits in a clip box that starts at the object's edge so it slides out from
  underneath; it comes out below unless below is cramped, and scrolls when it doesn't fit.
  Icon rails sit on the right (left if there's no room) and on top (below if there's no room).
