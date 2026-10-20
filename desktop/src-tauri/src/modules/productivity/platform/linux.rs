//! Linux (unverified on real hardware — see Memory.md).

use mpris::{PlaybackStatus, PlayerFinder};

use crate::{
    foundation::errors::{PrekuError, Result},
    modules::productivity::media::{friendly_app_name, MediaAction, NowPlaying},
};

/// No cheap change counter on Linux; the watcher compares content fingerprints instead.
pub fn clipboard_change_marker() -> Option<u64> {
    None
}

/// `arboard` can't read the `x-kde-passwordManagerHint` MIME type, so private copies can't be
/// detected yet (Memory.md: known issue).
pub fn clipboard_is_sensitive() -> bool {
    false
}

pub fn now_playing() -> Option<NowPlaying> {
    let player = PlayerFinder::new().ok()?.find_active().ok()?;
    let metadata = player.get_metadata().ok()?;
    let title = metadata.title()?.trim().to_owned();
    if title.is_empty() {
        return None;
    }
    Some(NowPlaying {
        title,
        artist: metadata
            .artists()
            .and_then(|artists| artists.first().map(|a| a.to_string())),
        app: friendly_app_name(player.identity()),
        playing: player.get_playback_status().ok()? == PlaybackStatus::Playing,
        can_next: player.can_go_next().unwrap_or(false),
        can_previous: player.can_go_previous().unwrap_or(false),
    })
}

pub fn media_command(action: MediaAction) -> Result<()> {
    let player = PlayerFinder::new()
        .map_err(|e| PrekuError::Window(e.to_string()))?
        .find_active()
        .map_err(|_| PrekuError::InvalidInput("Nothing is playing".into()))?;
    let result = match action {
        MediaAction::PlayPause => player.play_pause(),
        MediaAction::Next => player.next(),
        MediaAction::Previous => player.previous(),
    };
    result.map_err(|e| PrekuError::Window(e.to_string()))
}
