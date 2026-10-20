-- Wellbeing companion: completed breaks (for encouragement and the daily count) and the
-- optional mood check-in. Mood is sensitive: rows exist only if the user turned the
-- check-in on, and they never leave this device until the phone sync (and then only to the
-- user's own paired phone).

CREATE TABLE wellbeing_breaks (
    id         TEXT    PRIMARY KEY NOT NULL,
    kind       TEXT    NOT NULL, -- reminder kind, e.g. "eyes", "stand", "reset"
    day        TEXT    NOT NULL, -- local YYYY-MM-DD
    at         INTEGER NOT NULL, -- unix ms
    updated_at INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX wellbeing_breaks_day ON wellbeing_breaks (deleted, day);

CREATE TABLE mood_logs (
    day        TEXT    PRIMARY KEY NOT NULL, -- one check-in per day; a later one replaces it
    mood       TEXT    NOT NULL,             -- great | good | okay | low | rough
    updated_at INTEGER NOT NULL
);
