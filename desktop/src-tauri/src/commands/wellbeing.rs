//! Wellbeing commands (async: they touch the database and must not stall window events).

use std::sync::Arc;

use tauri::State;

use crate::{
    foundation::errors::IpcError,
    modules::wellbeing::{
        reminders::ReminderKind, settings::WellbeingSettings, WellbeingModule, WellbeingState,
    },
};

type Module<'a> = State<'a, Arc<WellbeingModule>>;
type Res<T> = Result<T, IpcError>;

#[tauri::command]
pub async fn wellbeing_get(module: Module<'_>) -> Res<WellbeingState> {
    Ok(module.state()?)
}

#[tauri::command]
pub async fn water_add(module: Module<'_>, ml: Option<u32>) -> Res<WellbeingState> {
    Ok(module.add_water(ml)?)
}

#[tauri::command]
pub async fn water_undo(module: Module<'_>) -> Res<WellbeingState> {
    Ok(module.undo_water()?)
}

#[tauri::command]
pub async fn habit_add(
    module: Module<'_>,
    name: String,
    target: u32,
    days: Option<u8>,
) -> Res<WellbeingState> {
    Ok(module.add_habit(
        &name,
        target,
        days.unwrap_or(crate::modules::wellbeing::habits::ALL_DAYS),
    )?)
}

#[tauri::command]
pub async fn habit_update(
    module: Module<'_>,
    id: String,
    name: String,
    target: u32,
    days: u8,
) -> Res<WellbeingState> {
    Ok(module.update_habit(&id, &name, target, days)?)
}

#[tauri::command]
pub async fn habit_delete(module: Module<'_>, id: String) -> Res<WellbeingState> {
    Ok(module.delete_habit(&id)?)
}

#[tauri::command]
pub async fn habit_log(module: Module<'_>, id: String, delta: i32) -> Res<WellbeingState> {
    Ok(module.log_habit(&id, delta)?)
}

#[tauri::command]
pub async fn wellbeing_settings_set(
    module: Module<'_>,
    settings: WellbeingSettings,
) -> Res<WellbeingState> {
    Ok(module.set_settings(settings)?)
}

/// "Later" on a companion prompt: it comes back after the snooze time.
#[tauri::command]
pub async fn wellbeing_prompt_later(module: Module<'_>, id: String) -> Res<WellbeingState> {
    Ok(module.prompt_later(&id)?)
}

/// A break the user finished (`completed`) or skipped, from a prompt or on their own.
#[tauri::command]
pub async fn wellbeing_break_done(
    module: Module<'_>,
    kind: ReminderKind,
    completed: bool,
) -> Res<WellbeingState> {
    Ok(module.break_done(kind, completed)?)
}

/// The optional daily mood check-in.
#[tauri::command]
pub async fn mood_log(module: Module<'_>, mood: String) -> Res<WellbeingState> {
    Ok(module.log_mood(&mood)?)
}
