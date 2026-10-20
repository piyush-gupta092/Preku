//! Control over captured data (usability U5): pause clipboard capture, keep clipboard items
//! and app-usage records only for a chosen time, export everything readable, delete it all.
//! Local only: nothing here ever leaves the PC, and exports never contain secrets.

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, PoisonError,
    },
};

use chrono::{Days, NaiveDate};
use rusqlite::{params, types::ValueRef, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use ts_rs::TS;

use super::{
    errors::{PrekuError, Result},
    storage::Storage,
};

const KEY: &str = "privacy";
pub const EVENT: &str = "app:privacy";
const DAY_MS: i64 = 24 * 60 * 60_000;
const MAX_KEEP_DAYS: u32 = 3_650;

/// What "Delete my data" removes: everything Preku recorded about the user's activity.
/// Settings, alert rules, watched sites, paired phones and saved sign-ins stay.
pub const ACTIVITY_TABLES: [&str; 9] = [
    "tasks",
    "focus_sessions",
    "habits",
    "habit_logs",
    "water_logs",
    "wellbeing_breaks",
    "mood_logs",
    "screen_time",
    "clipboard_items",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct PrivacySettings {
    /// Nothing copied is recorded while this is on.
    pub clipboard_paused: bool,
    /// Unpinned clipboard items older than this many days are removed (0 = keep the last 50,
    /// however old).
    pub clipboard_keep_days: u32,
    /// App-usage (screen time) records older than this many days are removed (0 = keep).
    pub usage_keep_days: u32,
}

pub struct Privacy {
    settings: Mutex<PrivacySettings>,
    /// Read by the clipboard watcher on every copy.
    clipboard_paused: AtomicBool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self::new(PrivacySettings::default())
    }
}

impl Privacy {
    fn new(settings: PrivacySettings) -> Self {
        Self {
            clipboard_paused: AtomicBool::new(settings.clipboard_paused),
            settings: Mutex::new(settings),
        }
    }

    pub fn load(storage: &Storage) -> Arc<Self> {
        let settings = storage
            .settings_get(KEY)
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        Arc::new(Self::new(settings))
    }

    pub fn settings(&self) -> PrivacySettings {
        *self.settings.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn clipboard_paused(&self) -> bool {
        self.clipboard_paused.load(Ordering::SeqCst)
    }

    pub fn set(&self, storage: &Storage, settings: PrivacySettings) -> Result<()> {
        if settings.clipboard_keep_days > MAX_KEEP_DAYS || settings.usage_keep_days > MAX_KEEP_DAYS
        {
            return Err(PrekuError::InvalidInput(format!(
                "Keep data for at most {MAX_KEEP_DAYS} days"
            )));
        }
        storage.settings_set(KEY, &serde_json::to_value(settings)?)?;
        self.clipboard_paused
            .store(settings.clipboard_paused, Ordering::SeqCst);
        *self.settings.lock().unwrap_or_else(PoisonError::into_inner) = settings;
        Ok(())
    }
}

fn remove_images(image_dir: &Path, files: &[String]) {
    for file in files {
        // Only plain file names stored by the clipboard module; never a path.
        if file.contains(['/', '\\']) || file.contains("..") {
            continue;
        }
        let _ = std::fs::remove_file(image_dir.join(file));
    }
}

/// Removes what is older than the user's limits. Returns how many rows went.
pub fn prune(
    conn: &Connection,
    settings: &PrivacySettings,
    image_dir: &Path,
    now: i64,
    today: NaiveDate,
) -> Result<usize> {
    let mut removed = 0;
    if settings.clipboard_keep_days > 0 {
        let cutoff = now - i64::from(settings.clipboard_keep_days) * DAY_MS;
        let files: Vec<String> = conn
            .prepare("SELECT image_file FROM clipboard_items WHERE pinned = 0 AND created_at < ?1 AND image_file IS NOT NULL")?
            .query_map([cutoff], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        removed += conn.execute(
            "DELETE FROM clipboard_items WHERE pinned = 0 AND created_at < ?1",
            [cutoff],
        )?;
        remove_images(image_dir, &files);
    }
    if settings.usage_keep_days > 0 {
        if let Some(cutoff) = today.checked_sub_days(Days::new(u64::from(settings.usage_keep_days)))
        {
            removed += conn.execute(
                "DELETE FROM screen_time WHERE day < ?1",
                params![cutoff.format("%Y-%m-%d").to_string()],
            )?;
        }
    }
    Ok(removed)
}

fn table_rows(conn: &Connection, table: &str, columns: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(&format!("SELECT {columns} FROM {table}"))?;
    let names: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
    let rows = stmt.query_map([], |row| {
        let mut object = Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(n) => json!(n),
                ValueRef::Text(text) => json!(String::from_utf8_lossy(text)),
                ValueRef::Blob(_) => Value::Null,
            };
            object.insert(name.clone(), value);
        }
        Ok(Value::Object(object))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Everything Preku recorded, as readable JSON. Not included: saved sign-ins and links (they
/// live in the OS keychain), paired phones, and clipboard images (only their sizes).
pub fn export(conn: &Connection, now: i64) -> Result<Value> {
    let mut data = Map::new();
    for table in ACTIVITY_TABLES {
        let columns = if table == "clipboard_items" {
            "id, kind, text, width, height, pinned, created_at"
        } else {
            "*"
        };
        data.insert(
            table.to_owned(),
            Value::Array(table_rows(conn, table, columns)?),
        );
    }
    // Preferences and module settings (no secrets are ever stored here, Rules.md §4).
    let settings: Vec<Value> = table_rows(conn, "settings", "key, value_json")?
        .into_iter()
        .filter(|row| {
            !row["key"]
                .as_str()
                .is_some_and(|key| key.starts_with("bridge"))
        })
        .collect();
    data.insert("settings".to_owned(), Value::Array(settings));
    Ok(json!({
        "app": "Preku",
        "exportedAt": now,
        "note": "Times are unix milliseconds. Not included: saved sign-ins and calendar links, paired phones, clipboard images.",
        "data": data,
    }))
}

/// Deletes everything in [`ACTIVITY_TABLES`] and the saved clipboard images.
pub fn delete_all(conn: &mut Connection, image_dir: &Path) -> Result<()> {
    let files: Vec<String> = conn
        .prepare("SELECT image_file FROM clipboard_items WHERE image_file IS NOT NULL")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let tx = conn.transaction()?;
    for table in ACTIVITY_TABLES {
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }
    tx.commit()?;
    remove_images(image_dir, &files);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> Storage {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                conn.execute_batch(
                    "INSERT INTO tasks (id, title, done, sort_order, created_at, updated_at) VALUES ('t1', 'Write', 0, 0, 1, 1);
                     INSERT INTO clipboard_items (id, kind, text, fingerprint, pinned, created_at) VALUES
                       ('old', 'text', 'old copy', 'f1', 0, 1000),
                       ('pin', 'text', 'pinned copy', 'f2', 1, 1000),
                       ('new', 'text', 'new copy', 'f3', 0, 900000000);
                     INSERT INTO screen_time (device, day, app, seconds, updated_at) VALUES
                       ('pc', '2026-09-01', 'Code', 60, 1), ('pc', '2026-10-10', 'Code', 60, 1);",
                )?;
                Ok(())
            })
            .unwrap();
        storage
            .settings_set("wellbeing", &json!({ "waterGoalMl": 2000 }))
            .unwrap();
        storage
            .settings_set("bridge.pc", &json!({ "pcId": "secret-ish" }))
            .unwrap();
        storage
    }

    fn count(storage: &Storage, table: &str) -> i64 {
        storage
            .with_conn(|conn| {
                Ok(conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
            })
            .unwrap()
    }

    #[test]
    fn old_records_go_but_pinned_and_recent_ones_stay() {
        let storage = seeded();
        let today = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let dir = std::env::temp_dir();
        let now = 900_000_000 + DAY_MS;

        // Limits off: nothing is touched.
        let off = PrivacySettings::default();
        assert_eq!(
            storage
                .with_conn(|c| prune(c, &off, &dir, now, today))
                .unwrap(),
            0
        );

        let limits = PrivacySettings {
            clipboard_keep_days: 7,
            usage_keep_days: 30,
            ..off
        };
        assert_eq!(
            storage
                .with_conn(|c| prune(c, &limits, &dir, now, today))
                .unwrap(),
            2
        );
        assert_eq!(
            count(&storage, "clipboard_items"),
            2,
            "pinned and recent stay"
        );
        assert_eq!(count(&storage, "screen_time"), 1);
    }

    #[test]
    fn the_export_is_readable_and_leaves_out_bridge_identity() {
        let storage = seeded();
        let export = storage.with_conn(|c| export(c, 42)).unwrap();
        assert_eq!(export["data"]["tasks"][0]["title"], "Write");
        assert_eq!(
            export["data"]["clipboard_items"].as_array().unwrap().len(),
            3
        );
        let keys: Vec<&str> = export["data"]["settings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|row| row["key"].as_str())
            .collect();
        assert!(keys.contains(&"wellbeing") && !keys.iter().any(|key| key.starts_with("bridge")));
        assert!(!export.to_string().contains("secret-ish"));
    }

    #[test]
    fn delete_removes_activity_and_keeps_settings() {
        let storage = seeded();
        storage
            .with_conn(|c| delete_all(c, &std::env::temp_dir()))
            .unwrap();
        for table in ACTIVITY_TABLES {
            assert_eq!(count(&storage, table), 0, "{table}");
        }
        assert!(storage.settings_get("wellbeing").unwrap().is_some());
    }

    #[test]
    fn settings_round_trip_and_the_pause_takes_effect_at_once() {
        let storage = Storage::open_in_memory().unwrap();
        let privacy = Privacy::load(&storage);
        assert_eq!(privacy.settings(), PrivacySettings::default());
        assert!(!privacy.clipboard_paused());

        let paused = PrivacySettings {
            clipboard_paused: true,
            clipboard_keep_days: 30,
            usage_keep_days: 90,
        };
        privacy.set(&storage, paused).unwrap();
        assert!(privacy.clipboard_paused());
        assert_eq!(Privacy::load(&storage).settings(), paused);
        assert!(privacy
            .set(
                &storage,
                PrivacySettings {
                    usage_keep_days: 99_999,
                    ..paused
                }
            )
            .is_err());
    }
}
