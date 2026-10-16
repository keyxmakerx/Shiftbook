-- The "next up" list: a few things to start with, written at the wrap-up and shown by
-- "Where was I?". Saving a new list soft-deletes the old one.
CREATE TABLE next_up_items (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    position   INTEGER NOT NULL,
    text       TEXT NOT NULL,
    done_at    TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_next_up_items_owner ON next_up_items (owner_id, position);
