-- Tickets on entries (round 14) get the same done / still open state as the Tickets tray.
-- Tickets already there count as done; new ones start still open. Their notepad is the
-- existing `note` column. Same conventions as 0001.
ALTER TABLE tickets ADD COLUMN done BOOLEAN NOT NULL DEFAULT TRUE;
