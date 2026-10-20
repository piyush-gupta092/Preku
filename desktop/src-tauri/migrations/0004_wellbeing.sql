-- Phase 4: wellbeing. Synced tables carry updated_at + deleted for the future phone sync
-- (Architecture.md §4.4). Dates are local calendar days, YYYY-MM-DD.

CREATE TABLE water_logs (
    id         TEXT    PRIMARY KEY NOT NULL,
    ml         INTEGER NOT NULL,
    at         INTEGER NOT NULL, -- unix ms
    day        TEXT    NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX water_day ON water_logs (deleted, day);

CREATE TABLE habits (
    id         TEXT    PRIMARY KEY NOT NULL,
    name       TEXT    NOT NULL,
    target     INTEGER NOT NULL DEFAULT 1, -- times per day
    sort_order INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE habit_logs (
    habit_id   TEXT    NOT NULL,
    day        TEXT    NOT NULL,
    count      INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (habit_id, day)
);

-- Seconds of active use per app per day. App = executable name, never window titles.
CREATE TABLE screen_time (
    device     TEXT    NOT NULL,
    day        TEXT    NOT NULL,
    app        TEXT    NOT NULL,
    seconds    INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (device, day, app)
);
