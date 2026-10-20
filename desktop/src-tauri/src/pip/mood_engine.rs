use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Moods the frontend knows how to draw (Design.md §6). `dizzy` is purely an interaction
/// effect in the frontend, so it is not listed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum PipMood {
    Idle,
    Happy,
    Focus,
    Alert,
    Sleepy,
    Celebrate,
    Sad,
}

/// Things that happen in the app that Pip reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipSignal {
    /// Task done, water logged, phone connected…
    Success,
    /// Notification, threshold crossed…
    Alert,
    /// Build failed, site down…
    Failure,
    /// Timer finished, streak milestone…
    Celebration,
    /// A focus session started / ended: Pip rests in the `focus` mood meanwhile.
    FocusStarted,
    FocusEnded,
}

impl PipSignal {
    /// (mood, how long it lasts, rank). A higher-rank reaction isn't cut short by a lower one.
    /// `None` for signals that change the resting mood instead of causing a reaction.
    fn reaction(self) -> Option<(PipMood, Duration, u8)> {
        Some(match self {
            Self::Success => (PipMood::Happy, Duration::from_millis(2500), 1),
            Self::Celebration => (PipMood::Celebrate, Duration::from_secs(3), 2),
            Self::Failure => (PipMood::Sad, Duration::from_secs(4), 3),
            Self::Alert => (PipMood::Alert, Duration::from_secs(3), 4),
            Self::FocusStarted | Self::FocusEnded => return None,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Reaction {
    mood: PipMood,
    until: Instant,
    rank: u8,
}

/// Maps events and time of day to Pip's mood.
#[derive(Debug, Default)]
pub struct MoodEngine {
    reaction: Option<Reaction>,
    focusing: bool,
}

impl MoodEngine {
    pub fn react(&mut self, signal: PipSignal, now: Instant) {
        let Some((mood, duration, rank)) = signal.reaction() else {
            self.focusing = signal == PipSignal::FocusStarted;
            return;
        };
        if let Some(current) = self.reaction {
            if current.until > now && current.rank > rank {
                return;
            }
        }
        self.reaction = Some(Reaction {
            mood,
            until: now + duration,
            rank,
        });
    }

    pub fn mood(&self, now: Instant, local_hour: u32) -> PipMood {
        match self.reaction {
            Some(reaction) if reaction.until > now => reaction.mood,
            _ if self.focusing => PipMood::Focus,
            _ => base_mood(local_hour),
        }
    }
}

/// Resting mood when nothing is happening: sleepy late at night (23:00–06:00).
fn base_mood(local_hour: u32) -> PipMood {
    if !(6..23).contains(&local_hour) {
        PipMood::Sleepy
    } else {
        PipMood::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rests_idle_by_day_and_sleepy_at_night() {
        let engine = MoodEngine::default();
        let now = Instant::now();
        assert_eq!(engine.mood(now, 10), PipMood::Idle);
        assert_eq!(engine.mood(now, 22), PipMood::Idle);
        assert_eq!(engine.mood(now, 23), PipMood::Sleepy);
        assert_eq!(engine.mood(now, 3), PipMood::Sleepy);
        assert_eq!(engine.mood(now, 6), PipMood::Idle);
    }

    #[test]
    fn reactions_expire() {
        let mut engine = MoodEngine::default();
        let now = Instant::now();
        engine.react(PipSignal::Success, now);
        assert_eq!(engine.mood(now, 12), PipMood::Happy);
        assert_eq!(engine.mood(now + Duration::from_secs(3), 12), PipMood::Idle);
    }

    #[test]
    fn alert_is_not_interrupted_by_success() {
        let mut engine = MoodEngine::default();
        let now = Instant::now();
        engine.react(PipSignal::Alert, now);
        engine.react(PipSignal::Success, now + Duration::from_millis(500));
        assert_eq!(
            engine.mood(now + Duration::from_secs(1), 12),
            PipMood::Alert
        );
    }

    #[test]
    fn focus_is_the_resting_mood_during_a_session() {
        let mut engine = MoodEngine::default();
        let now = Instant::now();
        engine.react(PipSignal::FocusStarted, now);
        assert_eq!(engine.mood(now, 2), PipMood::Focus);
        engine.react(PipSignal::Celebration, now);
        assert_eq!(engine.mood(now, 12), PipMood::Celebrate);
        engine.react(PipSignal::FocusEnded, now);
        assert_eq!(engine.mood(now + Duration::from_secs(5), 12), PipMood::Idle);
    }

    #[test]
    fn stronger_reaction_replaces_weaker_one() {
        let mut engine = MoodEngine::default();
        let now = Instant::now();
        engine.react(PipSignal::Success, now);
        engine.react(PipSignal::Failure, now);
        assert_eq!(engine.mood(now, 12), PipMood::Sad);
    }
}
