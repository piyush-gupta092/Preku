//! Quiet mode commands (usability U4): pause routine prompts for a while, resume, and choose
//! whether they hold during focus.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::foundation::{
    errors::IpcError,
    quiet::{Quiet, QuietState, EVENT},
    storage::{now_millis, Storage},
};

type Res<T> = Result<T, IpcError>;

fn changed(app: &AppHandle, quiet: &Quiet, storage: &Storage) -> Res<QuietState> {
    let now = now_millis();
    quiet.save(storage, now)?;
    let state = quiet.state(now);
    let _ = app.emit(EVENT, state);
    Ok(state)
}

/// Looks every few seconds at what is in front (a game, a presentation, a full-screen app),
/// whichever modules are on, and tells the UI when "busy" changes.
pub fn spawn_screen_watcher(app: AppHandle, quiet: Arc<Quiet>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let screen =
                tauri::async_runtime::spawn_blocking(crate::modules::wellbeing::platform::screen)
                    .await
                    .unwrap_or_default();
            if quiet.set_screen(screen) {
                let _ = app.emit(EVENT, quiet.state(now_millis()));
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

#[tauri::command]
pub async fn quiet_hold_fullscreen_apps(
    app: AppHandle,
    quiet: State<'_, Arc<Quiet>>,
    storage: State<'_, Arc<Storage>>,
    hold: bool,
) -> Res<QuietState> {
    quiet.set_hold_fullscreen_apps(hold);
    changed(&app, &quiet, &storage)
}

#[tauri::command]
pub async fn quiet_get(quiet: State<'_, Arc<Quiet>>) -> Res<QuietState> {
    Ok(quiet.state(now_millis()))
}

/// Pauses routine prompts until `until` (unix ms), or resumes them with `null`.
#[tauri::command]
pub async fn quiet_pause(
    app: AppHandle,
    quiet: State<'_, Arc<Quiet>>,
    storage: State<'_, Arc<Storage>>,
    until: Option<i64>,
) -> Res<QuietState> {
    quiet.pause_until(until, now_millis());
    changed(&app, &quiet, &storage)
}

#[tauri::command]
pub async fn quiet_hold_in_focus(
    app: AppHandle,
    quiet: State<'_, Arc<Quiet>>,
    storage: State<'_, Arc<Storage>>,
    hold: bool,
) -> Res<QuietState> {
    quiet.set_hold_in_focus(hold);
    changed(&app, &quiet, &storage)
}
