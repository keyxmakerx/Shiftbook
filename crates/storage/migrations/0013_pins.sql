-- Pins (round 14): an entry, a ticket or a note kept in view on the bottom bar, with an
-- optional day to chase it. Same conventions as 0001.
CREATE TABLE pins (
    id          TEXT PRIMARY KEY NOT NULL,
    owner_id    TEXT NOT NULL REFERENCES users (id),
    record_kind TEXT NOT NULL,
    record_id   TEXT NOT NULL,
    -- The day to chase it, YYYY-MM-DD, or NULL.
    chase_on    TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
);
CREATE UNIQUE INDEX ux_pins_record ON pins (owner_id, record_kind, record_id)
    WHERE deleted_at IS NULL;
