# Shiftbook

A private, offline desktop app for logging work hours, tickets, tasks, projects, notes and
recurring work (SSL certs, patching, audits). Built for anyone who needs help keeping track of
their day, with quick capture and as little friction as possible. Built with Tauri 2, Rust and
Svelte; runs on Windows, macOS and Linux. No server, no network access, data stays on your machine.

- Product spec: `docs/SPEC.md`
- Architecture: `docs/ARCHITECTURE.md`
- Security: `docs/SECURITY.md`
- Roadmap: `docs/ROADMAP.md`
- Import/export format: `docs/IMPORT_FORMAT.md`
- Decisions: `docs/DECISIONS.md` (made on the sign-off sheet, see `tools/signoff/`)

## Development
Prerequisites: Rust (stable), Node.js LTS, and the
[Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS
(on Linux: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`,
`librsvg2-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `build-essential`, `pkg-config`).
Node is only used to build the UI; the app itself ships without it.

Layout: `crates/core` (rules), `crates/storage` (database), `apps/desktop/src-tauri` (Tauri app),
`web/` (Svelte UI), `apps/server/` (Phase 2 placeholder). All npm commands run in `web/`.

```bash
cd web
npm ci                 # install the exact locked packages
npm run tauri dev      # run the desktop app (starts Vite, then the Tauri window)
npm run tauri build    # build installers into target/release/bundle/
```

Checks (the same ones CI runs on Windows, macOS and Linux):

```bash
# in web/
npm run check          # svelte-check (TypeScript + Svelte)
npm test               # Vitest
npm run build          # builds web/dist — needed before the Rust steps below

# in the repo root
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The Tauri crate embeds `web/dist` at compile time, so run `npm run build` once before
`cargo clippy`/`cargo test` on a fresh checkout. `npm run dev` alone serves the UI in a browser
at http://localhost:5173, but data calls only work inside the desktop app.

User data is stored outside the repo (OS app-data folder) and is gitignored.
