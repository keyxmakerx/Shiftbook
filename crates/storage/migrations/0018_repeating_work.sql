-- Repeating things hold work (round 15, card r15-repeating). A tray ticket can count under a
-- repeating thing as it can under a project, and its item remembers it for the next day. The
-- day's roll-up for a repeating thing is an ordinary tray entry logged against it. A kind of
-- repeating thing ("Certificates") can have a timesheet code, like a category. Same
-- conventions as 0001.
ALTER TABLE ticket_tray ADD COLUMN recurring_id TEXT REFERENCES recurring_tasks (id);
ALTER TABLE items ADD COLUMN recurring_id TEXT REFERENCES recurring_tasks (id);

-- The new expiry date a Done entry renewed a thing to (Renewed…), for its past renewals.
ALTER TABLE time_entries ADD COLUMN renewed_to TEXT;

CREATE TABLE kind_code_defaults (
    id                TEXT PRIMARY KEY NOT NULL,
    owner_id          TEXT NOT NULL REFERENCES users (id),
    kind              TEXT NOT NULL,
    timesheet_code_id TEXT NOT NULL REFERENCES timesheet_codes (id),
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    deleted_at        TEXT
);
CREATE UNIQUE INDEX ux_kind_code_defaults_kind
    ON kind_code_defaults (owner_id, lower(kind)) WHERE deleted_at IS NULL;

-- One roll-up per day and home: a project's (tray_project_id), a repeating thing's (the
-- thing it is logged against) or the plain one.
DROP INDEX ux_time_entries_tray;
CREATE UNIQUE INDEX ux_time_entries_tray
    ON time_entries (
        owner_id,
        date,
        COALESCE(tray_project_id, ''),
        (CASE WHEN tray_project_id IS NULL AND target_type = 'recurring' THEN target_id ELSE '' END)
    )
    WHERE source = 'tray' AND deleted_at IS NULL;
