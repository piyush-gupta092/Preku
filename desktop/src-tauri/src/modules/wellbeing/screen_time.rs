//! Screen time per app (PRD W3). Seconds are buffered in memory and flushed to SQLite every
//! minute. Only application names are recorded — never window titles (privacy, Rules.md §4).

use std::collections::HashMap;

use chrono::{Days, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use super::habits::day_key;
use crate::foundation::errors::Result;

pub const DEVICE: &str = "desktop";
const TOP_APPS: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppUsage {
    pub app: String,
    pub seconds: u32,
}

/// Seconds not yet written to the database, per (day, app).
#[derive(Debug, Default)]
pub struct Buffer(HashMap<(String, String), u32>);

impl Buffer {
    pub fn add(&mut self, day: NaiveDate, app: &str, seconds: u32) {
        *self.0.entry((day_key(day), app.to_owned())).or_default() += seconds;
    }

    pub fn flush(&mut self, conn: &mut Connection, now: i64) -> Result<()> {
        if self.0.is_empty() {
            return Ok(());
        }
        let tx = conn.transaction()?;
        for ((day, app), seconds) in &self.0 {
            tx.execute(
                "INSERT INTO screen_time (device, day, app, seconds, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(device, day, app) DO UPDATE
                   SET seconds = seconds + excluded.seconds, updated_at = excluded.updated_at",
                params![DEVICE, day, app, seconds, now],
            )?;
        }
        tx.commit()?;
        self.0.clear();
        Ok(())
    }
}

/// Top apps for a day (most used first) and the day's total, in seconds.
pub fn day_usage(conn: &Connection, day: NaiveDate) -> Result<(Vec<AppUsage>, u32)> {
    let mut stmt = conn.prepare(
        "SELECT app, seconds FROM screen_time WHERE device = ?1 AND day = ?2
         ORDER BY seconds DESC",
    )?;
    let all: Vec<AppUsage> = stmt
        .query_map(params![DEVICE, day_key(day)], |row| {
            Ok(AppUsage {
                app: row.get(0)?,
                seconds: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let total = all.iter().map(|u| u.seconds).sum();
    Ok((all.into_iter().take(TOP_APPS).collect(), total))
}

/// Total seconds per day for the 7 days ending `today` (oldest first).
pub fn week_totals(conn: &Connection, today: NaiveDate) -> Result<Vec<u32>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(SUM(seconds), 0) FROM screen_time WHERE device = ?1 AND day = ?2",
    )?;
    (0..7u64)
        .rev()
        .map(|back| {
            let day = today.checked_sub_days(Days::new(back)).unwrap_or(today);
            Ok(stmt.query_row(params![DEVICE, day_key(day)], |row| row.get(0))?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::storage::Storage;

    #[test]
    fn buffers_and_flushes_per_day_and_app() {
        let storage = Storage::open_in_memory().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        storage
            .with_conn(|conn| {
                let mut buffer = Buffer::default();
                buffer.add(today, "Code", 5);
                buffer.add(today, "Code", 5);
                buffer.add(today, "Chrome", 5);
                buffer.flush(conn, 1)?;
                buffer.add(today, "Code", 50);
                buffer.add(yesterday, "Code", 100);
                buffer.flush(conn, 2)?;

                let (apps, total) = day_usage(conn, today)?;
                assert_eq!(total, 65);
                assert_eq!(
                    apps[0],
                    AppUsage {
                        app: "Code".into(),
                        seconds: 60
                    }
                );

                let week = week_totals(conn, today)?;
                assert_eq!(week.len(), 7);
                assert_eq!(&week[5..], [100, 65]);
                Ok(())
            })
            .unwrap();
    }
}
