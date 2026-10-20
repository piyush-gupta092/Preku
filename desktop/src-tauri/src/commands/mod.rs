//! Thin `#[tauri::command]` handlers: validate → call foundation/module → return (Rules.md §7).

pub mod app;
pub mod bridge;
pub mod island;
pub mod live;
pub mod monitor;
pub mod preferences;
pub mod privacy;
pub mod productivity;
pub mod quiet;
pub mod settings;
pub mod skills;
pub mod weather;
pub mod wellbeing;
