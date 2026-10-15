# Work Tracker

A private, offline desktop app for logging work hours, tickets, tasks, projects and recurring
work (SSL certs, patching, audits) — built with Tauri 2, Rust and Svelte. Runs on Windows,
macOS and Linux. No server, no network access, data stays on your machine.

- Product spec: `docs/SPEC.md`
- Architecture: `docs/ARCHITECTURE.md`
- Security: `docs/SECURITY.md`
- Roadmap: `docs/ROADMAP.md`
- Import/export format: `docs/IMPORT_FORMAT.md`

## Development (after M0 scaffold)
Prerequisites: Rust (stable), Node.js LTS, and the Tauri 2 system prerequisites for your OS.

```bash
npm install            # in web/
npm run tauri dev      # run the desktop app
cargo test --workspace # run Rust tests
```

User data is stored outside the repo (OS app-data folder) and is gitignored.
