//! Quick tasks (PRD A2). Rows are never hard-deleted: `deleted = 1` is a tombstone so the
//! deletion can sync to the phone later (Architecture.md §4.4).
//!
//! Focus sessions can belong to a task (usability U1): each task carries how many sessions
//! ran to the end for it and how long was spent focusing on it (paused time excluded).

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use ts_rs::TS;

use super::timer::SessionRecord;
use crate::foundation::errors::{PrekuError, Result};

const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub done: bool,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    /// Focus sessions for this task that ran to the end.
    pub focus_sessions: u32,
    /// Seconds focused on this task, completed or stopped early.
    pub focus_s: u32,
    /// Picked as one of today's (up to three) priorities.
    pub priority_today: bool,
    /// Optional reminder time (unix ms).
    #[ts(type = "number | null")]
    pub due_at: Option<i64>,
}

/// A reminder this late (the PC was off, or asleep) is no longer worth popping up: the task
/// just shows as overdue in the list.
pub const STALE_REMINDER_MS: i64 = 12 * 60 * 60_000;

/// What to say about tasks whose reminder time has come.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueNotice {
    /// The task to name (the one that was due first).
    pub title: String,
    /// How many other tasks became due at the same time (after sleep, a long pause…): they
    /// are mentioned as a number instead of popping up one after another.
    pub others: usize,
}

/// At most this many priorities per day: a short list is the point.
pub const MAX_PRIORITIES: usize = 3;

/// Today's recap (usability U2 / PRD A14): counts of what is stored, never a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TodaySummary {
    /// Seconds of focus recorded today (paused time excluded; early stops included).
    pub focus_s: u32,
    /// Focus sessions that ran to the end today.
    pub sessions: u32,
    pub tasks_done: u32,
    pub priorities: u32,
    pub priorities_done: u32,
}

/// The local calendar day, as stored in `tasks.today_on`.
pub fn local_day() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Unix ms of the start of the local day.
pub fn local_day_start_ms() -> i64 {
    use chrono::TimeZone;
    let midnight = chrono::Local::now()
        .date_naive()
        .and_time(chrono::NaiveTime::MIN);
    chrono::Local
        .from_local_datetime(&midnight)
        .earliest()
        .map_or(0, |time| time.timestamp_millis())
}

fn validate_title(title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(PrekuError::InvalidInput("Task can't be empty".into()));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(PrekuError::InvalidInput(format!(
            "Task is longer than {MAX_TITLE_CHARS} characters"
        )));
    }
    Ok(title.to_owned())
}

fn not_found(id: &str) -> PrekuError {
    PrekuError::InvalidInput(format!("No task with id {id}"))
}

/// Open tasks first (today's priorities on top, then the user's order), then finished
/// ones, most recent first.
pub fn list(conn: &Connection) -> Result<Vec<Task>> {
    list_on(conn, &local_day())
}

pub fn list_on(conn: &Connection, today: &str) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM tasks t
             LEFT JOIN (SELECT task_id, SUM(completed) AS n, SUM(focused_s) AS s
                        FROM focus_sessions WHERE deleted = 0 AND task_id IS NOT NULL
                        GROUP BY task_id) f ON f.task_id = t.id
             WHERE t.deleted = 0
             ORDER BY t.done,
                      CASE WHEN t.done = 0 AND t.today_on = ?1 THEN 0 ELSE 1 END,
                      CASE WHEN t.done = 0 THEN t.sort_order END,
                      t.updated_at DESC"
    ))?;
    let rows = stmt.query_map([today], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

const COLUMNS: &str =
    "t.id, t.title, t.done, t.created_at, t.updated_at, COALESCE(f.n, 0), COALESCE(f.s, 0), \
     COALESCE(t.today_on = ?1, 0), t.due_at";

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        done: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        focus_sessions: row.get(5)?,
        focus_s: row.get(6)?,
        priority_today: row.get(7)?,
        due_at: row.get(8)?,
    })
}

