//! Habits with daily targets and streaks (PRD W6), plus water logs (W2).
//! Habits and water logs carry `updated_at`/`deleted` for the future phone sync.

use std::collections::BTreeSet;

use chrono::{Datelike, Days, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use ts_rs::TS;

use crate::foundation::errors::{PrekuError, Result};

/// Streak lengths Pip celebrates.
pub const MILESTONES: [u32; 7] = [3, 7, 14, 21, 30, 50, 100];
const MAX_NAME_CHARS: usize = 60;
const MAX_TARGET: u32 = 20;
/// How far back streaks look. Longer streaks still display (capped here).
const STREAK_LOOKBACK_DAYS: u64 = 400;

pub fn day_key(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

fn parse_day(key: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(key, "%Y-%m-%d").ok()
}

/// Every weekday (bit 0 = Monday … bit 6 = Sunday).
pub const ALL_DAYS: u8 = 0b111_1111;

/// Whether a habit with this weekday mask is meant for `day`.
pub fn scheduled(days: u8, day: NaiveDate) -> bool {
    days & (1 << day.weekday().num_days_from_monday()) != 0
}

/// Completed days in a row, counting back from today. Days the habit isn't scheduled for
/// never break a streak (and doing it anyway on such a day counts). Today doesn't break it
/// either while it is still open, so a streak isn't shown as lost in the morning.
pub fn streak(done_days: &BTreeSet<NaiveDate>, days: u8, today: NaiveDate) -> u32 {
    let mut count = 0;
    let mut day = today;
    for _ in 0..=STREAK_LOOKBACK_DAYS {
        if done_days.contains(&day) {
            count += 1;
        } else if scheduled(days, day) && day != today {
            break;
        }
        match day.checked_sub_days(Days::new(1)) {
            Some(previous) => day = previous,
            None => break,
        }
    }
    count
}

/// The most recent scheduled day before `today`.
fn previous_scheduled(days: u8, today: NaiveDate) -> Option<NaiveDate> {
    (1..=7)
        .filter_map(|back| today.checked_sub_days(Days::new(back)))
        .find(|day| scheduled(days, *day))
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HabitView {
    pub id: String,
    pub name: String,
    pub target: u32,
    pub count_today: u32,
    pub done_today: bool,
    pub streak: u32,
    /// Weekday mask (bit 0 = Monday … bit 6 = Sunday); 127 = every day.
    pub days: u8,
    pub scheduled_today: bool,
    /// Done today after missing the last scheduled day: worth a warm welcome back.
    pub came_back: bool,
}

fn validate_days(days: u8) -> Result<()> {
    if days == 0 || days > ALL_DAYS {
        return Err(PrekuError::InvalidInput(
            "Pick at least one day for this habit".into(),
        ));
    }
    Ok(())
}

fn validate(name: &str, target: u32) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(PrekuError::InvalidInput(format!(
            "Habit name must be 1-{MAX_NAME_CHARS} characters"
        )));
    }
    if !(1..=MAX_TARGET).contains(&target) {
        return Err(PrekuError::InvalidInput(format!(
            "Daily target must be 1-{MAX_TARGET}"
        )));
    }
    Ok(name.to_owned())
}

pub fn add(conn: &Connection, name: &str, target: u32, days: u8, now: i64) -> Result<()> {
    let name = validate(name, target)?;
    validate_days(days)?;
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM habits WHERE deleted = 0",
        [],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO habits (id, name, target, days, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            uuid::Uuid::new_v4().to_string(),
            name,
            target,
            days,
            next,
            now
        ],
    )?;
    Ok(())
}

/// Changes a habit's name, daily target or days. Its history stays as it is.
pub fn update(
    conn: &Connection,
    id: &str,
    name: &str,
    target: u32,
    days: u8,
    now: i64,
) -> Result<()> {
    let name = validate(name, target)?;
    validate_days(days)?;
    let changed = conn.execute(
        "UPDATE habits SET name = ?2, target = ?3, days = ?4, updated_at = ?5
         WHERE id = ?1 AND deleted = 0",
        params![id, name, target, days, now],
    )?;
    if changed == 0 {
        return Err(PrekuError::InvalidInput(format!("No habit {id}")));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE habits SET deleted = 1, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    Ok(())
}

/// Adds `delta` (±) to today's count, never below zero.
pub fn log(conn: &Connection, id: &str, day: NaiveDate, delta: i32, now: i64) -> Result<()> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM habits WHERE id = ?1 AND deleted = 0",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(PrekuError::InvalidInput(format!("No habit {id}")));
    }
    conn.execute(
        "INSERT INTO habit_logs (habit_id, day, count, updated_at) VALUES (?1, ?2, MAX(0, ?3), ?4)
         ON CONFLICT(habit_id, day) DO UPDATE
           SET count = MAX(0, count + ?3), updated_at = excluded.updated_at",
        params![id, day_key(day), delta, now],
    )?;
    Ok(())
}

