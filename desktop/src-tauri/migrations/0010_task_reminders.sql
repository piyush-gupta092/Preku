-- PRD A11: an optional reminder time per task (unix ms). due_notified is when Preku spoke up
-- about it, so a reminder is given once (again only if the user moves the time).
ALTER TABLE tasks ADD COLUMN due_at INTEGER;
ALTER TABLE tasks ADD COLUMN due_notified INTEGER;
CREATE INDEX tasks_due ON tasks (due_at) WHERE due_at IS NOT NULL;
