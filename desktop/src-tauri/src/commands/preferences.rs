use serde::Serialize;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::{
    actions,
    foundation::{
        errors::{IpcError, PrekuError},
        preferences::Preferences,
    },
    modules::{ModuleInfo, ModuleRegistry},
};

#[tauri::command]
pub fn preferences_get(app: AppHandle) -> Preferences {
    actions::preferences(&app)
}

#[tauri::command]
pub fn preferences_set(app: AppHandle, preferences: Preferences) -> Result<(), IpcError> {
    Ok(actions::update_preferences(&app, preferences)?)
}

/// A connected display, for the monitor picker.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorInfo {
    pub name: String,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
}

#[tauri::command]
pub fn monitors_list(app: AppHandle) -> Result<Vec<MonitorInfo>, IpcError> {
    let to_err = |e: tauri::Error| IpcError::from(PrekuError::Window(e.to_string()));
    let primary = app
        .primary_monitor()
        .map_err(to_err)?
        .and_then(|m| m.name().cloned());
    Ok(app
        .available_monitors()
        .map_err(to_err)?
        .into_iter()
        .filter_map(|m| {
            let name = m.name()?.clone();
            Some(MonitorInfo {
                primary: primary.as_ref() == Some(&name),
                width: m.size().width,
                height: m.size().height,
                name,
            })
        })
        .collect())
}

#[tauri::command]
pub fn modules_list(app: AppHandle, registry: State<'_, ModuleRegistry>) -> Vec<ModuleInfo> {
    registry.list(&actions::preferences(&app).modules)
}

/// Must be `async`: on Windows, creating a window from a sync command deadlocks WebView2
/// (blank, unclosable window and a frozen island). See `WebviewWindowBuilder::build` docs.
#[tauri::command]
pub async fn settings_open(app: AppHandle) -> Result<(), IpcError> {
    Ok(actions::open_settings(&app)?)
}
