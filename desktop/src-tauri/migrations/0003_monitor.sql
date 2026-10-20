-- Phase 3: system & dev monitor. Local only.

CREATE TABLE monitor_alert_rules (
    id          TEXT    PRIMARY KEY NOT NULL,
    metric      TEXT    NOT NULL CHECK (metric IN ('cpu', 'memory', 'disk', 'battery')),
    op          TEXT    NOT NULL CHECK (op IN ('above', 'below')),
    threshold   REAL    NOT NULL,
    duration_s  INTEGER NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL
);

-- Sensible defaults (PRD M2); users can edit or delete them.
INSERT INTO monitor_alert_rules VALUES
    ('default-cpu',     'cpu',     'above', 90, 30, 1, 0),
    ('default-memory',  'memory',  'above', 90, 60, 1, 0),
    ('default-disk',    'disk',    'above', 95,  0, 1, 0),
    ('default-battery', 'battery', 'below', 15,  0, 1, 0);

-- Things to watch: GitHub repos (owner/name) and uptime URLs.
CREATE TABLE watch_targets (
    id          TEXT    PRIMARY KEY NOT NULL,
    kind        TEXT    NOT NULL CHECK (kind IN ('github', 'uptime')),
    target      TEXT    NOT NULL,
    interval_s  INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    UNIQUE (kind, target)
);
