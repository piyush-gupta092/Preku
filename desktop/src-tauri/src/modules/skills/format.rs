//! The Preku Skill Format, version 1. The README ("Make a skill") is the public description,
//! `skills/schema/skill-1.schema.json` the editor schema, and this file the reference
//! implementation: when they disagree, this file wins.
//!
//! A v1 skill is **data only**: a JSON file that adds thoughts for "A moment for you", nudges,
//! and short synthesised sounds. It cannot run code, read the user's data or go online, so
//! installing one from anywhere is safe. Every field is checked against hard limits, and
//! unknown fields are refused (a typo is reported instead of silently ignored).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const FORMAT_VERSION: u32 = 1;
/// Largest skill file accepted.
pub const MAX_FILE_BYTES: usize = 256 * 1024;

const MAX_THOUGHTS: usize = 500;
const MAX_THOUGHT_CHARS: usize = 140;
const MAX_NUDGES: usize = 10;
const MAX_LINES: usize = 20;
const MAX_LINE_CHARS: usize = 60;
const MAX_SOUNDS: usize = 5;
const MAX_NOTES: usize = 12;
const MAX_SOUND_SECONDS: f32 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SkillAuthor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub url: Option<String>,
}

/// A line for "A moment for you". With `author`, it is shown as a quote by that person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SkillThought {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub author: Option<String>,
}

/// A nudge: one short line now and then, gone again in a minute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SkillNudge {
    /// Unique within the skill: `a-z`, `0-9`, `-`.
    pub id: String,
    /// One is shown each time, in turn.
    pub lines: Vec<String>,
    /// Minutes of active use between two of these nudges (5–240).
    pub every_min: u32,
    /// Land at an unpredictable moment around `every_min` (65%–125%) instead of on the dot.
    #[serde(default = "yes")]
    pub random: bool,
    /// ISO weekdays it may appear on, 1 = Monday … 7 = Sunday. Missing = every day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub days: Option<Vec<u8>>,
    /// Local time window it may appear in, `["09:00", "18:00"]`. Missing = any time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub between: Option<[String; 2]>,
    /// `"default"` (Preku's pop-up sound), `"none"`, or the id of a sound in this skill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sound: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Wave {
    Sine,
    Triangle,
    Square,
}

/// One note of a sound, played by the app's own synthesiser (no audio files).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SkillNote {
    /// Hz, 50–4000.
    pub freq: f32,
    /// Seconds from the start of the sound.
    pub at: f32,
    /// Seconds, 0.01–1.5.
    pub length: f32,
    /// 0–0.4 (quiet by design).
    pub gain: f32,
    #[serde(default = "sine")]
    pub wave: Wave,
}

