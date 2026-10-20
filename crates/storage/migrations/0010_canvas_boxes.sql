-- Canvas step 3 (round 12): boxes fold up to a count, and plain labelled boxes that only
-- group cards (no project). Same conventions as 0001: runs on SQLite and Postgres, TEXT ids,
-- soft deletes, no AUTOINCREMENT.
ALTER TABLE canvas_areas ADD COLUMN collapsed BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE canvas_labels (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    label      TEXT NOT NULL,
    x          INTEGER NOT NULL,
    y          INTEGER NOT NULL,
    w          INTEGER NOT NULL,
    h          INTEGER NOT NULL,
    collapsed  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_canvas_labels_owner ON canvas_labels (owner_id);
