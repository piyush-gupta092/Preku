use tauri::{AppHandle, State};

use crate::{
    foundation::errors::IpcError,
    island::controller::{IslandController, IslandState},
};

#[tauri::command]
pub fn island_get_state(controller: State<'_, IslandController>) -> IslandState {
    controller.state()
}

/// Whether the Windows desktop is in front (initial value; then `island:desktop` events).
#[tauri::command]
pub fn island_on_desktop(controller: State<'_, IslandController>) -> bool {
    controller.on_desktop()
}

/// The expanded panel's height for the current view (logical px, clamped).
#[tauri::command]
pub fn island_set_panel_height(
    app: AppHandle,
    controller: State<'_, IslandController>,
    height: f64,
) -> Result<(), IpcError> {
    Ok(controller.set_panel_height(&app, height)?)
}

#[tauri::command]
pub fn island_set_state(
    app: AppHandle,
    controller: State<'_, IslandController>,
    state: IslandState,
) -> Result<(), IpcError> {
    Ok(controller.set_state(&app, state)?)
}
