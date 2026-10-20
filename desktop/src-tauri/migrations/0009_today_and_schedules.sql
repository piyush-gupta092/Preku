-- Usability U2 / U7.
-- Today's priorities: a task can be picked for one local day (YYYY-MM-DD). It simply stops
-- being a priority when the day is over; nothing is forced or lost.
ALTER TABLE tasks ADD COLUMN today_on TEXT;
-- When a task was finished (unix ms), for the daily recap. Older finished tasks have none.
ALTER TABLE tasks ADD COLUMN done_at INTEGER;
-- Which weekdays a habit is for: bit 0 = Monday … bit 6 = Sunday. 127 = every day, which is
-- what every existing habit keeps.
ALTER TABLE habits ADD COLUMN days INTEGER NOT NULL DEFAULT 127;
