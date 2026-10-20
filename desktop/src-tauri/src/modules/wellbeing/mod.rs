//! Wellbeing (Phase 4, PRD W1–W3, W6, C10): break and water reminders, habits with streaks,
//! per-app screen time and Do-Not-Disturb during fullscreen apps.
//!
//! One background task samples activity every 5 s (foreground app, idle time, fullscreen),
//! feeds the reminder scheduler and the screen-time buffer, and flushes screen time every
//! minute. Everything stays on this device until the phone sync (Phase 8).

pub mod companion;
pub mod habits;
pub mod platform;
pub mod reminders;
pub mod screen_time;
pub mod settings;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard, PoisonError,
    },
    time::Duration,
};

use chrono::NaiveDate;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use ts_rs::TS;

use self::{
    habits::HabitView,
    reminders::{Prompt, ReminderKind, Scheduler, AWAY_AFTER_MS},
    screen_time::{AppUsage, Buffer},
    settings::WellbeingSettings,
};
use super::{Module, ModuleContext};
use crate::{
    foundation::{
        apps::friendly_app_name,
        errors::{PrekuError, Result},
        event_bus::AppEvent,
        storage::now_millis,
    },
    live::{LiveItem, LivePriority},
    pip::mood_engine::PipSignal,
};

pub const ID: &str = "wellbeing";

pub mod events {
    pub const STATE: &str = "wellbeing:state";
}

const TICK: Duration = Duration::from_secs(5);
const TICK_SECONDS: u32 = 5;
/// Flush screen time (and refresh the UI) every minute.
const FLUSH_EVERY_TICKS: u32 = 12;
/// An unanswered prompt goes away by itself after this long (no nagging).
const PROMPT_TTL_MS: i64 = 15 * 60_000;
/// After this long without an answer, a break invitation may be replaced by a nudge.
const STALE_BREAK_MS: i64 = 3 * 60_000;
/// Drive-by nudges (posture, water): Pip pops in, says it and leaves again.
const NUDGE_TTL_MS: i64 = 60_000;

