-- Reusable lists (round 15): a list is a task whose checklist is the point (shopping,
-- packing). Kept apart from ordinary tasks so the Tasks screen (M4) won't show them.
-- Numbered 16 to leave 15 to the items migration in flight. Same conventions as 0001.
ALTER TABLE tasks ADD COLUMN is_list BOOLEAN NOT NULL DEFAULT FALSE;
