use serde::Serialize;
use ts_rs::TS;

/// Every fallible function in the backend returns this error (Rules.md §3).
#[derive(Debug, thiserror::Error)]
pub enum PrekuError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("migration {version} failed: {source}")]
    Migration {
        version: u32,
        #[source]
        source: rusqlite::Error,
    },
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("secret store error: {0}")]
    Secret(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("window error: {0}")]
    Window(String),
    /// Phone bridge: TLS, sockets, certificates (details are logged, not shown).
    #[error("bridge error: {0}")]
    Bridge(String),
}

pub type Result<T> = std::result::Result<T, PrekuError>;

impl PrekuError {
    /// Stable, machine-readable code sent to the frontend.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(_) | Self::Migration { .. } => "database",
            Self::Io(_) => "io",
            Self::Json(_) => "serialization",
            Self::Secret(_) => "secret_store",
            Self::InvalidInput(_) => "invalid_input",
            Self::Window(_) => "window",
            Self::Bridge(_) => "bridge",
        }
    }

    /// User-facing message. Never contains paths, SQL or other internals.
    fn public_message(&self) -> String {
        match self {
            Self::Database(_) | Self::Migration { .. } => "Storage is unavailable".into(),
            Self::Io(_) => "A file operation failed".into(),
            Self::Json(_) => "Data could not be read".into(),
            Self::Secret(_) => "The system keychain is unavailable".into(),
            Self::InvalidInput(message) => message.clone(),
            Self::Window(_) => "The Preku window could not be updated".into(),
            Self::Bridge(_) => "The phone connection had a problem".into(),
        }
    }
}

/// Error shape returned by every Tauri command. Never contains internal paths or stack traces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct IpcError {
    pub code: String,
    pub message: String,
}

impl From<PrekuError> for IpcError {
    fn from(err: PrekuError) -> Self {
        tracing::error!(error = %err, "command failed");
        Self {
            code: err.code().to_owned(),
            message: err.public_message(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_error_hides_internal_details() {
        let err = PrekuError::Io(std::io::Error::other(r"C:\Users\secret\file.db locked"));
        let ipc = IpcError::from(err);
        assert_eq!(ipc.code, "io");
        assert!(!ipc.message.contains("secret"));
    }

    #[test]
    fn invalid_input_message_is_passed_through() {
        let ipc = IpcError::from(PrekuError::InvalidInput("Key is empty".into()));
        assert_eq!(
            ipc,
            IpcError {
                code: "invalid_input".into(),
                message: "Key is empty".into()
            }
        );
    }
}
