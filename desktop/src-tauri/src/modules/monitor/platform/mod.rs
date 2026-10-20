//! OS integrations for the monitor module (Rules.md §7).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{battery, serve_watch};
#[cfg(windows)]
pub use windows::{battery, serve_watch};

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

/// Bytes accepted per CLI connection (start + finish messages fit easily).
const MAX_CONNECTION_BYTES: u64 = 64 * 1024;

/// Reads newline-delimited messages from one CLI connection until it closes.
async fn read_lines<R, F>(stream: R, handler: F)
where
    R: AsyncRead + Unpin,
    F: Fn(String),
{
    let mut lines = BufReader::new(stream.take(MAX_CONNECTION_BYTES)).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        handler(line);
    }
}
