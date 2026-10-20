-- Phase 6: paired phones. A phone is identified by the SHA-256 fingerprint of its TLS client
-- certificate (pinned at pairing); the name is only for display.

CREATE TABLE bridge_devices (
    id         TEXT    PRIMARY KEY NOT NULL,
    name       TEXT    NOT NULL,
    cert_fp    TEXT    NOT NULL UNIQUE,
    paired_at  INTEGER NOT NULL,
    last_seen  INTEGER,
    updated_at INTEGER NOT NULL
);
