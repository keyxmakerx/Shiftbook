-- Snoozed tickets (round 21, idea approved 26 October): a still-open ticket kept out of a
-- tray's "Still open from before" until a day. One live row per ticket number; a new snooze
-- retires the old row. The ticket comes back on wake_on, or sooner if it is worked on after
-- snoozed_on (see shiftbook_core::tray::snoozed_until). Same conventions as 0001.
CREATE TABLE ticket_snoozes (
    id            TEXT PRIMARY KEY NOT NULL,
    owner_id      TEXT NOT NULL REFERENCES users (id),
    ticket_number TEXT NOT NULL,
    -- The day the snooze was made, and the day the ticket comes back, YYYY-MM-DD.
    snoozed_on    TEXT NOT NULL,
    wake_on       TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT
);
CREATE INDEX ix_ticket_snoozes_owner ON ticket_snoozes (owner_id, ticket_number);
