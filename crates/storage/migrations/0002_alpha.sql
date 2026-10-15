-- Alpha: project colours and archive, the Done tickets tray, custom fields,
-- crib sheet tick-offs and the Inbox. Same conventions as 0001: runs on SQLite and
-- Postgres, TEXT ids/dates/timestamps, INTEGER minutes, BOOLEAN flags, soft deletes,
-- no AUTOINCREMENT, no JSON functions.

ALTER TABLE projects ADD COLUMN color TEXT;
ALTER TABLE projects ADD COLUMN archived BOOLEAN NOT NULL DEFAULT FALSE;

-- NULL for entries the user logged; 'tray' for a day's roll-up of the Done tickets
-- tray (one per day). The tray keeps its own rows (ticket_tray) because the
-- tickets table needs an entry, and a tray ticket exists before any entry does.
ALTER TABLE time_entries ADD COLUMN source TEXT;
CREATE UNIQUE INDEX ux_time_entries_tray
    ON time_entries (owner_id, date) WHERE source = 'tray' AND deleted_at IS NULL;

CREATE TABLE ticket_tray (
    id            TEXT PRIMARY KEY NOT NULL,
    owner_id      TEXT NOT NULL REFERENCES users (id),
    date          TEXT NOT NULL,
    ticket_number TEXT NOT NULL,
    minutes       INTEGER NOT NULL DEFAULT 15 CHECK (minutes >= 0 AND minutes <= 480),
    note          TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);
CREATE INDEX ix_ticket_tray_day ON ticket_tray (owner_id, date);
CREATE UNIQUE INDEX ux_ticket_tray_ticket
    ON ticket_tray (owner_id, date, lower(ticket_number)) WHERE deleted_at IS NULL;

CREATE TABLE custom_fields (
    id            TEXT PRIMARY KEY NOT NULL,
    owner_id      TEXT NOT NULL REFERENCES users (id),
    applies_to    TEXT NOT NULL CHECK (applies_to IN ('entry', 'ticket', 'project')),
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('text', 'number', 'choice', 'link')),
    options       TEXT NOT NULL DEFAULT '',
    link_template TEXT,
    ticket_prefix TEXT,
    sort_order    INTEGER NOT NULL DEFAULT 0,
    hidden        BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);
CREATE UNIQUE INDEX ux_custom_fields_name
    ON custom_fields (owner_id, applies_to, lower(name)) WHERE deleted_at IS NULL;

-- record_id points at an entry, ticket or project depending on the field, so it
-- has no foreign key; the storage layer checks the field.
CREATE TABLE custom_values (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    field_id   TEXT NOT NULL REFERENCES custom_fields (id),
    record_id  TEXT NOT NULL,
    value      TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE UNIQUE INDEX ux_custom_values_record
    ON custom_values (owner_id, field_id, record_id) WHERE deleted_at IS NULL;
CREATE INDEX ix_custom_values_owner_record ON custom_values (owner_id, record_id);

CREATE TABLE timesheet_ticks (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    week_start TEXT NOT NULL,
    day        TEXT NOT NULL,
    line_key   TEXT NOT NULL,
    done       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE UNIQUE INDEX ux_timesheet_ticks_line
    ON timesheet_ticks (owner_id, day, line_key) WHERE deleted_at IS NULL;
CREATE INDEX ix_timesheet_ticks_week ON timesheet_ticks (owner_id, week_start);

CREATE TABLE inbox_items (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    text       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_inbox_items_owner ON inbox_items (owner_id, created_at);
