-- Notes (r7-notes card): Jot, Daily, Meeting, Page and MOP, organised by project and
-- tags. Tags are stored as one comma-separated text (see shiftbook_core::notes).
-- archived_at set = in the archive (left out of lists, still found by Find).
-- keep = never auto-archived.
CREATE TABLE notes (
    id          TEXT PRIMARY KEY NOT NULL,
    owner_id    TEXT NOT NULL REFERENCES users (id),
    kind        TEXT NOT NULL
        CHECK (kind IN ('jot', 'daily', 'meeting', 'page', 'mop')),
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    project_id  TEXT REFERENCES projects (id),
    tags        TEXT NOT NULL DEFAULT '',
    keep        BOOLEAN NOT NULL DEFAULT FALSE,
    archived_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
);
CREATE INDEX ix_notes_owner ON notes (owner_id, updated_at);
