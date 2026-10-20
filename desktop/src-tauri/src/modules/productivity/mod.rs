//! Productivity module (Phase 2, PRD A1–A4): focus timer, tasks, clipboard history, media.
//!
//! Runs three background jobs while enabled: a 1 s timer tick (tokio task), a clipboard
//! watcher thread and a media watcher thread (both blocking OS APIs). All UI state goes out
//! through the event bus as `productivity:*` events and live items.

pub mod calendar;
pub mod clipboard;
pub mod ics;
pub mod media;
mod platform;
pub mod tasks;
pub mod timer;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, PoisonError,
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;

use self::{
    clipboard::{Captured, ClipItem, ClipStore},
    media::{MediaAction, NowPlaying},
    tasks::Task,
    timer::{FocusTimer, SessionRecord, TimerSnapshot, TimerStatus},
};
use super::{Module, ModuleContext};
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        event_bus::AppEvent,
        storage::now_millis,
    },
    live::{LiveItem, LivePriority},
    pip::mood_engine::PipSignal,
};

pub const ID: &str = "productivity";

/// Tauri events emitted to the frontend.
pub mod events {
    pub const TIMER: &str = "productivity:timer";
    pub const TIMER_FINISHED: &str = "productivity:timer-finished";
    pub const TASKS: &str = "productivity:tasks";
    pub const CLIPBOARD: &str = "productivity:clipboard";
    pub const MEDIA: &str = "productivity:media";
    pub const TODAY: &str = "productivity:today";
}

const LIVE_TIMER: &str = "productivity.timer";
const LIVE_TIMER_DONE: &str = "productivity.timer-done";
const LIVE_TASK: &str = "productivity.task";
const LIVE_DUE: &str = "productivity.due";
/// How often task reminder times are checked.
const DUE_CHECK_EVERY_S: u32 = 15;
const DUE_ITEM_MS: i64 = 15 * 60_000;
const LIVE_MEDIA: &str = "productivity.media";
const LIVE_IDS: [&str; 5] = [LIVE_TIMER, LIVE_TIMER_DONE, LIVE_TASK, LIVE_MEDIA, LIVE_DUE];

const CLIPBOARD_POLL: Duration = Duration::from_millis(500);
const MEDIA_POLL: Duration = Duration::from_secs(2);
const DONE_ITEM_MS: i64 = 8_000;

struct Running {
    ctx: ModuleContext,
    clips: Arc<ClipStore>,
    stop: Arc<AtomicBool>,
    ticker: JoinHandle<()>,
    calendar: Arc<calendar::Calendar>,
    calendar_task: JoinHandle<()>,
}

#[derive(Default)]
pub struct ProductivityModule {
    timer: Arc<Mutex<FocusTimer>>,
    running: Mutex<Option<Running>>,
}

impl Module for ProductivityModule {
    fn id(&self) -> &'static str {
        ID
    }

    fn start(&self, ctx: ModuleContext) -> Result<()> {
        let clips = Arc::new(ClipStore::new(ctx.data_dir.join("clipboard")));
        let stop = Arc::new(AtomicBool::new(false));

        let ticker = {
            let (ctx, timer) = (ctx.clone(), self.timer.clone());
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                let mut seconds = 0u32;
                loop {
                    interval.tick().await;
                    tick_timer(&ctx, &timer);
                    seconds = seconds.wrapping_add(1);
                    if seconds % DUE_CHECK_EVERY_S == 0 {
                        check_due_tasks(&ctx);
                    }
                }
            })
        };
        spawn_clipboard_watcher(ctx.clone(), clips.clone(), stop.clone())?;
        spawn_media_watcher(ctx.clone(), stop.clone())?;

        publish_tasks(&ctx);
        publish_clipboard(&ctx, &clips);
        publish_timer(&ctx, &lock(&self.timer).snapshot(now_millis()));

        // Optional: does nothing (and contacts nothing) until the user adds a calendar.
        let calendar = calendar::Calendar::load(ctx.clone());
        let calendar_task = calendar.spawn();

        *lock(&self.running) = Some(Running {
            calendar,
            calendar_task,
            ctx,
            clips,
            stop,
            ticker,
        });
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let Some(running) = lock(&self.running).take() else {
            return Ok(());
        };
        running.stop.store(true, Ordering::SeqCst);
        running.ticker.abort();
        running.calendar_task.abort();
        running.calendar.stop();
        if let Some(record) = lock(&self.timer).stop(now_millis()) {
            save_session(&running.ctx, &record);
            running
                .ctx
                .bus
                .publish(AppEvent::Pip(PipSignal::FocusEnded));
        }
        for id in LIVE_IDS {
            running
                .ctx
                .bus
                .publish(AppEvent::LiveRemove { id: id.into() });
        }
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn emit(ctx: &ModuleContext, event: &'static str, payload: impl Serialize) {
    match serde_json::to_value(payload) {
        Ok(payload) => ctx.bus.publish(AppEvent::ModuleUpdate { event, payload }),
        Err(err) => tracing::warn!(error = %err, event, "failed to serialize module update"),
    }
}