/// How long an unanswered prompt of this kind stays.
fn prompt_ttl_ms(kind: ReminderKind) -> i64 {
    if matches!(kind, ReminderKind::Posture | ReminderKind::Water) {
        NUDGE_TTL_MS
    } else {
        PROMPT_TTL_MS
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WellbeingState {
    pub settings: WellbeingSettings,
    pub water_ml: u32,
    pub habits: Vec<HabitView>,
    pub screen_apps: Vec<AppUsage>,
    pub screen_total_s: u32,
    /// Daily totals for the last 7 days, oldest first (today last).
    pub screen_week_s: Vec<u32>,
    pub screen_supported: bool,
    /// A fullscreen app or presentation is running: reminders and pop-outs wait.
    pub dnd: bool,
    pub next_break_s: Option<u32>,
    /// The companion's current invitation, waiting for Take a break / Later.
    pub prompt: Option<Prompt>,
    /// Breaks the user finished today.
    pub breaks_today: u32,
    /// Today's mood check-in, if the user opted in and answered.
    pub mood_today: Option<String>,
    /// The last seven days as plain counts (the weekly reflection), oldest first.
    pub week: Vec<companion::DayReflection>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

struct Shared {
    ctx: ModuleContext,
    settings: Mutex<WellbeingSettings>,
    scheduler: Mutex<Scheduler>,
    buffer: Mutex<Buffer>,
    dnd: AtomicBool,
    supported: AtomicBool,
    prompt: Mutex<Option<Prompt>>,
}

#[derive(Default)]
pub struct WellbeingModule {
    running: Mutex<Option<(Arc<Shared>, JoinHandle<()>)>>,
}

impl Module for WellbeingModule {
    fn id(&self) -> &'static str {
        ID
    }

    fn start(&self, ctx: ModuleContext) -> Result<()> {
        let settings = settings::load(&ctx.storage)?;
        let shared = Arc::new(Shared {
            ctx,
            settings: Mutex::new(settings),
            scheduler: Mutex::new(Scheduler::seeded(now_millis() as u64)),
            buffer: Mutex::new(Buffer::default()),
            dnd: AtomicBool::new(false),
            supported: AtomicBool::new(true),
            prompt: Mutex::new(None),
        });
        let task = tauri::async_runtime::spawn(activity_loop(shared.clone()));
        *lock(&self.running) = Some((shared, task));
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if let Some((shared, task)) = lock(&self.running).take() {
            task.abort();
            // Keep the last minute of screen time.
            if let Err(err) = flush(&shared) {
                tracing::warn!(error = %err, "failed to save screen time on stop");
            }
            shared.ctx.publish(AppEvent::LiveClearModule { module: ID });
        }
        Ok(())
    }
}

fn flush(shared: &Shared) -> Result<()> {
    let mut buffer = lock(&shared.buffer);
    shared
        .ctx
        .storage
        .with_conn(|conn| buffer.flush(conn, now_millis()))
}

/// Short fallback titles; the UI replaces them with translated text via `kind`.
fn reminder_icon(kind: ReminderKind) -> &'static str {
    match kind {
        ReminderKind::Eyes => "👀",
        ReminderKind::Stand => "🧍",
        ReminderKind::Posture => "🪑",
        ReminderKind::Water => "💧",
        ReminderKind::ScreenBalance => "🌿",
        ReminderKind::Bedtime => "🌙",
        ReminderKind::Meal => "🍽️",
        ReminderKind::MoodCheckIn => "💬",
        ReminderKind::Reset => "🫧",
    }
}

async fn activity_loop(shared: Arc<Shared>) {
    let mut ticks: u32 = 0;
    loop {
        tokio::time::sleep(TICK).await;
        let activity = tauri::async_runtime::spawn_blocking(platform::sample)
            .await
            .unwrap_or_default();
        let day = today();
        let settings = lock(&shared.settings).clone();

        let active = activity.idle_ms.map_or(true, |idle| idle < AWAY_AFTER_MS);
        if settings.screen_time_enabled && active {
            if let Some(app) = activity.app.as_deref().and_then(friendly_app_name) {
                lock(&shared.buffer).add(day, &app, TICK_SECONDS);
            }
        }

        let water_goal_met = shared
            .ctx
            .storage
            .with_conn(|conn| habits::water_today(conn, day))
            .is_ok_and(|ml| ml >= settings.water_goal_ml);
        // Unanswered prompts fade away by themselves.
        let now_ms = now_millis();
        let expired = {
            let mut prompt = lock(&shared.prompt);
            let expired = prompt
                .as_ref()
                .is_some_and(|p| now_ms - p.created_at > prompt_ttl_ms(p.kind));
            if expired {
                *prompt = None;
            }
            expired
        };
        let can_prompt = lock(&shared.prompt).is_none();
        // A break invitation nobody answered for a few minutes doesn't hold nudges back.
        let stale_break = lock(&shared.prompt)
            .as_ref()
            .is_some_and(|p| !p.kind.is_nudge() && now_ms - p.created_at > STALE_BREAK_MS);
        lock(&shared.scheduler).set_nudge_may_replace(stale_break);
        let shown = lock(&shared.scheduler).tick(
            &settings,
            TICK.as_millis() as u64,
            activity.idle_ms,
            // A pause the user set, a focus session, a game or a presentation: prompts wait.
            shared.ctx.quiet.is_quiet(now_ms),
            water_goal_met,
            chrono::Local::now().naive_local(),
            can_prompt,
        );
        if let Some((kind, active_min)) = shown {
            // A nudge taking over from an old invitation: that one leaves the notch.
            clear_prompt(&shared, None);
            let prompt = Prompt {
                id: format!("{}.{now_ms}", kind.key()),
                kind,
                active_min,
                since_min: lock(&shared.scheduler).since_min(),
                created_at: now_ms,
            };
            *lock(&shared.prompt) = Some(prompt);
            shared.ctx.publish(AppEvent::LiveUpsert(
                LiveItem::new(
                    format!("wellbeing.{}", kind.key()),
                    ID,
                    LivePriority::Alert,
                    reminder_icon(kind),
                    None,
                )
                .with_kind(kind.key())
                .expires_in(prompt_ttl_ms(kind)),
            ));
            shared.ctx.publish(AppEvent::Pip(PipSignal::Alert));
        }

        let screen_busy = shared.ctx.quiet.screen_busy();
        let dnd_changed = shared.dnd.swap(screen_busy, Ordering::SeqCst) != screen_busy;
        let support_changed =
            shared.supported.swap(activity.supported, Ordering::SeqCst) != activity.supported;
        ticks = ticks.wrapping_add(1);
        let flush_now = ticks % FLUSH_EVERY_TICKS == 0;
        if flush_now {
            if let Err(err) = flush(&shared) {
                tracing::warn!(error = %err, "failed to save screen time");
            }
        }
        if dnd_changed || support_changed || flush_now || shown.is_some() || expired {
            emit_state(&shared);
        }
    }
}

/// Clears the waiting prompt (if it is of `kind`, or any kind with `None`) and its pill.
fn clear_prompt(shared: &Shared, kind: Option<ReminderKind>) {
    let mut prompt = lock(&shared.prompt);
    if let Some(current) = prompt.as_ref() {
        if kind.map_or(true, |k| k == current.kind) {
            shared.ctx.publish(AppEvent::LiveRemove {
                id: format!("wellbeing.{}", current.kind.key()),
            });
            *prompt = None;
        }
    }
}

fn build_state(shared: &Shared) -> Result<WellbeingState> {
    flush(shared)?;
    let settings = lock(&shared.settings).clone();
    let day = today();
    let (water_ml, habits, (screen_apps, screen_total_s), screen_week_s, breaks_today, mood, week) =
        shared.ctx.storage.with_conn(|conn| {
            Ok((
                habits::water_today(conn, day)?,
                habits::list(conn, day)?,
                screen_time::day_usage(conn, day)?,
                screen_time::week_totals(conn, day)?,
                companion::breaks_today(conn, day)?,
                companion::mood_today(conn, day)?,
                companion::week(conn, day)?,
            ))
        })?;
    // Only shown while the check-in is on (turning it off hides it again).
    let mood_shown = settings.mood_check_in_enabled;
    let mood_today = mood.filter(|_| mood_shown);
    Ok(WellbeingState {
        next_break_s: lock(&shared.scheduler).next_break_s(&settings),
        settings,
        water_ml,
        habits,
        screen_apps,
        screen_total_s,
        screen_week_s,
        screen_supported: shared.supported.load(Ordering::SeqCst),
        dnd: shared.dnd.load(Ordering::SeqCst),
        prompt: lock(&shared.prompt).clone(),
        breaks_today,
        mood_today,
        // Moods show only while the check-in is on.
        week: week
            .into_iter()
            .map(|mut day| {
                if !mood_shown {
                    day.mood = None;
                }
                day
            })
            .collect(),
    })
}

fn emit_state(shared: &Shared) {
    match build_state(shared) {
        Ok(state) => shared.ctx.emit(events::STATE, &state),
        Err(err) => tracing::warn!(error = %err, "failed to build wellbeing state"),
    }
}

impl WellbeingModule {
    fn shared(&self) -> Result<Arc<Shared>> {
        lock(&self.running)
            .as_ref()
            .map(|(shared, _)| shared.clone())
            .ok_or_else(|| PrekuError::InvalidInput("Wellbeing is turned off in Settings".into()))
    }

    pub fn state(&self) -> Result<WellbeingState> {
        let shared = self.shared()?;
        build_state(&shared)
    }

    /// Runs a change, broadcasts the new state and returns it.
    fn change(
        &self,
        f: impl FnOnce(&Shared, &mut rusqlite::Connection) -> Result<()>,
    ) -> Result<WellbeingState> {
        let shared = self.shared()?;
        shared.ctx.storage.with_conn(|conn| f(&shared, conn))?;
        let state = build_state(&shared)?;
        shared.ctx.emit(events::STATE, &state);
        Ok(state)
    }

    pub fn add_water(&self, ml: Option<u32>) -> Result<WellbeingState> {
        self.change(|shared, conn| {
            let settings = lock(&shared.settings).clone();
            let day = today();
            let before = habits::water_today(conn, day)?;
            habits::add_water(conn, ml.unwrap_or(settings.glass_ml), day, now_millis())?;
            let after = habits::water_today(conn, day)?;
            let reached_goal = before < settings.water_goal_ml && after >= settings.water_goal_ml;
            clear_prompt(shared, Some(ReminderKind::Water));
            // The water nudge counts from the last glass, however it was logged.
            lock(&shared.scheduler).break_done(ReminderKind::Water);
            shared.ctx.publish(AppEvent::Pip(if reached_goal {
                PipSignal::Celebration
            } else {
                PipSignal::Success
            }));
            Ok(())
        })
    }

    pub fn undo_water(&self) -> Result<WellbeingState> {
        self.change(|_, conn| habits::undo_water(conn, today(), now_millis()))
    }

    pub fn add_habit(&self, name: &str, target: u32, days: u8) -> Result<WellbeingState> {
        self.change(|_, conn| habits::add(conn, name, target, days, now_millis()))
    }

    pub fn update_habit(
        &self,
        id: &str,
        name: &str,
        target: u32,
        days: u8,
    ) -> Result<WellbeingState> {
        self.change(|_, conn| habits::update(conn, id, name, target, days, now_millis()))
    }

    pub fn delete_habit(&self, id: &str) -> Result<WellbeingState> {
        self.change(|_, conn| habits::delete(conn, id, now_millis()))
    }

    /// Logs a habit; Pip celebrates when today's check-in hits a streak milestone.
    pub fn log_habit(&self, id: &str, delta: i32) -> Result<WellbeingState> {
        self.change(|shared, conn| {
            let day = today();
            let was_done = habits::list(conn, day)?
                .iter()
                .any(|h| h.id == id && h.done_today);
            habits::log(conn, id, day, delta, now_millis())?;
            let after = habits::list(conn, day)?.into_iter().find(|h| h.id == id);
            if let Some(habit) = after.filter(|h| h.done_today && !was_done) {
                shared.ctx.publish(AppEvent::Pip(
                    // A milestone, or simply being back after a missed day: both are worth
                    // a little celebration (never a sad Pip for the days in between).
                    if habits::MILESTONES.contains(&habit.streak) || habit.came_back {
                        PipSignal::Celebration
                    } else {
                        PipSignal::Success
                    },
                ));
            }
            Ok(())
        })
    }

    /// "Later": the same reminder comes back after the snooze time.
    pub fn prompt_later(&self, id: &str) -> Result<WellbeingState> {
        self.change(|shared, _| {
            let kind = lock(&shared.prompt)
                .as_ref()
                .filter(|p| p.id == id)
                .map(|p| p.kind);
            if let Some(kind) = kind {
                let settings = lock(&shared.settings).clone();
                lock(&shared.scheduler).snooze(kind, &settings, chrono::Local::now().naive_local());
                clear_prompt(shared, Some(kind));
            }
            Ok(())
        })
    }

    /// The user finished (or skipped) a break of this kind, from a prompt or on their own.
    pub fn break_done(&self, kind: ReminderKind, completed: bool) -> Result<WellbeingState> {
        self.change(|shared, conn| {
            clear_prompt(shared, Some(kind));
            if completed {
                lock(&shared.scheduler).break_done(kind);
                companion::log_break(conn, kind, today(), now_millis())?;
                shared.ctx.publish(AppEvent::Pip(PipSignal::Success));
            }
            Ok(())
        })
    }

    /// Saves today's mood (only while the user has the check-in turned on).
    pub fn log_mood(&self, mood: &str) -> Result<WellbeingState> {
        self.change(|shared, conn| {
            if !lock(&shared.settings).mood_check_in_enabled {
                return Err(PrekuError::InvalidInput(
                    "Turn on the mood check-in in Wellbeing settings first".into(),
                ));
            }
            companion::set_mood(conn, today(), mood, now_millis())?;
            clear_prompt(shared, Some(ReminderKind::MoodCheckIn));
            Ok(())
        })
    }

    pub fn set_settings(&self, next: WellbeingSettings) -> Result<WellbeingState> {
        self.change(|shared, _| {
            settings::save(&shared.ctx.storage, &next)?;
            *lock(&shared.settings) = next;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posture_nudges_fade_sooner_than_breaks() {
        assert_eq!(prompt_ttl_ms(ReminderKind::Posture), 60_000);
        assert_eq!(prompt_ttl_ms(ReminderKind::Water), 60_000);
        assert_eq!(prompt_ttl_ms(ReminderKind::Eyes), 15 * 60_000);
    }

    #[test]
    fn posture_nudges_are_on_for_new_installs() {
        let settings = WellbeingSettings::default();
        assert!(settings.posture_enabled);
        assert_eq!(settings.posture_every_min, 8);
    }
}
