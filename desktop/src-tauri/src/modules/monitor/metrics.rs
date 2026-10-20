//! System metrics (PRD M1) via `sysinfo`, plus a short history for sparklines.

use std::collections::VecDeque;

use serde::Serialize;
use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};
use ts_rs::TS;

use super::platform;

/// Samples kept for sparklines (2 minutes at the 2 s detailed rate).
pub const HISTORY_LEN: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatteryInfo {
    pub percent: u8,
    pub charging: bool,
    /// The system's own estimate of the time left on battery, in seconds (only while
    /// unplugged, and only once the OS has one).
    pub secs_left: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricsSample {
    #[ts(type = "number")]
    pub ts: i64,
    /// All cores, 0–100.
    pub cpu: f32,
    #[ts(type = "number")]
    pub mem_used: u64,
    #[ts(type = "number")]
    pub mem_total: u64,
    #[ts(type = "number")]
    pub swap_used: u64,
    #[ts(type = "number")]
    pub swap_total: u64,
    /// The fullest fixed disk — the one that matters for "disk almost full".
    pub disk_name: String,
    #[ts(type = "number")]
    pub disk_used: u64,
    #[ts(type = "number")]
    pub disk_total: u64,
    /// Bytes per second since the previous sample.
    #[ts(type = "number")]
    pub net_rx: u64,
    #[ts(type = "number")]
    pub net_tx: u64,
    pub battery: Option<BatteryInfo>,
    /// Hottest CPU sensor in °C, where the OS exposes one (often not on Windows without admin).
    pub temperature: Option<f32>,
}

impl MetricsSample {
    pub fn memory_pct(&self) -> f32 {
        pct(self.mem_used, self.mem_total)
    }

    pub fn disk_pct(&self) -> f32 {
        pct(self.disk_used, self.disk_total)
    }

    pub fn swap_pct(&self) -> Option<f32> {
        (self.swap_total > 0).then(|| pct(self.swap_used, self.swap_total))
    }
}

fn pct(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0) as f32
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    #[ts(type = "number")]
    pub memory: u64,
}

/// Fixed-size history, oldest first.
#[derive(Debug, Default)]
pub struct History(VecDeque<MetricsSample>);

impl History {
    pub fn push(&mut self, sample: MetricsSample) {
        if self.0.len() == HISTORY_LEN {
            self.0.pop_front();
        }
        self.0.push_back(sample);
    }

    pub fn to_vec(&self) -> Vec<MetricsSample> {
        self.0.iter().cloned().collect()
    }

    pub fn latest(&self) -> Option<&MetricsSample> {
        self.0.back()
    }
}

/// The last hour as one averaged point per minute, so a brief spike and a sustained load
/// look different. Fed with every sample; a minute is closed when the next one starts.
#[derive(Debug, Default)]
pub struct HourHistory {
    minutes: VecDeque<MetricsSample>,
    bucket: Vec<MetricsSample>,
}

pub const HOUR_LEN: usize = 60;

impl HourHistory {
    /// Adds a sample; returns true when a minute was completed (the chart changed).
    pub fn push(&mut self, sample: MetricsSample) -> bool {
        let minute = |s: &MetricsSample| s.ts.div_euclid(60_000);
        let closed = self
            .bucket
            .first()
            .is_some_and(|first| minute(first) != minute(&sample));
        if closed {
            if let Some(average) = average(&self.bucket) {
                if self.minutes.len() == HOUR_LEN {
                    self.minutes.pop_front();
                }
                self.minutes.push_back(average);
            }
            self.bucket.clear();
        }
        self.bucket.push(sample);
        closed
    }

    pub fn to_vec(&self) -> Vec<MetricsSample> {
        self.minutes.iter().cloned().collect()
    }
}

/// Averages the rates and loads of a minute; totals and names come from its last sample.
fn average(samples: &[MetricsSample]) -> Option<MetricsSample> {
    let last = samples.last()?.clone();
    let n = samples.len() as f64;
    let mean = |pick: fn(&MetricsSample) -> f64| samples.iter().map(pick).sum::<f64>() / n;
    let temperatures: Vec<f32> = samples.iter().filter_map(|s| s.temperature).collect();
    Some(MetricsSample {
        cpu: mean(|s| f64::from(s.cpu)) as f32,
        mem_used: mean(|s| s.mem_used as f64) as u64,
        net_rx: mean(|s| s.net_rx as f64) as u64,
        net_tx: mean(|s| s.net_tx as f64) as u64,
        temperature: (!temperatures.is_empty())
            .then(|| temperatures.iter().sum::<f32>() / temperatures.len() as f32),
        ..last
    })
}

/// Owns the sysinfo handles; `sample()` must be called repeatedly (CPU usage and network
/// rates are deltas between calls).
pub struct Collector {
    system: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    last_ts: Option<i64>,
}

impl Default for Collector {
    fn default() -> Self {
        let mut system = System::new();
        system.refresh_cpu_usage();
        Self {
            system,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            last_ts: None,
        }
    }
}

