-- Meeting series (round 12): a meeting that repeats on set weekdays at a set time, so the
-- bell can remind about it. Same conventions as 0001: runs on SQLite and Postgres, TEXT ids,
-- INTEGER minutes, soft deletes, no AUTOINCREMENT.
CREATE TABLE meeting_series (
    id             TEXT PRIMARY KEY NOT NULL,
    owner_id       TEXT NOT NULL REFERENCES users (id),
    name           TEXT NOT NULL,
    -- Weekdays it meets, e.g. 'mon,thu'.
    days           TEXT NOT NULL,
    -- Start, in minutes after local midnight.
    at_minute      INTEGER NOT NULL CHECK (at_minute >= 0 AND at_minute < 1440),
    remind_minutes INTEGER NOT NULL DEFAULT 15 CHECK (remind_minutes >= 0 AND remind_minutes <= 120),
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    deleted_at     TEXT
);
CREATE INDEX ix_meeting_series_owner ON meeting_series (owner_id);