pub fn list(conn: &Connection, today: NaiveDate) -> Result<Vec<HabitView>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, target, days FROM habits WHERE deleted = 0 ORDER BY sort_order, created_at",
    )?;
    let habits: Vec<(String, String, u32, u8)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let since = today
        .checked_sub_days(Days::new(STREAK_LOOKBACK_DAYS))
        .map(day_key)
        .unwrap_or_default();

    let mut logs = conn.prepare(
        "SELECT day, count FROM habit_logs WHERE habit_id = ?1 AND day >= ?2 AND count > 0",
    )?;
    let mut views = Vec::with_capacity(habits.len());
    for (id, name, target, days) in habits {
        let rows: Vec<(String, u32)> = logs
            .query_map(params![id, since], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let done: BTreeSet<NaiveDate> = rows
            .iter()
            .filter(|(_, count)| *count >= target)
            .filter_map(|(day, _)| parse_day(day))
            .collect();
        let today_key = day_key(today);
        let count_today = rows
            .iter()
            .find(|(day, _)| *day == today_key)
            .map_or(0, |(_, count)| *count);
        let done_today = count_today >= target;
        let missed_last = previous_scheduled(days, today).is_some_and(|day| !done.contains(&day));
        views.push(HabitView {
            streak: streak(&done, days, today),
            came_back: done_today && missed_last && done.iter().any(|day| *day < today),
            days,
            scheduled_today: scheduled(days, today),
            done_today,
            id,
            name,
            target,
            count_today,
        });
    }
    Ok(views)
}

pub fn add_water(conn: &Connection, ml: u32, day: NaiveDate, now: i64) -> Result<()> {
    if !(10..=2000).contains(&ml) {
        return Err(PrekuError::InvalidInput("A drink is 10-2000 ml".into()));
    }
    conn.execute(
        "INSERT INTO water_logs (id, ml, at, day, updated_at) VALUES (?1, ?2, ?3, ?4, ?3)",
        params![uuid::Uuid::new_v4().to_string(), ml, now, day_key(day)],
    )?;
    Ok(())
}

/// Removes today's most recent drink (a mis-click).
pub fn undo_water(conn: &Connection, day: NaiveDate, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE water_logs SET deleted = 1, updated_at = ?2
         WHERE id = (SELECT id FROM water_logs WHERE day = ?1 AND deleted = 0
                     ORDER BY at DESC LIMIT 1)",
        params![day_key(day), now],
    )?;
    Ok(())
}

