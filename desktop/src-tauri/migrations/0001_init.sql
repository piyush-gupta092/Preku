-- Key/value store for user settings. Values are JSON documents.
CREATE TABLE settings (
    key        TEXT    PRIMARY KEY NOT NULL,
    value_json TEXT    NOT NULL,
    updated_at INTEGER NOT NULL -- unix epoch milliseconds
);
