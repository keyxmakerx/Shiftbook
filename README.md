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

## What the alpha does
The alpha is a working app you can use day to day for logging time:

- **Lock and protect:** a master password (and a recovery key) encrypts everything; Lock now,
  Ctrl+L and auto-lock after idle (minutes are set in Settings → Security). After unlocking with
  the recovery key you choose a new password straight away. Screenshots, screen sharing and
  copying text out are blocked by default; the **Protected** pill allows them for 10 minutes.
- **Desk (the Canvas, home):** your entries, tickets, notes and Inbox scraps as paper cards
  you arrange. Drag to move (it's remembered), drag empty paper or scroll to pan, Ctrl + scroll
  to zoom, Fit everything. Click an entry or a ticket and its **slide-out** opens underneath:
  the notepad (saved as you type), its tickets, its project, Bring up tomorrow, done or still
  open, more time and More. Double-click a card to open it; double-click empty paper to jot.
  Right-click anything for its paper menu with everything. The side panel has today's hours,
  Where was I?, Log work, the Tickets tray and the wrap-up. **Project areas**
  (right-click empty paper) give cards dropped into them that project; **links** join cards
  that share a ticket or an uncommon word, and you can draw your own; **Timeline** lays the
  Desk out by day and Back to my layout puts it back. The **toolbox** on the left has Move,
  Select (drag around cards to move them together) and Box: draw a box (or Shift + drag) and
  make it a project's area, a new project or just a label. Any box **folds up** to a count
  coloured grey (nothing waiting), blue (from today) or orange (still open or brought up).
  Tickets ticked done today gather in a **Done today** pile and leave the Desk at the end of
  the day (find them on the Done tickets board or with Find). Drop a card on a card to make a
  **pile** (click it to fan out); **Tidy into stacks** piles
  loose cards by project. The **drawers** (one per project plus your folders) slide out when
  you drag something: drop it on one to file it off the Desk; it stays in Find and on the
  timesheet. **Boards** open a project, a meeting series, the Daily notes, a folder or the
  **Archive** full screen, as a Notebook, a Board of its own or a List; links in notes show
  with Copy. See `docs/CANVAS.md`.
- **Day:** for starting and ending the day: today's hours against 8 h and the week total; **Log work** (paste a ticket or type a
  few words, press 15m/30m/45m/1h/2h — a ticket already logged today gets the time added to its
  entry); the **Tickets** tray (paste tickets as you work on them, each adds 15 minutes you
  can step up or down, and they roll up into one “Tickets” entry for the day; a box beside each
  ticket takes a few words, and its slide-out holds a full notepad; **untick** a ticket that
  isn't finished and it stays **Still open**, coming back each day until you tick it; give a
  ticket a project and its minutes count under that project instead); the entries list with
  +15m/+30m/+1h and ticket chips you click to copy (click an entry for the same slide-out as on
  the Desk); **Notes** (Jot a quick thought, Today's note that saves as you type, and recent
  meeting notes, all kept in Notes); the Inbox.
- **Windows, the bottom bar and the bell:** **Pop out** in a slide-out (or Open in a window on
  a right-click menu) puts a notepad or a note in a window that floats over everything and
  saves as you type. Drag its title bar to move it and its corner to resize it; ▴ rolls it up
  to the title bar, – minimises it to the bottom bar, ⤢ fills the screen, ✕ closes it. Windows
  come back where they were after you unlock. The thin **bottom bar** lines windows up from the
  left (click to show or tuck away, right-click for more), with today's hours and the **bell** on
  the right: its panel slides in with meetings about to start, things brought up for today and
  tickets still open. Meetings that repeat are set in **Settings → Meetings** (days, time, how
  early to remind); the bell opens that day's meeting note. All of it stays inside Shiftbook.
- **Saved responses:** the copy button on the Tickets tray lists messages you paste into tickets
  again and again (“Fixed and confirmed with the user”); pick one and it's copied. Write
  them in Settings → Responses.
- **Where was I?:** after you unlock, come back from a break of 30 minutes or more, or a new
  day starts, the Desk shows what to pick up: things you brought up for today (**Bring up
  tomorrow** in any slide-out or right-click menu picks the next work day), then the list from
  your wrap-up, or else the last thing you logged with its tickets and notepad. **Carry on** puts it into Log work;
  **Later** hides it until the next unlock.
