//! Background loop that turns bus events into UI state: the live-item queue and Pip's mood.

use std::{
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};

use chrono::Timelike;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast::{error::RecvError, Receiver};

use crate::{
    foundation::event_bus::AppEvent,
    live::{LiveItem, LiveQueue},
    pip::mood_engine::{MoodEngine, PipMood},
};

pub const LIVE_EVENT: &str = "live:items";
pub const MOOD_EVENT: &str = "pip:mood";

/// Expired items and finished reactions are noticed within this interval.
const TICK: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct LiveState(Mutex<LiveQueue>);

impl LiveState {
    pub fn items(&self) -> Vec<LiveItem> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .sorted()
    }
}

pub struct MoodState {
    engine: Mutex<MoodEngine>,
    current: Mutex<PipMood>,
}

impl Default for MoodState {
    fn default() -> Self {
        Self {
            engine: Mutex::new(MoodEngine::default()),
            current: Mutex::new(PipMood::Idle),
        }
    }
}

impl MoodState {
    pub fn current(&self) -> PipMood {
        *self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// `events` must be subscribed *before* modules start, so no event is missed.
pub fn spawn(app: AppHandle, mut events: Receiver<AppEvent>) {
    tauri::async_runtime::spawn(async move {
        refresh_mood(&app);
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                event = events.recv() => match event {
                    Ok(event) => handle(&app, event),
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "runtime fell behind the event bus");
                    }
                    Err(RecvError::Closed) => break,
                },
                _ = ticker.tick() => {
                    let pruned = with_queue(&app, |q| q.prune(now_ms()));
                    if pruned {
                        emit_items(&app);
                    }
                    refresh_mood(&app);
                }
            }
        }
    });
}

fn handle(app: &AppHandle, event: AppEvent) {
    match event {
        AppEvent::LiveUpsert(item) => {
            with_queue(app, |q| q.upsert(item));
            emit_items(app);
        }
        AppEvent::LiveRemove { id } => {
            if with_queue(app, |q| q.remove(&id)) {
                emit_items(app);
            }
        }
        AppEvent::LiveClearModule { module } => {
            if with_queue(app, |q| q.remove_module(module)) {
                emit_items(app);
            }
        }
        AppEvent::ModuleUpdate { event, payload } => {
            if let Err(err) = app.emit(event, payload) {
                tracing::warn!(error = %err, event, "failed to emit module update");
            }
        }
        AppEvent::Pip(signal) => {
            app.state::<MoodState>()
                .engine
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .react(signal, Instant::now());
            refresh_mood(app);
        }
    }
}

fn with_queue<T>(app: &AppHandle, f: impl FnOnce(&mut LiveQueue) -> T) -> T {
    let state = app.state::<LiveState>();
    let mut queue = state.0.lock().unwrap_or_else(PoisonError::into_inner);
    f(&mut queue)
}

fn emit_items(app: &AppHandle) {
    let items = app.state::<LiveState>().items();
    if let Err(err) = app.emit(LIVE_EVENT, items) {
        tracing::warn!(error = %err, "failed to emit live items");
    }
}

fn refresh_mood(app: &AppHandle) {
    let state = app.state::<MoodState>();
    let hour = chrono::Local::now().hour();
    let mood = state
        .engine
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .mood(Instant::now(), hour);
    let changed = {
        let mut current = state.current.lock().unwrap_or_else(PoisonError::into_inner);
        std::mem::replace(&mut *current, mood) != mood
    };
    if changed {
        if let Err(err) = app.emit(MOOD_EVENT, mood) {
            tracing::warn!(error = %err, "failed to emit pip mood");
        }
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
