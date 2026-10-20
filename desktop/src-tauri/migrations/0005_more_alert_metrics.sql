-- More alert metrics (swap, free disk, network, temperature). The original table restricted
-- `metric` with a CHECK constraint, which SQLite can't alter, so the table is rebuilt.
-- Metric names are validated in Rust (modules/monitor/alerts.rs).

CREATE TABLE monitor_alert_rules_new (
    id          TEXT    PRIMARY KEY NOT NULL,
    metric      TEXT    NOT NULL,
    op          TEXT    NOT NULL CHECK (op IN ('above', 'below')),
    threshold   REAL    NOT NULL,
    duration_s  INTEGER NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  INTEGER NOT NULL
);

INSERT INTO monitor_alert_rules_new
    SELECT id, metric, op, threshold, duration_s, enabled, created_at FROM monitor_alert_rules;

DROP TABLE monitor_alert_rules;
ALTER TABLE monitor_alert_rules_new RENAME TO monitor_alert_rules;
