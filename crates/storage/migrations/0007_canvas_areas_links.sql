-- Canvas step 2: project areas (a tinted region per project; cards dropped inside get that
-- project) and the links the owner drew or hid. Automatic links are worked out each time and
-- aren't stored; only the owner's changes to them are.
CREATE TABLE canvas_areas (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    project_id TEXT NOT NULL REFERENCES projects (id),
    x          INTEGER NOT NULL,
    y          INTEGER NOT NULL,
    w          INTEGER NOT NULL,
    h          INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE UNIQUE INDEX ux_canvas_areas_project ON canvas_areas (owner_id, project_id)
    WHERE deleted_at IS NULL;

-- A pair is stored in a fixed order (see shiftbook_storage::canvas::pair).
CREATE TABLE canvas_links (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    a_kind     TEXT NOT NULL,
    a_id       TEXT NOT NULL,
    b_kind     TEXT NOT NULL,
    b_id       TEXT NOT NULL,
    drawn      INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE UNIQUE INDEX ux_canvas_links_pair ON canvas_links (owner_id, a_kind, a_id, b_kind, b_id)
    WHERE deleted_at IS NULL;