fn sine() -> Wave {
    Wave::Sine
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SkillSound {
    pub id: String,
    pub name: String,
    pub notes: Vec<SkillNote>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SkillContributions {
    #[serde(default)]
    pub thoughts: Vec<SkillThought>,
    #[serde(default)]
    pub nudges: Vec<SkillNudge>,
    #[serde(default)]
    pub sounds: Vec<SkillSound>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SkillManifest {
    /// Editors use it to find the schema; Preku ignores it.
    #[serde(rename = "$schema", default, skip_serializing)]
    #[ts(skip)]
    pub schema: Option<String>,
    pub format_version: u32,
    /// Reverse-domain, lowercase: `io.github.alex.wrist-care`.
    pub id: String,
    pub name: String,
    /// Semantic version: `1.2.0`.
    pub version: String,
    pub author: SkillAuthor,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub homepage: Option<String>,
    pub contributes: SkillContributions,
}

fn plain_text(field: &str, text: &str, max: usize, problems: &mut Vec<String>) {
    let length = text.trim().chars().count();
    if length == 0 {
        problems.push(format!("{field} is empty"));
    } else if length > max {
        problems.push(format!(
            "{field} is {length} characters; the limit is {max}"
        ));
    }
    if text.chars().any(char::is_control) {
        problems.push(format!(
            "{field} contains control characters (line breaks, tabs…)"
        ));
    }
}

fn slug(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 32
        && !text.starts_with('-')
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `io.github.alex.wrist-care`: 2–6 lowercase slugs joined by dots.
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    (2..=6).contains(&parts.len()) && id.len() <= 100 && parts.iter().all(|p| slug(p))
}

fn valid_version(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    version.len() <= 40
        && parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 6 && p.chars().all(|c| c.is_ascii_digit()))
}

fn valid_url(url: &str) -> bool {
    url.len() <= 300
        && reqwest::Url::parse(url).is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some())
}

/// `"09:30"` → minutes since midnight.
pub fn clock_minutes(text: &str) -> Option<u32> {
    let (h, m) = text.split_once(':')?;
    if h.len() != 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Everything wrong with a manifest, in plain words (empty = it is valid).
pub fn problems(skill: &SkillManifest) -> Vec<String> {
    let mut out = Vec::new();
    if skill.format_version != FORMAT_VERSION {
        out.push(format!(
            "formatVersion is {}; this Preku reads version {FORMAT_VERSION}",
            skill.format_version
        ));
    }
    if !valid_id(&skill.id) {
        out.push(
            "id must look like io.github.yourname.skill-name (lowercase letters, digits, dashes, dots)"
                .into(),
        );
    }
    plain_text("name", &skill.name, 40, &mut out);
    plain_text("description", &skill.description, 200, &mut out);
    plain_text("author.name", &skill.author.name, 60, &mut out);
    if !valid_version(&skill.version) {
        out.push("version must be a semantic version such as 1.0.0".into());
    }
    for (field, url) in [
        ("author.url", &skill.author.url),
        ("homepage", &skill.homepage),
    ] {
        if url.as_deref().is_some_and(|u| !valid_url(u)) {
            out.push(format!("{field} must be an https:// address"));
        }
    }
    if let Some(license) = &skill.license {
        plain_text("license", license, 40, &mut out);
    }

    let c = &skill.contributes;
    if c.thoughts.is_empty() && c.nudges.is_empty() && c.sounds.is_empty() {
        out.push("contributes is empty: add thoughts, nudges or sounds".into());
    }
    if c.thoughts.len() > MAX_THOUGHTS {
        out.push(format!(
            "{} thoughts; the limit is {MAX_THOUGHTS}",
            c.thoughts.len()
        ));
    }
    for (i, thought) in c.thoughts.iter().enumerate() {
        plain_text(
            &format!("thoughts[{i}].text"),
            &thought.text,
            MAX_THOUGHT_CHARS,
            &mut out,
        );
        if let Some(author) = &thought.author {
            plain_text(&format!("thoughts[{i}].author"), author, 60, &mut out);
        }
    }

    if c.sounds.len() > MAX_SOUNDS {
        out.push(format!(
            "{} sounds; the limit is {MAX_SOUNDS}",
            c.sounds.len()
        ));
    }
    let mut sound_ids = HashSet::new();
    for (i, sound) in c.sounds.iter().enumerate() {
        if !slug(&sound.id) || matches!(sound.id.as_str(), "default" | "none") {
            out.push(format!(
                "sounds[{i}].id must be a short slug (not \"default\" or \"none\")"
            ));
        }
        if !sound_ids.insert(sound.id.as_str()) {
            out.push(format!("sounds[{i}].id \"{}\" is used twice", sound.id));
        }
        plain_text(&format!("sounds[{i}].name"), &sound.name, 30, &mut out);
        if sound.notes.is_empty() || sound.notes.len() > MAX_NOTES {
            out.push(format!("sounds[{i}] needs 1–{MAX_NOTES} notes"));
        }
        for (j, note) in sound.notes.iter().enumerate() {
            let at = format!("sounds[{i}].notes[{j}]");
            let finite = [note.freq, note.at, note.length, note.gain]
                .iter()
                .all(|v| v.is_finite());
            if !finite
                || !(50.0..=4000.0).contains(&note.freq)
                || !(0.0..=MAX_SOUND_SECONDS).contains(&note.at)
                || !(0.01..=1.5).contains(&note.length)
                || !(0.0..=0.4).contains(&note.gain)
            {
                out.push(format!(
                    "{at}: freq 50–4000 Hz, at 0–2 s, length 0.01–1.5 s, gain 0–0.4"
                ));
            } else if note.at + note.length > MAX_SOUND_SECONDS {
                out.push(format!(
                    "{at} ends after {MAX_SOUND_SECONDS} s; sounds must stay short"
                ));
            }
        }
    }

    if c.nudges.len() > MAX_NUDGES {
        out.push(format!(
            "{} nudges; the limit is {MAX_NUDGES}",
            c.nudges.len()
        ));
    }
    let mut nudge_ids = HashSet::new();
    for (i, nudge) in c.nudges.iter().enumerate() {
        let at = format!("nudges[{i}]");
        if !slug(&nudge.id) {
            out.push(format!("{at}.id must be a short slug (a-z, 0-9, -)"));
        }
        if !nudge_ids.insert(nudge.id.as_str()) {
            out.push(format!("{at}.id \"{}\" is used twice", nudge.id));
        }
        if nudge.lines.is_empty() || nudge.lines.len() > MAX_LINES {
            out.push(format!("{at} needs 1–{MAX_LINES} lines"));
        }
        for (j, line) in nudge.lines.iter().enumerate() {
            plain_text(&format!("{at}.lines[{j}]"), line, MAX_LINE_CHARS, &mut out);
        }
        if !(5..=240).contains(&nudge.every_min) {
            out.push(format!("{at}.everyMin must be 5–240 minutes"));
        }
        if let Some(days) = &nudge.days {
            if days.is_empty() || days.iter().any(|d| !(1..=7).contains(d)) {
                out.push(format!("{at}.days lists weekdays 1 (Monday) to 7 (Sunday)"));
            }
        }
        if let Some([from, to]) = &nudge.between {
            let (from, to) = (clock_minutes(from), clock_minutes(to));
            if from.is_none() || to.is_none() || from == to {
                out.push(format!(
                    "{at}.between must be two different times like [\"09:00\", \"18:00\"]"
                ));
            }
        }
        if let Some(sound) = &nudge.sound {
            if sound != "default" && sound != "none" && !sound_ids.contains(sound.as_str()) {
                out.push(format!(
                    "{at}.sound \"{sound}\" is not one of this skill's sounds"
                ));
            }
        }
    }
    out
}

/// Reads and checks a skill file's text: the manifest, or every problem found.
pub fn parse(text: &str) -> Result<SkillManifest, Vec<String>> {
    if text.len() > MAX_FILE_BYTES {
        return Err(vec![format!(
            "the file is larger than {} KB",
            MAX_FILE_BYTES / 1024
        )]);
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let skill: SkillManifest =
        serde_json::from_str(text).map_err(|err| vec![format!("not a valid skill file: {err}")])?;
    let problems = problems(&skill);
    if problems.is_empty() {
        Ok(skill)
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> serde_json::Value {
        serde_json::json!({
            "formatVersion": 1,
            "id": "io.github.alex.kind-words",
            "name": "Kind words",
            "version": "1.0.0",
            "author": { "name": "Alex" },
            "description": "A few gentle lines for the morning.",
            "contributes": { "thoughts": [{ "text": "Go gently today." }] }
        })
    }

    fn check(value: serde_json::Value) -> Result<SkillManifest, Vec<String>> {
        parse(&value.to_string())
    }

    #[test]
    fn the_shipped_examples_are_valid() {
        for (name, text) in [
            (
                "wrist-and-eye-care",
                include_str!("../../../../../skills/examples/wrist-and-eye-care.json"),
            ),
            (
                "kind-words",
                include_str!("../../../../../skills/examples/kind-words.json"),
            ),
        ] {
            assert!(parse(text).is_ok(), "{name}: {:?}", parse(text).err());
        }
    }

    #[test]
    fn the_readme_example_is_valid() {
        let readme = include_str!("../../../../../README.md").replace("\r\n", "\n");
        let start = readme
            .find("```json\n")
            .expect("README has a skill example")
            + 8;
        let end = start + readme[start..].find("\n```").unwrap();
        let example = &readme[start..end];
        assert!(parse(example).is_ok(), "{:?}", parse(example).err());
    }

    #[test]
    fn a_minimal_skill_is_accepted() {
        let skill = check(minimal()).unwrap();
        assert_eq!(skill.contributes.thoughts.len(), 1);
        assert!(skill.contributes.nudges.is_empty());
        // A byte-order mark (Notepad) is fine.
        assert!(parse(&format!("\u{feff}{}", minimal())).is_ok());
    }

    #[test]
    fn typos_and_unknown_fields_are_refused() {
        let mut typo = minimal();
        typo["contributes"]["thoghts"] = serde_json::json!([]);
        assert!(check(typo).unwrap_err()[0].contains("unknown field"));
        let mut code = minimal();
        code["contributes"]["script"] = serde_json::json!("alert(1)");
        assert!(check(code).is_err(), "there is no place for code in v1");
    }

    #[test]
    fn limits_are_enforced_with_plain_messages() {
        let mut bad = minimal();
        bad["id"] = serde_json::json!("Kind Words");
        bad["version"] = serde_json::json!("one");
        bad["formatVersion"] = serde_json::json!(2);
        bad["homepage"] = serde_json::json!("http://example.com");
        bad["contributes"] = serde_json::json!({
            "thoughts": [{ "text": "x".repeat(141) }, { "text": "line\nbreak" }],
            "nudges": [
                { "id": "wrists", "lines": ["Roll your wrists?"], "everyMin": 2, "days": [0, 8],
                  "between": ["9am", "18:00"], "sound": "trumpet" },
                { "id": "wrists", "lines": [], "everyMin": 30 }
            ],
            "sounds": [{ "id": "none", "name": "Loud",
                         "notes": [{ "freq": 20000, "at": 0, "length": 3, "gain": 1 }] }]
        });
        let all = check(bad).unwrap_err().join("\n");
        for expected in [
            "formatVersion is 2",
            "id must look like",
            "semantic version",
            "homepage must be an https://",
            "thoughts[0].text is 141 characters",
            "thoughts[1].text contains control characters",
            "nudges[0].everyMin must be 5–240",
            "nudges[0].days lists weekdays",
            "nudges[0].between must be",
            "nudges[0].sound \"trumpet\" is not one of",
            "nudges[1].id \"wrists\" is used twice",
            "nudges[1] needs 1–20 lines",
            "sounds[0].id must be a short slug",
            "sounds[0].notes[0]: freq 50–4000 Hz",
        ] {
            assert!(all.contains(expected), "missing \"{expected}\" in:\n{all}");
        }
    }

    #[test]
    fn oversized_or_empty_files_are_refused() {
        assert!(parse(&" ".repeat(MAX_FILE_BYTES + 1)).is_err());
        assert!(parse("").is_err());
        let mut empty = minimal();
        empty["contributes"] = serde_json::json!({});
        assert!(check(empty).unwrap_err()[0].contains("contributes is empty"));
    }

    #[test]
    fn reads_clock_times() {
        assert_eq!(clock_minutes("09:30"), Some(570));
        assert_eq!(clock_minutes("23:59"), Some(1439));
        assert_eq!(clock_minutes("24:00"), None);
        assert_eq!(clock_minutes("9:30"), None);
    }
}
