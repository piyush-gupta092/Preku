//! Now playing + media keys (PRD A4) — Windows SMTC / Linux MPRIS via `platform`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NowPlaying {
    pub title: String,
    pub artist: Option<String>,
    /// Friendly player name, e.g. "Spotify".
    pub app: Option<String>,
    pub playing: bool,
    pub can_next: bool,
    pub can_previous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MediaAction {
    PlayPause,
    Next,
    Previous,
}

pub use crate::foundation::apps::friendly_app_name;
