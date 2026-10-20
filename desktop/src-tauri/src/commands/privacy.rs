//! Data-control commands (usability U5): capture pause and retention, export, delete.

use std::{path::PathBuf, sync::Arc, time::Duration};

use tauri::{AppHandle, Emitter, State};

use crate::{
    actions,
    foundation::{
        errors::{IpcError, PrekuError},
        privacy::{self, Privacy, PrivacySettings, EVENT},
        storage::{now_millis, Storage},
    },
    modules::ModuleRegistry,
};

type Res<T> = Result<T, IpcError>;

/// Where the clipboard module keeps image files.
pub struct ImageDir(pub PathBuf);

fn prune_now(storage: &Storage, privacy: &Privacy, image_dir: &std::path::Path) {
    let settings = privacy.settings();
    let today = chrono::Local::now().date_naive();
    match storage.with_conn(|conn| privacy::prune(conn, &settings, image_dir, now_millis(), today))
    {
        Ok(0) => {}
        Ok(removed) => tracing::info!(removed, "removed records past the keep-for limit"),
        Err(err) => tracing::warn!(error = %err, "failed to remove old records"),
    }
}

/// Applies the keep-for limits at start and a few times a day.
pub fn spawn_pruner(storage: Arc<Storage>, privacy: Arc<Privacy>, image_dir: PathBuf) {
    tauri::async_runtime::spawn(async move {
        loop {
            prune_now(&storage, &privacy, &image_dir);
            tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

#[tauri::command]
pub async fn privacy_get(privacy: State<'_, Arc<Privacy>>) -> Res<PrivacySettings> {
    Ok(privacy.settings())
}

#[tauri::command]
pub async fn privacy_set(
    app: AppHandle,
    privacy: State<'_, Arc<Privacy>>,
    storage: State<'_, Arc<Storage>>,
    image_dir: State<'_, ImageDir>,
    settings: PrivacySettings,
) -> Res<PrivacySettings> {
    privacy.set(&storage, settings)?;
    prune_now(&storage, &privacy, &image_dir.0);
    let _ = app.emit(EVENT, settings);
    Ok(settings)
}

/// Asks where to save, then writes everything Preku recorded as one readable JSON file.
/// Returns the file's path, or `null` if the user cancelled.
#[tauri::command]
pub async fn privacy_export(
    app: AppHandle,
    storage: State<'_, Arc<Storage>>,
) -> Res<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let name = format!(
        "preku-data-{}.json",
        chrono::Local::now().format("%Y-%m-%d")
    );
    let picker = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_file_name(name)
            .add_filter("JSON", &["json"])
            .blocking_save_file()
    })
    .await
    .ok()
    .flatten()
    .and_then(|file| file.into_path().ok());
    let Some(path) = picked else { return Ok(None) };
    let data = storage.with_conn(|conn| privacy::export(conn, now_millis()))?;
    let text = serde_json::to_string_pretty(&data).map_err(PrekuError::from)?;
    tokio::fs::write(&path, text)
        .await
        .map_err(PrekuError::from)?;
    Ok(Some(path.display().to_string()))
}

/// Deletes everything Preku recorded about the user's activity. The UI confirms first and
/// passes `confirmed`; settings, paired phones and saved sign-ins are kept.
#[tauri::command]
pub async fn privacy_delete_all(
    app: AppHandle,
    storage: State<'_, Arc<Storage>>,
    image_dir: State<'_, ImageDir>,
    registry: State<'_, ModuleRegistry>,
    confirmed: bool,
) -> Res<()> {
    if !confirmed {
        return Err(PrekuError::InvalidInput("Deleting needs a confirmation".into()).into());
    }
    // Stop the modules first so nothing writes while the tables are emptied.
    let enabled = actions::preferences(&app).modules;
    registry.apply(&enabled, true);
    let result = storage.with_conn(|conn| privacy::delete_all(conn, &image_dir.0));
    registry.apply(&enabled, actions::preferences(&app).paused);
    result?;
    tracing::info!("the user deleted their recorded data");
    Ok(())
}
