//! Productivity commands. All `async` so they run off the main thread (media calls block on
//! OS APIs; DB calls are short but shouldn't stall window events either).

use std::sync::Arc;

use tauri::State;

use crate::{
    foundation::errors::{IpcError, PrekuError},
    modules::productivity::{
        clipboard::ClipItem,
        media::{MediaAction, NowPlaying},
        tasks::Task,
        timer::TimerSnapshot,
        ProductivityModule,
    },
};

type Module<'a> = State<'a, Arc<ProductivityModule>>;
type Res<T> = Result<T, IpcError>;

#[tauri::command]
pub async fn timer_get(module: Module<'_>) -> Res<TimerSnapshot> {
    Ok(module.timer_snapshot())
}

#[tauri::command]
pub async fn timer_start(
    module: Module<'_>,
    label: String,
    minutes: u32,
    task_id: Option<String>,
) -> Res<TimerSnapshot> {
    Ok(module.timer_start(&label, minutes, task_id.as_deref())?)
}

#[tauri::command]
pub async fn timer_dismiss_finished(module: Module<'_>) -> Res<TimerSnapshot> {
    Ok(module.timer_dismiss_finished()?)
}

#[tauri::command]
pub async fn timer_pause(module: Module<'_>) -> Res<TimerSnapshot> {
    Ok(module.timer_pause()?)
}

#[tauri::command]
pub async fn timer_resume(module: Module<'_>) -> Res<TimerSnapshot> {
    Ok(module.timer_resume()?)
}

#[tauri::command]
pub async fn timer_stop(module: Module<'_>) -> Res<TimerSnapshot> {
    Ok(module.timer_stop()?)
}

#[tauri::command]
pub async fn tasks_list(module: Module<'_>) -> Res<Vec<Task>> {
    Ok(module.tasks_list()?)
}

#[tauri::command]
pub async fn task_add(module: Module<'_>, title: String) -> Res<Vec<Task>> {
    Ok(module.task_add(&title)?)
}

#[tauri::command]
pub async fn task_rename(module: Module<'_>, id: String, title: String) -> Res<Vec<Task>> {
    Ok(module.task_rename(&id, &title)?)
}

#[tauri::command]
pub async fn task_set_done(module: Module<'_>, id: String, done: bool) -> Res<Vec<Task>> {
    Ok(module.task_set_done(&id, done)?)
}

#[tauri::command]
pub async fn task_set_priority(module: Module<'_>, id: String, on: bool) -> Res<Vec<Task>> {
    Ok(module.task_set_priority(&id, on)?)
}

#[tauri::command]
pub async fn task_set_due(module: Module<'_>, id: String, due_at: Option<i64>) -> Res<Vec<Task>> {
    Ok(module.task_set_due(&id, due_at)?)
}

#[tauri::command]
pub async fn today_summary(
    module: Module<'_>,
) -> Res<crate::modules::productivity::tasks::TodaySummary> {
    Ok(module.today_summary()?)
}

#[tauri::command]
pub async fn task_delete(module: Module<'_>, id: String) -> Res<Vec<Task>> {
    Ok(module.task_delete(&id)?)
}

#[tauri::command]
pub async fn tasks_reorder(module: Module<'_>, ids: Vec<String>) -> Res<Vec<Task>> {
    Ok(module.tasks_reorder(&ids)?)
}

#[tauri::command]
pub async fn clipboard_list(module: Module<'_>) -> Res<Vec<ClipItem>> {
    Ok(module.clipboard_list()?)
}

#[tauri::command]
pub async fn clipboard_copy(module: Module<'_>, id: String) -> Res<()> {
    Ok(module.clipboard_copy(&id)?)
}

#[tauri::command]
pub async fn clipboard_set_pinned(
    module: Module<'_>,
    id: String,
    pinned: bool,
) -> Res<Vec<ClipItem>> {
    Ok(module.clipboard_set_pinned(&id, pinned)?)
}

#[tauri::command]
pub async fn clipboard_delete(module: Module<'_>, id: String) -> Res<Vec<ClipItem>> {
    Ok(module.clipboard_delete(&id)?)
}

#[tauri::command]
pub async fn clipboard_clear(module: Module<'_>) -> Res<Vec<ClipItem>> {
    Ok(module.clipboard_clear()?)
}

#[tauri::command]
pub async fn clipboard_image(module: Module<'_>, id: String) -> Res<String> {
    Ok(module.clipboard_image(&id)?)
}

fn join_error(err: tauri::Error) -> IpcError {
    IpcError::from(PrekuError::Window(err.to_string()))
}

#[tauri::command]
pub async fn media_get(module: Module<'_>) -> Res<Option<NowPlaying>> {
    let module = Arc::clone(&module);
    tauri::async_runtime::spawn_blocking(move || module.media_now())
        .await
        .map_err(join_error)?
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn media_command(module: Module<'_>, action: MediaAction) -> Res<Option<NowPlaying>> {
    let module = Arc::clone(&module);
    tauri::async_runtime::spawn_blocking(move || module.media_command(action))
        .await
        .map_err(join_error)?
        .map_err(IpcError::from)
}

// ---------------------------------------------------------------- calendar (optional)

use crate::modules::productivity::calendar::CalendarState;

#[tauri::command]
pub async fn calendar_get(module: Module<'_>) -> Res<CalendarState> {
    Ok(module.calendar()?.state())
}

/// Adds a calendar link (an `.ics` feed, e.g. Google's or Outlook's private address).
#[tauri::command]
pub async fn calendar_add_link(
    module: Module<'_>,
    name: String,
    link: String,
) -> Res<CalendarState> {
    Ok(module.calendar()?.add_link(&name, &link)?)
}

/// Lets the user pick a local `.ics` file and adds it. Returns the unchanged state if they cancel.
#[tauri::command]
pub async fn calendar_add_file(
    app: tauri::AppHandle,
    module: Module<'_>,
    name: String,
) -> Res<CalendarState> {
    use tauri_plugin_dialog::DialogExt;
    let calendar = module.calendar()?;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Calendar", &["ics"])
            .blocking_pick_file()
    })
    .await
    .ok()
    .flatten()
    .and_then(|file| file.into_path().ok());
    match picked {
        Some(path) => Ok(calendar.add_file(&name, &path)?),
        None => Ok(calendar.state()),
    }
}

#[tauri::command]
pub async fn calendar_remove(module: Module<'_>, id: String) -> Res<CalendarState> {
    Ok(module.calendar()?.remove(&id)?)
}

#[tauri::command]
pub async fn calendar_set_enabled(
    module: Module<'_>,
    id: String,
    enabled: bool,
) -> Res<CalendarState> {
    Ok(module.calendar()?.set_enabled(&id, enabled)?)
}

#[tauri::command]
pub async fn calendar_set_options(
    module: Module<'_>,
    remind_min: u32,
    hide_titles: bool,
) -> Res<CalendarState> {
    Ok(module.calendar()?.set_options(remind_min, hide_titles)?)
}

#[tauri::command]
pub async fn calendar_refresh(module: Module<'_>) -> Res<CalendarState> {
    Ok(module.calendar()?.refresh_now())
}