impl Collector {
    pub fn sample(&mut self, now: i64) -> MetricsSample {
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.disks.refresh(true);
        self.networks.refresh(true);
        self.components.refresh(false);

        let (disk_name, disk_used, disk_total) = self
            .disks
            .list()
            .iter()
            .filter(|d| !d.is_removable() && d.total_space() > 0)
            .map(|d| {
                let used = d.total_space().saturating_sub(d.available_space());
                (d.mount_point().display().to_string(), used, d.total_space())
            })
            .max_by(|a, b| pct(a.1, a.2).total_cmp(&pct(b.1, b.2)))
            .unwrap_or_default();

        // `received()` is bytes since the previous refresh; turn it into a per-second rate.
        let elapsed_s = self
            .last_ts
            .map_or(1.0, |last| ((now - last).max(1) as f64) / 1000.0);
        self.last_ts = Some(now);
        let (rx, tx) = self
            .networks
            .iter()
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (rx + data.received(), tx + data.transmitted())
            });

        MetricsSample {
            ts: now,
            cpu: self.system.global_cpu_usage(),
            mem_used: self.system.used_memory(),
            mem_total: self.system.total_memory(),
            swap_used: self.system.used_swap(),
            swap_total: self.system.total_swap(),
            disk_name,
            disk_used,
            disk_total,
            net_rx: (rx as f64 / elapsed_s) as u64,
            net_tx: (tx as f64 / elapsed_s) as u64,
            battery: platform::battery(),
            temperature: self.cpu_temperature(),
        }
    }

    /// Hottest sensor that looks like a CPU, else the hottest sensor of any kind.
    fn cpu_temperature(&self) -> Option<f32> {
        let readings = || {
            self.components
                .iter()
                .filter_map(|c| c.temperature().map(|t| (c.label().to_lowercase(), t)))
                .filter(|(_, t)| t.is_finite() && *t > 0.0)
        };
        let is_cpu = |label: &str| {
            ["cpu", "package", "tctl", "core"]
                .iter()
                .any(|k| label.contains(k))
        };
        readings()
            .filter(|(label, _)| is_cpu(label))
            .map(|(_, t)| t)
            .reduce(f32::max)
            .or_else(|| readings().map(|(_, t)| t).reduce(f32::max))
    }

    /// Top processes by CPU. Costly (enumerates every process), so only while the
    /// Processes view is open.
    pub fn top_processes(&mut self, limit: usize) -> Vec<ProcessInfo> {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        let cores = self.system.cpus().len().max(1) as f32;
        let mut list: Vec<ProcessInfo> = self
            .system
            .processes()
            .values()
            .filter(|p| p.pid().as_u32() != 0)
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                // sysinfo reports per-core percent (can exceed 100); normalise to the machine.
                cpu: p.cpu_usage() / cores,
                memory: p.memory(),
            })
            .collect();
        list.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.memory.cmp(&a.memory)));
        list.truncate(limit);
        list
    }

    /// Kills a process. Refuses Preku itself and system PIDs (0 and 4 on Windows).
    pub fn kill(&mut self, pid: u32) -> bool {
        if pid <= 4 || pid == std::process::id() {
            return false;
        }
        self.system.refresh_processes(
            ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
            true,
        );
        self.system
            .process(sysinfo::Pid::from_u32(pid))
            .is_some_and(|p| p.kill())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_at(ts: i64, cpu: f32) -> MetricsSample {
        MetricsSample {
            ts,
            cpu,
            mem_used: 8,
            mem_total: 16,
            swap_used: 0,
            swap_total: 0,
            disk_name: "C:".into(),
            disk_used: 1,
            disk_total: 2,
            net_rx: (cpu as u64) * 10,
            net_tx: 0,
            battery: None,
            temperature: None,
        }
    }

    #[test]
    fn the_hour_chart_averages_each_minute() {
        let mut hour = HourHistory::default();
        // Minute 0: a 100% spike among idle samples averages out to 25%.
        for (second, cpu) in [(0, 0.0), (15, 100.0), (30, 0.0), (45, 0.0)] {
            assert!(!hour.push(sample_at(second * 1000, cpu)));
        }
        assert!(hour.to_vec().is_empty(), "the minute isn't over yet");
        // Minute 1 begins: minute 0 is closed.
        assert!(hour.push(sample_at(60_000, 80.0)));
        let points = hour.to_vec();
        assert_eq!(points.len(), 1);
        assert!((points[0].cpu - 25.0).abs() < 0.01);
        assert_eq!(points[0].net_rx, 250);

        // Sustained load stays high; and only the last 60 minutes are kept.
        for minute in 2..=70 {
            hour.push(sample_at(minute * 60_000, 80.0));
        }
        let points = hour.to_vec();
        assert_eq!(points.len(), HOUR_LEN);
        assert!(points.iter().all(|p| (p.cpu - 80.0).abs() < 0.01));
    }

    fn sample(ts: i64) -> MetricsSample {
        MetricsSample {
            ts,
            cpu: 0.0,
            mem_used: 1,
            mem_total: 4,
            swap_used: 0,
            swap_total: 0,
            disk_name: "C:\\".into(),
            disk_used: 3,
            disk_total: 4,
            net_rx: 0,
            net_tx: 0,
            battery: None,
            temperature: None,
        }
    }

    #[test]
    fn history_keeps_the_newest_samples() {
        let mut history = History::default();
        for ts in 0..(HISTORY_LEN as i64 + 5) {
            history.push(sample(ts));
        }
        let all = history.to_vec();
        assert_eq!(all.len(), HISTORY_LEN);
        assert_eq!(all[0].ts, 5);
        assert_eq!(history.latest().unwrap().ts, HISTORY_LEN as i64 + 4);
    }

    #[test]
    fn percentages() {
        let s = sample(0);
        assert_eq!(s.memory_pct(), 25.0);
        assert_eq!(s.disk_pct(), 75.0);
        assert_eq!(pct(5, 0), 0.0);
    }

    #[test]
    fn collector_produces_sane_numbers() {
        let mut collector = Collector::default();
        let s = collector.sample(1_000);
        assert!((0.0..=100.0).contains(&s.cpu));
        assert!(s.mem_total > 0 && s.mem_used <= s.mem_total);
    }

    #[test]
    fn refuses_to_kill_itself_or_system_processes() {
        let mut collector = Collector::default();
        assert!(!collector.kill(std::process::id()));
        assert!(!collector.kill(0));
        assert!(!collector.kill(4));
    }
}