fn live(id: &str, priority: LivePriority, title: String, subtitle: Option<String>) -> LiveItem {
    LiveItem::new(id, ID, priority, title, subtitle)
}

/// `mm:ss`, or `h:mm:ss` for an hour or more. Language-neutral, so no i18n needed.
fn clock(seconds: u32) -> String {
    let (h, m, s) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

// ---------------------------------------------------------------- timer

fn publish_timer(ctx: &ModuleContext, snapshot: &TimerSnapshot) {
    emit(ctx, events::TIMER, snapshot);
    // Routine prompts hold while the timer runs (not while it is paused or idle).
    if ctx
        .quiet
        .set_focusing(snapshot.status == TimerStatus::Running)
    {
        ctx.emit(
            crate::foundation::quiet::EVENT,
            ctx.quiet.state(now_millis()),
        );
    }
    if snapshot.status == TimerStatus::Idle {
        ctx.bus.publish(AppEvent::LiveRemove {
            id: LIVE_TIMER.into(),
        });
        return;
    }
    let paused = snapshot.status == TimerStatus::Paused;
    let mut item = live(
        LIVE_TIMER,
        LivePriority::Timer,
        snapshot.label.clone(),
        Some(format!(
            "{}{}",
            if paused { "⏸ " } else { "" },
            clock(snapshot.remaining_s)
        )),
    );
    let total = snapshot.total_s.max(1) as f32;
    item.progress = Some(1.0 - snapshot.remaining_s as f32 / total);
    ctx.bus.publish(AppEvent::LiveUpsert(item));
}

fn tick_timer(ctx: &ModuleContext, timer: &Mutex<FocusTimer>) {
    let now = now_millis();
    let (finished, snapshot, active) = {
        let mut timer = lock(timer);
        let finished = timer.tick(now);
        (finished, timer.snapshot(now), timer.is_active())
    };
    if let Some(record) = finished {
        save_session(ctx, &record);
        ctx.bus.publish(AppEvent::Pip(PipSignal::FocusEnded));
        ctx.bus.publish(AppEvent::Pip(PipSignal::Celebration));
        let mut done = live(
            LIVE_TIMER_DONE,
            LivePriority::Alert,
            record.label.clone(),
            Some("✓".into()),
        );
        done.expires_at = Some(now + DONE_ITEM_MS);
        ctx.bus.publish(AppEvent::LiveUpsert(done));
        emit(ctx, events::TIMER_FINISHED, &record.label);
        publish_timer(ctx, &snapshot);
    } else if active {
        publish_timer(ctx, &snapshot);
    }
}

/// Stores a session; a task's focus totals change, so the task list goes out again.
fn save_session(ctx: &ModuleContext, record: &SessionRecord) {
    match ctx
        .storage
        .with_conn(|conn| tasks::save_session(conn, record, now_millis()))
    {
        Ok(true) if record.task_id.is_some() => publish_tasks(ctx),
        Ok(true) => publish_today(ctx),
        Ok(false) => {}
        Err(err) => tracing::warn!(error = %err, "failed to save focus session"),
    }
}

// ---------------------------------------------------------------- tasks

fn publish_tasks(ctx: &ModuleContext) {
    let result = ctx
        .storage
        .with_conn(|conn| Ok((tasks::list(conn)?, tasks::top_open(conn)?)));
    match result {
        Ok((all, top)) => {
            emit(ctx, events::TASKS, &all);
            publish_today(ctx);
            ctx.bus.publish(match top {
                Some(task) => {
                    AppEvent::LiveUpsert(live(LIVE_TASK, LivePriority::Info, task.title, None))
                }
                None => AppEvent::LiveRemove {
                    id: LIVE_TASK.into(),
                },
            });
        }
        Err(err) => tracing::warn!(error = %err, "failed to load tasks"),
    }
}

/// Task reminders (PRD A11): one notice when a task's time comes. They wait while the user
/// paused reminders or is in a focus session, and then arrive as a single notice.
fn check_due_tasks(ctx: &ModuleContext) {
    let now = now_millis();
    if ctx.quiet.is_quiet(now) {
        return;
    }
    match ctx.storage.with_conn(|conn| tasks::take_due(conn, now)) {
        Ok(Some(notice)) => {
            let more = (notice.others > 0).then(|| format!("+{} more due", notice.others));
            let mut item = live(
                LIVE_DUE,
                LivePriority::Alert,
                format!("⏰ {}", notice.title),
                Some(more.unwrap_or_else(|| "due now".to_owned())),
            );
            item.expires_at = Some(now + DUE_ITEM_MS);
            ctx.bus.publish(AppEvent::LiveUpsert(item));
            ctx.bus.publish(AppEvent::Pip(PipSignal::Alert));
        }
        Ok(None) => {}
        Err(err) => tracing::warn!(error = %err, "failed to check task reminders"),
    }
}

/// Today's recap follows every task change and every stored session.
fn publish_today(ctx: &ModuleContext) {
    let summary = ctx
        .storage
        .with_conn(|conn| tasks::summary(conn, &tasks::local_day(), tasks::local_day_start_ms()));
    match summary {
        Ok(summary) => emit(ctx, events::TODAY, summary),
        Err(err) => tracing::warn!(error = %err, "failed to build today's recap"),
    }
}

// ---------------------------------------------------------------- clipboard

fn publish_clipboard(ctx: &ModuleContext, clips: &ClipStore) {
    match ctx.storage.with_conn(|conn| clips.list(conn)) {
        Ok(items) => emit(ctx, events::CLIPBOARD, &items),
        Err(err) => tracing::warn!(error = %err, "failed to load clipboard history"),
    }
}

fn read_clipboard(clipboard: &mut arboard::Clipboard) -> Option<Captured> {
    if let Ok(text) = clipboard.get_text() {
        return Some(Captured::Text(text));
    }
    clipboard.get_image().ok().map(|image| Captured::Image {
        width: image.width,
        height: image.height,
        rgba: image.bytes.into_owned(),
    })
}

fn spawn_clipboard_watcher(
    ctx: ModuleContext,
    clips: Arc<ClipStore>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    thread::Builder::new()
        .name("preku-clipboard".into())
        .spawn(move || {
            let mut clipboard = match arboard::Clipboard::new() {
                Ok(clipboard) => clipboard,
                Err(err) => {
                    tracing::warn!(error = %err, "clipboard unavailable; history disabled");
                    return;
                }
            };
            let mut last_marker = None;
            let mut last_fingerprint = None;
            while !stop.load(Ordering::SeqCst) {
                thread::sleep(CLIPBOARD_POLL);
                // Windows: skip unless the sequence number moved. Linux: always read.
                let marker = platform::clipboard_change_marker();
                if marker.is_some() {
                    if marker == last_marker {
                        continue;
                    }
                    last_marker = marker;
                }
                let sensitive = platform::clipboard_is_sensitive();
                let Some(captured) = read_clipboard(&mut clipboard) else {
                    continue;
                };
                if ctx.privacy.clipboard_paused() || !clipboard::should_record(&captured, sensitive)
                {
                    continue;
                }
                let fingerprint = clipboard::fingerprint(&captured);
                if last_fingerprint.as_ref() == Some(&fingerprint) {
                    continue;
                }
                last_fingerprint = Some(fingerprint);
                match ctx
                    .storage
                    .with_conn(|conn| clips.record(conn, &captured, now_millis()))
                {
                    Ok(()) => publish_clipboard(&ctx, &clips),
                    // Never log the content itself (Rules.md §4).
                    Err(err) => tracing::warn!(error = %err, "failed to store clipboard item"),
                }
            }
        })?;
    Ok(())
}

// ---------------------------------------------------------------- media

fn publish_media(ctx: &ModuleContext, now: &Option<NowPlaying>) {
    emit(ctx, events::MEDIA, now);
    ctx.bus.publish(match now {
        Some(track) if track.playing => AppEvent::LiveUpsert(live(
            LIVE_MEDIA,
            LivePriority::Media,
            track.title.clone(),
            track.artist.clone(),
        )),
        _ => AppEvent::LiveRemove {
            id: LIVE_MEDIA.into(),
        },
    });
}

fn spawn_media_watcher(ctx: ModuleContext, stop: Arc<AtomicBool>) -> Result<()> {
    thread::Builder::new()
        .name("preku-media".into())
        .spawn(move || {
            let mut last: Option<Option<NowPlaying>> = None;
            while !stop.load(Ordering::SeqCst) {
                let now = platform::now_playing();
                if last.as_ref() != Some(&now) {
                    publish_media(&ctx, &now);
                    last = Some(now);
                }
                thread::sleep(MEDIA_POLL);
            }
        })?;
    Ok(())
}

// ---------------------------------------------------------------- commands

impl ProductivityModule {
    fn running(&self) -> Result<(ModuleContext, Arc<ClipStore>)> {
        lock(&self.running)
            .as_ref()
            .map(|r| (r.ctx.clone(), r.clips.clone()))
            .ok_or_else(|| {
                PrekuError::InvalidInput("Productivity is turned off in Settings".into())
            })
    }

    /// The optional calendar (PRD A5).
    pub fn calendar(&self) -> Result<Arc<calendar::Calendar>> {
        lock(&self.running)
            .as_ref()
            .map(|r| r.calendar.clone())
            .ok_or_else(|| {
                PrekuError::InvalidInput("Productivity is turned off in Settings".into())
            })
    }

    pub fn timer_snapshot(&self) -> TimerSnapshot {
        lock(&self.timer).snapshot(now_millis())
    }

    /// Starts focusing; with a task, its current title is the label (no retyping).
    pub fn timer_start(
        &self,
        label: &str,
        minutes: u32,
        task_id: Option<&str>,
    ) -> Result<TimerSnapshot> {
        let (ctx, _) = self.running()?;
        let task = match task_id {
            Some(id) => Some(ctx.storage.with_conn(|conn| tasks::find_open(conn, id))?),
            None => None,
        };
        let label = task.as_ref().map_or(label, |task| task.title.as_str());
        let now = now_millis();
        let (abandoned, snapshot) = {
            let mut timer = lock(&self.timer);
            let abandoned =
                timer.start(label, task.as_ref().map(|t| t.id.clone()), minutes, now)?;
            (abandoned, timer.snapshot(now))
        };
        if let Some(record) = abandoned {
            save_session(&ctx, &record);
        }
        ctx.bus.publish(AppEvent::LiveRemove {
            id: LIVE_TIMER_DONE.into(),
        });
        ctx.bus.publish(AppEvent::Pip(PipSignal::FocusStarted));
        publish_timer(&ctx, &snapshot);
        Ok(snapshot)
    }

    /// Closes the "session finished" card without doing anything else.
    pub fn timer_dismiss_finished(&self) -> Result<TimerSnapshot> {
        let (ctx, _) = self.running()?;
        let snapshot = {
            let mut timer = lock(&self.timer);
            timer.dismiss_finished();
            timer.snapshot(now_millis())
        };
        emit(&ctx, events::TIMER, &snapshot);
        Ok(snapshot)
    }

    pub fn timer_pause(&self) -> Result<TimerSnapshot> {
        self.timer_change(PipSignal::FocusEnded, |timer, now| timer.pause(now))
    }

    pub fn timer_resume(&self) -> Result<TimerSnapshot> {
        self.timer_change(PipSignal::FocusStarted, |timer, now| timer.resume(now))
    }

    pub fn timer_stop(&self) -> Result<TimerSnapshot> {
        let (ctx, _) = self.running()?;
        let now = now_millis();
        let (record, snapshot) = {
            let mut timer = lock(&self.timer);
            (timer.stop(now), timer.snapshot(now))
        };
        if let Some(record) = record {
            save_session(&ctx, &record);
        }
        ctx.bus.publish(AppEvent::Pip(PipSignal::FocusEnded));
        publish_timer(&ctx, &snapshot);
        Ok(snapshot)
    }

    fn timer_change(
        &self,
        signal: PipSignal,
        change: impl FnOnce(&mut FocusTimer, i64),
    ) -> Result<TimerSnapshot> {
        let (ctx, _) = self.running()?;
        let now = now_millis();
        let snapshot = {
            let mut timer = lock(&self.timer);
            change(&mut timer, now);
            timer.snapshot(now)
        };
        ctx.bus.publish(AppEvent::Pip(signal));
        publish_timer(&ctx, &snapshot);
        Ok(snapshot)
    }

    pub fn tasks_list(&self) -> Result<Vec<Task>> {
        let (ctx, _) = self.running()?;
        ctx.storage.with_conn(|conn| tasks::list(conn))
    }

    /// Runs a task mutation, then broadcasts the new list.
    fn tasks_change(
        &self,
        change: impl FnOnce(&mut rusqlite::Connection, i64) -> Result<()>,
    ) -> Result<Vec<Task>> {
        let (ctx, _) = self.running()?;
        let list = ctx.storage.with_conn(|conn| {
            change(conn, now_millis())?;
            tasks::list(conn)
        })?;
        publish_tasks(&ctx);
        Ok(list)
    }

    pub fn task_add(&self, title: &str) -> Result<Vec<Task>> {
        self.tasks_change(|conn, now| tasks::add(conn, title, now).map(|_| ()))
    }

    pub fn task_rename(&self, id: &str, title: &str) -> Result<Vec<Task>> {
        self.tasks_change(|conn, now| tasks::rename(conn, id, title, now))
    }

    pub fn task_set_done(&self, id: &str, done: bool) -> Result<Vec<Task>> {
        let (ctx, _) = self.running()?;
        let list = self.tasks_change(|conn, now| tasks::set_done(conn, id, done, now))?;
        if done {
            ctx.bus.publish(AppEvent::Pip(PipSignal::Success));
            // "Mark done" on the finished card answers it.
            let snapshot = {
                let mut timer = lock(&self.timer);
                let answered = timer
                    .snapshot(now_millis())
                    .finished
                    .is_some_and(|f| f.task_id.as_deref() == Some(id));
                answered.then(|| {
                    timer.dismiss_finished();
                    timer.snapshot(now_millis())
                })
            };
            if let Some(snapshot) = snapshot {
                emit(&ctx, events::TIMER, &snapshot);
            }
        }
        Ok(list)
    }

    /// Picks or unpicks a task as one of today's priorities.
    pub fn task_set_priority(&self, id: &str, on: bool) -> Result<Vec<Task>> {
        self.tasks_change(|conn, now| tasks::set_priority(conn, id, on, &tasks::local_day(), now))
    }

    /// Sets or clears a task's reminder time.
    pub fn task_set_due(&self, id: &str, due_at: Option<i64>) -> Result<Vec<Task>> {
        let (ctx, _) = self.running()?;
        let list = self.tasks_change(|conn, now| tasks::set_due(conn, id, due_at, now))?;
        // The note about it (if it is showing) has been answered.
        ctx.bus.publish(AppEvent::LiveRemove {
            id: LIVE_DUE.into(),
        });
        Ok(list)
    }

    pub fn today_summary(&self) -> Result<tasks::TodaySummary> {
        let (ctx, _) = self.running()?;
        ctx.storage.with_conn(|conn| {
            tasks::summary(conn, &tasks::local_day(), tasks::local_day_start_ms())
        })
    }

    pub fn task_delete(&self, id: &str) -> Result<Vec<Task>> {
        self.tasks_change(|conn, now| tasks::delete(conn, id, now))
    }

    pub fn tasks_reorder(&self, ids: &[String]) -> Result<Vec<Task>> {
        self.tasks_change(|conn, now| tasks::reorder(conn, ids, now))
    }

    pub fn clipboard_list(&self) -> Result<Vec<ClipItem>> {
        let (ctx, clips) = self.running()?;
        ctx.storage.with_conn(|conn| clips.list(conn))
    }

    /// Puts an item back on the system clipboard (the watcher then bumps it to the top).
    pub fn clipboard_copy(&self, id: &str) -> Result<()> {
        let (ctx, clips) = self.running()?;
        let content = ctx.storage.with_conn(|conn| clips.content(conn, id))?;
        let to_err = |e: arboard::Error| PrekuError::Io(std::io::Error::other(e));
        let mut clipboard = arboard::Clipboard::new().map_err(to_err)?;
        match content {
            Captured::Text(text) => clipboard.set_text(text),
            Captured::Image {
                width,
                height,
                rgba,
            } => clipboard.set_image(arboard::ImageData {
                width,
                height,
                bytes: rgba.into(),
            }),
        }
        .map_err(to_err)
    }

    fn clipboard_change(
        &self,
        change: impl FnOnce(&ClipStore, &rusqlite::Connection) -> Result<()>,
    ) -> Result<Vec<ClipItem>> {
        let (ctx, clips) = self.running()?;
        let list = ctx.storage.with_conn(|conn| {
            change(&clips, conn)?;
            clips.list(conn)
        })?;
        emit(&ctx, events::CLIPBOARD, &list);
        Ok(list)
    }

    pub fn clipboard_set_pinned(&self, id: &str, pinned: bool) -> Result<Vec<ClipItem>> {
        self.clipboard_change(|clips, conn| clips.set_pinned(conn, id, pinned))
    }

    pub fn clipboard_delete(&self, id: &str) -> Result<Vec<ClipItem>> {
        self.clipboard_change(|clips, conn| clips.delete(conn, id))
    }

    pub fn clipboard_clear(&self) -> Result<Vec<ClipItem>> {
        self.clipboard_change(|clips, conn| clips.clear_unpinned(conn))
    }

    pub fn clipboard_image(&self, id: &str) -> Result<String> {
        let (ctx, clips) = self.running()?;
        ctx.storage.with_conn(|conn| clips.image_data_url(conn, id))
    }

    /// Blocking (OS media APIs): call from a blocking thread.
    pub fn media_now(&self) -> Result<Option<NowPlaying>> {
        self.running()?;
        Ok(platform::now_playing())
    }

    /// Blocking (OS media APIs): call from a blocking thread.
    pub fn media_command(&self, action: MediaAction) -> Result<Option<NowPlaying>> {
        let (ctx, _) = self.running()?;
        platform::media_command(action)?;
        let now = platform::now_playing();
        publish_media(&ctx, &now);
        Ok(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_format() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(25 * 60), "25:00");
        assert_eq!(clock(61), "01:01");
        assert_eq!(clock(3_725), "1:02:05");
    }
}
