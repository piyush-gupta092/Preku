use std::path::Path;

use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use super::errors::{PrekuError, Result};

/// Keeps the background log writer alive; dropping it flushes and stops file logging.
pub struct LogGuard {
    _guard: WorkerGuard,
}

/// Logs to `<log_dir>/preku.<date>.log` (7 days kept) and, in debug builds, to stderr.
/// Level is `info` unless overridden with the `PREKU_LOG` env var (e.g. `PREKU_LOG=debug`).
pub fn init(log_dir: &Path) -> Result<LogGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("preku")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir)
        .map_err(|e| PrekuError::Io(std::io::Error::other(e)))?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_env("PREKU_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = fmt::layer().with_writer(writer).with_ansi(false);
    let stderr_layer = cfg!(debug_assertions).then(|| fmt::layer().with_writer(std::io::stderr));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .map_err(|e| PrekuError::Io(std::io::Error::other(e)))?;

    Ok(LogGuard { _guard: guard })
}