/// Sets (or clears) a task's reminder time; a new time means a new reminder.
pub fn set_due(conn: &Connection, id: &str, due_at: Option<i64>, now: i64) -> Result<()> {
    if due_at.is_some_and(|due| due < now - 60_000) {
        return Err(PrekuError::InvalidInput(
            "Pick a time that is still ahead".into(),
        ));
    }
    let changed = conn.execute(
        "UPDATE tasks SET due_at = ?2, due_notified = NULL, updated_at = ?3
         WHERE id = ?1 AND deleted = 0",
        params![id, due_at, now],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Open tasks whose reminder time has come and that weren't announced yet. All of them are
/// marked as announced; the result is one notice (or none if they are all too old to be
/// worth interrupting for). Never a burst.
pub fn take_due(conn: &Connection, now: i64) -> Result<Option<DueNotice>> {
    let due: Vec<(String, i64)> = conn
        .prepare(
            "SELECT title, due_at FROM tasks
             WHERE deleted = 0 AND done = 0 AND due_at IS NOT NULL AND due_at <= ?1
               AND due_notified IS NULL
             ORDER BY due_at",
        )?
        .query_map([now], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if due.is_empty() {
        return Ok(None);
    }
    conn.execute(
        "UPDATE tasks SET due_notified = ?1
         WHERE deleted = 0 AND done = 0 AND due_at IS NOT NULL AND due_at <= ?1
           AND due_notified IS NULL",
        [now],
    )?;
    let fresh: Vec<&(String, i64)> = due
        .iter()
        .filter(|(_, due_at)| now - due_at <= STALE_REMINDER_MS)
        .collect();
    Ok(fresh.first().map(|(title, _)| DueNotice {
        title: title.clone(),
        others: fresh.len() - 1,
    }))
}

/// Picks (or unpicks) a task as one of today's priorities. Yesterday's picks simply lapse.
pub fn set_priority(conn: &Connection, id: &str, on: bool, today: &str, now: i64) -> Result<()> {
    if on {
        let picked: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE deleted = 0 AND done = 0 AND today_on = ?1 AND id != ?2",
            params![today, id],
            |row| row.get(0),
        )?;
        if picked >= MAX_PRIORITIES {
            return Err(PrekuError::InvalidInput(format!(
                "You've picked {MAX_PRIORITIES} priorities for today. Unpick one first, or finish one"
            )));
        }
    }
    let changed = conn.execute(
        "UPDATE tasks SET today_on = ?2, updated_at = ?3 WHERE id = ?1 AND deleted = 0",
        params![id, on.then_some(today), now],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// What was recorded since `day_start` (unix ms), for the Today recap.
pub fn summary(conn: &Connection, today: &str, day_start: i64) -> Result<TodaySummary> {
    let (focus_s, sessions) = conn.query_row(
        "SELECT COALESCE(SUM(focused_s), 0), COALESCE(SUM(completed), 0) FROM focus_sessions
         WHERE deleted = 0 AND started_at >= ?1",
        [day_start],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let tasks_done = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE deleted = 0 AND done = 1 AND done_at >= ?1",
        [day_start],
        |row| row.get(0),
    )?;
    let (priorities, priorities_done) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(done), 0) FROM tasks WHERE deleted = 0 AND today_on = ?1",
        [today],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(TodaySummary {
        focus_s,
        sessions,
        tasks_done,
        priorities,
        priorities_done,
    })
}

/// An open, not deleted task (to start focusing on it).
pub fn find_open(conn: &Connection, id: &str) -> Result<Task> {
    list(conn)?
        .into_iter()
        .find(|task| task.id == id && !task.done)
        .ok_or_else(|| PrekuError::InvalidInput("That task is done or no longer exists".into()))
}

/// Stores a focus session (for a task or not). Sessions shorter than a minute that were
/// stopped early aren't worth keeping.
pub fn save_session(conn: &Connection, record: &SessionRecord, now: i64) -> Result<bool> {
    if record.focused_s < 60 && !record.completed {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO focus_sessions
           (id, label, task_id, started_at, duration_s, focused_s, completed, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            uuid::Uuid::new_v4().to_string(),
            record.label,
            record.task_id,
            record.started_at,
            record.duration_s,
            record.focused_s,
            record.completed,
            now
        ],
    )?;
    Ok(true)
}

/// New tasks go to the top of the open list.
pub fn add(conn: &Connection, title: &str, now: i64) -> Result<Task> {
    let title = validate_title(title)?;
    let top: Option<i64> = conn
        .query_row(
            "SELECT MIN(sort_order) FROM tasks WHERE deleted = 0 AND done = 0",
            [],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        done: false,
        created_at: now,
        updated_at: now,
        focus_sessions: 0,
        focus_s: 0,
        priority_today: false,
        due_at: None,
    };
    conn.execute(
        "INSERT INTO tasks (id, title, done, sort_order, created_at, updated_at)
         VALUES (?1, ?2, 0, ?3, ?4, ?4)",
        params![task.id, task.title, top.unwrap_or(0) - 1, now],
    )?;
    Ok(task)
}

pub fn rename(conn: &Connection, id: &str, title: &str, now: i64) -> Result<()> {
    let title = validate_title(title)?;
    let changed = conn.execute(
        "UPDATE tasks SET title = ?2, updated_at = ?3 WHERE id = ?1 AND deleted = 0",
        params![id, title, now],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

pub fn set_done(conn: &Connection, id: &str, done: bool, now: i64) -> Result<()> {
    let changed = conn.execute(
        "UPDATE tasks SET done = ?2, done_at = ?4, updated_at = ?3 WHERE id = ?1 AND deleted = 0",
        params![id, done, now, done.then_some(now)],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET deleted = 1, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    Ok(())
}

/// Sets the order of open tasks to `ids` (first = top). Unknown ids are ignored.
pub fn reorder(conn: &mut Connection, ids: &[String], now: i64) -> Result<()> {
    let tx = conn.transaction()?;
    for (index, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE tasks SET sort_order = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted = 0 AND done = 0",
            params![id, index as i64, now],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// The task shown in the compact pill.
pub fn top_open(conn: &Connection) -> Result<Option<Task>> {
    Ok(list(conn)?.into_iter().find(|task| !task.done))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::storage::Storage;

    fn with_db(test: impl FnOnce(&mut Connection)) {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                test(conn);
                Ok(())
            })
            .unwrap();
    }

    fn titles(conn: &Connection) -> Vec<String> {
        list(conn).unwrap().into_iter().map(|t| t.title).collect()
    }

    #[test]
    fn new_tasks_go_on_top() {
        with_db(|conn| {
            add(conn, "first", 1).unwrap();
            add(conn, "  second  ", 2).unwrap();
            assert_eq!(titles(conn), ["second", "first"]);
            assert_eq!(top_open(conn).unwrap().unwrap().title, "second");
        });
    }

    #[test]
    fn done_tasks_sink_and_deleted_tasks_disappear() {
        with_db(|conn| {
            let a = add(conn, "a", 1).unwrap();
            let b = add(conn, "b", 2).unwrap();
            add(conn, "c", 3).unwrap();
            set_done(conn, &b.id, true, 4).unwrap();
            delete(conn, &a.id, 5).unwrap();
            assert_eq!(titles(conn), ["c", "b"]);
            assert_eq!(top_open(conn).unwrap().unwrap().title, "c");

            // Tombstone stays for sync.
            let deleted: i64 = conn
                .query_row("SELECT deleted FROM tasks WHERE id = ?1", [&a.id], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(deleted, 1);
        });
    }

    #[test]
    fn reorder_and_rename() {
        with_db(|conn| {
            let a = add(conn, "a", 1).unwrap();
            let b = add(conn, "b", 2).unwrap();
            let c = add(conn, "c", 3).unwrap();
            reorder(conn, &[a.id.clone(), c.id.clone(), b.id.clone()], 4).unwrap();
            assert_eq!(titles(conn), ["a", "c", "b"]);

            rename(conn, &c.id, "see", 5).unwrap();
            assert_eq!(titles(conn), ["a", "see", "b"]);
        });
    }

    fn session(task_id: Option<&str>, focused_s: u32, completed: bool) -> SessionRecord {
        SessionRecord {
            label: "label at the time".into(),
            task_id: task_id.map(Into::into),
            started_at: 1,
            duration_s: 1500,
            focused_s,
            completed,
        }
    }

    #[test]
    fn tasks_count_their_focus_sessions() {
        with_db(|conn| {
            let a = add(conn, "essay", 1).unwrap();
            let b = add(conn, "slides", 2).unwrap();
            assert!(save_session(conn, &session(Some(&a.id), 1500, true), 3).unwrap());
            assert!(save_session(conn, &session(Some(&a.id), 600, false), 4).unwrap());
            assert!(save_session(conn, &session(None, 1500, true), 5).unwrap());
            // Stopped within a minute: not kept.
            assert!(!save_session(conn, &session(Some(&b.id), 30, false), 6).unwrap());

            let find = |id: &str| {
                list(conn)
                    .unwrap()
                    .into_iter()
                    .find(|t| t.id == id)
                    .unwrap()
            };
            let essay = find(&a.id);
            assert_eq!((essay.focus_sessions, essay.focus_s), (1, 2100));
            assert_eq!((find(&b.id).focus_sessions, find(&b.id).focus_s), (0, 0));

            // Renaming keeps the history; the session keeps the label it ran under.
            rename(conn, &a.id, "essay draft", 7).unwrap();
            assert_eq!(find(&a.id).focus_s, 2100);
            let label: String = conn
                .query_row(
                    "SELECT label FROM focus_sessions WHERE task_id = ?1 LIMIT 1",
                    [&a.id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(label, "label at the time");

            // Deleting the task keeps its sessions.
            delete(conn, &a.id, 8).unwrap();
            let kept: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM focus_sessions WHERE task_id = ?1",
                    [&a.id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(kept, 2);
        });
    }

    #[test]
    fn up_to_three_priorities_for_the_day_and_they_lapse_tomorrow() {
        with_db(|conn| {
            let ids: Vec<String> = ["a", "b", "c", "d"]
                .iter()
                .enumerate()
                .map(|(i, title)| add(conn, title, i as i64).unwrap().id)
                .collect();
            let (mon, tue) = ("2026-10-12", "2026-10-13");
            for id in &ids[..3] {
                set_priority(conn, id, true, mon, 10).unwrap();
            }
            assert!(
                set_priority(conn, &ids[3], true, mon, 11).is_err(),
                "a fourth is refused"
            );
            // Picking one that is already picked is fine.
            set_priority(conn, &ids[0], true, mon, 12).unwrap();

            // Priorities come first; the rest keep the user's order (newest on top).
            let order: Vec<_> = list_on(conn, mon)
                .unwrap()
                .into_iter()
                .map(|t| (t.title, t.priority_today))
                .collect();
            assert_eq!(order[3], ("d".to_owned(), false));
            assert!(order[..3].iter().all(|(_, picked)| *picked));

            // Finishing one frees a slot.
            set_done(conn, &ids[0], true, 20).unwrap();
            set_priority(conn, &ids[3], true, mon, 21).unwrap();
            assert_eq!(
                summary(conn, mon, 0).unwrap(),
                TodaySummary {
                    focus_s: 0,
                    sessions: 0,
                    tasks_done: 1,
                    priorities: 4,
                    priorities_done: 1
                }
            );

            // Tomorrow nothing is a priority any more; the tasks are all still there.
            let tomorrow = list_on(conn, tue).unwrap();
            assert_eq!(tomorrow.len(), 4);
            assert!(tomorrow.iter().all(|t| !t.priority_today));
            assert_eq!(summary(conn, tue, 0).unwrap().priorities, 0);

            set_priority(conn, &ids[1], false, mon, 30).unwrap();
            assert!(!list_on(conn, mon)
                .unwrap()
                .iter()
                .any(|t| t.id == ids[1] && t.priority_today));
            assert!(set_priority(conn, "missing", true, mon, 31).is_err());
        });
    }

    #[test]
    fn the_recap_counts_only_what_was_recorded_today() {
        with_db(|conn| {
            let a = add(conn, "a", 1).unwrap();
            let day_start = 1_000_000;
            // Yesterday: a finished task and a session.
            set_done(conn, &a.id, true, day_start - 5).unwrap();
            let mut old = session(None, 1500, true);
            old.started_at = day_start - 10_000;
            save_session(conn, &old, day_start - 1).unwrap();
            assert_eq!(
                summary(conn, "2026-10-12", day_start).unwrap(),
                TodaySummary::default()
            );

            // Today: one completed session, one stopped early, one task done.
            for (focused, completed) in [(1500, true), (600, false)] {
                let mut record = session(None, focused, completed);
                record.started_at = day_start + 1;
                save_session(conn, &record, day_start + 2).unwrap();
            }
            let b = add(conn, "b", day_start + 3).unwrap();
            set_done(conn, &b.id, true, day_start + 4).unwrap();
            let recap = summary(conn, "2026-10-12", day_start).unwrap();
            assert_eq!(
                (recap.focus_s, recap.sessions, recap.tasks_done),
                (2100, 1, 1)
            );
            // Reopening a task takes it out of the count.
            set_done(conn, &b.id, false, day_start + 5).unwrap();
            assert_eq!(
                summary(conn, "2026-10-12", day_start).unwrap().tasks_done,
                0
            );
        });
    }

    #[test]
    fn a_task_reminder_is_given_once_and_never_as_a_burst() {
        with_db(|conn| {
            let hour = 60 * 60_000;
            let now = 100 * hour;
            let a = add(conn, "Call the bank", now).unwrap();
            let b = add(conn, "Send the invoice", now).unwrap();
            let c = add(conn, "Old errand", now).unwrap();
            assert!(
                set_due(conn, &a.id, Some(now - hour), now).is_err(),
                "a time in the past"
            );
            set_due(conn, &a.id, Some(now + hour), now).unwrap();
            assert_eq!(take_due(conn, now + hour - 1).unwrap(), None, "not yet");

            // Due: said once.
            let notice = take_due(conn, now + hour).unwrap().unwrap();
            assert_eq!(
                notice,
                DueNotice {
                    title: "Call the bank".into(),
                    others: 0
                }
            );
            assert_eq!(take_due(conn, now + hour + 5_000).unwrap(), None);
            assert_eq!(
                list(conn)
                    .unwrap()
                    .iter()
                    .find(|t| t.id == a.id)
                    .unwrap()
                    .due_at,
                Some(now + hour)
            );

            // "Later": a new time gives a new reminder.
            set_due(conn, &a.id, Some(now + 2 * hour), now + hour).unwrap();
            // The PC slept through three reminders: one notice that mentions the others, and
            // one that is far too old is left out of the count.
            set_due(conn, &b.id, Some(now + 3 * hour), now).unwrap();
            conn.execute(
                "UPDATE tasks SET due_at = ?2 WHERE id = ?1",
                params![c.id, now + hour],
            )
            .unwrap();
            let wake = now + hour + STALE_REMINDER_MS + hour / 2;
            let notice = take_due(conn, wake).unwrap().unwrap();
            assert_eq!(
                notice,
                DueNotice {
                    title: "Call the bank".into(),
                    others: 1
                }
            );
            assert_eq!(
                take_due(conn, wake + 1).unwrap(),
                None,
                "nothing more follows"
            );

            // Finished tasks and cleared reminders stay silent.
            let d = add(conn, "Done already", wake).unwrap();
            set_due(conn, &d.id, Some(wake + hour), wake).unwrap();
            set_done(conn, &d.id, true, wake + 1).unwrap();
            set_due(conn, &b.id, None, wake).unwrap();
            assert_eq!(take_due(conn, wake + 2 * hour).unwrap(), None);
            assert!(set_due(conn, "missing", None, wake).is_err());
        });
    }

    #[test]
    fn only_open_tasks_can_be_focused_on() {
        with_db(|conn| {
            let a = add(conn, "a", 1).unwrap();
            assert_eq!(find_open(conn, &a.id).unwrap().title, "a");
            set_done(conn, &a.id, true, 2).unwrap();
            assert!(find_open(conn, &a.id).is_err());
            assert!(find_open(conn, "missing").is_err());
        });
    }

    #[test]
    fn validates_titles_and_ids() {
        with_db(|conn| {
            assert!(add(conn, "   ", 1).is_err());
            assert!(add(conn, &"x".repeat(201), 1).is_err());
            assert!(set_done(conn, "missing", true, 1).is_err());
            assert!(rename(conn, "missing", "x", 1).is_err());
        });
    }
}
