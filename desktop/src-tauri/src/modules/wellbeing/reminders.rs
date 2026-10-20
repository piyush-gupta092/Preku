//! The wellbeing companion's reminders (PRD W1, W2 + the companion prompts). Pure: fed the
//! elapsed time, idle time, DND state and the local clock on every tick, so it is fully
//! testable.
//!
//! - Only *active* time counts: while the user is away (idle ≥ 2 min) nothing accumulates.
//! - Being away ≥ 5 min counts as a real break: eye, movement and posture timers start over.
//! - One prompt at a time, at least `MIN_GAP_MS` apart, so reminders never pile up.
//! - Do-Not-Disturb (fullscreen apps, presentations) defers; quiet hours skip (except the
//!   bedtime wind-down, which the user asked for).
//! - "Later" snoozes just that reminder; finishing a break restarts the matching timers.

use std::collections::HashMap;

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::settings::{parse_time, WellbeingSettings};

pub const AWAY_AFTER_MS: u64 = 2 * 60_000;
pub const BREAK_AFTER_MS: u64 = 5 * 60_000;
/// The shortest time between two prompts.
pub const MIN_GAP_MS: u64 = 10 * 60_000;
/// Random nudges land anywhere from 65% to 125% of their interval (8 min → about 5–10).
const JITTER_MIN: f32 = 0.65;
const JITTER_SPAN: f32 = 0.6;
/// Drive-by nudges (posture, water) keep their own, much shorter spacing: they are a
/// one-line pop-in that leaves by itself, so they don't wait for the gap between breaks.
pub const NUDGE_GAP_MS: u64 = 2 * 60_000;
/// A clock reminder (bedtime, meals, mood) only fires this soon after its time, so starting
/// the PC hours later doesn't bring up a stale one.
const CLOCK_WINDOW_MIN: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReminderKind {
    Eyes,
    /// Movement break (named "stand" since Phase 4).
    Stand,
    Posture,
    Water,
    ScreenBalance,
    Bedtime,
    Meal,
    MoodCheckIn,
    /// A breather the user asks for themselves (never scheduled).
    Reset,
}

/// When several are due at once, the first one here wins; the rest wait their turn.
const PRIORITY: [ReminderKind; 8] = [
    ReminderKind::Bedtime,
    ReminderKind::Meal,
    ReminderKind::Eyes,
    ReminderKind::Stand,
    ReminderKind::Posture,
    ReminderKind::Water,
    ReminderKind::ScreenBalance,
    ReminderKind::MoodCheckIn,
];

impl ReminderKind {
    /// Live-item id suffix and translation key.
    pub fn key(self) -> &'static str {
        match self {
            Self::Eyes => "reminder.eyes",
            Self::Stand => "reminder.stand",
            Self::Posture => "reminder.posture",
            Self::Water => "reminder.water",
            Self::ScreenBalance => "reminder.screen",
            Self::Bedtime => "reminder.bedtime",
            Self::Meal => "reminder.meal",
            Self::MoodCheckIn => "reminder.mood",
            Self::Reset => "reminder.reset",
        }
    }

    /// Stored name (break log).
    pub fn name(self) -> &'static str {
        self.key().trim_start_matches("reminder.")
    }

    /// A drive-by nudge: said in one line, gone in a minute, never a guided break.
    pub fn is_nudge(self) -> bool {
        matches!(self, Self::Posture | Self::Water)
    }

    fn is_clock(self) -> bool {
        matches!(self, Self::Bedtime | Self::Meal | Self::MoodCheckIn)
    }
}

