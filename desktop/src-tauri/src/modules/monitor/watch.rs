//! `preku watch <command>` (PRD M7): the CLI (`/cli`) reports command start/finish over a
//! per-user local channel (named pipe on Windows, unix socket on Linux), one JSON per line.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Longest accepted line; anything bigger is dropped (a command line, not a payload).
pub const MAX_LINE: usize = 8 * 1024;
const MAX_COMMAND_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum WatchMessage {
    #[serde(rename_all = "camelCase")]
    Started { id: String, command: String },
    #[serde(rename_all = "camelCase")]
    Finished {
        id: String,
        code: i32,
        duration_ms: u64,
    },
}

/// Parses one line from the CLI. Unknown or oversized messages are rejected.
pub fn parse(line: &str) -> Option<WatchMessage> {
    if line.len() > MAX_LINE {
        return None;
    }
    let message: WatchMessage = serde_json::from_str(line).ok()?;
    let valid_id = |id: &str| !id.is_empty() && id.len() <= 64;
    match &message {
        WatchMessage::Started { id, .. } | WatchMessage::Finished { id, .. } if valid_id(id) => {
            Some(message)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WatchState {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WatchRecord {
    pub id: String,
    pub command: String,
    pub state: WatchState,
    pub code: Option<i32>,
    #[ts(type = "number")]
    pub started_at: i64,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
}

/// Shortens long command lines for display.
pub fn display_command(command: &str) -> String {
    let command = command.trim();
    if command.chars().count() <= MAX_COMMAND_CHARS {
        return command.to_owned();
    }
    let cut: String = command.chars().take(MAX_COMMAND_CHARS - 1).collect();
    format!("{cut}…")
}

/// `1h 02m`, `3m 07s`, `12s`.
pub fn human_duration(ms: u64) -> String {
    let s = ms / 1000;
    match (s / 3600, (s / 60) % 60, s % 60) {
        (0, 0, sec) => format!("{sec}s"),
        (0, min, sec) => format!("{min}m {sec:02}s"),
        (h, min, _) => format!("{h}h {min:02}m"),
    }
}

/// Keeps the last few watches for the Dev view.
#[derive(Debug, Default)]
pub struct WatchLog(Vec<WatchRecord>);

const LOG_LEN: usize = 10;

impl WatchLog {
    pub fn apply(&mut self, message: &WatchMessage, now: i64) -> Option<WatchRecord> {
        match message {
            WatchMessage::Started { id, command } => {
                let record = WatchRecord {
                    id: id.clone(),
                    command: display_command(command),
                    state: WatchState::Running,
                    code: None,
                    started_at: now,
                    duration_ms: None,
                };
                self.0.insert(0, record.clone());
                self.0.truncate(LOG_LEN);
                Some(record)
            }
            WatchMessage::Finished {
                id,
                code,
                duration_ms,
            } => {
                let record = self.0.iter_mut().find(|r| &r.id == id)?;
                record.state = if *code == 0 {
                    WatchState::Succeeded
                } else {
                    WatchState::Failed
                };
                record.code = Some(*code);
                record.duration_ms = Some(*duration_ms);
                Some(record.clone())
            }
        }
    }

    pub fn records(&self) -> Vec<WatchRecord> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cli_messages() {
        assert_eq!(
            parse(r#"{"event":"started","id":"a1","command":"cargo build"}"#),
            Some(WatchMessage::Started {
                id: "a1".into(),
                command: "cargo build".into()
            })
        );
        assert_eq!(
            parse(r#"{"event":"finished","id":"a1","code":101,"durationMs":5300}"#),
            Some(WatchMessage::Finished {
                id: "a1".into(),
                code: 101,
                duration_ms: 5300
            })
        );
        assert_eq!(parse(r#"{"event":"rm -rf","id":"x"}"#), None);
        assert_eq!(parse(r#"{"event":"started","id":"","command":"x"}"#), None);
        assert_eq!(parse(&"x".repeat(MAX_LINE + 1)), None);
    }

    #[test]
    fn log_tracks_start_and_finish() {
        let mut log = WatchLog::default();
        log.apply(
            &WatchMessage::Started {
                id: "1".into(),
                command: "npm test".into(),
            },
            100,
        );
        let done = log
            .apply(
                &WatchMessage::Finished {
                    id: "1".into(),
                    code: 1,
                    duration_ms: 65_000,
                },
                200,
            )
            .unwrap();
        assert_eq!(done.state, WatchState::Failed);
        assert_eq!(human_duration(65_000), "1m 05s");
        assert!(log
            .apply(
                &WatchMessage::Finished {
                    id: "unknown".into(),
                    code: 0,
                    duration_ms: 1
                },
                300
            )
            .is_none());
    }

    #[test]
    fn formats() {
        assert_eq!(human_duration(9_400), "9s");
        assert_eq!(human_duration(3_725_000), "1h 02m");
        assert_eq!(display_command(&"a".repeat(200)).chars().count(), 120);
    }
}
