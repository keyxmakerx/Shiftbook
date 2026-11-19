-- Days off (round 21, idea approved 26 October): the mark that goes with a "Holiday" entry
-- booked as time off. The day counts as off while both this row and its entry are live, so
-- deleting the entry (or undoing the booking) takes the mark away with it. The ribbon draws
-- the day grey and the bell stays quiet on it. Same conventions as 0001.
CREATE TABLE days_off (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    -- The "Holiday" entry that carries the day's hours.
    entry_id   TEXT NOT NULL REFERENCES time_entries (id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
CREATE INDEX ix_days_off_owner ON days_off (owner_id, entry_id);
