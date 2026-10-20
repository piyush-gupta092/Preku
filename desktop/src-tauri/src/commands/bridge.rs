//! Phone bridge commands (Phase 6): pairing and the paired phone.

use std::sync::Arc;

use tauri::State;

use crate::{
    foundation::errors::IpcError,
    modules::bridge::{BridgeModule, BridgeState},
};

type Module<'a> = State<'a, Arc<BridgeModule>>;
type Res<T> = Result<T, IpcError>;

#[tauri::command]
pub async fn bridge_get(module: Module<'_>) -> Res<BridgeState> {
    Ok(module.state().await?)
}

/// Opens a 2-minute pairing window; the state carries the QR code.
#[tauri::command]
pub async fn bridge_pair_start(module: Module<'_>) -> Res<BridgeState> {
    Ok(module.start_pairing().await?)
}

#[tauri::command]
pub async fn bridge_pair_cancel(module: Module<'_>) -> Res<BridgeState> {
    Ok(module.cancel_pairing().await?)
}

#[tauri::command]
pub async fn bridge_unpair(module: Module<'_>, id: String) -> Res<BridgeState> {
    Ok(module.unpair(&id).await?)
}

/// Sends files (paths from a drop on the island or Settings) to the connected phone.
#[tauri::command]
pub async fn bridge_send_files(module: Module<'_>, paths: Vec<String>) -> Res<BridgeState> {
    let paths: Vec<std::path::PathBuf> = paths.into_iter().map(Into::into).collect();
    Ok(module.send_files(&paths).await?)
}

/// Sends text or a link to the connected phone.
#[tauri::command]
pub async fn bridge_send_text(module: Module<'_>, text: String) -> Res<BridgeState> {
    Ok(module.send_text(&text).await?)
}

#[tauri::command]
pub async fn bridge_clear_texts(module: Module<'_>) -> Res<BridgeState> {
    Ok(module.clear_texts().await?)
}

/// Opens a shared text in the browser, only if it is a plain http(s) link (checked in Rust;
/// the address goes to the system's URL handler as one argument, never through a shell).
#[tauri::command]
pub async fn bridge_open_link(module: Module<'_>, id: String) -> Res<()> {
    let url = module.text_link(&id)?;
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("rundll32.exe");
        command.args(["url.dll,FileProtocolHandler", &url]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(&url);
        command
    };
    command
        .spawn()
        .map_err(crate::foundation::errors::PrekuError::from)?;
    Ok(())
}

/// Opens the system file picker, then sends what the user chose.
#[tauri::command]
pub async fn bridge_pick_and_send(app: tauri::AppHandle, module: Module<'_>) -> Res<BridgeState> {
    use tauri_plugin_dialog::DialogExt;
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_files())
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
    let paths: Vec<std::path::PathBuf> = picked
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .collect();
    if paths.is_empty() {
        return Ok(module.state().await?);
    }
    Ok(module.send_files(&paths).await?)
}

#[tauri::command]
pub async fn bridge_cancel_transfer(module: Module<'_>, id: String) -> Res<BridgeState> {
    Ok(module.cancel_transfer(&id).await?)
}

/// Shows a received file in Explorer (or the inbox folder with `id = None`).
#[tauri::command]
pub async fn bridge_reveal(module: Module<'_>, id: Option<String>) -> Res<()> {
    let target = match id {
        Some(id) => module.received_path(&id)?,
        None => module.inbox()?,
    };
    reveal(&target).map_err(crate::foundation::errors::PrekuError::from)?;
    Ok(())
}

fn reveal(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let arg = if path.is_file() {
            format!("/select,\"{}\"", path.display())
        } else {
            format!("\"{}\"", path.display())
        };
        std::process::Command::new("explorer.exe")
            .raw_arg(arg)
            .spawn()?;
    }
    #[cfg(not(windows))]
    {
        let dir = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        std::process::Command::new("xdg-open").arg(dir).spawn()?;
    }
    Ok(())
}
