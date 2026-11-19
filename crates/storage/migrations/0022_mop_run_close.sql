-- Closing out a MOP run (round 21, card r21-mop-finish): when the run finished (Done or
-- Backed out; cleared, with the start, when it goes back to Draft or Ready), and what was
-- seen at each step (a list of [tick key, text] pairs, see shiftbook_core::change) for the
-- run log. Same conventions as 0001.
ALTER TABLE mop_runs ADD COLUMN finished_at TEXT;
ALTER TABLE mop_runs ADD COLUMN seen TEXT NOT NULL DEFAULT '';