- **Timer (optional, off by default):** turn it on in Settings → Your day. ▶ Start on one of
  today's entries runs a small clock under the top bar; ■ Stop offers to add the time rounded
  to 15 minutes, and to leave out stretches of 5 minutes or more away from the keyboard. Only
  minutes are saved. Locking pauses it.
- **Details drawer:** click an entry to change its title, hours, project, category, tickets
  (paste several at once), notes and your own custom fields.
- **Capture:** Ctrl+Space anywhere in the app, then one line such as
  `1.5h firewall cleanup CHG0012345 #network yesterday`. Enter logs it; Ctrl+Enter (or a line
  without hours) saves it to the Inbox. Type without a time to **Find**: the same box searches
  your entries, Inbox scraps, notes and commands.
- **Notes:** Jot, Daily, Meeting, Page and MOP notes, each with an optional project and tags,
  saved as you type. Search every note with the match highlighted, filter by kind, and
  **Archive** notes to get them out of the way (Include archived, or Find, still shows them).
  Auto-archive (Settings → Notes, 3 months by default, can be off) moves Jot, Daily and Meeting
  notes that haven't changed for that long, after a week of warning on the note with a Keep
  button. Capture's Find shows notes too, archived ones included.
- **Your day:** a daily target for each weekday (the ring uses it), a “Same as yesterday” row
  in Log work, and an optional end-of-day **wrap-up** card (off by default) with hours today,
  entries that need a code, the tray and a short “Tomorrow I start with…” list.
- **Ribbon:** the last four weeks as bars of coloured hours; click a day to open it. A weekday
  with nothing logged gets a dot, and clicking it opens Log work for that day.
- **Timesheet:** a “what to enter” crib sheet for a week — per day, each line is a timesheet code
  (or project), its hours and a ready-made comment with your tickets; Copy comment, Copy whole
  week (tab-separated), tick-offs that are remembered, an ENTERED stamp, effort per project and
  warnings (no code yet, a weekday with nothing logged, a day over 12 h).
- **Settings:** Look (theme, motion, accent, text size, project colours), Your day (targets,
  wrap-up, timer), Projects (add, rename, colour,
  default timesheet code, archive), Timesheet (your codes, category → code, comment format,
  grouping), Responses (saved messages for tickets), Fields (custom fields: text, number, choice, link — link fields with a ticket prefix
  such as `PROJ-` give matching tickets a “Copy link” option; links are never opened), Backups
  (Back up now, the daily automatic backups, Restore from a backup or a file), Security (auto-lock, change password, and **Delete all
  data** to start again; the unlock screen offers it too if the recovery key is lost).
- **Paper menus:** right-click an entry, ticket chip, ribbon day, tray ticket or project. The
  object lifts, a paper sheet pulls out from under it with every option, and icon buttons line
  its side and top. Click elsewhere or press Esc to fold it away. Shift+F10 or the Menu key
  opens it from the keyboard.
- **Keys:** N focuses Log work, Ctrl+Space capture, 1 to 5 switch views (Desk, Day, Timesheet, Notes, Settings), Ctrl+L lock, Ctrl + / − / 0
  text size, arrow keys move between entries, Esc closes menus, capture and the drawer. Hold
  Ctrl to see every shortcut.

Not in the alpha yet: Planner, pictures in notes, the meeting canvas, MOPs, OneNote import, the notes map, a global (outside the app)
capture shortcut, export/import, the picture editor, bundled fonts and signed installers.
See “Known gaps in alpha” in `docs/ROADMAP.md`.

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

To try the app exactly as it ships (embedded UI, strict CSP) without making installers:

```bash
cd web && npm run build && cd ..
cargo build -p shiftbook-desktop --features tauri/custom-protocol
./target/debug/shiftbook-desktop
```

A plain `cargo build` (without the feature) makes a development binary that loads the UI from
the Vite dev server, which is what `npm run tauri dev` uses.

User data is stored outside the repo (OS app-data folder) and is gitignored.
