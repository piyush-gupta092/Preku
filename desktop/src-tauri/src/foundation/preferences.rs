//! User preferences, stored as one JSON document under the `preferences` settings key.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{errors::Result, storage::Storage};

const KEY: &str = "preferences";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum IslandPosition {
    Left,
    #[default]
    Center,
    Right,
}

/// A place picked in Settings for the weather (from Open-Meteo's geocoding search).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WeatherPlace {
    pub name: String,
    /// Region and country, e.g. "Maharashtra, India".
    pub detail: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Pip's body (Design.md §6): all three share the face, expressions and dances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum MascotStyle {
    /// The original Pip: a round pebble with a small face, stubby arms and little feet.
    #[default]
    Classic,
    /// Round head, bean body, long noodle arms and legs.
    Sprout,
    /// A soft ghost-sheet cover with arms and legs poking out.
    Ghost,
    /// Big head on a plump body, short thick limbs.
    Chubby,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Every field has a default, so older or partial documents still load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Preferences {
    pub island_position: IslandPosition,
    /// Monitor name; `None` = primary monitor.
    pub monitor: Option<String>,
    pub theme: Theme,
    pub launch_at_login: bool,
    /// Module id → enabled. Missing ids use the module's own default.
    pub modules: BTreeMap<String, bool>,
    /// Island hidden and all modules stopped.
    pub paused: bool,
    /// Short sounds for events such as a finished focus timer.
    pub sounds: bool,
    /// Module tiles shown in the expanded notch, in order (user-selectable).
    pub notch_modules: Vec<String>,
    /// Corner radius of the compact notch shown on hover, in px (0 = square).
    pub pill_radius: u32,
    /// Activity key → shown in the notch (pops out / appears in the pill). Missing = shown.
    pub notch_activities: BTreeMap<String, bool>,
    /// Weather is opt-in: nothing is fetched until the user picks a place.
    pub weather_place: Option<WeatherPlace>,
    pub temperature_unit: TemperatureUnit,
    pub mascot_style: MascotStyle,
    /// Pip's body color as `#rrggbb`.
    pub mascot_color: String,
    /// The short first-run welcome was shown (finished or skipped); it never comes back.
    pub welcomed: bool,
    /// Show the developer tools (Docker, builds, websites, command watcher) in the System
    /// view. On by default so nothing disappears for people who use them.
    pub dev_tools: bool,
    /// "A moment for you": once a day Pip offers a short thought.
    pub daily_thought: bool,
    /// Show the Today card (next action + recap) on the notch's home view. Off by default:
    /// it always lives in Productivity › Tasks.
    pub today_on_home: bool,
    /// Thoughts the user saved (newest first).
    pub saved_thoughts: Vec<String>,
}

/// Every module tile, in default order.
pub const NOTCH_MODULES: [&str; 4] = ["productivity", "monitor", "wellbeing", "bridge"];
pub const MAX_PILL_RADIUS: u32 = 24;
const MAX_SAVED_THOUGHTS: usize = 100;
const MAX_THOUGHT_CHARS: usize = 300;
pub const DEFAULT_MASCOT_COLOR: &str = "#ffb38a";

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl Preferences {
    /// Repairs out-of-range values from older or hand-edited documents instead of failing.
    pub fn normalized(mut self) -> Self {
        let mut seen = std::collections::HashSet::new();
        self.notch_modules
            .retain(|m| NOTCH_MODULES.contains(&m.as_str()) && seen.insert(m.clone()));
        if self.notch_modules.is_empty() {
            self.notch_modules = NOTCH_MODULES.iter().map(|m| (*m).to_owned()).collect();
        }
        self.pill_radius = self.pill_radius.min(MAX_PILL_RADIUS);
        // Saved thoughts: short lines, no duplicates, a bounded list.
        let mut kept = std::collections::HashSet::new();
        self.saved_thoughts = self
            .saved_thoughts
            .into_iter()
            .map(|thought| {
                thought
                    .trim()
                    .chars()
                    .take(MAX_THOUGHT_CHARS)
                    .collect::<String>()
            })
            .filter(|thought| !thought.is_empty() && kept.insert(thought.clone()))
            .take(MAX_SAVED_THOUGHTS)
            .collect();
        self.mascot_color = if is_hex_color(&self.mascot_color) {
            self.mascot_color.to_ascii_lowercase()
        } else {
            DEFAULT_MASCOT_COLOR.to_owned()
        };
        self
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            island_position: IslandPosition::default(),
            monitor: None,
            theme: Theme::default(),
            launch_at_login: false,
            modules: BTreeMap::new(),
            paused: false,
            sounds: true,
            notch_modules: NOTCH_MODULES.iter().map(|m| (*m).to_owned()).collect(),
            pill_radius: 18,
            notch_activities: BTreeMap::new(),
            weather_place: None,
            temperature_unit: TemperatureUnit::default(),
            mascot_style: MascotStyle::default(),
            mascot_color: DEFAULT_MASCOT_COLOR.to_owned(),
            welcomed: false,
            dev_tools: true,
            daily_thought: true,
            today_on_home: false,
            saved_thoughts: Vec::new(),
        }
    }
}

