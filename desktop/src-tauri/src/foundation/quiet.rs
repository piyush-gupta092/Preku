//! Quiet mode (usability U4): one place every module asks "may I interrupt right now?".
//!
//! Routine prompts (wellbeing reminders and nudges, ordinary system alerts) wait while
//! - the user paused them for a while ("Pause for 30 min / 1 hour / until tomorrow"),
//! - a focus session is running and "hold reminders while I focus" is on (the default),
//! - a game or a presentation is on screen (always), or
//! - any other full-screen app is in front and the user asked for that too (off by default:
//!   working in a full-screen editor shouldn't silence reminders).
//!
//! Waiting prompts are not saved up into a burst: each module shows at most its one most
//! relevant reminder afterwards. Urgent things (low battery, almost-full disk) and what the
//! user explicitly asked for (a calendar heads-up, a finished timer) still come through.
//!
//! The full-screen state is sampled here, not by the Wellbeing module, so it works whichever
//! modules are on.

use std::sync::{
    atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{errors::Result, storage::Storage};

const KEY: &str = "quiet";
pub const EVENT: &str = "app:quiet";
/// A pause can't be set longer than this (a day and a bit covers "until tomorrow").
const MAX_PAUSE_MS: i64 = 36 * 60 * 60_000;

/// What is in front of the user, as far as interrupting goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Normal,
    /// An ordinary app in full screen (an editor, a browser, a video).
    FullscreenApp,
    /// A full-screen game or presentation mode: never interrupt.
    GameOrPresentation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct QuietState {
    /// Routine prompts are paused until this time (unix ms); null = not paused.
    #[ts(type = "number | null")]
    pub until: Option<i64>,
    /// Hold routine prompts while a focus session runs.
    pub hold_in_focus: bool,
    /// Also hold them while any full-screen app is in front (games and presentations
    /// always hold).
    pub hold_fullscreen_apps: bool,
    /// A focus session is running right now (not saved; set by the timer).
    pub focusing: bool,
    /// Held right now because of what is on screen (not saved): the notch doesn't pop out.
    pub screen_busy: bool,
}

impl Default for QuietState {
    fn default() -> Self {
        Self {
            until: None,
            hold_in_focus: true,
            hold_fullscreen_apps: false,
            focusing: false,
            screen_busy: false,
        }
    }
}

#[derive(Debug)]
pub struct Quiet {
    until: AtomicI64,
    hold_in_focus: AtomicBool,
    hold_fullscreen_apps: AtomicBool,
    focusing: AtomicBool,
    /// 0 = normal, 1 = full-screen app, 2 = game or presentation.
    screen: AtomicU8,
}

impl Default for Quiet {
    fn default() -> Self {
        Self::from_state(QuietState::default())
    }
}

impl Quiet {
    fn from_state(state: QuietState) -> Self {
        Self {
            until: AtomicI64::new(state.until.unwrap_or(0)),
            hold_in_focus: AtomicBool::new(state.hold_in_focus),
            hold_fullscreen_apps: AtomicBool::new(state.hold_fullscreen_apps),
            // Never restored: these describe this very moment.
            focusing: AtomicBool::new(false),
            screen: AtomicU8::new(0),
        }
    }

    /// Restores the saved pause and preferences (an expired pause is simply over).
    pub fn load(storage: &Storage) -> Arc<Self> {
        let state: QuietState = storage
            .settings_get(KEY)
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        Arc::new(Self::from_state(state))
    }

    /// The screen itself says "not now".
    pub fn screen_busy(&self) -> bool {
        match self.screen.load(Ordering::SeqCst) {
            2 => true,
            1 => self.hold_fullscreen_apps.load(Ordering::SeqCst),
            _ => false,
        }
    }

    pub fn state(&self, now: i64) -> QuietState {
        let until = self.until.load(Ordering::SeqCst);
        QuietState {
            until: (until > now).then_some(until),
            hold_in_focus: self.hold_in_focus.load(Ordering::SeqCst),
            hold_fullscreen_apps: self.hold_fullscreen_apps.load(Ordering::SeqCst),
            focusing: self.focusing.load(Ordering::SeqCst),
            screen_busy: self.screen_busy(),
        }
    }

    /// Routine prompts should wait.
    pub fn is_quiet(&self, now: i64) -> bool {
        self.until.load(Ordering::SeqCst) > now
            || (self.focusing.load(Ordering::SeqCst) && self.hold_in_focus.load(Ordering::SeqCst))
            || self.screen_busy()
    }

    /// Pauses routine prompts until `until` (clamped), or resumes with `None`.
    pub fn pause_until(&self, until: Option<i64>, now: i64) {
        let until = until.map_or(0, |t| t.clamp(now, now + MAX_PAUSE_MS));
        self.until.store(until, Ordering::SeqCst);
    }

    pub fn set_hold_in_focus(&self, hold: bool) {
        self.hold_in_focus.store(hold, Ordering::SeqCst);
    }

    pub fn set_hold_fullscreen_apps(&self, hold: bool) {
        self.hold_fullscreen_apps.store(hold, Ordering::SeqCst);
    }

    /// The focus timer started or stopped running. Returns true if that changed anything.
    pub fn set_focusing(&self, focusing: bool) -> bool {
        self.focusing.swap(focusing, Ordering::SeqCst) != focusing
    }

    /// What is on screen now. Returns true if "busy" changed.
    pub fn set_screen(&self, screen: Screen) -> bool {
        let before = self.screen_busy();
        let level = match screen {
            Screen::Normal => 0,
            Screen::FullscreenApp => 1,
            Screen::GameOrPresentation => 2,
        };
        self.screen.store(level, Ordering::SeqCst);
        self.screen_busy() != before
    }

    pub fn save(&self, storage: &Storage, now: i64) -> Result<()> {
        // Only the user's choices are worth keeping.
        let state = QuietState {
            focusing: false,
            screen_busy: false,
            ..self.state(now)
        };
        storage.settings_set(KEY, &serde_json::to_value(state)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pause_ends_by_itself_and_can_be_resumed_early() {
        let quiet = Quiet::default();
        assert!(!quiet.is_quiet(1_000));
        quiet.pause_until(Some(1_000 + 30 * 60_000), 1_000);
        assert!(quiet.is_quiet(1_000 + 29 * 60_000));
        assert_eq!(quiet.state(1_000).until, Some(1_000 + 30 * 60_000));
        assert!(
            !quiet.is_quiet(1_000 + 30 * 60_000),
            "over at the chosen time"
        );
        assert_eq!(quiet.state(1_000 + 31 * 60_000).until, None);

        quiet.pause_until(Some(i64::MAX), 1_000);
        assert_eq!(
            quiet.state(1_000).until,
            Some(1_000 + MAX_PAUSE_MS),
            "clamped"
        );
        quiet.pause_until(None, 2_000);
        assert!(!quiet.is_quiet(2_000));
    }

    #[test]
    fn focus_holds_prompts_only_while_the_user_wants_that() {
        let quiet = Quiet::default();
        assert!(quiet.set_focusing(true));
        assert!(!quiet.set_focusing(true), "no change the second time");
        assert!(quiet.is_quiet(0));
        quiet.set_hold_in_focus(false);
        assert!(!quiet.is_quiet(0));
        quiet.set_hold_in_focus(true);
        quiet.set_focusing(false);
        assert!(!quiet.is_quiet(0));
    }

    #[test]
    fn a_full_screen_editor_does_not_silence_reminders_but_a_game_does() {
        let quiet = Quiet::default();
        // VS Code in full screen: reminders still come (the default).
        assert!(!quiet.set_screen(Screen::FullscreenApp), "nothing changes");
        assert!(!quiet.is_quiet(0) && !quiet.state(0).screen_busy);
        // A game or a presentation always holds them.
        assert!(quiet.set_screen(Screen::GameOrPresentation));
        assert!(quiet.is_quiet(0) && quiet.state(0).screen_busy);
        assert!(quiet.set_screen(Screen::Normal));
        assert!(!quiet.is_quiet(0));
        // The user can ask for every full-screen app to hold them.
        quiet.set_hold_fullscreen_apps(true);
        assert!(quiet.set_screen(Screen::FullscreenApp));
        assert!(quiet.is_quiet(0));
    }

    #[test]
    fn choices_survive_a_restart_but_the_moment_does_not() {
        let storage = Storage::open_in_memory().unwrap();
        let quiet = Quiet::default();
        quiet.pause_until(Some(50_000), 1_000);
        quiet.set_hold_in_focus(false);
        quiet.set_hold_fullscreen_apps(true);
        quiet.set_focusing(true);
        quiet.set_screen(Screen::GameOrPresentation);
        quiet.save(&storage, 1_000).unwrap();

        let restored = Quiet::load(&storage);
        assert_eq!(
            restored.state(1_000),
            QuietState {
                until: Some(50_000),
                hold_in_focus: false,
                hold_fullscreen_apps: true,
                focusing: false,
                screen_busy: false,
            }
        );
        // First run: nothing saved.
        let fresh = Quiet::load(&Storage::open_in_memory().unwrap());
        assert_eq!(fresh.state(0), QuietState::default());
    }
}
