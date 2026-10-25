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
- Backups: `shiftbook_storage::backup` reads every table into JSON, gzips it and encrypts it
  with the DEK into a `.shiftbak` file (SECURITY.md "Backup file format"). Restore replaces every
  table in one transaction. Commands: `backup_now`, `list_backups`, `pick_backup_file`,
  `check_backup`, `restore_backup`; the daily automatic backup starts after `unlock`.
- Plugins: `dialog` (pick a backup file, called from Rust commands only; the WebView gets no
  dialog permission). `fs` (scoped to app data + user-picked paths) is allowed if export needs it.
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
| `model` | Plain serde records (SPEC §3 plus everything added since) |
| `error`, `validate` | `CoreError` with plain-English messages; input checks |
| `seed` | Generic default lists and frequencies (no timesheet codes) |
| `project`, `recurring`, `task` | Project hours and alerts, recurring next-due and alert state, task overdue (SPEC §4) |
| `timesheet`, `sheet` | Timesheet code resolution and totals; the weekly crib sheet (`build_week_sheet`) |
| `tickets`, `tray` | Splitting pasted tickets; the Tickets tray (minutes per ticket, still open, per-day roll-up plan) |
| `capture`, `search`, `quicklog` | The one-line capture parser, Find, and Log work (reuse an entry or plan a new one) |
| `ribbon` | The last four weeks as days of minutes per project |
| `fields` | Custom field kinds, option lists, value checks, ticket prefixes and link building |
| `prefs` | Preferences with defaults; project colour palette |
| `notes` | Note kinds, tags, titles, auto-archive, turning a jot into an entry |
| `canvas` | The Desk: which cards show, auto-placing, areas and boxes, links, timeline, piles, the same work folded into one card |
| `boards` | Boards you step into from the Desk (projects, meeting series, Daily notes, folders, Archive) |
| `nextup`, `resume` | The wrap-up's next-up list and Bring up; when "Where was I?" shows and what it offers |
| `bell`, `meetings`, `pins` | What the bell lists; repeating meetings and their reminders; pins on the bottom bar |
| `windows`, `timer` | Pop-out windows; the optional Now doing timer |
| `responses`, `wipe` | Saved responses; the word that confirms Delete all data |

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
| `wipe` | `delete_local_data`: delete the database, its side files and the vault (vault last) to start again |
| `db` | `open_encrypted(path, dek)`: SQLCipher pool with the key pragma first, then migrations |
| `backup` | Encrypted `.shiftbak` backups: write, check, restore (desktop only: reads SQLite's own table list) |
| `repo` | Repository traits + `SqliteStore` (local user, settings, list options, frequencies) |
| `work` | Projects, time entries, tickets, the Tickets tray and its roll-up, Log work |
| `extras` | Timesheet codes and category defaults, crib sheet tick-offs, custom fields and values, Inbox, preferences, saved responses, next up |
| `notes` | Notes, the archive and auto-archive |
| `canvas`, `piles`, `boards` | Where each card sits on the Desk, areas, boxes and links; piles and drawers; spots on each board |
| `meetings`, `pins` | Meeting series; pins |
| `fsutil`, `hex`, `error` | Private folders/files and atomic replace; hex helpers; `StorageError` |

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
delete_all_data("DELETE") ────▶  close pool, wipe::delete_local_data ── WebView reloads to setup
```
Errors reach the WebView only as codes (`unlock_failed`, `weak_password`, `busy`,
`already_set_up`, `not_set_up`, `locked`, `storage_error`). While a vault command runs (including
the back-off wait) the others answer `busy`. The frontend locks on `Lock now`, `Ctrl+L`, 15 minutes
without keyboard/mouse activity, and when its timer notices a gap of a minute or more (the
computer slept); after locking it reloads the page.

## Where things live (alpha)
This section is a map, not a full list: each file's opening comment says what it holds, and
`docs/DECISIONS.md` says why.

### Migrations (`crates/storage/migrations/`)
One file per step, all portable SQL (see "Storage" above). Each starts with a comment saying
what it adds and why.

| File | Adds |
|---|---|
| `0001_initial_schema` | The SPEC §3 tables |
| `0002_alpha` | Project colours and archive, `time_entries.source` (the tray's roll-up), `ticket_tray`, custom fields and values, crib sheet tick-offs, the Inbox |
| `0003_next_up` | The wrap-up's next-up list |
| `0004_notes` | Notes and their tags |
| `0005_saved_responses` | Saved responses |
| `0006_canvas`, `0007_canvas_areas_links` | Card spots on the Desk; project areas and links |
| `0008_tickets_open_and_bring_up` | Still-open tray tickets, Bring up |
| `0009_meeting_series` | Repeating meetings |
| `0010_canvas_boxes`, `0011_stacks_drawers`, `0012_board_spots` | Folding and labelled boxes; piles and drawers; spots on boards |
| `0013_pins` | Pins |
| `0014_ticket_done` | Done / still open on tickets that belong to entries |

**Why `ticket_tray` is its own table:** the `tickets` table requires a time entry, and SQLite
can't drop a `NOT NULL` without a table rebuild (not portable). Tray tickets exist before any
entry, so they live apart and every tray change re-plans the day's roll-up entry in the same
transaction (`shiftbook_core::tray::plan_rollup`). Since round 14 both tables carry done and a
notepad; the Desk's tickets box shows each number once (`web/src/lib/workTickets.ts`).

Repositories added since M1 are inherent async methods on `SqliteStore`; like the M1 ones they
take the `OwnerId`, bind every value, soft-delete and wrap multi-step changes in a transaction.
They become traits (or a second store type) when the Postgres implementation arrives.

### Desktop commands (`apps/desktop/src-tauri/src/`)
Every command has its own `allow-<command>` permission, declared in `build.rs` and granted in
`capabilities/default.json`; `lib.rs` registers them. A new command needs all three. The only
plugin is `dialog`, called from Rust (the WebView gets no dialog permission).

| File | Commands for |
|---|---|
| `lib.rs`, `session.rs`, `backoff.rs` | The vault: setup, unlock (with back-off), lock, change password, recovery key, delete all data; backups |
| `protect.rs` | Screenshot protection and the timed allow |
| `api.rs` | Preferences, projects, entries and tickets, Log work, capture and Find, the tray, ribbon, crib sheet, timesheet codes, custom fields, Inbox, responses, next up, Where was I?, the timer |
| `notes.rs` | Notes |
| `canvas.rs`, `boards.rs` | The Desk (cards, areas, boxes, links, piles, drawers, Tidy) and boards |
| `meetings.rs`, `pins.rs` | Meeting series and the bell; pins |

Each command's doc comment names the `DataApi` method it serves. Errors reach the WebView only
as codes (see `session.rs` and `web/src/lib/messages.ts`). The UI passes the user's local date
(`today`) because the app computes week-of and day words from it.

### Web UI (`web/src/`)
- `App.svelte`: setup, recovery key, unlock, new password, then the app (`pages/`).
- `pages/Shell.svelte`: the ribbon, the current view (sliding by view order), overlays, keyboard
  shortcuts (`lib/keys.ts`) and the copy guard (`lib/protection.svelte.ts`).
- `views/`: `Canvas` (the Desk), `Day`, `Timesheet`, `Notes`, `Settings` with `customize/*`.
- `components/`: cards, panels, windows, the bottom bar and bell, the paper menu and the rest.
- `lib/api/`: the only place that talks to the app (`DataApi`, `TauriApi`). `lib/*.svelte.ts`
  hold shared state filled from it; plain `lib/*.ts` files are small helpers, most with tests.
- **CSP:** styles are the compiled stylesheet plus CSSOM changes (Svelte `style:` directives,
  `element.style`), never inline `style` attributes, so `style-src 'self'` holds. Animations use
  CSS and the Web Animations API.
- **Paper menu:** the source element is hidden and a copy of it is drawn above everything with
  a lift; the sheet sits in a clip box that starts at the object's edge so it slides out from
  underneath; it comes out below unless below is cramped, and scrolls when it doesn't fit.
  Icon rails sit on the right (left if there's no room) and on top (below if there's no room).
