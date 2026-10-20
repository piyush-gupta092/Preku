use tokio::net::windows::named_pipe::ServerOptions;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

use super::read_lines;
use crate::{foundation::errors::Result, modules::monitor::metrics::BatteryInfo};

/// `BatteryFlag` bit meaning "no system battery" (desktop PCs).
const NO_BATTERY: u8 = 128;
const UNKNOWN_PERCENT: u8 = 255;

pub fn battery() -> Option<BatteryInfo> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid, writable struct for the duration of the call.
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    if status.BatteryFlag & NO_BATTERY != 0 || status.BatteryLifePercent == UNKNOWN_PERCENT {
        return None;
    }
    let charging = status.ACLineStatus == 1;
    Some(BatteryInfo {
        percent: status.BatteryLifePercent.min(100),
        charging,
        // u32::MAX means "unknown" (always while plugged in, and for the first minute or so
        // after unplugging).
        secs_left: (!charging && status.BatteryLifeTime != u32::MAX)
            .then_some(status.BatteryLifeTime),
    })
}

/// Per-user pipe name; the CLI (`cli/src/main.rs`) builds the same one.
fn pipe_name() -> String {
    let user: String = std::env::var("USERNAME")
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    format!(r"\\.\pipe\preku-watch-{user}")
}

/// Accepts CLI connections forever. The default pipe security lets only the creating user
/// (and admins/SYSTEM) write, and tokio rejects remote clients by default.
pub async fn serve_watch<F>(handler: F) -> Result<()>
where
    F: Fn(String) + Clone + Send + Sync + 'static,
{
    let name = pipe_name();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)?;
    loop {
        server.connect().await?;
        let client = server;
        server = ServerOptions::new().create(&name)?;
        let handler = handler.clone();
        tauri::async_runtime::spawn(async move { read_lines(client, handler).await });
    }
}
