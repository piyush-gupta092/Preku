//! Focus timer state machine (PRD A1). Pure: every method takes `now` in unix ms, so it is
//! fully testable. The module's tick task drives it and is the single source of truth.

use serde::Serialize;
use ts_rs::TS;

use crate::foundation::errors::{PrekuError, Result};

pub const MIN_MINUTES: u32 = 1;
pub const MAX_MINUTES: u32 = 180;
const MAX_LABEL_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TimerStatus {
    Idle,
    Running,
    Paused,
}

/// What the UI renders.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimerSnapshot {
    pub status: TimerStatus,
    pub label: String,
    pub total_s: u32,
    pub remaining_s: u32,
    /// Unix ms when a running timer ends (lets the UI count down smoothly between ticks).
    #[ts(type = "number | null")]
    pub ends_at: Option<i64>,
    /// The task this session is for, if it was started from one.
    pub task_id: Option<String>,
    /// The session that just ran out, until the user answers it or starts another.
    pub finished: Option<FinishedFocus>,
}

/// A session that ran to the end: the UI offers Mark done / Continue / Take a break. Never
/// acted on automatically.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FinishedFocus {
    pub label: String,
    pub task_id: Option<String>,
    /// Planned length, for "Continue" with the same length.
    pub total_s: u32,
    pub focused_s: u32,
}

/// A finished or abandoned session, to be stored in `focus_sessions`.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub label: String,
    pub task_id: Option<String>,
    pub started_at: i64,
    pub duration_s: u32,
    pub focused_s: u32,
    pub completed: bool,
}

#[derive(Debug, Clone)]
struct Session {
    label: String,
    task_id: Option<String>,
    total_ms: i64,
    started_at: i64,
    /// Running time accumulated before the current run segment.
    focused_ms: i64,
}

#[derive(Debug, Clone)]
enum State {
    Idle,
    Running {
        session: Session,
        segment_start: i64,
    },
    Paused {
        session: Session,
    },
}

#[derive(Debug)]
pub struct FocusTimer {
    state: State,
    finished: Option<FinishedFocus>,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            state: State::Idle,
            finished: None,
        }
    }
}

fn remaining_ms(session: &Session, running_since: Option<i64>, now: i64) -> i64 {
    let focused = session.focused_ms + running_since.map_or(0, |start| now - start);
    (session.total_ms - focused).max(0)
}

fn to_secs_ceil(ms: i64) -> u32 {
    u32::try_from((ms + 999) / 1000).unwrap_or(u32::MAX)
}

pub fn normalize_label(label: &str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        "Focus".to_owned()
    } else {
        trimmed.chars().take(MAX_LABEL_CHARS).collect()
    }
}

impl FocusTimer {
    /// Starts a new session, optionally for a task. Any session in progress is abandoned and
    /// returned for storage.
    pub fn start(
        &mut self,
        label: &str,
        task_id: Option<String>,
        minutes: u32,
        now: i64,
    ) -> Result<Option<SessionRecord>> {
        if !(MIN_MINUTES..=MAX_MINUTES).contains(&minutes) {
            return Err(PrekuError::InvalidInput(format!(
                "Timer length must be {MIN_MINUTES}-{MAX_MINUTES} minutes"
            )));
        }
        let abandoned = self.stop(now);
        self.finished = None;
        self.state = State::Running {
            session: Session {
                label: normalize_label(label),
                task_id,
                total_ms: i64::from(minutes) * 60_000,
                started_at: now,
                focused_ms: 0,
            },
            segment_start: now,
        };
        Ok(abandoned)
    }

    pub fn pause(&mut self, now: i64) {
        if let State::Running {
            session,
            segment_start,
        } = &self.state
        {
            let mut session = session.clone();
            session.focused_ms += now - segment_start;
            self.state = State::Paused { session };
        }
    }

    pub fn resume(&mut self, now: i64) {
        if let State::Paused { session } = &self.state {
            self.state = State::Running {
                session: session.clone(),
                segment_start: now,
            };
        }
    }

    /// Ends the session early. Returns it (not completed) if one was in progress.
    pub fn stop(&mut self, now: i64) -> Option<SessionRecord> {
        let record = match &self.state {
            State::Idle => None,
            State::Running {
                session,
                segment_start,
            } => Some(record(session, Some(*segment_start), now, false)),
            State::Paused { session } => Some(record(session, None, now, false)),
        };
        self.state = State::Idle;
        record
    }

    /// Advances time. Returns the completed session when the timer runs out.
    pub fn tick(&mut self, now: i64) -> Option<SessionRecord> {
        if let State::Running {
            session,
            segment_start,
        } = &self.state
        {
            if remaining_ms(session, Some(*segment_start), now) == 0 {
                let done = record(session, Some(*segment_start), now, true);
                self.finished = Some(FinishedFocus {
                    label: done.label.clone(),
                    task_id: done.task_id.clone(),
                    total_s: done.duration_s,
                    focused_s: done.focused_s,
                });
                self.state = State::Idle;
                return Some(done);
            }
        }
        None
    }

