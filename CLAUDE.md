# CLAUDE.md — Shiftbook

Instructions for Claude Code working in this repository. Read this file first, then `docs/SPEC.md`,
`docs/ARCHITECTURE.md`, `docs/SECURITY.md` and `docs/ROADMAP.md` before writing code.

## What this is
A personal work tracker for anyone who wants help keeping on top of their day (designed with ADHD
in mind): time log (hours, not start/end times), tickets per entry, task manager, projects,
recurring tasks with due-date alerts, notes, and a dashboard. It is a general-purpose side project:
the repo must never contain any employer's names, codes, data or internal details.
**Phase 1** is a cross-platform desktop app (Windows, macOS, Linux) built with **Tauri 2**.
**Phase 2** (later) is a self-hosted multi-user web app in Docker that reuses the same core and UI.
Every design decision must keep Phase 2 cheap — see "Non-negotiable rules".

## Tech stack (do not change without asking)
- **Desktop shell:** Tauri 2 (Rust). No Electron.
- **Core logic:** Rust library crate `crates/core` — pure functions, no I/O, no Tauri imports.
- **Storage:** Rust crate `crates/storage` using `sqlx` with SQLite (Phase 1). Must stay Postgres-compatible.
- **Frontend:** TypeScript + Svelte 5 + Vite, built as a static SPA in `web/`.
- **Tests:** `cargo test` for Rust; Vitest for frontend logic; Playwright smoke test later.
- **CI:** GitHub Actions — test on ubuntu/windows/macos; release builds via `tauri-apps/tauri-action`.

## Repository layout
```
crates/core/        domain models, validation, calculations (alerts, due dates, totals)
crates/storage/     repository traits + SQLite implementation, SQL migrations
crates/app/         views built from storage + core (the Desk), shared by desktop and server
apps/desktop/       Tauri app (src-tauri) — thin layer: commands call core + storage
apps/server/        Phase 2 only (Axum). Leave a README placeholder until Phase 2 starts.
web/                Svelte SPA. All data access goes through web/src/lib/api/
docs/               SPEC, ARCHITECTURE, SECURITY, ROADMAP, IMPORT_FORMAT
```

## Non-negotiable rules
1. **UI never touches storage directly.** All data access goes through the `DataApi` TypeScript
   interface in `web/src/lib/api/`. Phase 1 implements it with `TauriApi` (invoke commands).
   Phase 2 adds `HttpApi`. No `@tauri-apps/*` imports anywhere else in `web/`.
2. **Business rules live in `crates/core`** (alerts, next-due, week-of, totals) and are unit-tested.
   The frontend displays results; it does not re-implement rules.
3. **Every record has:** `id` (UUID v4/v7, never autoincrement), `owner_id`, `created_at`, `updated_at`,
   and soft-delete `deleted_at` where noted in the spec. Single-user now, multi-user later.
4. **Time is stored as integer minutes**, never floating-point hours. Dates are ISO `YYYY-MM-DD`
   (local calendar date); timestamps are UTC RFC 3339.
5. **SQL must run on SQLite and Postgres.** Avoid SQLite-only features. Use migrations in
   `crates/storage/migrations/`. No `AUTOINCREMENT`, no JSON1-only queries in core paths.
6. **No network access in the desktop app.** No remote URLs, CDNs, web fonts, analytics or telemetry.
   Strict CSP (see SECURITY.md). Tauri capabilities are allow-list only.
7. **Minimal dependencies.** Ask before adding any new crate or npm package; prefer the standard
   library. Pin versions; keep `Cargo.lock` and `package-lock.json` committed.
   Pre-approved (still mention when you add them): `tauri` + plugins `dialog`/`fs`, `serde`,
   `serde_json`, `sqlx` (sqlite, sqlcipher via `libsqlite3-sys`), `uuid`, `time` or `chrono`,
   `thiserror`, `argon2`, `chacha20poly1305`, `rand`, `zeroize`, `secrecy`, `rust_xlsxwriter`,
   `csv`, `zip`, `zstd` or `flate2`, `svelte`, `vite`, `typescript`, `vitest`, `ts-rs` (build-time
   only, behind each crate's `ts` feature).
8. **Never commit user data.** `*.db`, `*.sqlite*`, `*.json` exports and anything in `data/` or
   `local-data/` are gitignored. The import file lives outside the repo.
9. **Cross-platform:** use standard web APIs only (WebView2, WebKit and WebKitGTK must all work).
   Use Tauri's path APIs for app-data locations; never hard-code OS paths.
10. **Password protection is mandatory.** The database is always encrypted (SQLCipher) with a key
    from the user's master password (Argon2id) plus a one-time recovery key — see SECURITY.md
    "Encryption design". Backups are always encrypted (`.shiftbak`). Only explicit user exports
    (xlsx/csv/json) are readable, and they show a warning. Never log passwords or keys.
11. **Node is build-time only.** npm packages compile the UI into static files; the shipped app
    contains no Node runtime and no npm code runs on the user's machine except that compiled bundle.
12. **Accessible, simple UI:** the primary user is not a spreadsheet power user. Big obvious buttons,
    keyboard-friendly, clear empty states, no jargon.

## Working style
- Work in small, reviewable steps that match the milestones in `docs/ROADMAP.md`.
- Write/extend unit tests in `crates/core` for every rule before wiring UI.
- Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `npm run check`, `npm test`
  before saying a step is done.
- When a requirement is ambiguous, ask instead of guessing. Questions, mockups and diagrams go on
  the sign-off sheet (`tools/signoff/README.md`); the owner reviews visually and is not a programmer.
  Record approved decisions in `docs/DECISIONS.md`.
- Do not run commands that send data off the machine except `git push` to the configured remote
  and package installs the user approved.
