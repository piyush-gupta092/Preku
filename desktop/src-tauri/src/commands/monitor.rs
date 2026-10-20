//! Monitor commands. All `async`: several touch the network, the keychain or the process table.

use std::sync::Arc;

use tauri::State;

use crate::{
    foundation::errors::{IpcError, PrekuError},
    modules::monitor::{
        alerts::AlertRule,
        docker::ContainerAction,
        metrics::ProcessInfo,
        targets::{TargetKind, WatchTarget},
        MonitorModule, MonitorSnapshot, MonitorView,
    },
};

type Module<'a> = State<'a, Arc<MonitorModule>>;
type Res<T> = Result<T, IpcError>;

fn join_error(err: tauri::Error) -> IpcError {
    IpcError::from(PrekuError::Window(err.to_string()))
}

#[tauri::command]
pub async fn monitor_get(module: Module<'_>) -> Res<MonitorSnapshot> {
    Ok(module.snapshot()?)
}

#[tauri::command]
pub async fn monitor_set_view(module: Module<'_>, view: MonitorView) -> Res<()> {
    Ok(module.set_view(view)?)
}

#[tauri::command]
pub async fn process_kill(module: Module<'_>, pid: u32) -> Res<Vec<ProcessInfo>> {
    let module = Arc::clone(&module);
    tauri::async_runtime::spawn_blocking(move || module.kill(pid))
        .await
        .map_err(join_error)?
        .map_err(IpcError::from)
}

#[tauri::command]
pub async fn alert_repeat_set(module: Module<'_>, minutes: u32) -> Res<u32> {
    Ok(module.set_alert_repeat(minutes)?)
}

#[tauri::command]
pub async fn alert_rule_save(module: Module<'_>, rule: AlertRule) -> Res<Vec<AlertRule>> {
    Ok(module.save_rule(&rule)?)
}

#[tauri::command]
pub async fn alert_rule_delete(module: Module<'_>, id: String) -> Res<Vec<AlertRule>> {
    Ok(module.delete_rule(&id)?)
}

#[tauri::command]
pub async fn docker_action(module: Module<'_>, id: String, action: ContainerAction) -> Res<()> {
    Ok(module.docker_action(&id, action).await?)
}

#[tauri::command]
pub async fn github_token_set(module: Module<'_>, token: Option<String>) -> Res<()> {
    Ok(module.set_github_token(token.as_deref())?)
}

#[tauri::command]
pub async fn target_add(
    module: Module<'_>,
    kind: TargetKind,
    target: String,
) -> Res<Vec<WatchTarget>> {
    Ok(module.add_target(kind, &target)?)
}

#[tauri::command]
pub async fn target_delete(module: Module<'_>, id: String) -> Res<Vec<WatchTarget>> {
    Ok(module.delete_target(&id)?)
}
