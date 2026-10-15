# Decisions

Decisions made on the sign-off sheet (`tools/signoff/`), newest first.

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
