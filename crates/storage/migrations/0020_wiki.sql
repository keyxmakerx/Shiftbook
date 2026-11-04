-- The wiki (round 18, card r18-wiki). A wiki page is a note of the Page kind (MOP templates
-- show in the wiki too). wiki_spaces are the spaces pages sit in; wiki_places says which
-- space a page is in, which page it's under and its place among its siblings (a page with
-- no row, or no space, shows under "Not in a space"). note_versions keeps a page's earlier
-- text, at most one every few minutes while typing (rules in shiftbook_core::wiki).
-- Same conventions as 0001.
CREATE TABLE wiki_spaces (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    name       TEXT NOT NULL,
    color      TEXT,
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE wiki_places (
    -- The page's note id: one place per page.
    id         TEXT PRIMARY KEY NOT NULL REFERENCES notes (id),
    owner_id   TEXT NOT NULL REFERENCES users (id),
    space_id   TEXT REFERENCES wiki_spaces (id),
    parent_id  TEXT REFERENCES notes (id),
    position   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_wiki_places_space ON wiki_places (owner_id, space_id);

CREATE TABLE note_versions (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    note_id    TEXT NOT NULL REFERENCES notes (id),
    title      TEXT NOT NULL,
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_note_versions_note ON note_versions (owner_id, note_id, created_at);
