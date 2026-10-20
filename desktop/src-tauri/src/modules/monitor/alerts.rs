//! Threshold alerts (PRD M2): "CPU above 90% for 30 s" style rules, evaluated on every sample.

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::metrics::MetricsSample;
use crate::foundation::errors::{PrekuError, Result};

/// A firing alert only clears once the value is this many points back on the safe side,
/// so a value hovering at the threshold doesn't flap.
const HYSTERESIS: f32 = 2.0;
const MAX_DURATION_S: u32 = 3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Metric {
    /// % of all cores.
    Cpu,
    /// % of RAM in use.
    Memory,
    /// % of swap / page file in use.
    Swap,
    /// % used on the fullest fixed disk.
    Disk,
    /// GB free on the fullest fixed disk.
    DiskFree,
    /// % left, only while unplugged.
    Battery,
    /// Download speed, MB/s.
    NetDown,
    /// Upload speed, MB/s.
    NetUp,
    /// Hottest CPU sensor, °C (if the OS exposes one).
    Temperature,
}

impl Metric {
    pub const ALL: [Metric; 9] = [
        Metric::Cpu,
        Metric::Memory,
        Metric::Swap,
        Metric::Disk,
        Metric::DiskFree,
        Metric::Battery,
        Metric::NetDown,
        Metric::NetUp,
        Metric::Temperature,
    ];

