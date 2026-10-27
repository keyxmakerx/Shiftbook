# Decisions

Decisions made on the sign-off sheet (`tools/signoff/`), newest first.

## Round 15 — 27 Oct 2026 (the MOP notebook)
Card `r15-mopbook` (`tools/signoff/rounds/r15-mopbook.json`, mockups `mocks/r16-mopbook.html`),
approved. The owner writes MOPs in Notepad++ and wanted something like it:
- **Shape (A):** a floating window with a tab per MOP, like Notepad++. It rolls up, minimises to
  the bottom bar and fills the screen; where it is and which tabs are open come back next time.
- **Files:** drop files anywhere on Shiftbook (or Add MOP files…) to copy them in. Each becomes
  an encrypted MOP or template, picked per file; the files are never changed. Plain text for
  now; Word files are told to save as plain text first (.docx reading is a later step).
- **Templates:** Use as template keeps `[[blanks]]`; New from this template asks for each blank
  once and fills it in everywhere. Blanks left empty stay highlighted.
- **The owner's note:** "the option to have it be numbered" with character counts and what's
  selected, "other modes like maybe minimal", and "checkbox style that crosses out that section
  under it", "quickly editable via the same window". Built as three modes switched at the top of
  the window, all editable: Numbered (a line number toggle and a status bar with line, column,
  lines, characters and the selection), Minimal (just the text) and Checklist (ticking a heading
  or step crosses out the lines under it, down to the next one at the same level). Ticks are
  stored beside the MOP, not in its text, and Clear ticks starts a fresh run.
- **Save as a text file** writes an unencrypted copy after a warning (rule 10).

## Code review — 27 Oct 2026 (keeping tickets in step)
The owner asked for a step back over the whole app and approved the cleanup plan in chat
("confirmed please continue"), including ts-rs for the UI's types (decision card).
- **Back-dated work doesn't reopen a ticket.** Adding a ticket to a day before its newest day
  (logging forgotten work) takes the ticket as it stands: a done ticket stays done. Work on its
  newest day or later still reopens it, as before (`core::items::back_dated`).
- **Taking a ticket out puts its tick back.** Removing a tray row, an entry's ticket or a whole
  entry (and Undo) sets the ticket's done tick from its newest day still there
  (`core::items::done_from`). A ticket on no day any more leaves the Items view.
- **Mark done on a repeating thing** always makes or ticks its own entry, never the Tickets
  tray's roll-up, which comes and goes with the tray.
- **ts-rs for the UI's types** (decision card, "Yes, add ts-rs"): build-time only, behind a
  `ts` feature. The first check found two real mismatches, both fixed: a minimised window was
  sent as "minimised" where the UI expects "min" (it came back wrong after a restart), and the
  bell's repeating-thing line was sent `next_due` where the UI reads `nextDue` (no due day).
- **One change signal in the UI:** `refresh()` bumps `app.changed`; the Items view, drawers and
  floating windows load again on it. A floating notepad never overwrites text written elsewhere
  since it opened: it offers "Keep mine" or "Show the newer one" (closing keeps both).
- **Sync keeps tickets used on both devices.** Two items for one ticket number become one (the
  notepads merged, none lost) instead of the second being left out with its tray rows and
  entry tickets. After a merge each touched day's Tickets entries are counted again from its
  tray, and every touched ticket's copies are put back in step.
- **Kept on purpose:** the unused `timesheet_weeks` table stays, because dropping it would stop
  older backups and sync files from loading. Items' `kind`/`title` columns stay for the planned
  items work. The dead "unstack a card" command is gone (moving a card already does it).
- **Not split yet:** the biggest files (Canvas.svelte, work.rs, api.rs) wait until the other
  open branches touching them have landed, to avoid merge clashes.

## Round 15 — 27 Oct 2026 (lining cards up)
Card `r15-lineup` (`tools/signoff/rounds/r15-lineup.json`, mockups `mocks/r15-lineup-*.html`),
approved with A and the buttons:
- **Shift + drag a card lines it up** (A). Shift + drag on empty paper still draws a box. The
  owner asked for it to be "dynamic like Lucidchart" and to "consider drawn boxes and lining up
  inside them", so it uses smart guides: other cards' edges, a box's inside edges and middle,
  and even spacing with the row or column, each shown with red guide lines.
