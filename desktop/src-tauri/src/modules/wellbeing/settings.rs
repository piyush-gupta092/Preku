//! Wellbeing settings, stored as JSON under the `wellbeing` settings key.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::foundation::{
    errors::{PrekuError, Result},
    storage::Storage,
};

const KEY: &str = "wellbeing";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct WellbeingSettings {
    /// 20-20-20: every 20 min of screen use, look 20 ft (6 m) away for 20 s.
    pub eyes_enabled: bool,
    pub eyes_every_min: u32,
    /// Movement break ("stand" is the stored name from Phase 4).
    pub stand_enabled: bool,
    pub stand_every_min: u32,
    /// Suggest seated stretches instead of standing up or walking.
    pub seated_movement: bool,
    pub posture_enabled: bool,
    pub posture_every_min: u32,
    /// Posture and water nudges pop in at unpredictable moments around their interval
    /// (like someone walking past), instead of on the dot.
    pub random_nudges: bool,
    pub water_enabled: bool,
    pub water_every_min: u32,
    pub water_goal_ml: u32,
    pub glass_ml: u32,
    /// "You've been on this PC for a while": continuous use without a 5-minute break.
    pub screen_balance_enabled: bool,
    pub screen_balance_min: u32,
    /// Wind-down reminder `wind_down_min` before the user's chosen bedtime (HH:MM).
    pub bedtime_enabled: bool,
    pub bedtime: String,
    pub wind_down_min: u32,
    /// Meal reminders at the user's chosen times (HH:MM).
    pub meals_enabled: bool,
    pub meal_times: Vec<String>,
    /// Optional daily "How are you feeling?" (sensitive, so off until the user opts in).
    pub mood_check_in_enabled: bool,
    pub mood_check_in_time: String,
    /// No prompts in this window (bedtime still shows, since the user asked for it).
    pub quiet_hours_enabled: bool,
    pub quiet_start: String,
    pub quiet_end: String,
    /// "Later" waits this long.
    pub snooze_min: u32,
    /// A short "nice, you made time for yourself" after a completed break.
    pub encouragement_enabled: bool,
    pub screen_time_enabled: bool,
}

impl Default for WellbeingSettings {
    fn default() -> Self {
        Self {
            eyes_enabled: true,
            eyes_every_min: 20,
            stand_enabled: true,
            stand_every_min: 50,
            seated_movement: false,
            // A quick "sit up straight?" nudge (user request); existing choices are kept.
            posture_enabled: true,
            // With random timing this lands between about 5 and 10 minutes (user request).
            posture_every_min: 8,
            random_nudges: true,
            water_enabled: true,
            water_every_min: 60,
            water_goal_ml: 2000,
            glass_ml: 250,
            screen_balance_enabled: true,
            screen_balance_min: 120,
            bedtime_enabled: false,
            bedtime: "23:00".into(),
            wind_down_min: 30,
            meals_enabled: false,
            meal_times: vec!["13:00".into(), "20:00".into()],
            mood_check_in_enabled: false,
            mood_check_in_time: "18:00".into(),
            quiet_hours_enabled: false,
            quiet_start: "22:30".into(),
            quiet_end: "07:00".into(),
            snooze_min: 10,
            encouragement_enabled: true,
            screen_time_enabled: true,
        }
    }
}

/// Parses "HH:MM" (24 h).
pub fn parse_time(value: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(value.trim(), "%H:%M").ok()
}

impl WellbeingSettings {
    pub fn validate(&self) -> Result<()> {
        let minutes_ok = |m: u32| (5..=240).contains(&m);
        if !minutes_ok(self.eyes_every_min)
            || !minutes_ok(self.stand_every_min)
            || !minutes_ok(self.water_every_min)
            || !minutes_ok(self.posture_every_min)
        {
            return Err(PrekuError::InvalidInput(
                "Reminder intervals must be 5-240 minutes".into(),
            ));
        }
        if !(30..=480).contains(&self.screen_balance_min) {
            return Err(PrekuError::InvalidInput(
                "Screen balance must be 30-480 minutes".into(),
            ));
        }
        if !(500..=6000).contains(&self.water_goal_ml) || !(50..=1000).contains(&self.glass_ml) {
            return Err(PrekuError::InvalidInput(
                "Water goal must be 500-6000 ml and a glass 50-1000 ml".into(),
            ));
        }
        if !(5..=60).contains(&self.snooze_min) || self.wind_down_min > 120 {
            return Err(PrekuError::InvalidInput(
                "Snooze must be 5-60 minutes and wind-down at most 120".into(),
            ));
        }
        let times = [
            &self.bedtime,
            &self.mood_check_in_time,
            &self.quiet_start,
            &self.quiet_end,
        ];
        if times
            .into_iter()
            .chain(self.meal_times.iter())
            .any(|t| parse_time(t).is_none())
            || self.meal_times.len() > 4
        {
            return Err(PrekuError::InvalidInput(
                "Times must look like 13:30 (at most 4 meals)".into(),
            ));
        }
        Ok(())
    }
}

pub fn load(storage: &Storage) -> Result<WellbeingSettings> {
    let Some(value) = storage.settings_get(KEY)? else {
        return Ok(WellbeingSettings::default());
    };
    Ok(serde_json::from_value(value).unwrap_or_else(|err| {
        tracing::warn!(error = %err, "wellbeing settings unreadable, using defaults");
        WellbeingSettings::default()
    }))
}

pub fn save(storage: &Storage, settings: &WellbeingSettings) -> Result<()> {
    settings.validate()?;
    storage.settings_set(KEY, &serde_json::to_value(settings)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_round_trip() {
        let storage = Storage::open_in_memory().unwrap();
        let mut settings = load(&storage).unwrap();
        assert_eq!(settings, WellbeingSettings::default());
        settings.water_goal_ml = 2500;
        save(&storage, &settings).unwrap();
        assert_eq!(load(&storage).unwrap().water_goal_ml, 2500);
    }

    #[test]
    fn rejects_silly_values() {
        let storage = Storage::open_in_memory().unwrap();
        let settings = WellbeingSettings {
            eyes_every_min: 1,
            ..WellbeingSettings::default()
        };
        assert!(save(&storage, &settings).is_err());
        let settings = WellbeingSettings {
            glass_ml: 5000,
            ..WellbeingSettings::default()
        };
        assert!(save(&storage, &settings).is_err());
        let settings = WellbeingSettings {
            bedtime: "25:00".into(),
            ..WellbeingSettings::default()
        };
        assert!(save(&storage, &settings).is_err());
        let settings = WellbeingSettings {
            meal_times: vec!["noon".into()],
            ..WellbeingSettings::default()
        };
        assert!(save(&storage, &settings).is_err());
    }

    #[test]
    fn older_documents_get_the_new_defaults() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .settings_set(KEY, &serde_json::json!({ "eyesEveryMin": 30 }))
            .unwrap();
        let settings = load(&storage).unwrap();
        assert_eq!(settings.eyes_every_min, 30);
        assert!(
            !settings.mood_check_in_enabled,
            "sensitive data stays opt-in"
        );
        assert_eq!(settings.snooze_min, 10);
    }
}
