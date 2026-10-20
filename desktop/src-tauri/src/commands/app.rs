use serde::Serialize;
use ts_rs::TS;

/// Basic build information shown in the UI.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    /// Registered global shortcut to toggle the island, e.g. `ctrl+alt+p`.
    pub shortcut: Option<String>,
    /// The quick-capture shortcut, if one could be registered.
    pub capture_shortcut: Option<String>,
}

#[tauri::command]
pub fn app_info(
    app: tauri::AppHandle,
    shortcut: tauri::State<'_, crate::ActiveShortcut>,
    capture: tauri::State<'_, crate::ActiveCapture>,
) -> AppInfo {
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        shortcut: shortcut.0.map(str::to_owned),
        capture_shortcut: capture.0.map(|(name, _)| name.to_owned()),
    }
}
