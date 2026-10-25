-- Log a meeting from the bell (round 15). A series says how long it usually lasts and which
-- project its time goes to; `handled_on` is the last day its "Just finished" item was
-- logged or skipped, so it shows once a day at most. Same conventions as 0001.
ALTER TABLE meeting_series ADD COLUMN length_minutes INTEGER NOT NULL DEFAULT 30
    CHECK (length_minutes >= 5 AND length_minutes <= 480);
ALTER TABLE meeting_series ADD COLUMN project_id TEXT REFERENCES projects (id);
ALTER TABLE meeting_series ADD COLUMN handled_on TEXT;
