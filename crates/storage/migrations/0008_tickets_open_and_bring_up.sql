-- Step 1 after the Desk's step 2 (rounds 11 and 12): tickets that aren't done yet,
-- tickets with a project, and "Bring up tomorrow". Same conventions as 0001: runs on
-- SQLite and Postgres, no AUTOINCREMENT, no JSON functions.

-- A tray ticket can be still open (worked on, not finished). Existing rows were all
-- pasted as done.
ALTER TABLE ticket_tray ADD COLUMN done BOOLEAN NOT NULL DEFAULT TRUE;
-- The project a tray ticket's minutes count under (NULL = the day's "Tickets" entry).
ALTER TABLE ticket_tray ADD COLUMN project_id TEXT REFERENCES projects (id);
CREATE INDEX ix_ticket_tray_open ON ticket_tray (owner_id, done, date);

-- Tray roll-ups are now one per project per day: the project whose tray tickets the
-- entry rolls up (NULL = the plain "Tickets" entry, whose own project stays editable).
ALTER TABLE time_entries ADD COLUMN tray_project_id TEXT REFERENCES projects (id);
DROP INDEX ux_time_entries_tray;
CREATE UNIQUE INDEX ux_time_entries_tray
    ON time_entries (owner_id, date, COALESCE(tray_project_id, ''))
    WHERE source = 'tray' AND deleted_at IS NULL;

-- Things brought up from a card: the day they come up and the card they came from.
-- NULL show_from = the wrap-up's list.
ALTER TABLE next_up_items ADD COLUMN show_from TEXT;
ALTER TABLE next_up_items ADD COLUMN about_kind TEXT;
ALTER TABLE next_up_items ADD COLUMN about_id TEXT;
