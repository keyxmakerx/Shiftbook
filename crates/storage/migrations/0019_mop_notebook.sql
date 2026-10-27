-- The MOP notebook (round 16, card r15-mopbook). A note marked as a template offers "New MOP
-- from this template", which fills in its [[blanks]]. ticks holds what is ticked off in
-- Checklist mode, one line key per line (see shiftbook_core::mop), so the MOP text itself
-- stays clean. Same conventions as 0001.
ALTER TABLE notes ADD COLUMN template BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE notes ADD COLUMN ticks TEXT NOT NULL DEFAULT '';
