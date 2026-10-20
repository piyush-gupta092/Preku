//! When skill nudges appear. Pure (time and activity are passed in), so it's unit-tested.
//!
//! Each nudge has its own clock of *active* minutes: time away from the PC doesn't count, so
//! nobody comes back from lunch to a pile of nudges. A due nudge waits while Preku is quiet
//! (a pause, focus, a game or a presentation) or outside its days and hours, then shows once.
//! Two skill nudges are always at least [`GAP_MS`] apart.

use chrono::{Datelike, NaiveDateTime, Timelike};

use super::format::{clock_minutes, SkillManifest, SkillNudge};

/// No input for this long counts as away.
pub const AWAY_AFTER_MS: u64 = 2 * 60 * 1000;
/// Shortest time between two skill nudges.
pub const GAP_MS: u64 = 2 * 60 * 1000;
const JITTER_MIN: f64 = 0.65;
const JITTER_SPAN: f64 = 0.6;

/// A nudge to show now.
#[derive(Debug, Clone, PartialEq)]
pub struct Due {
    pub skill_id: String,
    pub nudge_id: String,
    pub line: String,
}

#[derive(Debug)]
struct Clock {
    skill_id: String,
    nudge_id: String,
    active_ms: u64,
    target_ms: u64,
    next_line: usize,
}

#[derive(Debug)]
pub struct Scheduler {
    clocks: Vec<Clock>,
    rng: u64,
    since_last_ms: u64,
}

/// May `nudge` appear at local time `now` (its weekdays and hours)?
pub fn allowed(nudge: &SkillNudge, now: NaiveDateTime) -> bool {
    let weekday = now.weekday().number_from_monday() as u8;
    if nudge
        .days
        .as_ref()
        .is_some_and(|days| !days.contains(&weekday))
    {
        return false;
    }
    let Some([from, to]) = &nudge.between else {
        return true;
    };
    let (Some(from), Some(to)) = (clock_minutes(from), clock_minutes(to)) else {
        return true;
    };
    let minute = now.hour() * 60 + now.minute();
    if from < to {
        (from..to).contains(&minute)
    } else {
        // An evening-to-morning window such as 22:00–06:00.
        minute >= from || minute < to
    }
}

impl Scheduler {
    pub fn seeded(seed: u64) -> Self {
        // splitmix64, so two close seeds (start-up times) differ at once.
        let mut mixed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self {
            clocks: Vec::new(),
            rng: (mixed ^ (mixed >> 31)) | 1,
            since_last_ms: GAP_MS,
        }
    }

