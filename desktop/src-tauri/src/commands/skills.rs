//! Skill commands (async: they read and write files).

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::{
    foundation::errors::IpcError,
    modules::skills::{SkillsService, SkillsState},
};

type Service<'a> = State<'a, Arc<SkillsService>>;
type Res<T> = Result<T, IpcError>;

/// After "Install a skill…": the new list, and the name of what was installed (none if the
/// picker was cancelled).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkillInstallResult {
    pub state: SkillsState,
    #[ts(optional)]
    pub installed: Option<String>,
}

#[tauri::command]
pub async fn skills_get(skills: Service<'_>) -> Res<SkillsState> {
    Ok(skills.state())
}

/// Lets the user pick a skill file and installs it. Problems come back as the error message,
/// one per line.
#[tauri::command]
pub async fn skill_install(app: tauri::AppHandle, skills: Service<'_>) -> Res<SkillInstallResult> {
    use tauri_plugin_dialog::DialogExt;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Install a Preku skill")
            .add_filter("Preku skill", &["json"])
            .blocking_pick_file()
    })
    .await
    .ok()
    .flatten()
    .and_then(|file| file.into_path().ok());
    match picked {
        Some(path) => {
            let (state, name) = skills.install_file(&path)?;
            Ok(SkillInstallResult {
                state,
                installed: Some(name),
            })
        }
        None => Ok(SkillInstallResult {
            state: skills.state(),
            installed: None,
        }),
    }
}

#[tauri::command]
pub async fn skill_set_enabled(skills: Service<'_>, id: String, enabled: bool) -> Res<SkillsState> {
    Ok(skills.set_enabled(&id, enabled)?)
}

#[tauri::command]
pub async fn skill_remove(skills: Service<'_>, id: String) -> Res<SkillsState> {
    Ok(skills.remove(&id)?)
}

#[tauri::command]
pub async fn skill_remove_broken(skills: Service<'_>, file: String) -> Res<SkillsState> {
    Ok(skills.remove_broken(&file)?)
}
