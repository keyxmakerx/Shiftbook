# The Canvas ("Desk")

The Canvas is where the owner works most of the day: a free board of paper cards they arrange,
pile, file away and step into. It is the home view and is called **Desk**. The structured day
view (ring, Log work, entries, Done tickets, Notes) is used to start and end the day; it gets
another name (see DECISIONS.md, round 9).

This page collects every Canvas decision from the sign-off rounds so none is lost. Each item
names the round it came from. Nothing here is optional unless it says so.

## Principle: the same data, laid out freely (round 9)
- Cards are the app's real records: time entries, tray tickets, notes of every kind, Inbox
  scraps and, later, tasks. Changing a card changes the record; the day view, Timesheet, Notes
  and Find all see it.
- The only data the Canvas adds is layout: where each card sits, which board it is on, which
  stack it is in, and which links the owner drew or removed.
- Business rules stay in `crates/core` (layout rules, timeline order, auto-links, tidy); the
  layout lives in storage like any other record (id, owner_id, timestamps, soft delete).

## Working on the Canvas
- Everything the day view can do works here (round 9): Log work, Ctrl+Space capture and the
  Done tickets tray add cards; double-click a card to open its details; right-click any card,
  area or empty paper for the paper menu (round 5, round 6) with the fold-out animation.
- Double-click empty paper to start a jot right there.
- Pan by dragging empty paper or scrolling; zoom with Ctrl + scroll, the zoom buttons or
  "Fit everything" (round 4 pick).
- Today's hours stay in view (the top bar), so the Canvas never hides the day's total.
- Find flies to a card and lifts it.
- Undo for every move that changes data (project, filing, archive, delete).

## Shape and meaning
- **Project areas** (round 5): tinted regions, one per project. Dragging a card into an area
  gives it that project; dragging it out takes the project away (with Undo).
- **Links** (round 2, round 4): lines drawn automatically between cards that share a ticket, a
  project or distinctive words; any link can be removed, and the owner can draw their own.
- **Timeline** (round 4): one button lays everything out along a date line; "Back to my layout"
  puts every card exactly where it was. Switching never disturbs the owner's arrangement.

## Keeping it tidy (round 8)
- **Stacks:** drop a card on a card to make a pile with a count; click to fan it out; drag a
  card out to take it off; double-click the name to rename it. **Tidy into stacks** piles loose
  cards by project.
- **Drawers:** a cabinet down the side, one drawer per project plus the owner's own folders.
  Drop a card or a pile on a drawer and it leaves the desk. Filed things stay in Find and still
  count on the timesheet.
- **Boards inside boards:** any card can become a board with its own canvas. Inside, the paper
  takes the board's tint and breadcrumbs show where you are (Desk › Meetings › Weekly ops).
  Shiftbook makes Meetings (a board per meeting series), Daily notes and Projects; the owner
  makes any others. A List view shows the same board as a list.
- **Archive:** at the bottom of the tree. Archive by hand; Daily notes go there by themselves
  after 2 weeks. Archived things stay in Find; Bring back returns them where they were.

## Look and motion (round 4, round 5)
- Coloured paper with a light grain and soft edges; real colour per project; no tape, torn
  edges or tilted cards. A dashed coral outline marks a scrap still in the Inbox.
- Purposeful motion: cards lift when picked up, sheets fold out, boards zoom in from the card
  you opened. Reduced motion is respected.

## Later, on top of the Canvas
- The meeting canvas (round 7): topics as coloured areas on one sheet, topic tabs that glide,
  See all, open items carried to the next meeting.
- Pictures in notes with the markup editor (round 4).

## Build order
1. **C1 Foundation (built):** the layout store; Desk shows entries, tray tickets, notes and
   scraps as cards; drag, pan and zoom, Fit everything; double-click to open or to jot; paper
   menus; a side panel with today's hours, "Where was I?", Log work, Done tickets and the
   wrap-up; the tab rename (Desk, Day, Timesheet, Notes, Settings on keys 1 to 5).
   Which records show: entries and Done tickets from the last 7 days, plus older ones the owner
   moved; every live note and Inbox scrap. The day's "Tickets" roll-up entry is not a card (its
   tickets are). Cards nobody moved are placed newest first in rows from the top left, around
   the moved ones.
2. **C2 Meaning (built):** project areas (right-click empty paper → Project area here; drag
   the name to move the area with its cards, the corner to resize; drop a card in to give it the
   project, drag it out of its own project's area to take the project away, both with Undo);
   links (automatic for a shared ticket, also one written in a note, and for an uncommon word
   only two cards use, at most two word links per card; dashed for words, solid for tickets;
   click a line to remove it; "Link to another card" on any card draws one; the Links button
   hides them all); the timeline (a column per day, oldest left, wrapping after 5 cards; Back
   to my layout restores the view; cards can't be dragged while it shows).
   **Boxes and the toolbox (built, round 12):** a toolbox down the left of the Desk (fold it
   away with its arrow) with Move (drag paper to pan), Select (drag around cards; dragging one
   moves them all; Box these puts a box around them) and Box (drag to draw a box). Shift + drag
   draws a box with any tool. A new box asks what it is: a project (the cards inside get it,
   with one Undo; a project that already has a box moves it here), a new project, or Just a
   label (a plain dashed box that only groups cards; double-click its name to rename it).
   Every box folds up (▴ on its name) to its name and a count; the count is grey when nothing
   is waiting, blue when something is from today and orange when a ticket is still open or
   something is brought up. Folded cards and their links hide until it opens again.
3. **C3 Tidy:** stacks, Tidy into stacks, drawers.
4. **C4 Boards:** boards inside boards with tint and breadcrumbs, the List view, the Archive.
5. **C5:** the meeting canvas.
