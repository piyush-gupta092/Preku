//! The island: a frameless, always-on-top window anchored to the top edge of the screen
//! (Architecture.md §3.1).

pub mod controller;
mod geometry;
pub(crate) mod platform;

/// Label of the island window in `tauri.conf.json`.
pub const ISLAND_WINDOW: &str = "island";
