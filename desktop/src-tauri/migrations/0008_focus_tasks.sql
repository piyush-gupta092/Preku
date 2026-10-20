-- Usability U1: a focus session can belong to a task. Optional, so standalone timers and
-- every earlier session keep working; the label is still stored, so renaming or deleting a
-- task never rewrites history.
ALTER TABLE focus_sessions ADD COLUMN task_id TEXT;
CREATE INDEX focus_sessions_task ON focus_sessions (task_id) WHERE task_id IS NOT NULL;