    /// Unit shown next to values; language-neutral.
    pub fn unit(self) -> &'static str {
        match self {
            Self::DiskFree => "GB",
            Self::NetDown | Self::NetUp => "MB/s",
            Self::Temperature => "°C",
            _ => "%",
        }
    }

    /// Largest sensible threshold, for validation.
    fn max_threshold(self) -> f32 {
        match self {
            Self::DiskFree | Self::NetDown | Self::NetUp => 100_000.0,
            Self::Temperature => 150.0,
            _ => 100.0,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Swap => "swap",
            Self::Disk => "disk",
            Self::DiskFree => "diskFree",
            Self::Battery => "battery",
            Self::NetDown => "netDown",
            Self::NetUp => "netUp",
            Self::Temperature => "temperature",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
}

impl Metric {
    /// Worth interrupting focus or a pause for: something the user must act on soon.
    pub fn is_urgent(self) -> bool {
        matches!(
            self,
            Self::Battery | Self::Disk | Self::DiskFree | Self::Temperature
        )
    }
}

/// What the notch says when a rule fires: a plain problem and the number that matters,
/// e.g. ("Disk space low", "4 GB left").
pub fn alert_text(metric: Metric, op: Comparison, value: f32) -> (String, String) {
    let (title, detail) = match (metric, op) {
        (Metric::DiskFree, Comparison::Below) => ("Disk space low", format!("{value:.0} GB left")),
        (Metric::Disk, Comparison::Above) => ("Disk almost full", format!("{value:.0}% used")),
        (Metric::Battery, Comparison::Below) => ("Battery low", format!("{value:.0}% left")),
        (Metric::Cpu, Comparison::Above) => ("CPU busy for a while", format!("{value:.0}%")),
        (Metric::Memory, Comparison::Above) => ("Memory almost full", format!("{value:.0}% used")),
        (Metric::Swap, Comparison::Above) => ("Swap filling up", format!("{value:.0}% used")),
        (Metric::Temperature, Comparison::Above) => ("Running hot", format!("{value:.0} °C")),
        (Metric::NetDown, Comparison::Above) => ("Heavy download", format!("{value:.0} MB/s")),
        (Metric::NetUp, Comparison::Above) => ("Heavy upload", format!("{value:.0} MB/s")),
        // An unusual direction (e.g. "CPU below 5%"): say what was measured.
        (metric, op) => {
            let side = if op == Comparison::Above {
                "above"
            } else {
                "below"
            };
            return (
                format!("{} {side} your limit", metric.key()),
                format!("{value:.0} {}", metric.unit()),
            );
        }
    };
    (title.to_owned(), detail)
}

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
const MIB: f64 = 1024.0 * 1024.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Comparison {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AlertRule {
    pub id: String,
    pub metric: Metric,
    pub op: Comparison,
    /// Percent (0–100) for every metric.
    pub threshold: f32,
    /// How long the condition must hold before firing; 0 = immediately.
    pub duration_s: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlertEvent {
    Fired { rule: AlertRule, value: f32 },
    Resolved { rule_id: String },
}

/// The metric's value in percent, or `None` when it doesn't apply (no battery, or the
/// battery is charging — a low battery on the charger isn't worth an alert).
pub fn metric_value(sample: &MetricsSample, metric: Metric) -> Option<f32> {
    match metric {
        Metric::Cpu => Some(sample.cpu),
        Metric::Memory => Some(sample.memory_pct()),
        Metric::Disk => (sample.disk_total > 0).then(|| sample.disk_pct()),
        Metric::Battery => sample
            .battery
            .filter(|b| !b.charging)
            .map(|b| f32::from(b.percent)),
        Metric::Swap => sample.swap_pct(),
        Metric::DiskFree => (sample.disk_total > 0)
            .then(|| (sample.disk_total.saturating_sub(sample.disk_used) as f64 / GIB) as f32),
        Metric::NetDown => Some((sample.net_rx as f64 / MIB) as f32),
        Metric::NetUp => Some((sample.net_tx as f64 / MIB) as f32),
        Metric::Temperature => sample.temperature,
    }
}

#[derive(Debug, Default, Clone)]
struct RuleState {
    breach_since: Option<i64>,
    firing: bool,
    /// When the user was last told (for "remind me again while it lasts").
    last_fired: i64,
}

/// Remembers, per rule, how long its condition has held and whether it is firing.
#[derive(Debug, Default)]
pub struct AlertEngine {
    /// Say it again this often while a problem lasts (0 = say it once).
    repeat_ms: i64,
    states: HashMap<String, RuleState>,
}

impl AlertEngine {
    pub fn set_repeat_min(&mut self, minutes: u32) {
        self.repeat_ms = i64::from(minutes) * 60_000;
    }

    pub fn evaluate(&mut self, rules: &[AlertRule], sample: &MetricsSample) -> Vec<AlertEvent> {
        let repeat_ms = self.repeat_ms;
        let mut events = Vec::new();
        // Forget rules that were deleted or disabled; resolve them if they were firing.
        self.states.retain(|id, state| {
            let alive = rules.iter().any(|r| &r.id == id && r.enabled);
            if !alive && state.firing {
                events.push(AlertEvent::Resolved {
                    rule_id: id.clone(),
                });
            }
            alive
        });

        for rule in rules.iter().filter(|r| r.enabled) {
            let state = self.states.entry(rule.id.clone()).or_default();
            let Some(value) = metric_value(sample, rule.metric) else {
                state.breach_since = None;
                if std::mem::take(&mut state.firing) {
                    events.push(AlertEvent::Resolved {
                        rule_id: rule.id.clone(),
                    });
                }
                continue;
            };
            let breached = match rule.op {
                Comparison::Above => value > rule.threshold,
                Comparison::Below => value < rule.threshold,
            };
            let recovered = match rule.op {
                Comparison::Above => value <= rule.threshold - HYSTERESIS,
                Comparison::Below => value >= rule.threshold + HYSTERESIS,
            };

            if breached {
                let since = *state.breach_since.get_or_insert(sample.ts);
                let held_ms = sample.ts - since;
                let again =
                    state.firing && repeat_ms > 0 && sample.ts - state.last_fired >= repeat_ms;
                if (!state.firing && held_ms >= i64::from(rule.duration_s) * 1000) || again {
                    state.firing = true;
                    state.last_fired = sample.ts;
                    events.push(AlertEvent::Fired {
                        rule: rule.clone(),
                        value,
                    });
                }
            } else {
                state.breach_since = None;
                if state.firing && recovered {
                    state.firing = false;
                    events.push(AlertEvent::Resolved {
                        rule_id: rule.id.clone(),
                    });
                }
            }
        }
        events
    }
}

pub fn validate(rule: &AlertRule) -> Result<()> {
    let max = rule.metric.max_threshold();
    if !(0.0..=max).contains(&rule.threshold) {
        return Err(PrekuError::InvalidInput(format!(
            "Threshold must be 0-{max} {}",
            rule.metric.unit()
        )));
    }
    if rule.duration_s > MAX_DURATION_S {
        return Err(PrekuError::InvalidInput(
            "Duration can be at most one hour".into(),
        ));
    }
    Ok(())
}

fn metric_from_str(value: &str) -> Metric {
    Metric::from_key(value).unwrap_or(Metric::Cpu)
}

fn metric_to_str(metric: Metric) -> &'static str {
    metric.key()
}

pub fn list(conn: &Connection) -> Result<Vec<AlertRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, metric, op, threshold, duration_s, enabled FROM monitor_alert_rules
         ORDER BY created_at, id",
    )?;
    let rows = stmt.query_map([], |row| {
        let metric: String = row.get(1)?;
        let op: String = row.get(2)?;
        Ok(AlertRule {
            id: row.get(0)?,
            metric: metric_from_str(&metric),
            op: if op == "below" {
                Comparison::Below
            } else {
                Comparison::Above
            },
            threshold: row.get(3)?,
            duration_s: row.get(4)?,
            enabled: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Inserts a new rule (empty id) or updates an existing one.
pub fn save(conn: &Connection, rule: &AlertRule, now: i64) -> Result<AlertRule> {
    validate(rule)?;
    let mut rule = rule.clone();
    if rule.id.is_empty() {
        rule.id = uuid::Uuid::new_v4().to_string();
    }
    conn.execute(
        "INSERT INTO monitor_alert_rules (id, metric, op, threshold, duration_s, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET metric = excluded.metric, op = excluded.op,
           threshold = excluded.threshold, duration_s = excluded.duration_s,
           enabled = excluded.enabled",
        params![
            rule.id,
            metric_to_str(rule.metric),
            if rule.op == Comparison::Below { "below" } else { "above" },
            rule.threshold,
            rule.duration_s,
            rule.enabled,
            now
        ],
    )?;
    Ok(rule)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM monitor_alert_rules WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerts_say_what_is_wrong_in_plain_words() {
        assert_eq!(
            alert_text(Metric::DiskFree, Comparison::Below, 4.2),
            ("Disk space low".to_owned(), "4 GB left".to_owned())
        );
        assert_eq!(
            alert_text(Metric::Battery, Comparison::Below, 15.0).1,
            "15% left"
        );
        assert_eq!(
            alert_text(Metric::Temperature, Comparison::Above, 91.4).1,
            "91 °C"
        );
        assert_eq!(
            alert_text(Metric::Cpu, Comparison::Above, 96.0).0,
            "CPU busy for a while"
        );
        // Odd directions still say something true.
        assert_eq!(
            alert_text(Metric::Cpu, Comparison::Below, 3.0).0,
            "cpu below your limit"
        );
        // Only things the user must act on interrupt quiet time.
        assert!(Metric::Battery.is_urgent() && Metric::DiskFree.is_urgent());
        assert!(!Metric::Cpu.is_urgent() && !Metric::NetDown.is_urgent());
    }
    use crate::{foundation::storage::Storage, modules::monitor::metrics::BatteryInfo};

    fn sample(ts: i64, cpu: f32) -> MetricsSample {
        MetricsSample {
            ts,
            cpu,
            mem_used: 50,
            mem_total: 100,
            swap_used: 0,
            swap_total: 0,
            disk_name: "C:\\".into(),
            disk_used: 10,
            disk_total: 100,
            net_rx: 0,
            net_tx: 0,
            battery: None,
            temperature: None,
        }
    }

    fn rule(metric: Metric, op: Comparison, threshold: f32, duration_s: u32) -> AlertRule {
        AlertRule {
            id: format!("{metric:?}"),
            metric,
            op,
            threshold,
            duration_s,
            enabled: true,
        }
    }

    fn fired(events: &[AlertEvent]) -> usize {
        events
            .iter()
            .filter(|e| matches!(e, AlertEvent::Fired { .. }))
            .count()
    }

    #[test]
    fn fires_only_after_the_duration_and_only_once() {
        let rules = [rule(Metric::Cpu, Comparison::Above, 90.0, 30)];
        let mut engine = AlertEngine::default();
        assert_eq!(fired(&engine.evaluate(&rules, &sample(0, 95.0))), 0);
        assert_eq!(fired(&engine.evaluate(&rules, &sample(29_000, 95.0))), 0);
        assert_eq!(fired(&engine.evaluate(&rules, &sample(30_000, 95.0))), 1);
        assert_eq!(fired(&engine.evaluate(&rules, &sample(40_000, 99.0))), 0);
    }

    #[test]
    fn a_lasting_problem_is_repeated_only_if_the_user_asked() {
        let rules = [rule(Metric::Cpu, Comparison::Above, 90.0, 0)];
        let minute = 60_000;
        // Default: said once, however long it lasts.
        let mut once = AlertEngine::default();
        assert_eq!(fired(&once.evaluate(&rules, &sample(0, 95.0))), 1);
        assert_eq!(fired(&once.evaluate(&rules, &sample(60 * minute, 95.0))), 0);

        // "Again every 30 minutes while it lasts".
        let mut repeating = AlertEngine::default();
        repeating.set_repeat_min(30);
        assert_eq!(fired(&repeating.evaluate(&rules, &sample(0, 95.0))), 1);
        assert_eq!(
            fired(&repeating.evaluate(&rules, &sample(29 * minute, 95.0))),
            0
        );
        assert_eq!(
            fired(&repeating.evaluate(&rules, &sample(30 * minute, 95.0))),
            1
        );
        assert_eq!(
            fired(&repeating.evaluate(&rules, &sample(45 * minute, 95.0))),
            0
        );
        // Once it is over, nothing more is said.
        repeating.evaluate(&rules, &sample(50 * minute, 10.0));
        assert_eq!(
            fired(&repeating.evaluate(&rules, &sample(90 * minute, 10.0))),
            0
        );
    }

    #[test]
    fn a_dip_resets_the_timer() {
        let rules = [rule(Metric::Cpu, Comparison::Above, 90.0, 30)];
        let mut engine = AlertEngine::default();
        engine.evaluate(&rules, &sample(0, 95.0));
        engine.evaluate(&rules, &sample(20_000, 50.0));
        assert_eq!(fired(&engine.evaluate(&rules, &sample(40_000, 95.0))), 0);
        assert_eq!(fired(&engine.evaluate(&rules, &sample(70_000, 95.0))), 1);
    }

    #[test]
    fn resolves_with_hysteresis() {
        let rules = [rule(Metric::Cpu, Comparison::Above, 90.0, 0)];
        let mut engine = AlertEngine::default();
        assert_eq!(fired(&engine.evaluate(&rules, &sample(0, 91.0))), 1);
        assert!(
            engine.evaluate(&rules, &sample(1, 89.0)).is_empty(),
            "within margin"
        );
        assert_eq!(
            engine.evaluate(&rules, &sample(2, 87.0)),
            [AlertEvent::Resolved {
                rule_id: "Cpu".into()
            }]
        );
    }

    #[test]
    fn battery_ignores_charging_and_missing_battery() {
        let rules = [rule(Metric::Battery, Comparison::Below, 15.0, 0)];
        let mut engine = AlertEngine::default();
        assert!(
            engine.evaluate(&rules, &sample(0, 0.0)).is_empty(),
            "desktop PC"
        );

        let mut low = sample(1, 0.0);
        low.battery = Some(BatteryInfo {
            percent: 10,
            charging: true,
            secs_left: None,
        });
        assert!(engine.evaluate(&rules, &low).is_empty(), "charging");

        low.battery = Some(BatteryInfo {
            percent: 10,
            charging: false,
            secs_left: None,
        });
        assert_eq!(fired(&engine.evaluate(&rules, &low)), 1);
    }

    #[test]
    fn disabling_a_firing_rule_resolves_it() {
        let mut rules = vec![rule(Metric::Cpu, Comparison::Above, 50.0, 0)];
        let mut engine = AlertEngine::default();
        engine.evaluate(&rules, &sample(0, 80.0));
        rules[0].enabled = false;
        assert!(matches!(
            engine.evaluate(&rules, &sample(1, 80.0))[..],
            [AlertEvent::Resolved { .. }]
        ));
    }

    #[test]
    fn new_metrics_have_units_and_values() {
        let mut s = sample(0, 0.0);
        s.net_rx = 52 * 1024 * 1024;
        s.disk_used = 0;
        s.disk_total = 10 * 1024 * 1024 * 1024;
        assert_eq!(metric_value(&s, Metric::NetDown), Some(52.0));
        assert_eq!(metric_value(&s, Metric::DiskFree), Some(10.0));
        assert_eq!(metric_value(&s, Metric::Swap), None, "no swap file");
        assert_eq!(metric_value(&s, Metric::Temperature), None, "no sensor");
        assert_eq!(Metric::NetDown.unit(), "MB/s");

        // Low free space: "below 20 GB".
        let rules = [rule(Metric::DiskFree, Comparison::Below, 20.0, 0)];
        assert_eq!(fired(&AlertEngine::default().evaluate(&rules, &s)), 1);

        // Per-metric threshold ranges.
        assert!(validate(&rule(Metric::NetDown, Comparison::Above, 500.0, 0)).is_ok());
        assert!(validate(&rule(Metric::Cpu, Comparison::Above, 500.0, 0)).is_err());
        assert!(validate(&rule(Metric::Temperature, Comparison::Above, 200.0, 0)).is_err());
    }

    #[test]
    fn every_metric_round_trips_through_its_key() {
        for metric in Metric::ALL {
            assert_eq!(metric_from_str(metric_to_str(metric)), metric);
        }
    }

    #[test]
    fn store_round_trip_and_defaults() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .with_conn(|conn| {
                assert_eq!(list(conn)?.len(), 4, "seeded defaults");
                let mut new = rule(Metric::Memory, Comparison::Above, 80.0, 10);
                new.id.clear();
                let saved = save(conn, &new, 1)?;
                assert!(!saved.id.is_empty());

                let mut edited = saved.clone();
                edited.threshold = 85.0;
                save(conn, &edited, 2)?;
                assert!(list(conn)?.iter().any(|r| r.threshold == 85.0));

                delete(conn, &saved.id)?;
                assert_eq!(list(conn)?.len(), 4);

                let mut bad = edited;
                bad.threshold = 150.0;
                assert!(save(conn, &bad, 3).is_err());
                Ok(())
            })
            .unwrap();
    }
}
