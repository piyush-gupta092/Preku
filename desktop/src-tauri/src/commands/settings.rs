use serde_json::Value;
use tauri::State;

use crate::foundation::{errors::IpcError, state::AppState};

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>, key: String) -> Result<Option<Value>, IpcError> {
    Ok(state.storage.settings_get(&key)?)
}

#[tauri::command]
pub fn settings_set(state: State<'_, AppState>, key: String, value: Value) -> Result<(), IpcError> {
    Ok(state.storage.settings_set(&key, &value)?)
}
