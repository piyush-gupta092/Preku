//! Feature modules and their registry (Architecture.md §3.2).
//!
//! A module is started when enabled and stopped when disabled or when Preku is paused.
//! A stopped module must do no background work. A module that fails to start is marked
//! `degraded` instead of crashing the app (Rules.md §3).

pub mod bridge;
pub mod demo;
pub mod monitor;
pub mod productivity;
pub mod skills;
pub mod weather;
pub mod wellbeing;

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use serde::Serialize;
use ts_rs::TS;

use crate::foundation::{
    errors::Result,
    event_bus::{AppEvent, EventBus},
    privacy::Privacy,
    quiet::Quiet,
    storage::Storage,
};

/// What a module gets to work with.
#[derive(Clone)]
pub struct ModuleContext {
    pub bus: EventBus,
    pub storage: Arc<Storage>,
    /// App data folder, for files a module keeps outside SQLite (e.g. clipboard images).
    pub data_dir: PathBuf,
    /// "May I interrupt right now?" (paused by the user, or holding during focus).
    pub quiet: Arc<Quiet>,
    /// The user's choices about captured data (clipboard pause, how long to keep things).
    pub privacy: Arc<Privacy>,
}

impl ModuleContext {
    /// Sends module-specific state to the UI as the Tauri event `event`.
    pub fn emit(&self, event: &'static str, payload: impl serde::Serialize) {
        match serde_json::to_value(payload) {
            Ok(payload) => self.bus.publish(AppEvent::ModuleUpdate { event, payload }),
            Err(err) => tracing::warn!(error = %err, event, "failed to serialize module update"),
        }
    }

    pub fn publish(&self, event: AppEvent) {
        self.bus.publish(event);
    }
}

pub trait Module: Send + Sync {
    fn id(&self) -> &'static str;

    /// Used when the user has never toggled this module.
    fn enabled_by_default(&self) -> bool {
        true
    }

    /// Spawn pollers/listeners. Must return quickly.
    fn start(&self, ctx: ModuleContext) -> Result<()>;

    /// Release every resource and remove the module's live items.
    fn stop(&self) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum ModuleStatus {
    Running,
    Stopped,
    Degraded { message: String },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModuleInfo {
    pub id: String,
    pub enabled: bool,
    pub status: ModuleStatus,
}

pub struct ModuleRegistry {
    ctx: ModuleContext,
    modules: Vec<Arc<dyn Module>>,
    status: Mutex<HashMap<&'static str, ModuleStatus>>,
}

impl ModuleRegistry {
    pub fn new(ctx: ModuleContext, modules: Vec<Arc<dyn Module>>) -> Self {
        let status = modules
            .iter()
            .map(|m| (m.id(), ModuleStatus::Stopped))
            .collect();
        Self {
            ctx,
            modules,
            status: Mutex::new(status),
        }
    }

    fn is_enabled(module: &dyn Module, enabled: &BTreeMap<String, bool>) -> bool {
        enabled
            .get(module.id())
            .copied()
            .unwrap_or_else(|| module.enabled_by_default())
    }

    /// Starts/stops modules so that exactly the enabled ones run (none while paused).
    pub fn apply(&self, enabled: &BTreeMap<String, bool>, paused: bool) {
        for module in &self.modules {
            let should_run = !paused && Self::is_enabled(module.as_ref(), enabled);
            let running = self.status_of(module.id()) == ModuleStatus::Running;
            match (should_run, running) {
                (true, false) => self.start(module.as_ref()),
                (false, true) => self.stop(module.as_ref()),
                _ => {}
            }
        }
    }

    fn start(&self, module: &dyn Module) {
        let status = match module.start(self.ctx.clone()) {
            Ok(()) => {
                tracing::info!(module = module.id(), "module started");
                ModuleStatus::Running
            }
            Err(err) => {
                tracing::error!(module = module.id(), error = %err, "module failed to start");
                ModuleStatus::Degraded {
                    message: err.to_string(),
                }
            }
        };
        self.set_status(module.id(), status);
    }

    fn stop(&self, module: &dyn Module) {
        if let Err(err) = module.stop() {
            tracing::warn!(module = module.id(), error = %err, "module failed to stop cleanly");
        }
        tracing::info!(module = module.id(), "module stopped");
        self.set_status(module.id(), ModuleStatus::Stopped);
    }

    pub fn stop_all(&self) {
        for module in &self.modules {
            if self.status_of(module.id()) == ModuleStatus::Running {
                self.stop(module.as_ref());
            }
        }
    }

    pub fn list(&self, enabled: &BTreeMap<String, bool>) -> Vec<ModuleInfo> {
        self.modules
            .iter()
            .map(|m| ModuleInfo {
                id: m.id().to_owned(),
                enabled: Self::is_enabled(m.as_ref(), enabled),
                status: self.status_of(m.id()),
            })
            .collect()
    }

    fn status_of(&self, id: &str) -> ModuleStatus {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(id)
            .cloned()
            .unwrap_or(ModuleStatus::Stopped)
    }

    fn set_status(&self, id: &'static str, status: ModuleStatus) {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id, status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::errors::PrekuError;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Probe {
        starts: AtomicUsize,
        stops: AtomicUsize,
        fail: bool,
    }

    impl Module for Probe {
        fn id(&self) -> &'static str {
            if self.fail {
                "broken"
            } else {
                "probe"
            }
        }
        fn start(&self, _ctx: ModuleContext) -> Result<()> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(PrekuError::InvalidInput("boom".into()))
            } else {
                Ok(())
            }
        }
        fn stop(&self) -> Result<()> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn registry(modules: Vec<Arc<dyn Module>>) -> ModuleRegistry {
        let ctx = ModuleContext {
            bus: EventBus::default(),
            storage: Arc::new(Storage::open_in_memory().unwrap()),
            data_dir: std::env::temp_dir(),
            quiet: Arc::default(),
            privacy: Arc::default(),
        };
        ModuleRegistry::new(ctx, modules)
    }

    #[test]
    fn starts_enabled_and_stops_disabled_modules() {
        let probe = Arc::new(Probe::default());
        let reg = registry(vec![probe.clone()]);
        let mut enabled = BTreeMap::new();

        reg.apply(&enabled, false); // default: enabled
        reg.apply(&enabled, false); // idempotent
        assert_eq!(probe.starts.load(Ordering::SeqCst), 1);

        enabled.insert("probe".into(), false);
        reg.apply(&enabled, false);
        assert_eq!(probe.stops.load(Ordering::SeqCst), 1);
        assert_eq!(reg.list(&enabled)[0].status, ModuleStatus::Stopped);
    }

    #[test]
    fn pause_stops_everything() {
        let probe = Arc::new(Probe::default());
        let reg = registry(vec![probe.clone()]);
        reg.apply(&BTreeMap::new(), false);
        reg.apply(&BTreeMap::new(), true);
        assert_eq!(probe.stops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn failing_module_is_degraded_not_fatal() {
        let broken = Arc::new(Probe {
            fail: true,
            ..Probe::default()
        });
        let reg = registry(vec![broken]);
        reg.apply(&BTreeMap::new(), false);
        assert!(matches!(
            reg.list(&BTreeMap::new())[0].status,
            ModuleStatus::Degraded { .. }
        ));
    }
}
