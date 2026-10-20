//! System & Dev Monitor (Phase 3, PRD M1–M7).
//!
//! Background jobs while enabled (all stop with the module):
//! - metrics: every 2 s while the Monitor view is open, else every 10 s (Rules.md §5);
//!   processes are only enumerated while the Processes view is open
//! - docker: every 5 s when Docker answers, every 30 s when it doesn't
//! - GitHub Actions: every 60 s (slower without a token, to stay in the anonymous rate limit)
//! - uptime: each URL on its own interval (≥ 30 s)
//! - the `preku watch` channel (named pipe / unix socket)

pub mod alerts;
pub mod ci;
pub mod docker;
pub mod metrics;
mod platform;
pub mod targets;
pub mod uptime;
pub mod watch;

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tokio::sync::Notify;
use ts_rs::TS;

use self::{
    alerts::{AlertEngine, AlertEvent, AlertRule},
    ci::{CiEvent, CiRun, CiStatus},
    docker::{ContainerAction, DockerState},
    metrics::{Collector, History, MetricsSample, ProcessInfo},
    targets::{TargetKind, WatchTarget},
    uptime::{Tracker, UptimeEvent, UptimeStatus},
    watch::{WatchLog, WatchMessage, WatchRecord, WatchState},
};
use super::{Module, ModuleContext};
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        event_bus::AppEvent,
        secrets,
        storage::now_millis,
    },
    live::{LiveItem, LivePriority},
    pip::mood_engine::PipSignal,
};

pub const ID: &str = "monitor";
/// Module settings that aren't rules or targets (JSON in the settings table).
const SETTINGS_KEY: &str = "monitor";

pub mod events {
    pub const METRICS: &str = "monitor:metrics";
    pub const HISTORY_HOUR: &str = "monitor:history-hour";
    pub const PROCESSES: &str = "monitor:processes";
    pub const RULES: &str = "monitor:rules";
    pub const DOCKER: &str = "monitor:docker";
    pub const CI: &str = "monitor:ci";
    pub const UPTIME: &str = "monitor:uptime";
    pub const WATCHES: &str = "monitor:watches";
    pub const TARGETS: &str = "monitor:targets";
}

const DETAILED_EVERY: Duration = Duration::from_secs(2);
const IDLE_EVERY: Duration = Duration::from_secs(10);
const DOCKER_EVERY: Duration = Duration::from_secs(5);
const DOCKER_RETRY: Duration = Duration::from_secs(30);
const CI_EVERY: Duration = Duration::from_secs(60);
const UPTIME_TICK: Duration = Duration::from_secs(5);
const PROCESS_LIMIT: usize = 12;

/// What the UI is currently showing; decides how hard the module works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MonitorView {
    #[default]
    Off,
    Overview,
    Processes,
}

/// Everything the Monitor views need on open; afterwards events keep it fresh.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorSnapshot {
    pub history: Vec<MetricsSample>,
    /// The last hour, one averaged point per minute (oldest first).
    pub history_hour: Vec<MetricsSample>,
    /// Remind again every this many minutes while an alert lasts (0 = once).
    pub alert_repeat_min: u32,
    /// Last known top processes (refreshed only while the Processes view is open).
    pub processes: Vec<ProcessInfo>,
    pub rules: Vec<AlertRule>,
    pub docker: DockerState,
    pub ci: Vec<CiStatus>,
    pub uptime: Vec<UptimeStatus>,
    pub watches: Vec<WatchRecord>,
    pub targets: Vec<WatchTarget>,
    pub github_token_set: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Shared {
    ctx: ModuleContext,
    http: reqwest::Client,
    collector: Mutex<Collector>,
    history: Mutex<History>,
    hour: Mutex<metrics::HourHistory>,
    alert_repeat_min: Mutex<u32>,
    processes: Mutex<Vec<ProcessInfo>>,
    view: Mutex<MonitorView>,
    rules: Mutex<Vec<AlertRule>>,
    engine: Mutex<AlertEngine>,
    docker: Mutex<DockerState>,
    expected_stops: Mutex<HashSet<String>>,
    ci: Mutex<Vec<CiStatus>>,
    uptime: Mutex<Vec<UptimeStatus>>,
    watches: Mutex<WatchLog>,
    /// Wake a loop early (view changed, target added, docker action…).
    wake_metrics: Notify,
    wake_docker: Notify,
    wake_ci: Notify,
    wake_uptime: Notify,
}

