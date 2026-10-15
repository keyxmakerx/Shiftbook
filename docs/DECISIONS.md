# Decisions

Decisions made on the sign-off sheet (`tools/signoff/`), newest first.

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