    /// 0 ≤ x < 1 (xorshift64).
    fn unit(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    fn target_ms(&mut self, nudge: &SkillNudge) -> u64 {
        let every = u64::from(nudge.every_min) * 60_000;
        if nudge.random {
            (every as f64 * (JITTER_MIN + JITTER_SPAN * self.unit())) as u64
        } else {
            every
        }
    }

    /// Keeps the clocks of nudges that still exist; new nudges start from zero.
    fn sync(&mut self, skills: &[&SkillManifest]) {
        let mut old = std::mem::take(&mut self.clocks);
        for skill in skills {
            for nudge in &skill.contributes.nudges {
                let found = old
                    .iter()
                    .position(|c| c.skill_id == skill.id && c.nudge_id == nudge.id);
                let clock = match found {
                    Some(i) => old.swap_remove(i),
                    None => Clock {
                        skill_id: skill.id.clone(),
                        nudge_id: nudge.id.clone(),
                        active_ms: 0,
                        target_ms: self.target_ms(nudge),
                        // Start somewhere in the list, so the first line isn't always the same.
                        next_line: (self.unit() * nudge.lines.len() as f64) as usize,
                    },
                };
                self.clocks.push(clock);
            }
        }
    }

    /// Advances time by `elapsed_ms`. Returns the nudge to show now, if one is due and may
    /// show. `can_show` is false while another skill nudge is still on screen.
    pub fn tick(
        &mut self,
        skills: &[&SkillManifest],
        elapsed_ms: u64,
        idle_ms: Option<u64>,
        quiet: bool,
        now: NaiveDateTime,
        can_show: bool,
    ) -> Option<Due> {
        self.sync(skills);
        self.since_last_ms = self.since_last_ms.saturating_add(elapsed_ms);
        // Idle time unknown (unsupported platform): count it as active.
        if idle_ms.is_some_and(|idle| idle >= AWAY_AFTER_MS) {
            return None;
        }
        for clock in &mut self.clocks {
            clock.active_ms = clock.active_ms.saturating_add(elapsed_ms);
        }
        if quiet || !can_show || self.since_last_ms < GAP_MS {
            return None;
        }

        let nudge_of = |clock: &Clock| {
            skills
                .iter()
                .find(|s| s.id == clock.skill_id)
                .and_then(|s| s.contributes.nudges.iter().find(|n| n.id == clock.nudge_id))
        };
        // The most overdue nudge that may show now.
        let index = self
            .clocks
            .iter()
            .enumerate()
            .filter(|(_, c)| c.active_ms >= c.target_ms)
            .filter(|(_, c)| nudge_of(c).is_some_and(|n| allowed(n, now)))
            .max_by_key(|(_, c)| c.active_ms - c.target_ms)
            .map(|(i, _)| i)?;
        let nudge = nudge_of(&self.clocks[index])?.clone();
        let target = self.target_ms(&nudge);
        let clock = &mut self.clocks[index];
        let line = nudge.lines[clock.next_line % nudge.lines.len()].clone();
        clock.next_line = clock.next_line.wrapping_add(1);
        clock.active_ms = 0;
        clock.target_ms = target;
        self.since_last_ms = 0;
        Some(Due {
            skill_id: clock.skill_id.clone(),
            nudge_id: clock.nudge_id.clone(),
            line,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::skills::format::parse;

    const MINUTE: u64 = 60_000;
    const TICK: u64 = 15_000;

    fn skill(nudges: serde_json::Value) -> SkillManifest {
        parse(
            &serde_json::json!({
                "formatVersion": 1, "id": "io.github.alex.test", "name": "Test",
                "version": "1.0.0", "author": { "name": "Alex" }, "description": "Test skill.",
                "contributes": { "nudges": nudges }
            })
            .to_string(),
        )
        .unwrap()
    }

    /// Monday 2026-10-19 at `h:m`.
    fn at(h: u32, m: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 19)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    /// Ticks for `minutes`; returns what was shown.
    fn run(
        scheduler: &mut Scheduler,
        skill: &SkillManifest,
        minutes: u64,
        idle_ms: Option<u64>,
        quiet: bool,
        now: NaiveDateTime,
    ) -> Vec<Due> {
        (0..minutes * MINUTE / TICK)
            .filter_map(|_| scheduler.tick(&[skill], TICK, idle_ms, quiet, now, true))
            .collect()
    }

    #[test]
    fn a_fixed_nudge_comes_on_time_with_lines_in_turn() {
        let skill = skill(serde_json::json!([
            { "id": "wrists", "lines": ["a", "b"], "everyMin": 10, "random": false }
        ]));
        let mut scheduler = Scheduler::seeded(1);
        assert!(run(&mut scheduler, &skill, 9, Some(0), false, at(10, 0)).is_empty());
        let shown = run(&mut scheduler, &skill, 22, Some(0), false, at(10, 0));
        assert_eq!(shown.len(), 3, "at 10, 20 and 30 minutes");
        assert_ne!(shown[0].line, shown[1].line, "lines take turns");
        assert_eq!(shown[0].line, shown[2].line);
        assert_eq!(shown[0].nudge_id, "wrists");
    }

    #[test]
    fn time_away_and_quiet_time_dont_bring_nudges() {
        let skill = skill(serde_json::json!([
            { "id": "eyes", "lines": ["Look away"], "everyMin": 10, "random": false }
        ]));
        let mut scheduler = Scheduler::seeded(2);
        // An hour away from the PC: the clock doesn't move.
        assert!(run(
            &mut scheduler,
            &skill,
            60,
            Some(10 * MINUTE),
            false,
            at(10, 0)
        )
        .is_empty());
        // Due while quiet: it waits…
        assert!(run(&mut scheduler, &skill, 15, Some(0), true, at(10, 0)).is_empty());
        // …and shows once when quiet ends, not once per missed interval.
        assert_eq!(
            run(&mut scheduler, &skill, 1, Some(0), false, at(10, 0)).len(),
            1
        );
    }

    #[test]
    fn days_and_hours_are_respected() {
        let skill = skill(serde_json::json!([
            { "id": "w", "lines": ["x"], "everyMin": 5, "random": false,
              "days": [1, 2, 3, 4, 5], "between": ["09:00", "18:00"] }
        ]));
        let mut scheduler = Scheduler::seeded(3);
        assert!(run(&mut scheduler, &skill, 30, Some(0), false, at(20, 0)).is_empty());
        assert_eq!(
            run(&mut scheduler, &skill, 1, Some(0), false, at(9, 0)).len(),
            1
        );
        let sunday = at(10, 0) - chrono::Duration::days(1);
        assert!(run(&mut scheduler, &skill, 30, Some(0), false, sunday).is_empty());

        // A window across midnight.
        let night = SkillNudge {
            between: Some(["22:00".into(), "06:00".into()]),
            days: None,
            ..skill.contributes.nudges[0].clone()
        };
        assert!(allowed(&night, at(23, 30)) && allowed(&night, at(5, 59)));
        assert!(!allowed(&night, at(6, 0)) && !allowed(&night, at(12, 0)));
    }

    #[test]
    fn random_nudges_vary_and_keep_a_gap() {
        let skill = skill(serde_json::json!([
            { "id": "a", "lines": ["a"], "everyMin": 5 },
            { "id": "b", "lines": ["b"], "everyMin": 5 }
        ]));
        let mut scheduler = Scheduler::seeded(4);
        let mut shown_at = Vec::new();
        for step in 0..(8 * 60 * MINUTE / TICK) {
            if scheduler
                .tick(&[&skill], TICK, Some(0), false, at(10, 0), true)
                .is_some()
            {
                shown_at.push(step * TICK);
            }
        }
        let gaps: Vec<u64> = shown_at.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.iter().all(|&gap| gap >= GAP_MS), "{gaps:?}");
        assert!(gaps.iter().any(|&g| g != gaps[0]), "random, not on the dot");
        // Two nudges every ~5 min (jittered to 3¼–6¼ min) over 8 hours.
        assert!((100..=300).contains(&shown_at.len()), "{}", shown_at.len());
    }

    #[test]
    fn nothing_shows_while_another_skill_nudge_is_up() {
        let skill = skill(serde_json::json!([
            { "id": "a", "lines": ["a"], "everyMin": 5, "random": false }
        ]));
        let mut scheduler = Scheduler::seeded(5);
        for _ in 0..(10 * MINUTE / TICK) {
            assert!(scheduler
                .tick(&[&skill], TICK, Some(0), false, at(10, 0), false)
                .is_none());
        }
        assert!(scheduler
            .tick(&[&skill], TICK, Some(0), false, at(10, 0), true)
            .is_some());
    }
}
