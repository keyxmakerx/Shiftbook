-- Shiftbook initial schema (SPEC §3). Written to run on SQLite and Postgres:
-- TEXT ids (UUID v7 strings), TEXT dates (YYYY-MM-DD) and timestamps (RFC 3339 UTC),
-- INTEGER minutes, BOOLEAN flags, no AUTOINCREMENT, no JSON functions.
-- Every table has id, created_at, updated_at and deleted_at (soft delete).
-- Every table except users has owner_id; a user is its own owner.
-- Notes tables arrive in a later milestone.

CREATE TABLE users (
    id           TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT
);

CREATE TABLE list_options (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    list_name  TEXT NOT NULL
        CHECK (list_name IN ('category', 'status', 'priority', 'recurring_type')),
    value      TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    color      TEXT,
    hidden     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE UNIQUE INDEX ux_list_options_value
    ON list_options (owner_id, list_name, lower(value)) WHERE deleted_at IS NULL;

CREATE TABLE frequencies (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    label      TEXT NOT NULL,
    months     INTEGER NOT NULL DEFAULT 0 CHECK (months >= 0),
    days       INTEGER NOT NULL DEFAULT 0 CHECK (days >= 0),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE UNIQUE INDEX ux_frequencies_label
    ON frequencies (owner_id, lower(label)) WHERE deleted_at IS NULL;

CREATE TABLE timesheet_codes (
    id          TEXT PRIMARY KEY NOT NULL,
    owner_id    TEXT NOT NULL REFERENCES users (id),
    area        TEXT NOT NULL,
    phase_label TEXT NOT NULL DEFAULT '',
    code        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    hidden      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT
);
CREATE INDEX ix_timesheet_codes_owner ON timesheet_codes (owner_id, sort_order);

CREATE TABLE category_code_defaults (
    id                TEXT PRIMARY KEY NOT NULL,
    owner_id          TEXT NOT NULL REFERENCES users (id),
    category          TEXT NOT NULL,
    timesheet_code_id TEXT NOT NULL REFERENCES timesheet_codes (id),
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    deleted_at        TEXT
);
CREATE UNIQUE INDEX ux_category_code_defaults_category
    ON category_code_defaults (owner_id, lower(category)) WHERE deleted_at IS NULL;

CREATE TABLE projects (
    id                        TEXT PRIMARY KEY NOT NULL,
    owner_id                  TEXT NOT NULL REFERENCES users (id),
    name                      TEXT NOT NULL,
    description               TEXT NOT NULL DEFAULT '',
    requested_by              TEXT,
    due_date                  TEXT,
    status                    TEXT,
    priority                  TEXT,
    estimated_minutes         INTEGER CHECK (estimated_minutes IS NULL OR estimated_minutes >= 0),
    default_timesheet_code_id TEXT REFERENCES timesheet_codes (id),
    created_at                TEXT NOT NULL,
    updated_at                TEXT NOT NULL,
    deleted_at                TEXT
);
CREATE INDEX ix_projects_owner ON projects (owner_id);

CREATE TABLE recurring_tasks (
    id                        TEXT PRIMARY KEY NOT NULL,
    owner_id                  TEXT NOT NULL REFERENCES users (id),
    name                      TEXT NOT NULL,
    type                      TEXT,
    frequency_id              TEXT REFERENCES frequencies (id),
    expiration_date           TEXT,
    warn_days_before          INTEGER CHECK (warn_days_before IS NULL OR warn_days_before >= 0),
    last_done_before_tracker  TEXT,
    alerts_enabled            BOOLEAN NOT NULL DEFAULT TRUE,
    active                    BOOLEAN NOT NULL DEFAULT TRUE,
    notes                     TEXT NOT NULL DEFAULT '',
    default_timesheet_code_id TEXT REFERENCES timesheet_codes (id),
    created_at                TEXT NOT NULL,
    updated_at                TEXT NOT NULL,
    deleted_at                TEXT
);
CREATE INDEX ix_recurring_tasks_owner ON recurring_tasks (owner_id);

-- target_id points at projects or recurring_tasks depending on target_type, so it
-- has no foreign key; the storage layer checks it.
CREATE TABLE time_entries (
    id                 TEXT PRIMARY KEY NOT NULL,
    owner_id           TEXT NOT NULL REFERENCES users (id),
    date               TEXT NOT NULL,
    minutes            INTEGER NOT NULL CHECK (minutes >= 0 AND minutes <= 1440),
    target_type        TEXT NOT NULL DEFAULT 'none'
        CHECK (target_type IN ('project', 'recurring', 'none')),
    target_id          TEXT,
    category           TEXT,
    description        TEXT NOT NULL DEFAULT '',
    status             TEXT,
    priority           TEXT,
    next_step          TEXT,
    timesheet_code_id  TEXT REFERENCES timesheet_codes (id),
    legacy_ticket_text TEXT,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL,
    deleted_at         TEXT,
    CHECK ((target_type = 'none' AND target_id IS NULL)
        OR (target_type <> 'none' AND target_id IS NOT NULL))
);
CREATE INDEX ix_time_entries_owner_date ON time_entries (owner_id, date);
CREATE INDEX ix_time_entries_target ON time_entries (owner_id, target_type, target_id);

CREATE TABLE timesheet_weeks (
    id           TEXT PRIMARY KEY NOT NULL,
    owner_id     TEXT NOT NULL REFERENCES users (id),
    week_start   TEXT NOT NULL,
    submitted_at TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT
);
CREATE UNIQUE INDEX ux_timesheet_weeks_start
    ON timesheet_weeks (owner_id, week_start) WHERE deleted_at IS NULL;

CREATE TABLE tickets (
    id            TEXT PRIMARY KEY NOT NULL,
    owner_id      TEXT NOT NULL REFERENCES users (id),
    time_entry_id TEXT NOT NULL REFERENCES time_entries (id),
    ticket_number TEXT NOT NULL,
    note          TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);
CREATE INDEX ix_tickets_entry ON tickets (time_entry_id);
CREATE INDEX ix_tickets_number ON tickets (owner_id, lower(ticket_number));

CREATE TABLE tasks (
    id                TEXT PRIMARY KEY NOT NULL,
    owner_id          TEXT NOT NULL REFERENCES users (id),
    title             TEXT NOT NULL,
    notes             TEXT NOT NULL DEFAULT '',
    project_id        TEXT REFERENCES projects (id),
    recurring_task_id TEXT REFERENCES recurring_tasks (id),
    status            TEXT,
    priority          TEXT,
    due_date          TEXT,
    completed_at      TEXT,
    sort_order        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    deleted_at        TEXT
);
CREATE INDEX ix_tasks_owner ON tasks (owner_id, sort_order);

CREATE TABLE task_checklist_items (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    task_id    TEXT NOT NULL REFERENCES tasks (id),
    text       TEXT NOT NULL,
    done       BOOLEAN NOT NULL DEFAULT FALSE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_task_checklist_items_task ON task_checklist_items (task_id, sort_order);

CREATE TABLE settings (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    UNIQUE (owner_id, key)
);
