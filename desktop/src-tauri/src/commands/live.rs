use tauri::State;

use crate::{
    live::LiveItem,
    pip::mood_engine::PipMood,
    runtime::{LiveState, MoodState},
};

/// Initial snapshot; afterwards the frontend listens to `live:items`.
#[tauri::command]
pub fn live_get_items(live: State<'_, LiveState>) -> Vec<LiveItem> {
    live.items()
}

/// Initial snapshot; afterwards the frontend listens to `pip:mood`.
#[tauri::command]
pub fn pip_get_mood(mood: State<'_, MoodState>) -> PipMood {
    mood.current()
}
