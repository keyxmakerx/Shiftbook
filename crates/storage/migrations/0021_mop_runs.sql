-- MOPs on their change ticket (round 18, card r18-mop). A MOP is a note of the MOP kind;
-- its row here ties it to the ticket's item, remembers the template it came from, where it
-- stands (draft, ready, running, done, backed_out), its change window (local day and
-- minutes after midnight, like a meeting's start) and when each line was ticked
-- (one "when<TAB>tick key" per line, UTC; see shiftbook_core::change). Same conventions as
-- 0001.
CREATE TABLE mop_runs (
    -- The MOP's note id: one row per MOP.
    id          TEXT PRIMARY KEY NOT NULL REFERENCES notes (id),
    owner_id    TEXT NOT NULL REFERENCES users (id),
    item_id     TEXT NOT NULL REFERENCES items (id),
    template_id TEXT REFERENCES notes (id),
    status      TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'ready', 'running', 'done', 'backed_out')),
    window_on   TEXT,
    window_at   INTEGER,
    tick_times  TEXT NOT NULL DEFAULT '',
    started_at  TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
);
CREATE INDEX ix_mop_runs_item ON mop_runs (owner_id, item_id);
