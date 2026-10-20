//! Demo module: cycles through sample live items so the island and Pip can be seen working
//! before real modules exist. On by default in debug builds only; toggle it in Settings.

use std::{
    sync::{Mutex, PoisonError},
    time::Duration,
};

use tauri::async_runtime::JoinHandle;

use super::{Module, ModuleContext};
use crate::{
    foundation::{errors::Result, event_bus::AppEvent},
    live::{LiveItem, LivePriority},
    pip::mood_engine::PipSignal,
};

const ID: &str = "demo";
const ITEM_ID: &str = "demo.item";
const SCENE_EVERY: Duration = Duration::from_secs(15);
const SCENE_LASTS_MS: i64 = 8_000;

struct Scene {
    priority: LivePriority,
    title: &'static str,
    subtitle: &'static str,
    progress: Option<f32>,
    signal: Option<PipSignal>,
}

// Demo-only text, deliberately not localised.
const SCENES: &[Scene] = &[
    Scene {
        priority: LivePriority::Info,
        title: "Hello from Pip",
        subtitle: "Demo item",
        progress: None,
        signal: Some(PipSignal::Success),
    },
    Scene {
        priority: LivePriority::Timer,
        title: "Focus",
        subtitle: "15 min left",
        progress: Some(0.4),
        signal: None,
    },
    Scene {
        priority: LivePriority::Alert,
        title: "Disk almost full",
        subtitle: "Demo alert",
        progress: None,
        signal: Some(PipSignal::Alert),
    },
    Scene {
        priority: LivePriority::Info,
        title: "Build passed",
        subtitle: "Demo celebration",
        progress: None,
        signal: Some(PipSignal::Celebration),
    },
];

#[derive(Default)]
pub struct DemoModule {
    running: Mutex<Option<(JoinHandle<()>, ModuleContext)>>,
}

impl Module for DemoModule {
    fn id(&self) -> &'static str {
        ID
    }

    fn enabled_by_default(&self) -> bool {
        cfg!(debug_assertions)
    }

    fn start(&self, ctx: ModuleContext) -> Result<()> {
        let bus = ctx.bus.clone();
        let task = tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(SCENE_EVERY);
            for scene in SCENES.iter().cycle() {
                ticker.tick().await;
                let mut item = LiveItem::new(
                    ITEM_ID,
                    ID,
                    scene.priority,
                    scene.title,
                    Some(scene.subtitle.into()),
                )
                .expires_in(SCENE_LASTS_MS);
                item.progress = scene.progress;
                bus.publish(AppEvent::LiveUpsert(item));
                if let Some(signal) = scene.signal {
                    bus.publish(AppEvent::Pip(signal));
                }
            }
        });
        *self.running.lock().unwrap_or_else(PoisonError::into_inner) = Some((task, ctx));
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if let Some((task, ctx)) = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            task.abort();
            ctx.bus.publish(AppEvent::LiveRemove { id: ITEM_ID.into() });
        }
        Ok(())
    }
}
