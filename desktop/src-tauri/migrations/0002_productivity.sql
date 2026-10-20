-- Phase 2: productivity module. Synced tables carry updated_at + deleted (tombstone) for the
-- future phone sync (Architecture.md §4.4). Times are unix epoch milliseconds.

CREATE TABLE tasks (
    id         TEXT    PRIMARY KEY NOT NULL,
    title      TEXT    NOT NULL,
    done       INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX tasks_active ON tasks (deleted, done, sort_order);

CREATE TABLE focus_sessions (
    id          TEXT    PRIMARY KEY NOT NULL,
    label       TEXT    NOT NULL,
    started_at  INTEGER NOT NULL,
    duration_s  INTEGER NOT NULL, -- planned length
    focused_s   INTEGER NOT NULL, -- time actually spent running
    completed   INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

-- Local only, never synced (Rules.md §4: clipboard content stays on this machine).
CREATE TABLE clipboard_items (
    id          TEXT    PRIMARY KEY NOT NULL,
    kind        TEXT    NOT NULL CHECK (kind IN ('text', 'image')),
    text        TEXT,
    image_file  TEXT,
    width       INTEGER,
    height      INTEGER,
    fingerprint TEXT    NOT NULL,
    pinned      INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);
CREATE INDEX clipboard_order ON clipboard_items (pinned, created_at);
CREATE UNIQUE INDEX clipboard_fingerprint ON clipboard_items (fingerprint);
