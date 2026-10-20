//! Uptime pings (PRD M6).

use std::time::{Duration, Instant};

use serde::Serialize;
use ts_rs::TS;

/// A site is declared down only after this many failed checks in a row (avoids flaps).
pub const FAILURES_BEFORE_DOWN: u32 = 2;
pub const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UptimeStatus {
    pub url: String,
    /// `None` until the first check finishes.
    pub up: Option<bool>,
    #[ts(type = "number | null")]
    pub latency_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub error: Option<String>,
    #[ts(type = "number | null")]
    pub checked_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UptimeEvent {
    Down,
    Recovered,
}

/// Per-URL memory between checks.
#[derive(Debug, Default, Clone)]
pub struct Tracker {
    failures: u32,
    down: bool,
}

impl Tracker {
    /// Feeds one check result; returns a transition to report, if any.
    pub fn record(&mut self, ok: bool) -> Option<UptimeEvent> {
        if ok {
            self.failures = 0;
            return std::mem::take(&mut self.down).then_some(UptimeEvent::Recovered);
        }
        self.failures += 1;
        if !self.down && self.failures >= FAILURES_BEFORE_DOWN {
            self.down = true;
            return Some(UptimeEvent::Down);
        }
        None
    }

    pub fn is_down(&self) -> bool {
        self.down
    }
}

/// 2xx and 3xx count as up.
pub async fn check(client: &reqwest::Client, url: &str, now: i64) -> UptimeStatus {
    let started = Instant::now();
    let result = client.get(url).timeout(TIMEOUT).send().await;
    let latency_ms = Some(started.elapsed().as_millis() as u64);
    match result {
        Ok(response) => {
            let code = response.status();
            UptimeStatus {
                url: url.into(),
                up: Some(code.is_success() || code.is_redirection()),
                latency_ms,
                http_status: Some(code.as_u16()),
                error: None,
                checked_at: Some(now),
            }
        }
        Err(err) => UptimeStatus {
            url: url.into(),
            up: Some(false),
            latency_ms: None,
            http_status: None,
            error: Some(if err.is_timeout() {
                "Timed out".into()
            } else if err.is_connect() {
                "Can't connect".into()
            } else {
                "Request failed".into()
            }),
            checked_at: Some(now),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_two_failures_to_go_down_and_one_success_to_recover() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.record(true), None);
        assert_eq!(tracker.record(false), None);
        assert_eq!(tracker.record(false), Some(UptimeEvent::Down));
        assert_eq!(tracker.record(false), None, "reported once");
        assert!(tracker.is_down());
        assert_eq!(tracker.record(true), Some(UptimeEvent::Recovered));
        assert_eq!(tracker.record(true), None);
    }

    #[test]
    fn a_single_blip_is_ignored() {
        let mut tracker = Tracker::default();
        tracker.record(false);
        assert_eq!(tracker.record(true), None);
        assert!(!tracker.is_down());
    }
}
