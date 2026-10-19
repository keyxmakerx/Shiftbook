-- The Canvas (the Desk): where the owner put each card. Cards are the app's own records
-- (time entries, Done tickets, notes, Inbox scraps); this table only holds their spots.
-- A record with no live row here is placed automatically each time it shows.
CREATE TABLE canvas_cards (
    id          TEXT PRIMARY KEY NOT NULL,
    owner_id    TEXT NOT NULL REFERENCES users (id),
    record_kind TEXT NOT NULL,
    record_id   TEXT NOT NULL,
    x           INTEGER NOT NULL,
    y           INTEGER NOT NULL,
    z           INTEGER NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
);
CREATE UNIQUE INDEX ux_canvas_cards_record ON canvas_cards (owner_id, record_kind, record_id)
    WHERE deleted_at IS NULL;