pub fn water_today(conn: &Connection, day: NaiveDate) -> Result<u32> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(ml), 0) FROM water_logs WHERE day = ?1 AND deleted = 0",
        [day_key(day)],
        |row| row.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::storage::Storage;

    fn d(s: &str) -> NaiveDate {
        parse_day(s).unwrap()
    }

    #[test]
    fn streak_counts_back_from_today_or_yesterday() {
        let done: BTreeSet<_> = ["2026-10-05", "2026-10-06", "2026-10-07", "2026-10-09"]
            .into_iter()
            .map(d)
            .collect();
        assert_eq!(streak(&done, ALL_DAYS, d("2026-10-09")), 1);
        assert_eq!(
            streak(&done, ALL_DAYS, d("2026-10-08")),
            3,
            "yesterday still counts"
        );
        assert_eq!(
            streak(&done, ALL_DAYS, d("2026-10-11")),
            0,
            "two days missed"
        );
        assert_eq!(streak(&BTreeSet::new(), ALL_DAYS, d("2026-10-09")), 0);
    }

    #[test]
    fn streak_crosses_month_and_year_boundaries() {
        let done: BTreeSet<_> = ["2025-12-30", "2025-12-31", "2026-01-01"]
            .into_iter()
            .map(d)
            .collect();
        assert_eq!(streak(&done, ALL_DAYS, d("2026-01-01")), 3);
    }

    #[test]
    fn habits_track_targets_and_streaks() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                add(conn, "Stretch", 2, ALL_DAYS, 1)?;
                let id = list(conn, d("2026-10-09"))?[0].id.clone();
                for day in ["2026-10-07", "2026-10-08"] {
                    log(conn, &id, d(day), 2, 1)?;
                }
                log(conn, &id, d("2026-10-09"), 1, 1)?;
                let view = &list(conn, d("2026-10-09"))?[0];
                assert_eq!(
                    (view.count_today, view.done_today, view.streak),
                    (1, false, 2)
                );

                log(conn, &id, d("2026-10-09"), 1, 1)?;
                let view = &list(conn, d("2026-10-09"))?[0];
                assert_eq!((view.done_today, view.streak), (true, 3));

                log(conn, &id, d("2026-10-09"), -5, 1)?;
                assert_eq!(
                    list(conn, d("2026-10-09"))?[0].count_today,
                    0,
                    "never negative"
                );

                delete(conn, &id, 2)?;
                assert!(list(conn, d("2026-10-09"))?.is_empty());
                assert!(log(conn, &id, d("2026-10-09"), 1, 3).is_err());
                Ok(())
            })
            .unwrap();
    }

    /// Monday, Wednesday, Friday.
    const MWF: u8 = 0b001_0101;

    #[test]
    fn days_off_never_break_a_streak() {
        // 2026-10-05 is a Monday.
        let done: BTreeSet<_> = ["2026-10-05", "2026-10-07", "2026-10-09"]
            .into_iter()
            .map(d)
            .collect();
        assert!(scheduled(MWF, d("2026-10-05")) && !scheduled(MWF, d("2026-10-06")));
        // Friday done; Tuesday and Thursday were never expected.
        assert_eq!(streak(&done, MWF, d("2026-10-09")), 3);
        // The weekend is off: still 3 on Sunday, and on Monday morning before checking in.
        assert_eq!(streak(&done, MWF, d("2026-10-11")), 3);
        assert_eq!(streak(&done, MWF, d("2026-10-12")), 3);
        // Monday passed without it: on Tuesday the streak is over.
        assert_eq!(streak(&done, MWF, d("2026-10-13")), 0);
        // Doing it on an unplanned day counts as a bonus.
        let mut bonus = done.clone();
        bonus.insert(d("2026-10-08"));
        assert_eq!(streak(&bonus, MWF, d("2026-10-09")), 4);
        // With every day planned, the same history has gaps.
        assert_eq!(streak(&done, ALL_DAYS, d("2026-10-09")), 1);
    }

    #[test]
    fn schedules_can_change_and_coming_back_is_noticed() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                add(conn, "Walk", 1, ALL_DAYS, 1)?;
                let id = list(conn, d("2026-10-05"))?[0].id.clone();
                log(conn, &id, d("2026-10-05"), 1, 1)?;
                // Tuesday and Wednesday missed, back on Thursday.
                assert!(
                    !list(conn, d("2026-10-08"))?[0].came_back,
                    "not done yet today"
                );
                log(conn, &id, d("2026-10-08"), 1, 1)?;
                let view = list(conn, d("2026-10-08"))?.remove(0);
                assert!(view.came_back && view.scheduled_today);
                assert_eq!(view.streak, 1);
                // The day after a done day is just a normal day.
                log(conn, &id, d("2026-10-09"), 1, 1)?;
                assert!(!list(conn, d("2026-10-09"))?[0].came_back);

                // Switch to Mon/Wed/Fri: Saturday isn't planned, the history stays.
                update(conn, &id, "Long walk", 1, MWF, 2)?;
                let view = list(conn, d("2026-10-10"))?.remove(0);
                assert_eq!(
                    (view.name.as_str(), view.days, view.scheduled_today),
                    ("Long walk", MWF, false)
                );
                assert!(update(conn, &id, "x", 1, 0, 3).is_err());
                assert!(update(conn, "missing", "x", 1, MWF, 3).is_err());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn validates_habits() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                assert!(add(conn, " ", 1, ALL_DAYS, 1).is_err());
                assert!(add(conn, "x", 0, ALL_DAYS, 1).is_err());
                assert!(add(conn, "x", 1, 0, 1).is_err(), "no days picked");
                assert!(add(conn, "x", 1, 200, 1).is_err());
                assert!(add(conn, &"x".repeat(61), 1, ALL_DAYS, 1).is_err());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn water_adds_up_per_day_and_undo_removes_the_last_drink() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                let today = d("2026-10-09");
                add_water(conn, 250, today, 1)?;
                add_water(conn, 500, today, 2)?;
                add_water(conn, 250, d("2026-10-08"), 0)?;
                assert_eq!(water_today(conn, today)?, 750);
                undo_water(conn, today, 3)?;
                assert_eq!(water_today(conn, today)?, 250);
                assert!(add_water(conn, 5, today, 4).is_err());
                Ok(())
            })
            .unwrap();
    }
}