/// Loads preferences; a corrupt document falls back to defaults instead of blocking startup.
pub fn load(storage: &Storage) -> Result<Preferences> {
    let Some(value) = storage.settings_get(KEY)? else {
        return Ok(Preferences::default());
    };
    Ok(serde_json::from_value::<Preferences>(value)
        .unwrap_or_else(|err| {
            tracing::warn!(error = %err, "preferences unreadable, using defaults");
            Preferences::default()
        })
        .normalized())
}

pub fn save(storage: &Storage, preferences: &Preferences) -> Result<()> {
    let preferences = preferences.clone().normalized();
    storage.settings_set(KEY, &serde_json::to_value(&preferences)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_thoughts_are_tidied() {
        let prefs = Preferences {
            saved_thoughts: vec![
                "  Rest is part of the work.  ".into(),
                String::new(),
                "Rest is part of the work.".into(),
                "x".repeat(500),
            ],
            ..Preferences::default()
        }
        .normalized();
        assert_eq!(prefs.saved_thoughts.len(), 2);
        assert_eq!(prefs.saved_thoughts[0], "Rest is part of the work.");
        assert_eq!(prefs.saved_thoughts[1].chars().count(), MAX_THOUGHT_CHARS);
        // Older preference files have no list at all.
        let old: Preferences = serde_json::from_str("{}").unwrap();
        assert!(old.saved_thoughts.is_empty() && old.daily_thought);
    }
    use serde_json::json;

    #[test]
    fn round_trips_through_storage() {
        let storage = Storage::open_in_memory().unwrap();
        assert_eq!(load(&storage).unwrap(), Preferences::default());

        let mut prefs = Preferences {
            island_position: IslandPosition::Right,
            theme: Theme::Dark,
            ..Preferences::default()
        };
        prefs.modules.insert("demo".into(), false);
        save(&storage, &prefs).unwrap();
        assert_eq!(load(&storage).unwrap(), prefs);
    }

    #[test]
    fn notch_settings_are_repaired() {
        let prefs = Preferences {
            notch_modules: vec!["monitor".into(), "nonsense".into()],
            pill_radius: 99,
            ..Preferences::default()
        }
        .normalized();
        assert_eq!(prefs.notch_modules, ["monitor"]);
        assert_eq!(prefs.pill_radius, MAX_PILL_RADIUS);

        let empty = Preferences {
            notch_modules: vec![],
            ..Preferences::default()
        }
        .normalized();
        assert_eq!(empty.notch_modules.len(), 4, "never zero tiles");
    }

    #[test]
    fn mascot_color_is_a_hex_color() {
        let ok = Preferences {
            mascot_color: "#A0C4FF".into(),
            ..Preferences::default()
        }
        .normalized();
        assert_eq!(ok.mascot_color, "#a0c4ff");

        for bad in ["red", "#12345", "#gggggg", "", "#1234567"] {
            let prefs = Preferences {
                mascot_color: bad.into(),
                ..Preferences::default()
            }
            .normalized();
            assert_eq!(prefs.mascot_color, DEFAULT_MASCOT_COLOR, "{bad:?}");
        }
    }

    #[test]
    fn partial_and_corrupt_documents_fall_back_to_defaults() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .settings_set(KEY, &json!({ "theme": "light" }))
            .unwrap();
        let prefs = load(&storage).unwrap();
        assert_eq!(prefs.theme, Theme::Light);
        assert_eq!(prefs.island_position, IslandPosition::Center);

        storage.settings_set(KEY, &json!({ "theme": 42 })).unwrap();
        assert_eq!(load(&storage).unwrap(), Preferences::default());
    }
}
