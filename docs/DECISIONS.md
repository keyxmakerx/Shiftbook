# Decisions

Decisions made on the sign-off sheet (`tools/signoff/`), newest first.

## Round 7 — 16 Oct 2026
- **Week start:** a "My week starts on" setting (any day, Monday by default); approved and merged.
- **"Where was I?":** approved. After a lock, a break of 30 minutes or more, or a new day, the Desk
  shows the last task, its tickets and its next step, with Carry on / Something else / Later.
- **"Now doing" timer:** approved as an optional feature, off by default (the owner wouldn't use
  it). Rounds to 15 minutes, saves minutes only, suggests leaving out time away from the keyboard.
- **Log work:** Enter logs the line when it has a time in it (`INC0045678 50m`), otherwise moves to the
  time buttons. A "Same as yesterday" row offers yesterday's routine entries as one-click buttons.
- **Capture finds things:** typing searches entries, days and Inbox scraps, and runs commands (Lock,
  Book holiday, go to a view). A line with hours still logs.
- **Views:** Planner and Canvas are hidden until they're built, so 1 to 3 match the tabs. Customize
  stays a tab.
- **Readability:** a daily target per weekday for the Desk ring, a slightly darker hint grey, Undo that
  stays 10 seconds and waits while the pointer is over it. Text size: asked whether "I don't want a
  constant text size" means a quick Ctrl + / Ctrl − zoom; building that with the setting beside it.
- **Wrap-up and gaps:** an optional end-of-day wrap-up (off by default) that takes a short list of
  next things, shared with the "Where was I?" card; weekdays with nothing logged get a dot on the ribbon.
- **First-run screens:** restyle setup, unlock and the recovery key in the paper look, after the
  sign-up size fix (PR #7) lands.
- **Small fixes:** time buttons on the keyboard-selected entry, a This week button on the Timesheet,
  Tab stays inside capture and the details panel, plainer names. Shortcuts: holding Ctrl shows an
  overlay of every shortcut (not a ? key), with a subtle hint about it inside the app.

## Round 6 — 15 Oct 2026
- **Timesheet view:** a "what to enter" crib sheet, not a data-entry grid. Logging happens on the
  Desk (paste a ticket, press 15m/30m/1h) or via capture; the Timesheet view lists, per day, the
  code, hours and a ready-made comment with ticket numbers, with tick-offs and a small effort
  summary.
- **Look:** keep pushing: user-editable project colours, right-click menus everywhere (fold-out,
  close on click-away), a slide-out details panel, and more motion including directional view
  transitions; motion setting Still / Calm / Lively.
- **M1 follow-ups:** after a recovery-key unlock, ask for a new password at once; add "Make a new
  recovery key" next milestone; keep the Copy key button with its warning and 60 s clipboard clear.

## Round 5 — 15 Oct 2026
- **Navigation:** a time ribbon across the top (days as bars of coloured hours), the Desk as home,
  and switchable views: Planner (week/month/year, formerly "Zoom"), Canvas and Timesheet.
- **Canvas:** coloured paper; dragging a card into or out of a project area changes its project
  (with Undo); right-click opens a fold-out menu.
- **Protection:** while protected the window is hidden from screenshots and screen shares (text
  included) and copying text out is blocked; the timed allow button lifts both. The timesheet's
  Copy button keeps working.
- **Core rules:** all choices on the rules card approved (a plain number up to 24 is hours, above
  is minutes).

## Round 4 — 15 Oct 2026
- **Layout direction:** keep the free canvas with a timeline mode (switching must not disturb the
  user's own arrangement), a calendar planner (week, month, year) that folds open, and
  daily/jot/meeting notes in an always-available side strip. Main navigation decided in round 5.
- **Look:** coloured paper without clichés (no tape, torn edges or tilted cards), more colour.
- **Picture editor tools:** box, arrow, highlight, hide (flattened, unrecoverable), crop, text
  labels, numbered steps.
- **Capture protection:** on by default, with a timed "allow screenshots" button (default 10 min).
  Uses Tauri's content protection; reliable on Windows, partial on macOS, unavailable on Linux.
- **M1:** approved to start, with sqlx + SQLCipher, argon2, chacha20poly1305, rand, zeroize,
  secrecy, uuid, time and thiserror. Research current security guidance and CVEs first.

## Round 3 — 15 Oct 2026
- **Look:** the technical skins were also rejected; the shared sidebar-plus-list layout is the
  problem. Round 4 compares layouts as unstyled wireframes before any styling.
- **Notes map:** a free canvas the user arranges, with an option to lay it out as a timeline.
- **New features from feedback:** pictures in notes with a small markup editor (box, arrow,
  highlight, hide, crop), and protection against screenshots and screen sharing with a timed allow
  toggle.
- **Sign-off sheet:** notes to Claude accept pictures.

## M0 — 15 Oct 2026
- TypeScript held at 6.0.x because `svelte-check` 4.7 does not support TypeScript 7 yet.
- The desktop capability grants only `allow-app-info` (not even `core:default`); every later
  command or plugin permission is added explicitly.

## Round 2 — 15 Oct 2026
- **Quick capture:** one box for logging, notes and search, with letters-in-order matching and an
  Inbox for scraps. Works from anywhere via `tauri-plugin-global-shortcut` (approved; needs no admin
  rights), default `Ctrl+Shift+Space`, changeable in Settings.
- **Portable:** runs without an installer on all three systems. Data lives in the user profile by
  default; a `portable` file next to the app keeps data beside it. How others get it: later.
- **Node:** confirmed build-only; build output is never committed; releases are built by CI.
- **Roadmap:** quick capture in M2, look and motion M3, notes M6, notes map M7, portable builds in
  hardening (see round 2 card). ROADMAP.md is updated once the look is settled.
- **Notes map:** links are made automatically (shared tickets, projects, words) and can be removed.
  The main pages are the list view; the map has no separate list mode. Map style: round 3.
- **Look:** paper directions rejected; technical directions in round 3. Keep sliding sheets.
- **Docs:** employer-specific content removed (history rewrite pending a permission).

## Round 1 — 15 Oct 2026
- **Name:** Shiftbook everywhere. Backups `.shiftbak`, data file `shiftbook.db`,
  app identifier `io.github.keyxmakerx.shiftbook`.
- **Scope:** a general-purpose personal tool for anyone (designed with ADHD in mind), not a
  company app. The repo never holds any employer's names, codes, data or internal details;
  workplace timesheet codes and category mappings were removed from the spec (users add their own).
- **New goals** (planned in round 2): meeting notes, quick capture that works when you can't
  remember a name, a process-style notes map, a paper-like look with purposeful motion, and running
  without an installer.
- **Starting point:** fresh repo; docs committed as the base; nothing reused from other projects
  except the sign-off sheet idea. Claude leads the build; the owner reviews visually.
- **Node:** build-time only. The shipped app contains no Node runtime (CLAUDE.md rule 11).
- **Theme:** follow the OS, with a manual light/dark switch in Settings.
- **Sign-off sheet:** visuals first; motion only on cards about motion; a summary card at the end
  of each milestone.
