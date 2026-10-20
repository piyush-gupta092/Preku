use std::sync::{Arc, Mutex};

use super::{preferences::Preferences, storage::Storage};

/// Shared application state, registered with `app.manage` and read via `tauri::State`.
pub struct AppState {
    pub storage: Arc<Storage>,
    /// In-memory copy of what is saved; change it only through `actions::update_preferences`.
    pub preferences: Mutex<Preferences>,
}