    /// The user answered the finished card (or closed it).
    pub fn dismiss_finished(&mut self) {
        self.finished = None;
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    pub fn snapshot(&self, now: i64) -> TimerSnapshot {
        match &self.state {
            State::Idle => TimerSnapshot {
                status: TimerStatus::Idle,
                label: String::new(),
                total_s: 0,
                remaining_s: 0,
                ends_at: None,
                task_id: None,
                finished: self.finished.clone(),
            },
            State::Running {
                session,
                segment_start,
            } => {
                let remaining = remaining_ms(session, Some(*segment_start), now);
                TimerSnapshot {
                    status: TimerStatus::Running,
                    label: session.label.clone(),
                    total_s: to_secs_ceil(session.total_ms),
                    remaining_s: to_secs_ceil(remaining),
                    ends_at: Some(now + remaining),
                    task_id: session.task_id.clone(),
                    finished: None,
                }
            }
            State::Paused { session } => TimerSnapshot {
                status: TimerStatus::Paused,
                label: session.label.clone(),
                total_s: to_secs_ceil(session.total_ms),
                remaining_s: to_secs_ceil(remaining_ms(session, None, now)),
                ends_at: None,
                task_id: session.task_id.clone(),
                finished: None,
            },
        }
    }
}

fn record(
    session: &Session,
    running_since: Option<i64>,
    now: i64,
    completed: bool,
) -> SessionRecord {
    let focused = session.focused_ms + running_since.map_or(0, |start| now - start);
    SessionRecord {
        label: session.label.clone(),
        task_id: session.task_id.clone(),
        started_at: session.started_at,
        duration_s: to_secs_ceil(session.total_ms),
        focused_s: to_secs_ceil(focused.min(session.total_ms)),
        completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    #[test]
    fn counts_down_and_completes() {
        let mut timer = FocusTimer::default();
        timer.start("Write docs", None, 25, 0).unwrap();
        let snap = timer.snapshot(10 * MIN);
        assert_eq!(snap.status, TimerStatus::Running);
        assert_eq!(snap.remaining_s, 15 * 60);
        assert_eq!(snap.ends_at, Some(25 * MIN));

        assert_eq!(timer.tick(25 * MIN - 1), None);
        let done = timer.tick(25 * MIN).unwrap();
        assert!(done.completed);
        assert_eq!(done.focused_s, 25 * 60);
        assert_eq!(done.label, "Write docs");
        assert!(!timer.is_active());
    }

    #[test]
    fn pause_freezes_the_countdown() {
        let mut timer = FocusTimer::default();
        timer.start("", None, 10, 0).unwrap();
        timer.pause(4 * MIN);
        assert_eq!(timer.snapshot(9 * MIN).remaining_s, 6 * 60);
        assert_eq!(timer.tick(60 * MIN), None, "paused timers never finish");

        timer.resume(20 * MIN);
        assert_eq!(timer.snapshot(21 * MIN).remaining_s, 5 * 60);
        let done = timer.tick(26 * MIN).unwrap();
        assert_eq!(done.focused_s, 10 * 60);
        assert_eq!(done.label, "Focus", "empty label falls back to Focus");
    }

    #[test]
    fn stopping_early_records_an_incomplete_session() {
        let mut timer = FocusTimer::default();
        timer.start("Read", None, 30, 0).unwrap();
        let record = timer.stop(12 * MIN).unwrap();
        assert!(!record.completed);
        assert_eq!(record.focused_s, 12 * 60);
        assert_eq!(timer.stop(13 * MIN), None);
    }

    #[test]
    fn starting_again_abandons_the_current_session() {
        let mut timer = FocusTimer::default();
        timer.start("A", None, 25, 0).unwrap();
        let abandoned = timer.start("B", None, 5, MIN).unwrap().unwrap();
        assert_eq!(abandoned.label, "A");
        assert_eq!(timer.snapshot(MIN).label, "B");
    }

    #[test]
    fn rejects_out_of_range_lengths() {
        let mut timer = FocusTimer::default();
        assert!(timer.start("x", None, 0, 0).is_err());
        assert!(timer.start("x", None, 181, 0).is_err());
        assert!(!timer.is_active());
    }

    #[test]
    fn a_task_session_remembers_its_task_and_offers_a_finish_card() {
        let mut timer = FocusTimer::default();
        timer.start("Essay", Some("t1".into()), 25, 0).unwrap();
        assert_eq!(timer.snapshot(MIN).task_id.as_deref(), Some("t1"));
        assert_eq!(timer.snapshot(MIN).finished, None);

        let done = timer.tick(25 * MIN).unwrap();
        assert_eq!(done.task_id.as_deref(), Some("t1"));
        let finished = timer.snapshot(25 * MIN).finished.unwrap();
        assert_eq!(finished.task_id.as_deref(), Some("t1"));
        assert_eq!((finished.total_s, finished.focused_s), (1500, 1500));

        // It stays until answered, and a new session replaces it.
        assert!(timer.snapshot(40 * MIN).finished.is_some());
        timer.dismiss_finished();
        assert_eq!(timer.snapshot(40 * MIN).finished, None);
        timer.tick(25 * MIN);
        timer
            .start("Essay", Some("t1".into()), 1, 50 * MIN)
            .unwrap();
        assert_eq!(timer.tick(51 * MIN).unwrap().task_id.as_deref(), Some("t1"));
        timer.start("Next", None, 5, 52 * MIN).unwrap();
        assert_eq!(timer.snapshot(52 * MIN).finished, None);
    }

    #[test]
    fn stopping_early_offers_no_finish_card() {
        let mut timer = FocusTimer::default();
        timer.start("Essay", Some("t1".into()), 25, 0).unwrap();
        let record = timer.stop(10 * MIN).unwrap();
        assert!(!record.completed);
        assert_eq!(record.task_id.as_deref(), Some("t1"));
        assert_eq!(timer.snapshot(10 * MIN).finished, None);
    }

    #[test]
    fn rounds_remaining_seconds_up() {
        let mut timer = FocusTimer::default();
        timer.start("x", None, 1, 0).unwrap();
        assert_eq!(timer.snapshot(59_001).remaining_s, 1);
    }
}
