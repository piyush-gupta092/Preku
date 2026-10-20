//! The companion's records: completed breaks (for the daily count and encouragement) and the
//! optional mood check-in. Mood is only stored when the user opted in (the check-in is off by
//! default) and stays on this device.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use super::reminders::ReminderKind;
use crate::foundation::errors::{PrekuError, Result};

/// The check-in answers, happiest first.
pub const MOODS: [&str; 5] = ["great", "good", "okay", "low", "rough"];

fn day_key(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

pub fn log_break(conn: &Connection, kind: ReminderKind, day: NaiveDate, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO wellbeing_breaks (id, kind, day, at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![
            uuid::Uuid::new_v4().to_string(),
            kind.name(),
            day_key(day),
            now
        ],
    )?;
    Ok(())
}

pub fn breaks_today(conn: &Connection, day: NaiveDate) -> Result<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM wellbeing_breaks WHERE deleted = 0 AND day = ?1",
        [day_key(day)],
        |row| row.get(0),
    )?;
    Ok(u32::try_from(count).unwrap_or(0))
}

pub fn set_mood(conn: &Connection, day: NaiveDate, mood: &str, now: i64) -> Result<()> {
    if !MOODS.contains(&mood) {
        return Err(PrekuError::InvalidInput(format!("Unknown mood: {mood}")));
    }
    conn.execute(
        "INSERT INTO mood_logs (day, mood, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(day) DO UPDATE SET mood = excluded.mood, updated_at = excluded.updated_at",
        params![day_key(day), mood, now],
    )?;
    Ok(())
}

/// One day of the weekly reflection: plain counts of what was logged. No scores, no targets
/// missed, nothing inferred.
#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayReflection {
    /// Local day, YYYY-MM-DD.
    pub day: String,
    pub water_ml: u32,
    pub breaks: u32,
    /// Habits completed that day (reaching their daily target).
    pub habits_done: u32,
    /// The check-in of that day, if the user made one.
    pub mood: Option<String>,
}

/// The seven days ending today, oldest first.
pub fn week(conn: &Connection, today: NaiveDate) -> Result<Vec<DayReflection>> {
    let mut days = Vec::with_capacity(7);
    for back in (0..7u64).rev() {
        let Some(date) = today.checked_sub_days(chrono::Days::new(back)) else {
            continue;
        };
        let day = date.format("%Y-%m-%d").to_string();
        let water_ml = conn.query_row(
            "SELECT COALESCE(SUM(ml), 0) FROM water_logs WHERE day = ?1 AND deleted = 0",
            [&day],
            |row| row.get(0),
        )?;
        let breaks = conn.query_row(
            "SELECT COUNT(*) FROM wellbeing_breaks WHERE day = ?1 AND deleted = 0",
            [&day],
            |row| row.get(0),
        )?;
        let habits_done = conn.query_row(
            "SELECT COUNT(*) FROM habit_logs l JOIN habits h ON h.id = l.habit_id
             WHERE l.day = ?1 AND h.deleted = 0 AND l.count >= h.target",
            [&day],
            |row| row.get(0),
        )?;
        let mood = conn
            .query_row("SELECT mood FROM mood_logs WHERE day = ?1", [&day], |row| {
                row.get(0)
            })
            .optional()?;
        days.push(DayReflection {
            day,
            water_ml,
            breaks,
            habits_done,
            mood,
        });
    }
    Ok(days)
}

pub fn mood_today(conn: &Connection, day: NaiveDate) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT mood FROM mood_logs WHERE day = ?1",
            [day_key(day)],
            |row| row.get(0),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_week_lists_plain_counts_per_day() {
        let storage = crate::foundation::storage::Storage::open_in_memory().unwrap();
        let day = |d: u32| NaiveDate::from_ymd_opt(2026, 10, d).unwrap();
        storage
            .with_conn(|conn| {
                use crate::modules::wellbeing::habits;
                habits::add(conn, "Walk", 1, habits::ALL_DAYS, 1)?;
                habits::add(conn, "Read", 2, habits::ALL_DAYS, 1)?;
                let ids: Vec<String> = habits::list(conn, day(10))?
                    .into_iter()
                    .map(|h| h.id)
                    .collect();
                habits::log(conn, &ids[0], day(9), 1, 1)?;
                habits::log(conn, &ids[1], day(9), 1, 1)?; // 1 of 2: not completed
                habits::add_water(conn, 250, day(9), 1)?;
                habits::add_water(conn, 500, day(10), 1)?;
                log_break(conn, ReminderKind::Eyes, day(10), 1)?;
                log_break(conn, ReminderKind::Stand, day(10), 2)?;
                set_mood(conn, day(10), "good", 3)?;

                let week = week(conn, day(10))?;
                assert_eq!(week.len(), 7);
                assert_eq!(week[0].day, "2026-10-04");
                assert_eq!(
                    week[0],
                    DayReflection {
                        day: "2026-10-04".into(),
                        water_ml: 0,
                        breaks: 0,
                        habits_done: 0,
                        mood: None
                    }
                );
                assert_eq!(
                    (week[5].water_ml, week[5].habits_done, week[5].breaks),
                    (250, 1, 0)
                );
                assert_eq!(
                    week[6],
                    DayReflection {
                        day: "2026-10-10".into(),
                        water_ml: 500,
                        breaks: 2,
                        habits_done: 0,
                        mood: Some("good".into())
                    }
                );
                Ok(())
            })
            .unwrap();
    }
    use crate::foundation::storage::Storage;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 10).unwrap()
    }

    #[test]
    fn counts_todays_breaks() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                log_break(conn, ReminderKind::Eyes, day(), 1)?;
                log_break(conn, ReminderKind::Stand, day(), 2)?;
                log_break(conn, ReminderKind::Eyes, day().pred_opt().unwrap(), 3)?;
                assert_eq!(breaks_today(conn, day())?, 2);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn one_mood_per_day_and_only_known_moods() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                assert_eq!(mood_today(conn, day())?, None);
                set_mood(conn, day(), "okay", 1)?;
                set_mood(conn, day(), "good", 2)?;
                assert_eq!(mood_today(conn, day())?.as_deref(), Some("good"));
                assert!(set_mood(conn, day(), "ecstatic", 3).is_err());
                Ok(())
            })
            .unwrap();
    }
}
