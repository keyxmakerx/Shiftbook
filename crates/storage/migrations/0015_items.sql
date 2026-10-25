-- Items (round 15, step 1): one record per ticket number, however many tray days and
-- entries it's on. The item holds the done tick, the one notepad and the project it was
-- last given; tray rows and entry tickets point at it and keep a copy of its tick and
-- notepad. Rows from before this migration are linked on the next unlock (in Rust, so the
-- new ids are UUID v7 like every other record). Same conventions as 0001.
CREATE TABLE items (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    kind       TEXT NOT NULL CHECK (kind IN ('ticket', 'task', 'chore')),
    -- The ticket number as first written, e.g. INC0045678; NULL for tasks and chores.
    number     TEXT,
    title      TEXT NOT NULL DEFAULT '',
    done       BOOLEAN NOT NULL DEFAULT FALSE,
    note       TEXT,
    project_id TEXT REFERENCES projects (id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE UNIQUE INDEX ux_items_number ON items (owner_id, lower(number))
    WHERE number IS NOT NULL AND deleted_at IS NULL;

ALTER TABLE tickets ADD COLUMN item_id TEXT REFERENCES items (id);
ALTER TABLE ticket_tray ADD COLUMN item_id TEXT REFERENCES items (id);
CREATE INDEX ix_tickets_item ON tickets (item_id);
CREATE INDEX ix_ticket_tray_item ON ticket_tray (item_id);
