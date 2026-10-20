-- Canvas: stacks and drawers (round 8). A card can sit in a pile (stack_id) or be filed in
-- a drawer (drawer: 'project:<id>' or 'folder:<id>') and so be off the Desk. Filing only
-- tidies: the records are untouched and still count everywhere. Same conventions as 0001.
ALTER TABLE canvas_cards ADD COLUMN stack_id TEXT;
ALTER TABLE canvas_cards ADD COLUMN drawer TEXT;

CREATE TABLE canvas_stacks (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    name       TEXT,
    x          INTEGER NOT NULL,
    y          INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_canvas_stacks_owner ON canvas_stacks (owner_id);

-- Folders the owner makes in the cabinet, beside one drawer per project.
CREATE TABLE canvas_folders (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_canvas_folders_owner ON canvas_folders (owner_id);