struct Running {
    shared: Arc<Shared>,
    tasks: Vec<JoinHandle<()>>,
}

#[derive(Default)]
pub struct MonitorModule {
    running: Mutex<Option<Running>>,
}

impl Module for MonitorModule {
    fn id(&self) -> &'static str {
        ID
    }

    fn start(&self, ctx: ModuleContext) -> Result<()> {
        let rules = ctx.storage.with_conn(|conn| alerts::list(conn))?;
        let http = reqwest::Client::builder()
            .user_agent(concat!("Preku/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(ci::http_err)?;
        let alert_repeat_min = ctx
            .storage
            .settings_get(SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|value| value.get("alertRepeatMin")?.as_u64())
            .map_or(0, |minutes| minutes.min(240) as u32);
        let shared = Arc::new(Shared {
            ctx,
            http,
            collector: Mutex::new(Collector::default()),
            history: Mutex::new(History::default()),
            hour: Mutex::new(metrics::HourHistory::default()),
            alert_repeat_min: Mutex::new(alert_repeat_min),
            processes: Mutex::new(Vec::new()),
            view: Mutex::new(MonitorView::Off),
            rules: Mutex::new(rules),
            engine: Mutex::new({
                let mut engine = AlertEngine::default();
                engine.set_repeat_min(alert_repeat_min);
                engine
            }),
            docker: Mutex::new(DockerState {
                available: false,
                containers: Vec::new(),
            }),
            expected_stops: Mutex::new(HashSet::new()),
            ci: Mutex::new(Vec::new()),
            uptime: Mutex::new(Vec::new()),
            watches: Mutex::new(WatchLog::default()),
            wake_metrics: Notify::new(),
            wake_docker: Notify::new(),
            wake_ci: Notify::new(),
            wake_uptime: Notify::new(),
        });

        let tasks = vec![
            tauri::async_runtime::spawn(metrics_loop(shared.clone())),
            tauri::async_runtime::spawn(docker_loop(shared.clone())),
            tauri::async_runtime::spawn(ci_loop(shared.clone())),
            tauri::async_runtime::spawn(uptime_loop(shared.clone())),
            tauri::async_runtime::spawn(watch_server(shared.clone())),
        ];
        *lock(&self.running) = Some(Running { shared, tasks });
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if let Some(running) = lock(&self.running).take() {
            for task in running.tasks {
                task.abort();
            }
            running
                .shared
                .ctx
                .publish(AppEvent::LiveClearModule { module: ID });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- metrics + alerts

async fn metrics_loop(shared: Arc<Shared>) {
    loop {
        let view = *lock(&shared.view);
        let worker = shared.clone();
        // sysinfo calls block briefly (processes can take tens of ms): keep them off the
        // async workers.
        let sampled = tauri::async_runtime::spawn_blocking(move || {
            let mut collector = lock(&worker.collector);
            let sample = collector.sample(now_millis());
            let processes =
                (view == MonitorView::Processes).then(|| collector.top_processes(PROCESS_LIMIT));
            (sample, processes)
        })
        .await;
        if let Ok((sample, processes)) = sampled {
            lock(&shared.history).push(sample.clone());
            shared.ctx.emit(events::METRICS, &sample);
            let minute_closed = lock(&shared.hour).push(sample.clone());
            if minute_closed {
                shared
                    .ctx
                    .emit(events::HISTORY_HOUR, lock(&shared.hour).to_vec());
            }
            if let Some(processes) = processes {
                shared.ctx.emit(events::PROCESSES, &processes);
                *lock(&shared.processes) = processes;
            }
            check_alerts(&shared, &sample);
        }
        let wait = if view == MonitorView::Off {
            IDLE_EVERY
        } else {
            DETAILED_EVERY
        };
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = shared.wake_metrics.notified() => {}
        }
    }
}

fn check_alerts(shared: &Shared, sample: &MetricsSample) {
    let rules = lock(&shared.rules).clone();
    let events = lock(&shared.engine).evaluate(&rules, sample);
    for event in events {
        match event {
            AlertEvent::Fired { rule, value } => {
                let (title, detail) = alerts::alert_text(rule.metric, rule.op, value);
                // During focus or a pause only urgent alerts pop out; the rest show quietly.
                let interrupt = rule.metric.is_urgent() || !shared.ctx.quiet.is_quiet(now_millis());
                let priority = if interrupt {
                    LivePriority::Alert
                } else {
                    LivePriority::Info
                };
                shared.ctx.publish(AppEvent::LiveUpsert(LiveItem::new(
                    format!("monitor.alert.{}", rule.id),
                    ID,
                    priority,
                    title,
                    Some(detail),
                )));
                if interrupt {
                    shared.ctx.publish(AppEvent::Pip(PipSignal::Alert));
                }
            }
            AlertEvent::Resolved { rule_id } => {
                shared.ctx.publish(AppEvent::LiveRemove {
                    id: format!("monitor.alert.{rule_id}"),
                });
            }
        }
    }
}

// ---------------------------------------------------------------- docker

async fn docker_loop(shared: Arc<Shared>) {
    let mut client = None;
    let mut previous = Vec::new();
    loop {
        if client.is_none() {
            client = docker::connect().ok();
        }
        let listed = match &client {
            Some(docker) => docker::list(docker).await.ok(),
            None => None,
        };
        let wait = match listed {
            Some(containers) => {
                let crashes = {
                    let mut expected = lock(&shared.expected_stops);
                    let crashes = docker::unexpected_exits(&previous, &containers, &expected);
                    // An expected stop is "used up" once Docker shows it as not running.
                    expected.retain(|id| {
                        containers
                            .iter()
                            .any(|c| &c.id == id && c.state == "running")
                    });
                    crashes
                };
                for (name, code) in crashes {
                    shared.ctx.publish(AppEvent::LiveUpsert(
                        LiveItem::new(
                            format!("monitor.docker.{name}"),
                            ID,
                            LivePriority::Alert,
                            format!("🐳 {name}"),
                            Some(format!("✗ {code}")),
                        )
                        .expires_in(30_000),
                    ));
                    shared.ctx.publish(AppEvent::Pip(PipSignal::Failure));
                }
                previous = containers.clone();
                set_docker(
                    &shared,
                    DockerState {
                        available: true,
                        containers,
                    },
                );
                DOCKER_EVERY
            }
            None => {
                client = None;
                previous.clear();
                set_docker(
                    &shared,
                    DockerState {
                        available: false,
                        containers: Vec::new(),
                    },
                );
                DOCKER_RETRY
            }
        };
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = shared.wake_docker.notified() => {}
        }
    }
}

fn set_docker(shared: &Shared, state: DockerState) {
    let changed = {
        let mut current = lock(&shared.docker);
        let changed = *current != state;
        *current = state.clone();
        changed
    };
    if changed {
        shared.ctx.emit(events::DOCKER, &state);
    }
}

// ---------------------------------------------------------------- GitHub Actions

fn public_error(err: &PrekuError, fallback: &str) -> String {
    match err {
        PrekuError::InvalidInput(message) => message.clone(),
        _ => fallback.to_owned(),
    }
}

async fn ci_loop(shared: Arc<Shared>) {
    let mut last: HashMap<String, CiRun> = HashMap::new();
    loop {
        let repos = shared
            .ctx
            .storage
            .with_conn(|conn| targets::list(conn, TargetKind::Github))
            .unwrap_or_default();
        let token = secrets::get(ci::TOKEN_KEY).ok().flatten();

        let mut statuses = Vec::with_capacity(repos.len());
        for target in &repos {
            let repo = &target.target;
            match ci::latest_run(&shared.http, repo, token.as_deref()).await {
                Ok(run) => {
                    if let Some(run) = &run {
                        if let Some(event) = ci::transition(last.get(repo), run) {
                            publish_ci(&shared, repo, event);
                        }
                        last.insert(repo.clone(), run.clone());
                    }
                    statuses.push(CiStatus {
                        repo: repo.clone(),
                        run,
                        error: None,
                    });
                }
                Err(err) => statuses.push(CiStatus {
                    repo: repo.clone(),
                    run: last.get(repo).cloned(),
                    error: Some(public_error(&err, "Can't reach GitHub")),
                }),
            }
        }
        last.retain(|repo, _| repos.iter().any(|t| &t.target == repo));
        *lock(&shared.ci) = statuses.clone();
        shared.ctx.emit(events::CI, &statuses);

        // Anonymous API calls are limited to 60/hour, so spread repos out without a token.
        let wait = if token.is_some() {
            CI_EVERY
        } else {
            CI_EVERY * (repos.len().max(1) as u32)
        };
        tokio::select! {
            _ = tokio::time::sleep(wait) => {}
            _ = shared.wake_ci.notified() => {}
        }
    }
}

fn publish_ci(shared: &Shared, repo: &str, event: CiEvent) {
    let id = format!("monitor.ci.{repo}");
    let (item, signal) = match event {
        CiEvent::Started(run) => (
            LiveItem::new(
                id,
                ID,
                LivePriority::Action,
                repo,
                Some(format!("⏳ {}", run.name)),
            ),
            None,
        ),
        CiEvent::Succeeded(run) => (
            LiveItem::new(
                id,
                ID,
                LivePriority::Info,
                repo,
                Some(format!("✓ {}", run.name)),
            )
            .expires_in(20_000),
            Some(PipSignal::Success),
        ),
        CiEvent::Failed(run) => (
            LiveItem::new(
                id,
                ID,
                LivePriority::Alert,
                repo,
                Some(format!("✗ {}", run.name)),
            )
            .expires_in(60_000),
            Some(PipSignal::Failure),
        ),
    };
    shared.ctx.publish(AppEvent::LiveUpsert(item));
    if let Some(signal) = signal {
        shared.ctx.publish(AppEvent::Pip(signal));
    }
}

// ---------------------------------------------------------------- uptime

fn host_of(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or(url)
        .to_owned()
}

async fn uptime_loop(shared: Arc<Shared>) {
    let mut trackers: HashMap<String, Tracker> = HashMap::new();
    let mut due: HashMap<String, i64> = HashMap::new();
    let mut statuses: HashMap<String, UptimeStatus> = HashMap::new();
    loop {
        let urls = shared
            .ctx
            .storage
            .with_conn(|conn| targets::list(conn, TargetKind::Uptime))
            .unwrap_or_default();
        let mut changed = false;

        // Forget removed URLs (and their live items).
        let before = statuses.len();
        statuses.retain(|url, _| urls.iter().any(|t| &t.target == url));
        for url in trackers
            .keys()
            .filter(|u| !urls.iter().any(|t| &&t.target == u))
        {
            shared.ctx.publish(AppEvent::LiveRemove {
                id: format!("monitor.uptime.{url}"),
            });
        }
        trackers.retain(|url, _| urls.iter().any(|t| &t.target == url));
        due.retain(|url, _| urls.iter().any(|t| &t.target == url));
        changed |= statuses.len() != before;

        for target in &urls {
            let now = now_millis();
            if due.get(&target.target).is_some_and(|at| now < *at) {
                continue;
            }
            let status = uptime::check(&shared.http, &target.target, now).await;
            let ok = status.up == Some(true);
            let event = trackers
                .entry(target.target.clone())
                .or_default()
                .record(ok);
            if let Some(event) = event {
                publish_uptime(&shared, &target.target, event);
            }
            due.insert(
                target.target.clone(),
                now + i64::from(target.interval_s) * 1000,
            );
            statuses.insert(target.target.clone(), status);
            changed = true;
        }

        if changed {
            let ordered: Vec<UptimeStatus> = urls
                .iter()
                .filter_map(|t| statuses.get(&t.target).cloned())
                .collect();
            *lock(&shared.uptime) = ordered.clone();
            shared.ctx.emit(events::UPTIME, &ordered);
        }
        tokio::select! {
            _ = tokio::time::sleep(UPTIME_TICK) => {}
            _ = shared.wake_uptime.notified() => {}
        }
    }
}

fn publish_uptime(shared: &Shared, url: &str, event: UptimeEvent) {
    let id = format!("monitor.uptime.{url}");
    let host = host_of(url);
    match event {
        UptimeEvent::Down => {
            shared.ctx.publish(AppEvent::LiveUpsert(LiveItem::new(
                id,
                ID,
                LivePriority::Alert,
                host,
                Some("✗".into()),
            )));
            shared.ctx.publish(AppEvent::Pip(PipSignal::Failure));
        }
        UptimeEvent::Recovered => {
            shared.ctx.publish(AppEvent::LiveUpsert(
                LiveItem::new(id, ID, LivePriority::Info, host, Some("✓".into()))
                    .expires_in(15_000),
            ));
            shared.ctx.publish(AppEvent::Pip(PipSignal::Success));
        }
    }
}

// ---------------------------------------------------------------- preku watch

async fn watch_server(shared: Arc<Shared>) {
    let handler_shared = shared.clone();
    let handler = move |line: String| handle_watch_line(&handler_shared, &line);
    if let Err(err) = platform::serve_watch(handler).await {
        tracing::warn!(error = %err, "`preku watch` channel unavailable");
    }
}

fn handle_watch_line(shared: &Shared, line: &str) {
    let Some(message) = watch::parse(line) else {
        tracing::debug!("ignored malformed watch message");
        return;
    };
    let record = lock(&shared.watches).apply(&message, now_millis());
    let Some(record) = record else {
        return;
    };
    let id = format!("monitor.watch.{}", record.id);
    match (&message, record.state) {
        (WatchMessage::Started { .. }, _) => {
            shared.ctx.publish(AppEvent::LiveUpsert(LiveItem::new(
                id,
                ID,
                LivePriority::Action,
                record.command.clone(),
                Some("⏳".into()),
            )));
        }
        (
            WatchMessage::Finished {
                code, duration_ms, ..
            },
            state,
        ) => {
            let ok = state == WatchState::Succeeded;
            let subtitle = if ok {
                format!("✓ {}", watch::human_duration(*duration_ms))
            } else {
                format!("✗ {code} · {}", watch::human_duration(*duration_ms))
            };
            // Alert priority so the island pops out when a long command finishes.
            shared.ctx.publish(AppEvent::LiveUpsert(
                LiveItem::new(
                    id,
                    ID,
                    LivePriority::Alert,
                    record.command.clone(),
                    Some(subtitle),
                )
                .expires_in(20_000),
            ));
            shared.ctx.publish(AppEvent::Pip(if ok {
                PipSignal::Celebration
            } else {
                PipSignal::Failure
            }));
        }
    }
    shared
        .ctx
        .emit(events::WATCHES, lock(&shared.watches).records());
}

// ---------------------------------------------------------------- commands

impl MonitorModule {
    fn shared(&self) -> Result<Arc<Shared>> {
        lock(&self.running)
            .as_ref()
            .map(|r| r.shared.clone())
            .ok_or_else(|| {
                PrekuError::InvalidInput("System monitor is turned off in Settings".into())
            })
    }

    pub fn snapshot(&self) -> Result<MonitorSnapshot> {
        let shared = self.shared()?;
        let targets = shared.ctx.storage.with_conn(|conn| {
            let mut all = targets::list(conn, TargetKind::Github)?;
            all.extend(targets::list(conn, TargetKind::Uptime)?);
            Ok(all)
        })?;
        let snapshot = MonitorSnapshot {
            history: lock(&shared.history).to_vec(),
            history_hour: lock(&shared.hour).to_vec(),
            alert_repeat_min: *lock(&shared.alert_repeat_min),
            processes: lock(&shared.processes).clone(),
            rules: lock(&shared.rules).clone(),
            docker: lock(&shared.docker).clone(),
            ci: lock(&shared.ci).clone(),
            uptime: lock(&shared.uptime).clone(),
            watches: lock(&shared.watches).records(),
            targets,
            github_token_set: secrets::get(ci::TOKEN_KEY).ok().flatten().is_some(),
        };
        Ok(snapshot)
    }

    pub fn set_view(&self, view: MonitorView) -> Result<()> {
        let shared = self.shared()?;
        let changed = std::mem::replace(&mut *lock(&shared.view), view) != view;
        if changed {
            shared.wake_metrics.notify_one();
        }
        Ok(())
    }

    /// Blocking (process table): call from a blocking thread.
    pub fn kill(&self, pid: u32) -> Result<Vec<ProcessInfo>> {
        let shared = self.shared()?;
        let mut collector = lock(&shared.collector);
        if !collector.kill(pid) {
            return Err(PrekuError::InvalidInput(
                "Couldn't end that process (it may need admin rights)".into(),
            ));
        }
        let processes = collector.top_processes(PROCESS_LIMIT);
        shared.ctx.emit(events::PROCESSES, &processes);
        *lock(&shared.processes) = processes.clone();
        Ok(processes)
    }

    pub fn save_rule(&self, rule: &AlertRule) -> Result<Vec<AlertRule>> {
        let shared = self.shared()?;
        let rules = shared.ctx.storage.with_conn(|conn| {
            alerts::save(conn, rule, now_millis())?;
            alerts::list(conn)
        })?;
        *lock(&shared.rules) = rules.clone();
        shared.ctx.emit(events::RULES, &rules);
        Ok(rules)
    }

    /// "Remind me again while it lasts": every `minutes` (0 = say it once).
    pub fn set_alert_repeat(&self, minutes: u32) -> Result<u32> {
        if minutes > 240 {
            return Err(PrekuError::InvalidInput(
                "Repeat at most every 240 minutes".into(),
            ));
        }
        let shared = self.shared()?;
        shared.ctx.storage.settings_set(
            SETTINGS_KEY,
            &serde_json::json!({ "alertRepeatMin": minutes }),
        )?;
        *lock(&shared.alert_repeat_min) = minutes;
        lock(&shared.engine).set_repeat_min(minutes);
        Ok(minutes)
    }

    pub fn delete_rule(&self, id: &str) -> Result<Vec<AlertRule>> {
        let shared = self.shared()?;
        let rules = shared.ctx.storage.with_conn(|conn| {
            alerts::delete(conn, id)?;
            alerts::list(conn)
        })?;
        *lock(&shared.rules) = rules.clone();
        shared.ctx.emit(events::RULES, &rules);
        Ok(rules)
    }

    pub async fn docker_action(&self, id: &str, action: ContainerAction) -> Result<()> {
        let shared = self.shared()?;
        if action != ContainerAction::Start {
            lock(&shared.expected_stops).insert(id.to_owned());
        }
        let docker = docker::connect()?;
        docker::act(&docker, id, action).await?;
        shared.wake_docker.notify_one();
        Ok(())
    }

    /// Stores the token in the OS keychain (`None` or empty removes it). Never logged.
    pub fn set_github_token(&self, token: Option<&str>) -> Result<()> {
        let shared = self.shared()?;
        match token.map(str::trim).filter(|t| !t.is_empty()) {
            Some(token) => secrets::set(ci::TOKEN_KEY, token)?,
            None => secrets::delete(ci::TOKEN_KEY)?,
        }
        shared.wake_ci.notify_one();
        Ok(())
    }

    pub fn add_target(&self, kind: TargetKind, input: &str) -> Result<Vec<WatchTarget>> {
        let shared = self.shared()?;
        shared
            .ctx
            .storage
            .with_conn(|conn| targets::add(conn, kind, input, now_millis()))?;
        self.after_targets_change(&shared)
    }

    pub fn delete_target(&self, id: &str) -> Result<Vec<WatchTarget>> {
        let shared = self.shared()?;
        shared
            .ctx
            .storage
            .with_conn(|conn| targets::delete(conn, id))?;
        self.after_targets_change(&shared)
    }

    fn after_targets_change(&self, shared: &Shared) -> Result<Vec<WatchTarget>> {
        let all = shared.ctx.storage.with_conn(|conn| {
            let mut all = targets::list(conn, TargetKind::Github)?;
            all.extend(targets::list(conn, TargetKind::Uptime)?);
            Ok(all)
        })?;
        shared.ctx.emit(events::TARGETS, &all);
        shared.wake_ci.notify_one();
        shared.wake_uptime.notify_one();
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts() {
        assert_eq!(host_of("https://example.com/health"), "example.com");
        assert_eq!(host_of("http://localhost:3000"), "localhost:3000");
    }
}
