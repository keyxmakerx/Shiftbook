-- Saved responses: messages kept for pasting into tickets, copied from the Done tickets tray.
-- Saving the list soft-deletes the old rows and inserts the new ones in order.
CREATE TABLE saved_responses (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    position   INTEGER NOT NULL,
    name       TEXT NOT NULL,
    text       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_saved_responses_owner ON saved_responses (owner_id, position);