/// A reminder waiting for the user's answer (Take a break · Later).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Prompt {
    pub id: String,
    pub kind: ReminderKind,
    /// Minutes of use since the last real break, as measured (for honest wording such as
    /// "You've been on this PC for 52 minutes"; not a guess about sitting).
    pub active_min: u32,
    /// Minutes of active use since this reminder last came up or was done (measured), for
    /// lines such as "It's been an hour, drink some water".
    pub since_min: u32,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct Scheduler {
    eyes_ms: u64,
    stand_ms: u64,
    posture_ms: u64,
    water_ms: u64,
    /// Active use since the last real break.
    session_ms: u64,
    since_prompt_ms: u64,
    since_nudge_ms: u64,
    /// A break invitation has waited unanswered long enough that a nudge may take its place.
    nudge_may_replace: bool,
    screen_shown: bool,
    screen_snooze_until_ms: u64,
    /// Clock reminders already shown today: (kind, slot) → day.
    fired: HashMap<(ReminderKind, usize), NaiveDate>,
    snoozed_until: HashMap<ReminderKind, NaiveDateTime>,
    /// State of the small random generator behind the nudge timing.
    rng: u64,
    /// This round's share of the interval for the posture / water nudge (1.0 = on the dot).
    posture_factor: f32,
    water_factor: f32,
    since_min: u32,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self {
            eyes_ms: 0,
            stand_ms: 0,
            posture_ms: 0,
            water_ms: 0,
            session_ms: 0,
            // The first reminder doesn't wait for a gap.
            since_prompt_ms: MIN_GAP_MS,
            since_nudge_ms: NUDGE_GAP_MS,
            nudge_may_replace: false,
            screen_shown: false,
            screen_snooze_until_ms: 0,
            fired: HashMap::new(),
            snoozed_until: HashMap::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            posture_factor: 1.0,
            water_factor: 1.0,
            since_min: 0,
        }
    }
}

impl Scheduler {
    /// A scheduler whose nudges are unpredictable from the first one (the app seeds it with
    /// the clock; `default()` starts on the dot, which keeps tests exact).
    pub fn seeded(seed: u64) -> Self {
        // Mix the seed (splitmix64) so close seeds, such as two start-up times, differ at once.
        let mut mixed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let mut scheduler = Self {
            rng: (mixed ^ (mixed >> 31)) | 1,
            ..Self::default()
        };
        scheduler.posture_factor = scheduler.roll();
        scheduler.water_factor = scheduler.roll();
        scheduler
    }

    /// Next share of the interval, `JITTER_MIN`…`JITTER_MIN + JITTER_SPAN` (xorshift64).
    fn roll(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        let unit = (self.rng >> 40) as f32 / (1u64 << 24) as f32;
        JITTER_MIN + JITTER_SPAN * unit
    }

    /// Tells the scheduler, before a tick, that the prompt on screen is an old unanswered
    /// break invitation, so a due nudge may replace it instead of waiting behind it.
    pub fn set_nudge_may_replace(&mut self, allowed: bool) {
        self.nudge_may_replace = allowed;
    }

    /// Minutes of active use behind the reminder `tick` just returned.
    pub fn since_min(&self) -> u32 {
        self.since_min
    }
}

fn interval_ms(minutes: u32) -> u64 {
    u64::from(minutes) * 60_000
}

/// Whether `time` falls in the quiet hours (the window may cross midnight).
pub fn in_quiet_hours(settings: &WellbeingSettings, time: NaiveTime) -> bool {
    if !settings.quiet_hours_enabled {
        return false;
    }
    let (Some(start), Some(end)) = (
        parse_time(&settings.quiet_start),
        parse_time(&settings.quiet_end),
    ) else {
        return false;
    };
    if start <= end {
        start <= time && time < end
    } else {
        time >= start || time < end
    }
}

/// Clock reminders for today: (kind, slot, time).
fn clock_targets(settings: &WellbeingSettings) -> Vec<(ReminderKind, usize, NaiveTime)> {
    let mut targets = Vec::new();
    if settings.bedtime_enabled {
        if let Some(bed) = parse_time(&settings.bedtime) {
            let wind_down = Duration::minutes(i64::from(settings.wind_down_min));
            targets.push((
                ReminderKind::Bedtime,
                0,
                bed.overflowing_sub_signed(wind_down).0,
            ));
        }
    }
    if settings.meals_enabled {
        for (slot, meal) in settings.meal_times.iter().enumerate() {
            if let Some(time) = parse_time(meal) {
                targets.push((ReminderKind::Meal, slot, time));
            }
        }
    }
    if settings.mood_check_in_enabled {
        if let Some(time) = parse_time(&settings.mood_check_in_time) {
            targets.push((ReminderKind::MoodCheckIn, 0, time));
        }
    }
    targets
}

