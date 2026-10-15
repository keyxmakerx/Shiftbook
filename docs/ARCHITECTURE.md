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

## Storage, vault and unlock (M1)
`crates/storage` modules:

| Module | What it holds |
|---|---|
| `vault` | `VaultDir` (create, unlock with password or recovery key, change password, regenerate recovery key), `Vault`/`Dek` (the unlocked key, wiped on drop), `VaultError` |
| `recovery` | `RecoveryKey`: 256 random bits shown as 13 groups of 4 Crockford base32 characters; forgiving parser |
| `db` | `open_encrypted(path, dek)`: SQLCipher pool with the key pragma first, then migrations |
| `repo` | Repository traits + `SqliteStore` (local user, settings, list options, frequencies) |
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
