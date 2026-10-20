use std::{
    path::Path,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use super::{
    errors::{PrekuError, Result},
    validate,
};

/// Ordered schema migrations. Append new files; never edit an applied one (Rules.md §7).
/// The number of applied migrations is tracked in SQLite's `user_version` pragma.
const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_init.sql"),
    include_str!("../../migrations/0002_productivity.sql"),
    include_str!("../../migrations/0003_monitor.sql"),
    include_str!("../../migrations/0004_wellbeing.sql"),
    include_str!("../../migrations/0005_more_alert_metrics.sql"),
    include_str!("../../migrations/0006_wellbeing_companion.sql"),
    include_str!("../../migrations/0007_bridge.sql"),
    include_str!("../../migrations/0008_focus_tasks.sql"),
    include_str!("../../migrations/0009_today_and_schedules.sql"),
    include_str!("../../migrations/0010_task_reminders.sql"),
];

/// Local SQLite database (WAL mode). One connection behind a mutex is plenty for this app.
pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock can't leave SQLite half-written (statements are
        // atomic), so recovering the connection is safe.
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `f` with the connection locked. Module stores use this for their own tables.
    pub fn with_conn<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        f(&mut self.lock())
    }

    pub fn settings_get(&self, key: &str) -> Result<Option<Value>> {
        validate::key("Setting", key)?;
        let raw: Option<String> = self
            .lock()
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(raw.map(|s| serde_json::from_str(&s)).transpose()?)
    }

    pub fn settings_set(&self, key: &str, value: &Value) -> Result<()> {
        validate::key("Setting", key)?;
        self.lock().execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                                            updated_at = excluded.updated_at",
            params![key, value.to_string(), now_millis()],
        )?;
        Ok(())
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let applied: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied as usize) {
        let version = index as u32 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)
            .map_err(|source| PrekuError::Migration { version, source })?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        tracing::info!(version, "applied database migration");
    }
    Ok(())
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn applies_all_migrations_once() {
        let storage = Storage::open_in_memory().unwrap();
        let mut conn = storage.lock();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version as usize, MIGRATIONS.len());
        // Running again must be a no-op.
        migrate(&mut conn).unwrap();
    }

    #[test]
    fn settings_round_trip_and_overwrite() {
        let storage = Storage::open_in_memory().unwrap();
        assert_eq!(storage.settings_get("theme").unwrap(), None);

        storage.settings_set("theme", &json!("dark")).unwrap();
        assert_eq!(storage.settings_get("theme").unwrap(), Some(json!("dark")));

        storage
            .settings_set("theme", &json!({ "mode": "light" }))
            .unwrap();
        assert_eq!(
            storage.settings_get("theme").unwrap(),
            Some(json!({ "mode": "light" }))
        );
    }

    #[test]
    fn rejects_invalid_keys() {
        let storage = Storage::open_in_memory().unwrap();
        assert!(matches!(
            storage.settings_get("Not Valid"),
            Err(PrekuError::InvalidInput(_))
        ));
    }

    #[test]
    fn persists_to_disk() {
        let dir = std::env::temp_dir().join(format!("preku-test-{}", std::process::id()));
        let path = dir.join("nested").join("preku.db");
        {
            let storage = Storage::open(&path).unwrap();
            storage
                .settings_set("launch.at-login", &json!(true))
                .unwrap();
        }
        let storage = Storage::open(&path).unwrap();
        assert_eq!(
            storage.settings_get("launch.at-login").unwrap(),
            Some(json!(true))
        );
        drop(storage);
        let _ = std::fs::remove_dir_all(dir);
    }
}
