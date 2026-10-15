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
- Data path: Tauri `app_data_dir()` + `work-tracker.db`; user can relocate via Settings
  (stored in a small config file in app config dir).
- Backups: copy DB with SQLite online backup API (`VACUUM INTO`) to `backups/YYYY-MM-DD.db`.
- Plugins allowed: `dialog` (pick backup/import file), `fs` scoped to app data + user-picked paths.
  Not allowed: `shell`, `http`, `opener` for remote URLs, updater (until signing is set up).

## Server specifics (Phase 2 — plan only)
- Axum + tower; same `core`/`storage` crates; Postgres (or SQLite for tiny installs).
- Auth: OIDC (Authorization Code + PKCE) against the company IdP; session cookie
  (`HttpOnly; Secure; SameSite=Lax`). Optional local accounts with Argon2id for small installs.
- RBAC: `user`, `admin`. Admin panel: users, roles, shared list options, audit log, team reports.
- Reverse proxy: `BASE_PATH` env, honour `X-Forwarded-Proto/Host/For` only from `TRUSTED_PROXIES`,
  `/healthz` (liveness) and `/readyz` (DB check).
- Docker: multi-stage build → distroless/non-root image; config via env; data via volume or
  external Postgres; migrations on start.
- Import: users upload their desktop JSON export; UUIDs make it a clean merge.
- Optional Phase 3: desktop ↔ server sync using `updated_at` + soft deletes.
