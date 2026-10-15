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
- Data path: Tauri `app_data_dir()` + `shiftbook.db`; user can relocate via Settings
  (stored in a small config file in app config dir).
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
