//! Skills: small add-ons anyone can write and share (the Preku Skill Format, see `format`).
//!
//! Installed skills live as `<data>/skills/<id>.json`; which ones are switched on is the
//! setting `skills`. Skills aren't a module the user switches on or off as a whole: each skill
//! has its own switch, and an installed skill is on until the user turns it off.
//!
//! The UI reads thoughts and sounds straight from the installed list. Nudges are timed here,
//! one background task ticking every 15 s, and appear as ordinary live items
//! (`skill.<skill id>.<nudge id>`) so they share the notch, quiet mode and activity settings
//! with everything else.

pub mod format;
pub mod schedule;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use ts_rs::TS;

use self::{format::SkillManifest, schedule::Scheduler};
use super::ModuleContext;
use crate::{
    foundation::{
        errors::{PrekuError, Result},
        event_bus::AppEvent,
        preferences,
        storage::now_millis,
    },
    live::{LiveItem, LivePriority},
    pip::mood_engine::PipSignal,
};

/// The `module` of skill live items.
pub const ID: &str = "skills";
pub const STATE_EVENT: &str = "skills:changed";
const SETTINGS_KEY: &str = "skills";
const TICK: Duration = Duration::from_secs(15);
/// A nudge leaves the notch by itself after a minute.
const NUDGE_TTL_MS: i64 = 60_000;
/// Most skills one person can install.
const MAX_INSTALLED: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstalledSkill {
    pub manifest: SkillManifest,
    pub enabled: bool,
}

