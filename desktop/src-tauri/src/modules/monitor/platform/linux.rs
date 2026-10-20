//! Linux (unverified on real hardware — see Memory.md).

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use tokio::net::UnixListener;

use super::read_lines;
use crate::{foundation::errors::Result, modules::monitor::metrics::BatteryInfo};

pub fn battery() -> Option<BatteryInfo> {
    let dir = fs::read_dir("/sys/class/power_supply")
        .ok()?
        .find_map(|entry| {
            let path = entry.ok()?.path();
            let kind = fs::read_to_string(path.join("type")).ok()?;
            (kind.trim() == "Battery").then_some(path)
        })?;
    let percent: u8 = fs::read_to_string(dir.join("capacity"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    let status = fs::read_to_string(dir.join("status")).unwrap_or_default();
    Some(BatteryInfo {
        percent: percent.min(100),
        charging: matches!(status.trim(), "Charging" | "Full"),
        secs_left: None,
    })
}

/// Per-user socket path; the CLI (`cli/src/main.rs`) builds the same one.
fn socket_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("preku-watch.sock"),
        None => {
            let user = std::env::var("USER").unwrap_or_default();
            std::env::temp_dir().join(format!("preku-watch-{user}.sock"))
        }
    }
}

pub async fn serve_watch<F>(handler: F) -> Result<()>
where
    F: Fn(String) + Clone + Send + Sync + 'static,
{
    let path = socket_path();
    // A stale socket from a crashed run would make bind fail.
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    loop {
        let (stream, _) = listener.accept().await?;
        let handler = handler.clone();
        tauri::async_runtime::spawn(async move { read_lines(stream, handler).await });
    }
}
