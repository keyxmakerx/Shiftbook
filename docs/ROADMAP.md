# Roadmap

Each milestone should end with passing tests and a working build on all three OSes (CI).

## Phase 1 — Desktop (Tauri)
- **M0 Scaffold:** Cargo workspace (`crates/core`, `crates/storage`, `apps/desktop`), Svelte+Vite
  `web/`, Tauri 2 config with strict CSP and minimal capabilities, `.gitignore`, CI workflow
  (fmt, clippy, tests on ubuntu/windows/macos), Dependabot config. App opens a blank "Today" page.
- **M1 Core + encrypted storage:** models, migrations, repositories, all business rules from
  SPEC §4 with unit tests (port every Excel alert state as a test case). Seed default lists +
  frequencies. **Vault + SQLCipher encryption, first-run setup (password + recovery key), unlock
  screen, auto-lock, change password** (SPEC §2.9, SECURITY "Encryption design"), with tests for
  wrong password, recovery-key unlock, password change, and DB unreadable without key.
- **M2 Time log + tickets:** Today screen quick log, hours parsing, + time, ticket chips (multi-paste),
  Time Log week view with muted past days, edit/delete (soft).
- **M3 Projects + recurring:** CRUD screens, calculated fields, alert badges, "Mark done",
  alerts_enabled toggle for routine items.
- **M4 Tasks:** list + board views, link to project/recurring, log time from task, checklists.
- **M5 Dashboard + timesheet:** cards, SVG charts; timesheet codes (user-defined,
  category defaults, project/recurring defaults, per-entry override, Unassigned warnings),
  Holiday quick entry, code × day timesheet grid with copy / .xlsx / .csv export.
- **M6 Data management:** Settings lists, data location, **encrypted backups** (automatic +
  Backup now + Restore, `.shiftbak` format with round-trip tests across password and recovery key),
  **exports to Excel (.xlsx), CSV (+zip) and JSON** with the unprotected-file warning,
  import of `tracker-import.json` (see IMPORT_FORMAT.md).
- **M7 Hardening:** security review of crypto/unlock code, accessibility pass, keyboard shortcuts,
  release workflow producing Windows `.msi/.exe`, macOS `.dmg`, Linux `.AppImage/.deb`.

## Alpha (Oct 2026)
The owner moved from mockups to a working app. The alpha covers M2 (time log and tickets), the
look and motion from the approved prototype, the timesheet crib sheet and timesheet codes from
M5, quick capture inside the app, the Done tickets tray, custom fields and the paper menus. See
the README for what it does.

### Known gaps in alpha
- **Planner** (week/month/year): hidden until it is built. **Canvas:** step 1 of 5 is built
  (it is the Desk); the rest is listed in `docs/CANVAS.md`.
- **Notes:** the core is in (kinds, project and tags, search, archive, auto-archive, notes in
  Find). Still to come, in this order: the meeting canvas, MOPs with Run mode and export, then
  imports (paste, MOP import with its check screen, OneNote sections), and the **notes map**.
  The Desk has a Notes card (Jot, Today's Daily note, Meetings); a strip that stays open on every
  view is still to come.
- **Global capture shortcut** (`tauri-plugin-global-shortcut`, Ctrl+Shift+Space from anywhere):
  capture only works while the Shiftbook window has focus.
- **Export and import** (xlsx/csv/json, Import JSON/Excel) — M6. Encrypted backups are in:
  automatic once a day after unlock (last 14 kept), Back up now and Restore. Still to do for
  backups: a backup on app close, a setting for how many to keep, and a restore test in CI.
- **Picture editor** for notes.
- **Bundled fonts:** the look uses Fraunces / Instrument Sans / JetBrains Mono where installed
  and otherwise Georgia, Segoe UI/system-ui and Consolas/monospace. Bundling the OFL fonts
  inside the app (no network) is a follow-up.
- **Code signing** (Windows Authenticode, Apple notarisation) and the portable builds.
- Smaller gaps: recurring work, tasks and the dashboard (M3–M5) have no screens yet; custom
  fields on tickets and projects can be defined (link fields already work on tickets) but values
  can only be filled in on entries; the tray's roll-up entry starts with no project; the Planner
  has no keyboard shortcut yet (1 to 5 are Desk, Day, Timesheet, Notes, Settings).

## Phase 2 — Hosted (only if adopted by the team)
- **S1** `apps/server` with Axum reusing core/storage; Postgres migrations verified in CI.
- **S2** `HttpApi` adapter in `web/`; same UI served as static files by the server.
- **S3** OIDC login, users table, RBAC, per-owner authorization tests.
- **S4** Admin panel: users/roles, shared list options, team reports, audit log.
- **S5** Docker image (multi-stage, non-root), reverse-proxy settings, health checks, compose example.
- **S6** Import desktop JSON exports; optional desktop ↔ server sync (Phase 3).
