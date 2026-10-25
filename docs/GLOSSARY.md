# Glossary

The words Shiftbook uses, what each one means, and where it lives in the code. Several of these
had other names in earlier rounds; the old names are listed so `DECISIONS.md` still reads.

| Word | What it is | Earlier names | In the code |
|---|---|---|---|
| **Entry** | Some work on one day with its time in minutes, a title, an optional project and tickets. | Time entry | `time_entries`, `TimeEntry` |
| **Work** | The same title and project across several days. The Desk shows it as one card ("4 h · 2 days"); Day and the Timesheet keep each day's entry. | Same work | `canvas::work_groups` |
| **Ticket** | A ticket number pasted onto an entry. Starts **still open**; tick it when it's done. Has its own notepad. | Ticket chip | `tickets`, `Ticket` |
| **Tray ticket** | A ticket pasted into the Tickets tray, with its own minutes. Still-open ones come back each day. | Done ticket | `ticket_tray`, `TrayItem` |
| **Tickets tray** | Where you paste tickets as you work on them. The day's tray tickets roll up into one "Tickets" entry (one per project). | Done tickets tray | `tray` |
| **Roll-up entry** | The entry Shiftbook keeps from a day's tray tickets. Its time follows the tray. | Tray entry | `time_entries.source = 'tray'` |
| **Tickets box** | A work card's tickets, each number once across its entries and folded-in tray tickets; ticking one ticks every copy. | Tickets strip | `web/src/lib/workTickets.ts` |
| **Jot** | A quick note. Can be turned into an entry with **Log it as an entry**. | | `notes`, kind `jot` |
| **Daily, Meeting, Page, MOP** | The other note kinds. A MOP is a step-by-step procedure (method of procedure). | | `notes` |
| **Inbox scrap** | A line saved from capture without a time, to sort later. | | `inbox_items` |
| **Desk** | The home view: your records as paper cards you arrange. | Canvas | `views/Canvas.svelte`, `canvas` |
| **Day** | The view for starting and ending the day: hours ring, Log work, the tray, entries, notes. | Today, Desk (before round 9) | `views/Day.svelte` |
| **Project area** | A tinted region on the Desk; cards dropped in get its project. | | `canvas_areas` |
| **Box** | A box drawn on the Desk: a project's area, a new project or a plain label. Any box folds up to a coloured count. | Label | `canvas_labels`, `canvas::box_status` |
| **Pile** | Cards dropped on each other; click to fan out. **Done today** is the pile Shiftbook keeps. | Stack | `canvas_stacks`, `piles` |
| **Drawer** | Where a card is filed off the Desk: one per project plus your folders. Filed cards still count and show in Find. | Cabinet | `piles` |
| **Board** | A full screen you step into (a project, a meeting series, Daily notes, a folder, the Archive) as a Notebook, a Board or a List. | Sub-canvas | `boards` |
| **Link** | A line between cards that share a ticket or an uncommon word, or one you drew. | | `canvas_links`, `canvas::auto_links` |
| **Pin** | An entry, ticket or note kept as a chip on the bottom bar, optionally with a day to chase it. | | `pins` |
| **Bring up** | Ask for something to come up first in Where was I? on the next work day. | | `next_up_items`, `nextup` |
| **Next up** | The short "Tomorrow I start with…" list from the wrap-up. | | `next_up_items` |
| **Where was I?** | The card that offers a way back in after unlocking, a break or a new day. | Resume | `resume` |
| **Wrap-up** | The optional end-of-day card. | | `WrapUp.svelte` |
| **Bell** | The panel of meetings about to start, things brought up and tickets still open. | | `bell` |
| **Window** | A notepad or note popped out to float over Shiftbook. | | `windows` |
| **Capture / Find** | Ctrl+Space: one line that logs, saves to the Inbox, or searches. | | `capture`, `search` |
| **Crib sheet** | The Timesheet view: per day, the code, hours and a ready-made comment. | Timesheet grid | `sheet` |
| **Timesheet code** | Your own code for a timesheet system. An entry gets one from itself, its project, or its category. | | `timesheet` |
| **Ribbon** | The last four weeks as bars of coloured hours along the top. | | `ribbon` |
| **Vault** | The file holding the database key, locked by your password and the recovery key. | | `vault` |
| **Backup** | An encrypted `.shiftbak` file. Exports (not built yet) are the only readable files. | | `backup` |