impl Scheduler {
    /// Advances by `elapsed_ms` and returns the reminder to show now, with the observed
    /// minutes of use. `idle_ms = None` means "unknown" and is treated as active.
    /// `can_prompt = false` (a prompt is still waiting for an answer) keeps due reminders
    /// waiting.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        settings: &WellbeingSettings,
        elapsed_ms: u64,
        idle_ms: Option<u64>,
        dnd: bool,
        water_goal_met: bool,
        now: NaiveDateTime,
        can_prompt: bool,
    ) -> Option<(ReminderKind, u32)> {
        let idle = idle_ms.unwrap_or(0);
        self.since_prompt_ms = self.since_prompt_ms.saturating_add(elapsed_ms);
        self.since_nudge_ms = self.since_nudge_ms.saturating_add(elapsed_ms);
        if idle >= BREAK_AFTER_MS {
            self.took_break();
            return None;
        }
        if idle < AWAY_AFTER_MS {
            for timer in [
                &mut self.eyes_ms,
                &mut self.stand_ms,
                &mut self.posture_ms,
                &mut self.water_ms,
                &mut self.session_ms,
            ] {
                *timer += elapsed_ms;
            }
        }
        if water_goal_met {
            self.water_ms = 0;
        }

        let quiet = in_quiet_hours(settings, now.time());
        let mut due: Vec<(ReminderKind, Option<usize>)> = Vec::new();
        let jitter = |factor: f32| if settings.random_nudges { factor } else { 1.0 };

        for (enabled, every, timer, kind, factor) in [
            (
                settings.eyes_enabled,
                settings.eyes_every_min,
                &mut self.eyes_ms,
                ReminderKind::Eyes,
                1.0,
            ),
            (
                settings.stand_enabled,
                settings.stand_every_min,
                &mut self.stand_ms,
                ReminderKind::Stand,
                1.0,
            ),
            (
                settings.posture_enabled,
                settings.posture_every_min,
                &mut self.posture_ms,
                ReminderKind::Posture,
                jitter(self.posture_factor),
            ),
            (
                settings.water_enabled,
                settings.water_every_min,
                &mut self.water_ms,
                ReminderKind::Water,
                jitter(self.water_factor),
            ),
        ] {
            if !enabled {
                *timer = 0;
            } else if *timer >= (interval_ms(every) as f32 * factor) as u64 {
                if quiet {
                    // Quiet hours skip reminders instead of saving them up.
                    *timer = 0;
                } else {
                    due.push((kind, None));
                }
            }
        }

        let screen_after =
            interval_ms(settings.screen_balance_min).max(self.screen_snooze_until_ms);
        if settings.screen_balance_enabled && !self.screen_shown && self.session_ms >= screen_after
        {
            if quiet {
                self.screen_shown = true;
            } else {
                due.push((ReminderKind::ScreenBalance, None));
            }
        }

        let today = now.date();
        for (kind, slot, time) in clock_targets(settings) {
            if self.fired.get(&(kind, slot)) == Some(&today) {
                continue;
            }
            let late = (now - today.and_time(time)).num_minutes();
            if (0..CLOCK_WINDOW_MIN).contains(&late) {
                if quiet && kind != ReminderKind::Bedtime {
                    self.fired.insert((kind, slot), today);
                } else {
                    due.push((kind, Some(slot)));
                }
            }
        }
        for (&kind, &until) in &self.snoozed_until {
            if now >= until && !(quiet && kind != ReminderKind::Bedtime) {
                due.push((kind, None));
            }
        }

        if dnd {
            return None;
        }
        // Breaks: one at a time, well apart. Nudges: their own short spacing, and they don't
        // queue behind a break invitation nobody answered.
        let pick = |nudges: bool| {
            PRIORITY
                .iter()
                .filter(|wanted| wanted.is_nudge() == nudges)
                .find_map(|wanted| due.iter().find(|(kind, _)| kind == wanted).copied())
        };
        let break_ok = can_prompt && self.since_prompt_ms >= MIN_GAP_MS;
        let nudge_ok =
            (can_prompt || self.nudge_may_replace) && self.since_nudge_ms >= NUDGE_GAP_MS;
        let (kind, slot) = break_ok
            .then(|| pick(false))
            .flatten()
            .or_else(|| nudge_ok.then(|| pick(true)).flatten())?;

        let since_ms = match kind {
            ReminderKind::Eyes => self.eyes_ms,
            ReminderKind::Stand => self.stand_ms,
            ReminderKind::Posture => self.posture_ms,
            ReminderKind::Water => self.water_ms,
            _ => self.session_ms,
        };
        self.since_min = u32::try_from(since_ms / 60_000).unwrap_or(u32::MAX);
        match kind {
            ReminderKind::Eyes => self.eyes_ms = 0,
            ReminderKind::Stand => self.stand_ms = 0,
            ReminderKind::Posture => {
                self.posture_ms = 0;
                self.posture_factor = self.roll();
            }
            ReminderKind::Water => {
                self.water_ms = 0;
                self.water_factor = self.roll();
            }
            ReminderKind::ScreenBalance => self.screen_shown = true,
            _ => {}
        }
        // A snoozed clock reminder was already marked as fired when it first showed.
        if kind.is_clock() && self.snoozed_until.remove(&kind).is_none() {
            self.fired.insert((kind, slot.unwrap_or(0)), today);
        }
        if kind.is_nudge() {
            self.since_nudge_ms = 0;
        } else {
            self.since_prompt_ms = 0;
        }
        let active_min = u32::try_from(self.session_ms / 60_000).unwrap_or(u32::MAX);
        Some((kind, active_min))
    }

    /// A real break (away ≥ 5 min, or the user finished a "time away" break).
    fn took_break(&mut self) {
        self.eyes_ms = 0;
        self.stand_ms = 0;
        self.posture_ms = 0;
        self.session_ms = 0;
        self.screen_shown = false;
        self.screen_snooze_until_ms = 0;
    }

    /// "Later": bring this reminder back after the snooze time.
    pub fn snooze(&mut self, kind: ReminderKind, settings: &WellbeingSettings, now: NaiveDateTime) {
        let snooze_ms = interval_ms(settings.snooze_min);
        let back_in = |every: u32| interval_ms(every).saturating_sub(snooze_ms);
        match kind {
            ReminderKind::Eyes => self.eyes_ms = back_in(settings.eyes_every_min),
            ReminderKind::Stand => self.stand_ms = back_in(settings.stand_every_min),
            // "Later" means the snooze time exactly, not a random one.
            ReminderKind::Posture => {
                self.posture_ms = back_in(settings.posture_every_min);
                self.posture_factor = 1.0;
            }
            ReminderKind::Water => {
                self.water_ms = back_in(settings.water_every_min);
                self.water_factor = 1.0;
            }
            ReminderKind::ScreenBalance => {
                self.screen_shown = false;
                self.screen_snooze_until_ms = self.session_ms + snooze_ms;
            }
            ReminderKind::Bedtime | ReminderKind::Meal | ReminderKind::MoodCheckIn => {
                let minutes = i64::from(settings.snooze_min);
                self.snoozed_until
                    .insert(kind, now + Duration::minutes(minutes));
            }
            ReminderKind::Reset => {}
        }
        // The snoozed reminder may come back after the snooze, even if that's under the gap.
        self.since_prompt_ms = MIN_GAP_MS.saturating_sub(snooze_ms);
    }

    /// The user finished a break: restart the timers it covers.
    pub fn break_done(&mut self, kind: ReminderKind) {
        match kind {
            ReminderKind::Eyes | ReminderKind::Reset => self.eyes_ms = 0,
            ReminderKind::Stand => {
                self.stand_ms = 0;
                self.eyes_ms = 0;
                self.posture_ms = 0;
            }
            ReminderKind::Posture => self.posture_ms = 0,
            ReminderKind::Water => self.water_ms = 0,
            ReminderKind::ScreenBalance => self.took_break(),
            ReminderKind::Bedtime | ReminderKind::Meal | ReminderKind::MoodCheckIn => {}
        }
    }

    /// Seconds of active use until the next break reminder (eyes or movement).
    pub fn next_break_s(&self, settings: &WellbeingSettings) -> Option<u32> {
        let remaining = |enabled: bool, every: u32, timer: u64| {
            enabled.then(|| interval_ms(every).saturating_sub(timer) / 1000)
        };
        [
            remaining(settings.eyes_enabled, settings.eyes_every_min, self.eyes_ms),
            remaining(
                settings.stand_enabled,
                settings.stand_every_min,
                self.stand_ms,
            ),
        ]
        .into_iter()
        .flatten()
        .min()
        .map(|s| u32::try_from(s).unwrap_or(u32::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    fn at(hh: u32, mm: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 10)
            .unwrap()
            .and_hms_opt(hh, mm, 0)
            .unwrap()
    }

    /// Mid-afternoon: no quiet hours, no clock reminders unless a test sets them.
    fn noon() -> NaiveDateTime {
        at(15, 0)
    }

    fn nothing() -> WellbeingSettings {
        WellbeingSettings {
            eyes_enabled: false,
            stand_enabled: false,
            posture_enabled: false,
            water_enabled: false,
            screen_balance_enabled: false,
            random_nudges: false,
            ..WellbeingSettings::default()
        }
    }

    fn only_eyes() -> WellbeingSettings {
        WellbeingSettings {
            eyes_enabled: true,
            ..nothing()
        }
    }

    fn tick(s: &mut Scheduler, settings: &WellbeingSettings, ms: u64) -> Option<ReminderKind> {
        s.tick(settings, ms, Some(0), false, false, noon(), true)
            .map(|(kind, _)| kind)
    }

    #[test]
    fn eyes_after_twenty_active_minutes() {
        let settings = only_eyes();
        let mut s = Scheduler::default();
        assert_eq!(tick(&mut s, &settings, 19 * MIN), None);
        assert_eq!(s.next_break_s(&settings), Some(60));
        assert_eq!(tick(&mut s, &settings, MIN), Some(ReminderKind::Eyes));
        assert_eq!(tick(&mut s, &settings, MIN), None, "timer restarted");
    }

    #[test]
    fn away_time_does_not_count_and_a_long_absence_is_a_break() {
        let settings = only_eyes();
        let mut s = Scheduler::default();
        tick(&mut s, &settings, 15 * MIN);
        // Idle for 3 minutes: paused, not reset.
        let r = s.tick(
            &settings,
            10 * MIN,
            Some(3 * MIN),
            false,
            false,
            noon(),
            true,
        );
        assert!(r.is_none());
        assert_eq!(s.next_break_s(&settings), Some(5 * 60));
        // Away for 6 minutes: a real break, so the timer starts over.
        s.tick(&settings, MIN, Some(6 * MIN), false, false, noon(), true);
        assert_eq!(s.next_break_s(&settings), Some(20 * 60));
    }

    #[test]
    fn dnd_and_an_open_prompt_defer_until_they_end() {
        let settings = only_eyes();
        let mut s = Scheduler::default();
        assert!(s
            .tick(&settings, 25 * MIN, Some(0), true, false, noon(), true)
            .is_none());
        assert!(s
            .tick(&settings, 0, Some(0), false, false, noon(), false)
            .is_none());
        assert_eq!(tick(&mut s, &settings, 0), Some(ReminderKind::Eyes));
    }

    #[test]
    fn posture_nudges_follow_their_interval_when_random_timing_is_off() {
        let settings = WellbeingSettings {
            posture_enabled: WellbeingSettings::default().posture_enabled,
            posture_every_min: WellbeingSettings::default().posture_every_min,
            ..nothing()
        };
        let mut s = Scheduler::default();
        assert_eq!(tick(&mut s, &settings, 7 * MIN), None);
        assert_eq!(tick(&mut s, &settings, MIN), Some(ReminderKind::Posture));
        assert_eq!(tick(&mut s, &settings, 7 * MIN), None);
        assert_eq!(tick(&mut s, &settings, MIN), Some(ReminderKind::Posture));
    }

    #[test]
    fn nudges_do_not_wait_behind_breaks() {
        let settings = WellbeingSettings {
            eyes_enabled: true,
            eyes_every_min: 20,
            posture_enabled: true,
            posture_every_min: 8,
            ..nothing()
        };
        let mut s = Scheduler::default();
        let mut shown = Vec::new();
        for minute in 1..=30u32 {
            if let Some(kind) = tick(&mut s, &settings, MIN) {
                shown.push((minute, kind));
            }
        }
        // Posture every 8 minutes, the eye break at 20: neither delays the other.
        assert_eq!(
            shown,
            [
                (8, ReminderKind::Posture),
                (16, ReminderKind::Posture),
                (20, ReminderKind::Eyes),
                (24, ReminderKind::Posture),
            ]
        );
    }

    #[test]
    fn a_nudge_replaces_only_an_old_unanswered_break_invitation() {
        let settings = WellbeingSettings {
            posture_enabled: true,
            posture_every_min: 8,
            ..nothing()
        };
        let waiting = |s: &mut Scheduler, ms: u64| {
            s.tick(&settings, ms, Some(0), false, false, noon(), false)
                .map(|(kind, _)| kind)
        };
        let mut s = Scheduler::default();
        // A fresh break invitation is on screen: the due nudge waits.
        assert_eq!(waiting(&mut s, 9 * MIN), None);
        // It has been ignored for a while: the nudge may take its place.
        s.set_nudge_may_replace(true);
        assert_eq!(waiting(&mut s, MIN), Some(ReminderKind::Posture));
        // Two nudges are never closer than the nudge gap, however short the interval.
        let quick = WellbeingSettings {
            posture_every_min: 5,
            random_nudges: false,
            ..settings.clone()
        };
        let mut s = Scheduler::default();
        assert_eq!(tick(&mut s, &quick, 5 * MIN), Some(ReminderKind::Posture));
        assert_eq!(tick(&mut s, &quick, MIN), None);
    }

    #[test]
    fn random_nudges_land_around_the_interval_and_vary() {
        let settings = WellbeingSettings {
            posture_enabled: true,
            posture_every_min: 30,
            random_nudges: true,
            ..nothing()
        };
        let mut s = Scheduler::seeded(42);
        let mut gaps = Vec::new();
        let mut minutes = 0u32;
        while gaps.len() < 12 {
            minutes += 1;
            if tick(&mut s, &settings, MIN) == Some(ReminderKind::Posture) {
                // The measured minutes go out with the prompt.
                assert_eq!(s.since_min(), minutes);
                gaps.push(minutes);
                minutes = 0;
            }
        }
        // 65%…125% of 30 minutes, and not the same every time.
        assert!(gaps.iter().all(|gap| (19..=38).contains(gap)), "{gaps:?}");
        let distinct: std::collections::HashSet<_> = gaps.iter().collect();
        assert!(distinct.len() >= 4, "{gaps:?}");
        // Another day, another rhythm.
        let mut other = Scheduler::seeded(7);
        let mut first = 0u32;
        while tick(&mut other, &settings, MIN).is_none() {
            first += 1;
        }
        assert_ne!(first + 1, gaps[0]);
    }

    #[test]
    fn later_brings_a_random_nudge_back_after_the_snooze_time() {
        let settings = WellbeingSettings {
            water_enabled: true,
            water_every_min: 60,
            snooze_min: 10,
            random_nudges: true,
            ..nothing()
        };
        let mut s = Scheduler::seeded(3);
        while tick(&mut s, &settings, MIN).is_none() {}
        s.snooze(ReminderKind::Water, &settings, noon());
        assert_eq!(tick(&mut s, &settings, 9 * MIN), None);
        assert_eq!(tick(&mut s, &settings, MIN), Some(ReminderKind::Water));
    }

    #[test]
    fn water_stops_once_the_goal_is_met() {
        let settings = WellbeingSettings {
            water_enabled: true,
            ..nothing()
        };
        let mut s = Scheduler::default();
        assert!(s
            .tick(&settings, 90 * MIN, Some(0), false, true, noon(), true)
            .is_none());
        assert_eq!(tick(&mut s, &settings, 60 * MIN), Some(ReminderKind::Water));
    }

    #[test]
    fn unknown_idle_counts_as_active_and_disabled_reminders_stay_quiet() {
        let mut s = Scheduler::default();
        assert!(s
            .tick(&nothing(), 500 * MIN, None, false, false, noon(), true)
            .is_none());
        let settings = only_eyes();
        assert_eq!(
            s.tick(&settings, 20 * MIN, None, false, false, noon(), true)
                .map(|(k, _)| k),
            Some(ReminderKind::Eyes)
        );
    }

    #[test]
    fn one_prompt_at_a_time_with_a_gap_between() {
        let settings = WellbeingSettings {
            eyes_enabled: true,
            stand_enabled: true,
            stand_every_min: 20,
            ..nothing()
        };
        let mut s = Scheduler::default();
        // Both are due together: eyes first, movement waits for the gap.
        assert_eq!(tick(&mut s, &settings, 20 * MIN), Some(ReminderKind::Eyes));
        assert_eq!(tick(&mut s, &settings, 5 * MIN), None);
        assert_eq!(tick(&mut s, &settings, 5 * MIN), Some(ReminderKind::Stand));
    }

    #[test]
    fn later_brings_the_same_reminder_back_after_the_snooze() {
        let settings = WellbeingSettings {
            snooze_min: 10,
            ..only_eyes()
        };
        let mut s = Scheduler::default();
        assert_eq!(tick(&mut s, &settings, 20 * MIN), Some(ReminderKind::Eyes));
        s.snooze(ReminderKind::Eyes, &settings, noon());
        assert_eq!(tick(&mut s, &settings, 9 * MIN), None);
        assert_eq!(tick(&mut s, &settings, MIN), Some(ReminderKind::Eyes));
    }

    #[test]
    fn quiet_hours_skip_reminders_but_not_the_bedtime_wind_down() {
        let settings = WellbeingSettings {
            quiet_hours_enabled: true,
            quiet_start: "22:00".into(),
            quiet_end: "07:00".into(),
            bedtime_enabled: true,
            bedtime: "23:00".into(),
            wind_down_min: 30,
            ..only_eyes()
        };
        let mut s = Scheduler::default();
        let night = at(22, 35);
        assert_eq!(
            s.tick(&settings, 25 * MIN, Some(0), false, false, night, true)
                .map(|(k, _)| k),
            Some(ReminderKind::Bedtime)
        );
        // The eye reminder was skipped, not saved up for the morning.
        assert_eq!(s.next_break_s(&settings), Some(20 * 60));
        assert!(in_quiet_hours(&settings, at(3, 0).time()));
        assert!(!in_quiet_hours(&settings, at(12, 0).time()));
    }

    #[test]
    fn clock_reminders_fire_once_a_day_and_never_hours_late() {
        let settings = WellbeingSettings {
            meals_enabled: true,
            meal_times: vec!["13:00".into()],
            ..nothing()
        };
        let mut s = Scheduler::default();
        let run = |s: &mut Scheduler, now| {
            s.tick(&settings, MIN, Some(0), false, false, now, true)
                .map(|(k, _)| k)
        };
        assert_eq!(run(&mut s, at(12, 59)), None);
        assert_eq!(run(&mut s, at(13, 5)), Some(ReminderKind::Meal));
        assert_eq!(run(&mut s, at(13, 20)), None, "once a day");

        // Starting the PC at 16:00 doesn't bring up lunch.
        let mut late = Scheduler::default();
        assert_eq!(run(&mut late, at(16, 0)), None);
    }

    #[test]
    fn screen_balance_reports_the_observed_minutes_once_per_session() {
        let settings = WellbeingSettings {
            screen_balance_enabled: true,
            screen_balance_min: 60,
            ..nothing()
        };
        let mut s = Scheduler::default();
        assert_eq!(
            s.tick(&settings, 62 * MIN, Some(0), false, false, noon(), true),
            Some((ReminderKind::ScreenBalance, 62))
        );
        assert_eq!(tick(&mut s, &settings, 30 * MIN), None, "once per session");
        // A real break starts a new session.
        s.tick(&settings, MIN, Some(6 * MIN), false, false, noon(), true);
        assert_eq!(tick(&mut s, &settings, 59 * MIN), None);
        assert_eq!(
            tick(&mut s, &settings, 2 * MIN),
            Some(ReminderKind::ScreenBalance)
        );
    }

    #[test]
    fn a_movement_break_also_rests_the_eyes() {
        let settings = WellbeingSettings {
            stand_enabled: true,
            ..only_eyes()
        };
        let mut s = Scheduler::default();
        tick(&mut s, &settings, 15 * MIN);
        s.break_done(ReminderKind::Stand);
        assert_eq!(s.next_break_s(&settings), Some(20 * 60));
        assert_eq!(ReminderKind::ScreenBalance.name(), "screen");
    }
}
