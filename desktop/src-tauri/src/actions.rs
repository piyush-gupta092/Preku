//! App-level actions shared by commands, the tray and the shortcut handler.

use std::sync::PoisonError;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    foundation::{
        errors::{PrekuError, Result},
        preferences::{self, Preferences},
        state::AppState,
    },
    island::controller::{IslandController, Placement},
    modules::ModuleRegistry,
    tray,
};

pub const SETTINGS_WINDOW: &str = "settings";
pub const PREFERENCES_EVENT: &str = "preferences:changed";

pub fn preferences(app: &AppHandle) -> Preferences {
    app.state::<AppState>()
        .preferences
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Saves new preferences and applies every side effect, then tells all windows.
pub fn update_preferences(app: &AppHandle, next: Preferences) -> Result<()> {
    let next = next.normalized();
    let state = app.state::<AppState>();
    preferences::save(&state.storage, &next)?;
    let previous = std::mem::replace(
        &mut *state
            .preferences
            .lock()
            .unwrap_or_else(PoisonError::into_inner),
        next.clone(),
    );

    let placement = |p: &Preferences| Placement {
        position: p.island_position,
        monitor: p.monitor.clone(),
    };
    let controller = app.state::<IslandController>();
    if placement(&previous) != placement(&next) {
        controller.set_placement(app, placement(&next))?;
    }
    controller.set_paused(app, next.paused)?;
    app.state::<ModuleRegistry>()
        .apply(&next.modules, next.paused);
    if previous.launch_at_login != next.launch_at_login {
        sync_autostart(app, next.launch_at_login);
    }
    tray::sync(app, &next);

    app.emit(PREFERENCES_EVENT, &next)
        .map_err(|e| PrekuError::Window(e.to_string()))
}

pub fn toggle_pause(app: &AppHandle) {
    let mut next = preferences(app);
    next.paused = !next.paused;
    if let Err(err) = update_preferences(app, next) {
        tracing::warn!(error = %err, "failed to toggle pause");
    }
}

/// Makes the OS login item match the preference. Failures are logged, never fatal.
pub fn sync_autostart(app: &AppHandle, enabled: bool) {
    // A development build only works next to its dev server, so it must never register
    // itself to start with Windows (it used to, which left a startup entry that opened a
    // window that couldn't load).
    if cfg!(debug_assertions) {
        return;
    }
    let autolaunch = app.autolaunch();
    // When on, always write the entry again: it stores this program's path, which changes
    // when Preku is installed, updated or moved. "Already enabled" is not enough.
    let result = if enabled {
        autolaunch.enable()
    } else {
        match autolaunch.is_enabled() {
            Ok(true) => autolaunch.disable(),
            Ok(false) => Ok(()),
            Err(err) => Err(err),
        }
    };
    if let Err(err) = result {
        tracing::warn!(error = %err, enabled, "failed to update launch at login");
    }
}

/// Opens the settings window, or brings it to the front if it is already open.
pub fn open_settings(app: &AppHandle) -> Result<()> {
    let to_err = |e: tauri::Error| PrekuError::Window(e.to_string());
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        window.unminimize().map_err(to_err)?;
        window.show().map_err(to_err)?;
        return window.set_focus().map_err(to_err);
    }
    WebviewWindowBuilder::new(app, SETTINGS_WINDOW, WebviewUrl::App("index.html".into()))
        .title("Preku Settings")
        .inner_size(920.0, 680.0)
        .min_inner_size(760.0, 540.0)
        .center()
        .build()
        .map_err(to_err)?;
    Ok(())
}
