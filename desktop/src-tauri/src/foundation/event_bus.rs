//! In-process event bus. Modules publish here and never call each other directly
//! (Architecture.md §3.2). The runtime (`runtime.rs`) is the main subscriber.

use tokio::sync::broadcast;

use crate::{live::LiveItem, pip::mood_engine::PipSignal};

#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Add or replace a live item (matched by id).
    LiveUpsert(LiveItem),
    LiveRemove {
        id: String,
    },
    /// Remove every live item of a module (e.g. when it stops).
    LiveClearModule {
        module: &'static str,
    },
    /// Something happened that Pip should react to.
    Pip(PipSignal),
    /// Module-specific state for the UI, forwarded as a Tauri event named `event`.
    ModuleUpdate {
        event: &'static str,
        payload: serde_json::Value,
    },
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }
}

impl EventBus {
    /// Publishing with no subscribers is fine (e.g. during shutdown), so errors are ignored.
    pub fn publish(&self, event: AppEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribers_receive_published_events() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        bus.publish(AppEvent::Pip(PipSignal::Success));
        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::Pip(PipSignal::Success))
        ));
    }

    #[test]
    fn publishing_without_subscribers_does_not_fail() {
        EventBus::default().publish(AppEvent::LiveRemove { id: "x".into() });
    }
}