- **Line up buttons:** In a row, In a column and Tidy grid on the selection bar, with Undo.

## Round 15 — 27 Oct 2026 (copying responses while Protected)
- **Responses stay one click away:** each response in Settings → Responses gets a Copy button,
  and the Tickets card's copy button gets the word "Responses". Both copy even while copying is
  blocked; Ctrl+C blocking itself is unchanged.

## Round 15 — 26 Oct 2026 (turning protection off)
- **Copy and screenshot protection switches:** Settings → Security gets two switches, "Block
  copying text" and "Hide from screenshots", both on by default. Turning one off asks once with a
  short warning. With one off, the Protected button guards only the other; with both off it
  leaves the top bar. The recovery key's Copy button keeps its 60-second clipboard clear.

## Round 15 — 27 Oct 2026 (putting entries away)
Card `r15-putaway` (`tools/signoff/rounds/r15-putaway.json`, mockups `mocks/r16-putaway.html`),
all approved. The owner suggested a built-in drawer called Archive, which is how it's built:
- **Put away** (entry cards and done tickets, in the card's menu) files the card in the
  **Archive** drawer, always first in the cabinet. An entry card takes its other days of the
  same work with it. Undo is offered; the Archive drawer and the Archive board have Back on the
  Desk. Hours, tickets and notes are never touched.
- **Tidy up…** on the Desk's bottom bar: everything from before today, everything older than
  this week, or the cards picked with Select. Notes, open tickets and pinned cards stay.
- **How long entries stay:** Settings → Look → The Desk, 1, 3, 7 (default) or 14 days.
- **Cards I moved** leave after the same time, unless pinned, counted from the later of their
  own day and the day they were last moved (so a card brought back doesn't vanish at once).
  A switch keeps moved cards until they're put away, as before.

## Round 15 — 26 Oct 2026 (changing an entry's name)
Card `r15-rename` (`tools/signoff/rounds/r15-rename.json`, mockups `mocks/r16-rename.html`),
both parts approved:
- **Rename on the entry:** a ✎ appears when you point at an entry on Day or an entry card on the
  Desk; F2 or **Rename** (first in its right-click menu) does the same. The name becomes a box:
  Enter or clicking away saves, Esc cancels, and a note offers Undo. Where the entry isn't shown
  as a row or card (in a pile), Rename opens its details instead.
- **In the details:** the "What" box is now **Name**, with a ✎, and shows **✓ Saved** for a
  moment after a change. A blank name puts the old one back.

## Round 15 — 26 Oct 2026 (work that keeps coming back)
Card `r15-repeating` (`tools/signoff/rounds/r15-repeating.json`, mockups
`mocks/r16-repeating.html`); the owner asked where certificate renewals belong ("would that be
a project?") and approved all three, with the recommended picks.
- **Repeating things hold work** ("Build it"): a repeating thing is a peer of projects. Entries,
  Log work (`#name`), entry details, the Tickets tray and a ticket's item can count time under
  it; tray tickets roll up per project or repeating thing. Logging work against it doesn't mark
  it done.
- **Codes** ("On the thing and its kind"): a repeating thing can have its own timesheet code,
  and a kind of repeating thing gets a line under Customize → Timesheet → Default codes. Order:
  entry → project or repeating thing → its kind → category → Tickets → Unassigned.
- **Start renewal and past renewals** ("Build it"): Due soon has Start renewal (Log work opens
  filed under the thing) and Renewed…; Renewed… keeps the new expiry date, and the Repeating view
  lists past renewals with their tickets, time and renewed-to date.

## Round 15 — 26 Oct 2026 (one record per ticket, task or chore: the mockups)
Card `r15-things` (`tools/signoff/rounds/r15-items.json`, mockups `mocks/r15-items.html`),
approved.
- **Bell** ("Good"): each item once, with Open, Tomorrow and Done; a ticket shown to follow up
  isn't counted again under still open.
- **Menu** ("Good"): one **Follow up** on every menu, the item slide-out and a pin's chip
  (Later today, next work day, In 3 days, Next week, Pick a day, No follow-up). A pin's chase day
  and "Bring up tomorrow" are the same follow-up day now.
- **Log work** ("Ask me, as shown"): typing the title of earlier work asks once "Add to …?";
  work already logged that day joins without asking. A known ticket shows its time so far,
  project and notepad, in Log work and the tray.
- **Items view** ("Good, add it as a view"): Open / Follow up / Done, by project and kind, with
  find; it takes over the Done tickets board.
- **Desk cards for tasks and chores** ("Good"): needs Tasks (roadmap M4) first; asked in the
  thread whether to build Tasks now or as its own step.

## Round 15 — 26 Oct 2026 (a timesheet code for tickets)
Card `r15-codes` (`tools/signoff/rounds/r15-codes.json`, mockups `mocks/r16-codes.html`); the
owner asked "why can't I have a default code for tickets?" and ticked all three. They asked
that long codes still fit.
- **A code for tickets:** Customize → Timesheet's "Categories → codes" becomes **Default
  codes**, with a **Tickets** line on top. A day's Tickets entry gets it when nothing else
  gives it a code: entry → project → category → **tickets code** → Unassigned. Tickets with a
  project keep the project's code. It applies to past weeks too.
- **A code on each entry:** entry details show a **Timesheet code** picker. Its first choice
  is "Use the usual code (… , from the project / category / your Tickets code)"; picking
  another sets it for that entry only (the Tickets entry too).
- **Pick a code on the Timesheet:** an Unassigned line has **Pick a code**. On a line with a
  Tickets entry, "Use it for tickets from now on" (on by default) sets the tickets code;
  otherwise, or unticked, it sets the code on that line's entries.
- **Long codes:** pickers never grow past their box; a long chosen code is written out in full
  under the picker, and Pick a code's choices wrap inside their buttons.

## Round 15 — 25 Oct 2026 (one item per ticket)
- **One item per ticket, task or chore** (asked in chat: "I like all your recommendations";
  write-up "Tickets, plans and projects: one clearer model"): a new **item** record that
  tray tickets, entry tickets and later tasks and chores point at, with one done tick, one
  notepad and one follow-up date. Projects stay as they are. Built in four steps; anything
  visible waits for its mockup on the sign-off sheet (card `r15-things`), which the owner
  asked for before any UI change.
- **Step 1 (built, nothing visible moves):** one item per ticket number (ignoring case).
  Ticking a ticket done finishes it everywhere it appears; unticking reopens it from that
  day on; its notepad is the same everywhere; pasting it again finds it (open again, with its
  notepad and the project it was last given). Tickets from before items join theirs on
  unlock: the newest row decides done and project, and different notepads are merged under
  "Earlier note" lines so none is lost. A ticket number that comes back later is the same
  ticket, reopened, with its old notes (recommended pick on the card).
## Round 15 — 25 Oct 2026 (home use and repeating things)
- **What's Shiftbook for?** (r15-home): a real goal, not a side mode. First run asks **Work,
  Home or Both** after the recovery key; Settings → Look can change it any time. Home turns the
  hours targets off (0 every day), locks after 60 minutes instead of 15, hides the Timesheet
  (keys become 1 to 6) and the week-pack day options, shows home starter categories (Home,
  Money, Health, Family, Car, Admin, Errands) and repeating kinds (Bills, Home upkeep, Car,
  Health, Documents, Birthdays), and swaps words (Tickets → References, Meetings → Regular
  events, MOP → Step-by-step guide, Log work → "Done something?"). Values the user added are
  never touched. Both keeps the work words and adds the home pieces. Rules: `core::home`.
- **Done without time** (r15-home): in Home and Both, Log work has a **✓ Done** button that saves
  the line with no minutes; such entries show a tick instead of 0h. With no hours target the
  Desk and Day count "things done" instead of the hours ring.
- **Lists** (r15-home, built with Tasks/M4): reusable checklists (shopping, packing, before we go
  away) on their own screen (key 6, or 5 at home). Paste several lines to add them all; **Reset
  for next time** unticks everything and keeps the items. Stored as tasks with `is_list` and their
  checklist items (migration 0016).
- **Repeating things** (r15-recurring): the M3 recurring screen is built now (key 5, or 4 at
  home): things that repeat (pick how often and when it's next due) or expire on a date (with how
  often they renew). **Mark done** adds a no-time Done entry on the day, with Undo; things that
  expire ask for the new expiry date. Overdue and due-soon things show on the Desk (**Due soon**)
  and in the bell with Done. **Yearly** (already a starter frequency) covers birthdays and renewals.
- **While you were away** (r15-recurring): on unlock (and when the day changes) the bell opens on
  what came due, or what you said to chase, since Shiftbook was last open. No new plugin and no
  system notifications. Last-open day is kept in prefs (`app.last_open`).
- **Keys:** 1 Desk, 2 Day, 3 Timesheet, 4 Notes, 5 Repeating, 6 Lists, 7 Settings (at home the
  Timesheet is hidden and the rest move up one).

## Round 15 — Android — 25 Oct 2026
Proposal: [Shiftbook in your pocket](https://claude.ai/artifact/CufAB8vtd1KpGsRzwTH967);
cards `r15-android` and `r15-android-ideas` (`tools/signoff/rounds/r15-android.json`). The
owner approved every recommendation in chat and asked for mockups before any UI is built, so
the screens are on the card and still need sign-off.
- **Scope:** the phone starts as a companion (log work, capture, today, the bell, reading
  notes), not the full Desk canvas. Same Tauri 2 app, core and storage; no separate codebase.
- **Phone and computer data:** an encrypted sync file each side makes and merges, moved by the
  user any way they like. Shiftbook still never goes online. Merges by id and `updated_at`,
  newer edit wins, safety backup first.
- **Fingerprint unlock:** offered, off by default. Built on Android's own BiometricPrompt
  (Android 11 or newer) rather than `tauri-plugin-biometric`, so no new package; see SECURITY.md.
- **Due reminders:** local notifications that count what is due and never show titles. Built
  with a little Kotlin of our own rather than `tauri-plugin-notification`.
- **Install:** a signed APK from GitHub releases; no Play Store or F-Droid for now.
- **First step:** a test build that sets up, locks and unlocks on an Android emulator, to prove
  SQLCipher builds for Android. If it can't, stop and ask (SECURITY.md).
- **Ideas A–H all approved** (sign-off, 25 Oct): "do all these and push it, I'll test it". As
  built: swipe right on an entry for +15 min, left for Timer and Delete (Undo on each);
  hour and recent-project buttons in capture; Share → Shiftbook with a choice of the tray, one
  entry or a scrap; long-press icon shortcuts (Log work, Start the timer, Jot a note); the timer
  asks about time in the background from 5 minutes on (`core::timer::away_question`); a timer
  tile and a quiet "Timer running" notification while Shiftbook is in the background; a
  wrap-up nudge; "Where was I" as on the computer.
- **Phone security defaults:** lock after leaving the app, default 1 minute (straight away, 1,
  5 or 15); screenshots blocked (`FLAG_SECURE`) with the same Protected pill as the computer;
  Android cloud backup and device transfer off; no INTERNET permission in release builds.
  Password hashing on a phone stops at 256 MiB of memory and adds passes instead.
- **Nudges** (due reminder, wrap-up): off by default, work days only, counts and times never
  titles (`core::nudge`). The app plans the next one each time it's opened; Android shows it
  then. Turning one on is when Android asks for permission to notify.
- **Sync file** (Settings → Phone and computer, on both): "Make a sync file" saves a
  `.shiftsync` (the backup format and encryption, everything in it); "Open a sync file" shows
  what it would bring in, then merges after a safety backup. Rows match by id, the newer edit
  wins, deletes travel (and can be brought back), settings stay on each device. Edits made on
  both since the last swap are listed by name. Only copies that started from the same data can
  swap: set the phone up by restoring a computer backup. A file from another key asks for that
  device's password or recovery key, as a backup does. Something that clashes with a different
  record here on a "one per day" rule (both used the same day's Tickets tray) is left out and
  counted.
- **No new packages for the phone so far:** notifications, the tile, shortcuts, sharing and the
  file picker are a little Kotlin of our own (`gen/android/.../PhonePlugin.kt` and friends)
  instead of the notification plugin.

## Round 15, quality of life — 25 Oct 2026
Built as pictured on the `r15-qol-*` mockups; the owner will try them and say which help most.
- **Paste on the Desk:** text that is only ticket numbers goes to the Tickets tray and asks
  "Time on them so far?" where the pointer is; anything else becomes a jot there (Undo).
- **Find does everything; `/` opens it:** commands grouped Go to / Do / Settings (boards by name,
  wrap up the day, back up now, copy for stand-up, fit, tidy, timeline, timer, each Settings
  page), days like "fri", "last friday" or "12 oct", and "help" lists everything.
- **Bring up…** offers a few work days, next week and Pick a day…, not only tomorrow.
- **Copy for stand-up:** your last day with work and what you start with today, as plain lines;
  the two headings are yours to name (Settings → Your day).
- **Ctrl+Z undo with a history:** the last 20 changes with an Undo button; typing "undo" in Find
  lists them, and undoing an older one undoes the ones after it. Redo where the change knows how.
- **Your own time buttons:** up to five, used in Log work, on jots and for tickets.
- **Log a meeting from the bell:** a series says how long it usually lasts and its project; when it
  finishes the bell offers Log / Other time… / Skip, linking the entry to the meeting note.
- **Just today** (bottom bar, key T) fades cards that aren't from today, still open, brought up
  for today or pinned.
- **Timesheet reminder:** from midday on your last work day the bell says the week isn't entered
  yet, until every day is ticked off; "Remind me at…" hides it for two hours.
- **Quick unlock with a PIN:** as written in SECURITY.md. Because the PIN is never stored, after
  the day's first password unlock a small box asks for today's PIN (Skip is fine). The owner was
  asked on a card whether to keep that box or set it only from Settings.
- **The 11 newer ideas** (`r15-qol-new`) wait until the owner has tried these.
- **Drag with the right button too** (card `r15-rightdrag`, approved 26 Oct): Settings → Look →
  Mouse, off by default. When on, a right press that moves drags cards, piles and areas, slides
  the Desk, or moves a window or drawer; one that doesn't move still opens the menu, on every
  system (the menu is held until release where the browser sends it on press). Selecting and
  Box stay on the left button. Pref `ui.right_drag`; menu timing in `web/src/lib/rightDrag.ts`.

## Round 14 — 20 Oct 2026
- **Done tickets** (asked in chat after trying 0.4.0): tickets ticked done today gather in a
  **Done today** pile on the Desk; at the end of the day they leave the Desk. Still-open tickets
  stay loose. Old ones are found on the **Done tickets** board (newest first, with a filter)
  and in **Find**, which now searches tickets by number, notepad and project.
- **Pins** (asked in chat, no mockup needed): Pin to the bottom bar on any entry, ticket or
  note's right-click menu. Chips sit on the bottom bar (12 at most); click one to open it in a
  window, right-click to set a day to chase it (Today, Tomorrow, In 3 days, Next week) or
  unpin. From the chase day the chip turns orange and the bell lists it with Open and
  Tomorrow.
- **Tickets start still open** (round 14): a pasted ticket starts Still open and is ticked
  when done (the round tick stays). Adding tickets asks **"Time on it so far?"** (5m to 2h,
  15m if skipped). The Tickets and Log work boxes stay separate; Log work's example no longer
  looks like a ticket.
- **Same work, one card** (round 14): on the Desk, entries with the same title and project
  (e.g. a copy made by adding time today) show as one card: the newest day's entry, with the
  total time and the days ("4 h · 2 days"). A Tickets-tray ticket with a number the work has
  folds into its tickets strip. In the tickets box each number shows once; ticking it ticks
  it everywhere it appears. Day and the timesheet still keep each day's entry.
- **Tickets on entries** (round 14): a ticket added to an entry (Log work, Attach) has the
  same round tick (starts still open) and its own notepad as Tickets-tray tickets; its time
  is the entry's. On the Desk they fold into a strip on the entry's card ("3 tickets · 1
  open"); clicking it opens a box over the Desk with the entry and a card per ticket. Open
  ones count in the bell.
- **Jots into entries** (round 14): a jot's menu (Desk card, Day's Notes card, Notes) has
  **Log it as an entry** with 15m to 2h. Its title becomes what was done, the rest the
  entry's notes, ticket numbers its tickets, and it keeps its project; it lands on today (or
  the day open on Day) and the jot moves to the archive. Undo puts both back.
- **Adding time to an older entry** (round 14): +15m/+30m/+1h lands on the day being worked
  (today on the Desk, the open day on Day): on that day's entry with the same title and
  project, or a copy of the entry there. The older day is never changed by it.
- **Scroll wheel:** on the Desk it zooms around the pointer (was Ctrl + scroll; round 4).
  Shift + scroll or dragging empty paper pans. Over a right-click menu, a drawer's list or a
  slide-out, the wheel scrolls that instead.

## Round 13
- **Desk steps 3 and 4** (boxes, the toolbox and folding boxes up; piles, drawers and Tidy
  into stacks): shown as built, with nothing new to decide. Changes asked for after trying
  them are in round 14.

## Round 12 — 19 Oct 2026
- **Click and right-click:** one click on an entry or ticket (Desk card, Day row, tray ticket)
  slides out a panel underneath with the everyday things: notepad, tickets, project, Bring up
  tomorrow, done / still open and more time. Start the timer shows there only when the timer is
  turned on. Right-click (or More) keeps the full menu, Delete included, on Day and the Desk.
- **Tray tickets with a project:** their minutes count under that project (a "Tickets" entry per
  project per day) instead of the day's plain "Tickets" entry.
- **Windows, bottom bar and bell:** as pictured. The bell shows meeting reminders (a day and time
  per meeting series), recurring tasks due, things brought up for today and tickets still open.
  In-app only for now; no Windows notifications.
- **Full screen** for projects, meeting series, Daily notes and any note or entry: Notebook,
  Board (the sub-canvas) and List; notes can hold pictures and **URL links**.
- **Drawing boxes:** Shift + drag on empty paper, then pick a project, make one or keep it a
  plain label. Also asked for: a collapsible **toolbox** (ways to move around, multi-select, a
  Project tool to move, create and edit projects), moving a project's box moves everything in
  it, and a **collapse** button on project boxes that folds the cards into a count whose colour
  shows their state (all done grey, something coming up orange, and so on).
- **Build order:** as proposed (slide-out and round 11 picks; windows, bar and bell; drawing
  boxes; stacks and drawers; boards, full screen and Archive; pictures, then the meeting
  canvas), all worked in one living pull request.

## Rounds 10 and 11 — 19 Oct 2026
- **Desk step 2** (project areas, links, timeline): approved as built; keep going.
- **A notepad under each entry and ticket**, on the Desk cards and on Day, saved as you type.
- **Bring up tomorrow** in every right-click menu; tomorrow "Where was I?" shows it first.
- **Tickets that aren't done:** the tray becomes **Tickets** with a done tick on each. Unticked
  is **Still open**: its time still counts today and it comes back in tomorrow's tray.
- **Asked for more** (pictured in round 12): a click opens a slide-out under the card or row with
  the everyday options and right-click keeps the full menu (archive, delete, …); the notepad pops
  out into a movable window; a thin bottom bar holds minimised windows on the left and a
  notification bell on the right (to-dos, meeting reminders) whose panel slides in from the right;
  tickets can be attached to projects; a project's full screen holds several notes and pictures;
  boxes on the Desk can be drawn.

## Round 9 — 19 Oct 2026
- **Desk notes and ticket notes:** built as shown (Notes card under Done tickets with Jot, Today
  and Meetings; a few words beside each tray ticket).
- **The Canvas becomes Desk:** the owner works there most of the day, so the Canvas is the home
  view and is called Desk. The structured day view is for starting and ending the day and gets
  a new name (asked on r9-full). The Canvas keeps every capability decided so far; the full
  list and build order are in `docs/CANVAS.md`. It shows the same records as the day view; it
  only adds layout.
- **Releases:** the app's version comes from the release tag, and a release builds once.

## Round 8 — 19 Oct 2026
- **Canvas tidying:** stacks (drop a card on a card; click to fan out) and drawers (one per
  project plus the user's own folders), with a **Tidy into stacks** button. Filed things stay in
  Find and still count on the timesheet.
- **Boards:** any card can be a board with its own canvas, tinted in its colour, with
  breadcrumbs. Shiftbook makes Meetings (a board per meeting series), Daily notes and Projects;
  the user makes any others. The List view shows the same tree. An Archive sits at the bottom of
  the tree; daily notes go there by themselves after 2 weeks; archived things stay in Find.
  This replaces two Round 7 Notes choices: boards now organise notes alongside projects and
  tags, and Daily notes auto-archive after 2 weeks (Jot and Meeting keep the 3-month default).
- **Ticket sign-offs:** rules and per-ticket matching were rejected: which tickets need a
  closing message is random, and Shiftbook only knows the ticket number. Built instead: one copy
  button on the Done tickets tray opens a quick menu of saved responses; picking one copies it.
  No gaps, no matching. Responses are written in Settings → Responses (at most 30).
- **Round 6** (prototype v2, timesheet crib sheet, spreadsheet lessons): approved and built in
  the alpha; codes are typed into Shiftbook by the user.

## Round 7, Notes cards — 16 Oct 2026
- **Notes and archive:** kinds Jot, Daily, Meeting, Page and MOP, organised by project and tags
  (no notebooks). Archived notes leave the lists but search still finds them. Auto-archive is a
  setting, with a soft warning on the note before it moves ("moves to the archive in N days",
  with Keep). Notes search extends the Find box (r7-find), not a second search.
- **MOPs:** fixed sections (why, checks, steps with expected results, rollback) and a Run mode
  that saves a run record. Import and export must be easy and cope with varied documents without
  an AI: rule-based import (numbered lines are steps, "Verify"/"Expected" lines attach to the
  step above, headings sort lines into checks and rollback) with a check screen where each line
  shows where it landed and can be moved; fixes are remembered per template. Export to Word,
  PDF, text and a round-trip Shiftbook MOP file. Approved on the r7-import card.
- **OneNote:** both ways (paste a page, then import sections exported as Word files);
  `quick-xml` approved for reading Word files. Import should handle whatever the notebook holds.
- **Meetings (approved on r7-meeting v5):** one big canvas. Each topic is a coloured area on one
  sheet; topic tabs along the top glide the view to their area, and See all pulls back to the
  whole meeting. Click empty paper to write there, drag blocks, a recent-topics strip, right-click
  a line to make a todo, task, decision or question, a count at the top. Kept throughout: `#name`
  to jump to or start a topic, `[]` `!` `?` marks, an optional agenda written beforehand, open
  items carry to the next meeting in a series, one-click time log on wrap up. The canvas must be
  easy to tidy (a Tidy up that lines blocks up, blocks that snap and don't overlap).
  Earlier rejected tries: topic rail plus results panel (v1); running log, outline and topic
  cards (v2); OneNote-style free page, trail, agenda zones and zoom board (v3).
- **Build order (owner approved, 17 Oct):** notes core, then the meeting canvas, then MOPs, then
  imports. Notes core built: a Notes view (key 3; Settings moves to 4), auto-archive after 3
  months by default (Settings → Notes; 1, 2, 3, 6 or 12 months, or off) for Jot, Daily and
  Meeting notes only, warning on the note for the last 7 days; editing or Keep stops it.

## Round 7 — 16 Oct 2026
- **Week start:** a "My week starts on" setting (any day, Monday by default); approved and merged.
- **"Where was I?":** approved. After a lock, a break of 30 minutes or more, or a new day, the Desk
  shows the wrap-up's "next up" list (first item, then a checklist) or else the last task, its
  tickets and its next step, with Carry on / Something else / Later. Built.
- **"Now doing" timer:** approved as an optional feature, off by default (the owner wouldn't use
  it). Rounds to 15 minutes, saves minutes only, suggests leaving out time away from the keyboard.
  Built: Settings → Your day → Timer; ▶ Start on today's entries; quiet stretches of 5 minutes
  or more count as away; locking keeps it as a paused timer (entry and seconds only).
- **Backups:** approved and merged: a daily automatic encrypted backup after unlock (last 14
  kept), Back up now, and Restore with a preview and a safety copy first. **Delete all data keeps
  the backups**; they still need the password or recovery key they were made with.
- **Log work:** Enter logs the line when it has a time in it (`INC0045678 50m`), otherwise moves to the
  time buttons. A "Same as yesterday" row offers yesterday's routine entries as one-click buttons.
- **Capture finds things:** typing searches entries, days and Inbox scraps, and runs commands (Lock,
  Book holiday, go to a view). A line with hours still logs.
- **Views:** Planner and Canvas are hidden until they're built, so 1 to 3 match the tabs. The settings tab
  stays, renamed **Settings** (owner's pick on the "Done: round 7" card).
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
- **Round 7 build:** approved on the "Done: round 7" card and merged.

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