/// A file in the skills folder that couldn't be read (e.g. edited by hand into a broken state).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrokenSkill {
    pub file: String,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkillsState {
    pub skills: Vec<InstalledSkill>,
    pub broken: Vec<BrokenSkill>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reads every skill in `dir`, sorted by name. A missing folder is simply "no skills".
fn read_dir(dir: &Path, enabled: &BTreeMap<String, bool>) -> SkillsState {
    let mut state = SkillsState::default();
    let Ok(entries) = fs::read_dir(dir) else {
        return state;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parsed = fs::read_to_string(&path)
            .map_err(|err| vec![err.to_string()])
            .and_then(|text| format::parse(&text));
        match parsed {
            // The file name is the id: a renamed copy would shadow or confuse the real one.
            Ok(manifest) if file == format!("{}.json", manifest.id) => {
                state.skills.push(InstalledSkill {
                    enabled: enabled.get(&manifest.id).copied().unwrap_or(true),
                    manifest,
                })
            }
            Ok(manifest) => state.broken.push(BrokenSkill {
                file,
                problems: vec![format!("the file should be named {}.json", manifest.id)],
            }),
            Err(problems) => state.broken.push(BrokenSkill { file, problems }),
        }
    }
    state.skills.sort_by_key(|s| s.manifest.name.to_lowercase());
    state.broken.sort_by(|a, b| a.file.cmp(&b.file));
    state
}

struct Shared {
    ctx: ModuleContext,
    dir: PathBuf,
    state: Mutex<SkillsState>,
    /// The skill nudge on screen: (live item id, shown at).
    showing: Mutex<Option<(String, i64)>>,
}

impl Shared {
    fn enabled_map(&self) -> BTreeMap<String, bool> {
        self.ctx
            .storage
            .settings_get(SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default()
    }

    fn reload(&self) -> SkillsState {
        let state = read_dir(&self.dir, &self.enabled_map());
        *lock(&self.state) = state.clone();
        self.ctx.emit(STATE_EVENT, &state);
        state
    }

    fn clear_nudge(&self) {
        if let Some((id, _)) = lock(&self.showing).take() {
            self.ctx.publish(AppEvent::LiveRemove { id });
        }
    }
}

/// Owns the installed skills and the nudge timer. Started once at launch; while Preku is
/// paused the timer keeps still.
pub struct SkillsService {
    shared: Arc<Shared>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl SkillsService {
    pub fn new(ctx: ModuleContext) -> Self {
        let dir = ctx.data_dir.join("skills");
        let shared = Arc::new(Shared {
            ctx,
            dir,
            state: Mutex::new(SkillsState::default()),
            showing: Mutex::new(None),
        });
        shared.reload();
        Self {
            shared,
            task: Mutex::new(None),
        }
    }

    pub fn start(&self) {
        let task = tauri::async_runtime::spawn(nudge_loop(self.shared.clone()));
        if let Some(old) = lock(&self.task).replace(task) {
            old.abort();
        }
    }

    pub fn state(&self) -> SkillsState {
        lock(&self.shared.state).clone()
    }

    /// Installs (or updates) the skill in `text`. Every problem is reported, one per line.
    pub fn install(&self, text: &str) -> Result<(SkillsState, String)> {
        let manifest = format::parse(text)
            .map_err(|problems| PrekuError::InvalidInput(problems.join("\n")))?;
        let current = self.state();
        let update = current.skills.iter().any(|s| s.manifest.id == manifest.id);
        if !update && current.skills.len() >= MAX_INSTALLED {
            return Err(PrekuError::InvalidInput(format!(
                "Preku holds up to {MAX_INSTALLED} skills; remove one first"
            )));
        }
        fs::create_dir_all(&self.shared.dir)?;
        // Stored as Preku wrote it (re-serialised): never the raw bytes that were picked.
        let body = serde_json::to_string_pretty(&manifest)?;
        let path = self.shared.dir.join(format!("{}.json", manifest.id));
        let partial = path.with_extension("json.part");
        fs::write(&partial, body)?;
        fs::rename(&partial, &path)?;
        let name = manifest.name.clone();
        Ok((self.shared.reload(), name))
    }

    pub fn install_file(&self, path: &Path) -> Result<(SkillsState, String)> {
        let size = fs::metadata(path)?.len();
        if size > format::MAX_FILE_BYTES as u64 {
            return Err(PrekuError::InvalidInput(format!(
                "the file is larger than {} KB",
                format::MAX_FILE_BYTES / 1024
            )));
        }
        self.install(&fs::read_to_string(path)?)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<SkillsState> {
        if !format::valid_id(id) {
            return Err(PrekuError::InvalidInput("unknown skill".into()));
        }
        let mut map = self.shared.enabled_map();
        map.insert(id.to_owned(), enabled);
        self.shared
            .ctx
            .storage
            .settings_set(SETTINGS_KEY, &serde_json::to_value(&map)?)?;
        if !enabled {
            self.clear_skill_nudge(id);
        }
        Ok(self.shared.reload())
    }

    pub fn remove(&self, id: &str) -> Result<SkillsState> {
        // The id becomes a file name: only well-formed ids, so no path can escape the folder.
        if !format::valid_id(id) {
            return Err(PrekuError::InvalidInput("unknown skill".into()));
        }
        let path = self.shared.dir.join(format!("{id}.json"));
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        let mut map = self.shared.enabled_map();
        if map.remove(id).is_some() {
            self.shared
                .ctx
                .storage
                .settings_set(SETTINGS_KEY, &serde_json::to_value(&map)?)?;
        }
        self.clear_skill_nudge(id);
        Ok(self.shared.reload())
    }

    /// Removes a broken file from the skills folder (by its file name).
    pub fn remove_broken(&self, file: &str) -> Result<SkillsState> {
        let known = self.state().broken.iter().any(|b| b.file == file);
        if !known || file.contains(['/', '\\']) || file.starts_with('.') {
            return Err(PrekuError::InvalidInput("unknown file".into()));
        }
        fs::remove_file(self.shared.dir.join(file))?;
        Ok(self.shared.reload())
    }

    fn clear_skill_nudge(&self, skill_id: &str) {
        let prefix = format!("skill.{skill_id}.");
        let showing = lock(&self.shared.showing)
            .as_ref()
            .is_some_and(|(id, _)| id.starts_with(&prefix));
        if showing {
            self.shared.clear_nudge();
        }
    }
}

impl Drop for SkillsService {
    fn drop(&mut self) {
        if let Some(task) = lock(&self.task).take() {
            task.abort();
        }
    }
}

async fn nudge_loop(shared: Arc<Shared>) {
    let mut scheduler = Scheduler::seeded(now_millis() as u64);
    loop {
        tokio::time::sleep(TICK).await;
        let now_ms = now_millis();
        // A nudge nobody touched leaves after a minute (the live item expires by itself).
        let showing = {
            let mut showing = lock(&shared.showing);
            if showing
                .as_ref()
                .is_some_and(|(_, at)| now_ms - at > NUDGE_TTL_MS)
            {
                *showing = None;
            }
            showing.is_some()
        };
        let paused = preferences::load(&shared.ctx.storage).is_ok_and(|p| p.paused);
        if paused {
            continue;
        }
        let skills = lock(&shared.state).skills.clone();
        let enabled: Vec<&SkillManifest> = skills
            .iter()
            .filter(|s| s.enabled)
            .map(|s| &s.manifest)
            .collect();
        if enabled.iter().all(|s| s.contributes.nudges.is_empty()) {
            continue;
        }
        let activity = tauri::async_runtime::spawn_blocking(super::wellbeing::platform::sample)
            .await
            .unwrap_or_default();
        let due = scheduler.tick(
            &enabled,
            TICK.as_millis() as u64,
            activity.idle_ms,
            shared.ctx.quiet.is_quiet(now_ms),
            chrono::Local::now().naive_local(),
            !showing,
        );
        let Some(due) = due else {
            continue;
        };
        let Some(skill) = enabled.iter().find(|s| s.id == due.skill_id) else {
            continue;
        };
        let id = format!("skill.{}.{}", due.skill_id, due.nudge_id);
        *lock(&shared.showing) = Some((id.clone(), now_ms));
        shared.ctx.publish(AppEvent::LiveUpsert(
            LiveItem::new(
                id,
                ID,
                LivePriority::Alert,
                due.line,
                Some(skill.name.clone()),
            )
            .expires_in(NUDGE_TTL_MS),
        ));
        shared.ctx.publish(AppEvent::Pip(PipSignal::Alert));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::{event_bus::EventBus, storage::Storage};

    const KIND_WORDS: &str = include_str!("../../../../../skills/examples/kind-words.json");
    const WRISTS: &str = include_str!("../../../../../skills/examples/wrist-and-eye-care.json");

    fn service() -> (SkillsService, PathBuf) {
        // Tests run in parallel: each gets its own folder.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "preku-skills-{}-{}-{n}",
            std::process::id(),
            now_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        let ctx = ModuleContext {
            bus: EventBus::default(),
            storage: Arc::new(Storage::open_in_memory().unwrap()),
            data_dir: dir.clone(),
            quiet: Arc::default(),
            privacy: Arc::default(),
        };
        (SkillsService::new(ctx), dir)
    }

    #[test]
    fn install_toggle_update_and_remove() {
        let (skills, dir) = service();
        assert!(skills.state().skills.is_empty());

        let (_, name) = skills.install(WRISTS).unwrap();
        assert_eq!(name, "Wrist & eye care");
        skills.install(KIND_WORDS).unwrap();
        let state = skills.state();
        assert_eq!(state.skills.len(), 2);
        assert_eq!(
            state.skills[0].manifest.name, "Kind words",
            "sorted by name"
        );
        assert!(state.skills.iter().all(|s| s.enabled), "on once installed");
        assert!(dir.join("skills/app.preku.kind-words.json").exists());

        let state = skills.set_enabled("app.preku.kind-words", false).unwrap();
        assert!(!state.skills[0].enabled);
        // Installing a new version keeps the user's switch.
        let newer = KIND_WORDS.replace("\"1.0.0\"", "\"1.1.0\"");
        skills.install(&newer).unwrap();
        let state = skills.state();
        assert_eq!(state.skills.len(), 2);
        assert_eq!(state.skills[0].manifest.version, "1.1.0");
        assert!(!state.skills[0].enabled);

        let state = skills.remove("app.preku.kind-words").unwrap();
        assert_eq!(state.skills.len(), 1);
        assert!(!dir.join("skills/app.preku.kind-words.json").exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn bad_files_and_ids_are_refused() {
        let (skills, dir) = service();
        let err = skills.install("{\"formatVersion\": 1}").unwrap_err();
        assert!(err.to_string().contains("not a valid skill file"));
        for id in ["../preku", "..\\x", "a", "UPPER.case"] {
            assert!(skills.remove(id).is_err(), "{id}");
            assert!(skills.set_enabled(id, true).is_err(), "{id}");
        }
        assert!(skills.remove_broken("../preku.db").is_err());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn broken_or_misnamed_files_are_listed_not_loaded() {
        let (skills, dir) = service();
        let folder = dir.join("skills");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("broken.json"), "{ nope").unwrap();
        fs::write(folder.join("renamed.json"), KIND_WORDS).unwrap();
        fs::write(folder.join("notes.txt"), "ignored").unwrap();
        let state = skills.shared.reload();
        assert!(state.skills.is_empty());
        assert_eq!(state.broken.len(), 2);
        assert!(state.broken[1].problems[0].contains("app.preku.kind-words.json"));
        let state = skills.remove_broken("broken.json").unwrap();
        assert_eq!(state.broken.len(), 1);
        fs::remove_dir_all(dir).ok();
    }
}
