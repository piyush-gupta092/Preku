use windows::{
    core::{HSTRING, PCWSTR},
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    },
    Win32::{
        Foundation::HGLOBAL,
        System::{
            DataExchange::{
                CloseClipboard, GetClipboardData, GetClipboardSequenceNumber,
                IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
            },
            Memory::{GlobalLock, GlobalUnlock},
        },
    },
};

use crate::{
    foundation::errors::{PrekuError, Result},
    modules::productivity::media::{friendly_app_name, MediaAction, NowPlaying},
};

/// Changes whenever anything is copied; cheap to poll.
pub fn clipboard_change_marker() -> Option<u64> {
    // SAFETY: no preconditions.
    Some(u64::from(unsafe { GetClipboardSequenceNumber() }))
}

fn format_id(name: &str) -> u32 {
    let name = HSTRING::from(name);
    // SAFETY: `name` is a valid NUL-terminated wide string for the duration of the call.
    unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) }
}

fn has_format(id: u32) -> bool {
    // SAFETY: no preconditions.
    id != 0 && unsafe { IsClipboardFormatAvailable(id) }.is_ok()
}

/// Password managers mark copies as private with these well-known formats:
/// - `ExcludeClipboardContentFromMonitorProcessing` (any value) — "don't record this"
/// - `CanIncludeInClipboardHistory` with value 0 — Windows' own history honours it too
/// - `Clipboard Viewer Ignore` — older convention
pub fn clipboard_is_sensitive() -> bool {
    if has_format(format_id("ExcludeClipboardContentFromMonitorProcessing"))
        || has_format(format_id("Clipboard Viewer Ignore"))
    {
        return true;
    }
    let can_include = format_id("CanIncludeInClipboardHistory");
    has_format(can_include) && read_dword(can_include) == Some(0)
}

fn read_dword(format: u32) -> Option<u32> {
    // SAFETY: the clipboard is opened and closed on this thread; the handle is only used
    // while open, and the lock is released before closing.
    unsafe {
        OpenClipboard(None).ok()?;
        let value = GetClipboardData(format).ok().and_then(|handle| {
            let global = HGLOBAL(handle.0);
            let ptr = GlobalLock(global) as *const u32;
            if ptr.is_null() {
                return None;
            }
            let value = ptr.read_unaligned();
            let _ = GlobalUnlock(global);
            Some(value)
        });
        let _ = CloseClipboard();
        value
    }
}

/// The session to show and control: one that is playing if there is any (a paused player in
/// front shouldn't hide the music behind it), otherwise the one Windows calls current.
fn current_session() -> Option<Session> {
    let manager = SessionManager::RequestAsync().ok()?.join().ok()?;
    let playing = manager.GetSessions().ok().and_then(|sessions| {
        sessions.into_iter().find(|session| {
            session
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .is_ok_and(|status| status == PlaybackStatus::Playing)
        })
    });
    playing.or_else(|| manager.GetCurrentSession().ok())
}

pub fn now_playing() -> Option<NowPlaying> {
    let session = current_session()?;
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let title = props.Title().ok()?.to_string();
    if title.trim().is_empty() {
        return None;
    }
    let artist = props
        .Artist()
        .ok()
        .map(|a| a.to_string())
        .filter(|a| !a.trim().is_empty());
    let info = session.GetPlaybackInfo().ok()?;
    let controls = info.Controls().ok()?;
    Some(NowPlaying {
        title,
        artist,
        app: session
            .SourceAppUserModelId()
            .ok()
            .and_then(|id| friendly_app_name(&id.to_string())),
        playing: info.PlaybackStatus().ok()? == PlaybackStatus::Playing,
        can_next: controls.IsNextEnabled().unwrap_or(false),
        can_previous: controls.IsPreviousEnabled().unwrap_or(false),
    })
}

pub fn media_command(action: MediaAction) -> Result<()> {
    let to_err = |e: windows::core::Error| PrekuError::Window(e.to_string());
    let session =
        current_session().ok_or_else(|| PrekuError::InvalidInput("Nothing is playing".into()))?;
    let operation = match action {
        MediaAction::PlayPause => session.TryTogglePlayPauseAsync(),
        MediaAction::Next => session.TrySkipNextAsync(),
        MediaAction::Previous => session.TrySkipPreviousAsync(),
    };
    operation.map_err(to_err)?.join().map_err(to_err)?;
    Ok(())
}

#[cfg(test)]
mod diag {
    use super::*;

    /// `cargo test --lib diag -- --ignored --nocapture`: what Windows reports right now.
    #[test]
    #[ignore = "prints this PC's media sessions"]
    fn print_media_sessions() {
        let manager = SessionManager::RequestAsync().unwrap().join().unwrap();
        let sessions = manager.GetSessions().unwrap();
        println!("sessions: {}", sessions.Size().unwrap());
        for session in sessions {
            let status = session.GetPlaybackInfo().and_then(|i| i.PlaybackStatus());
            let app = session.SourceAppUserModelId().map(|s| s.to_string());
            let has_title = session
                .TryGetMediaPropertiesAsync()
                .ok()
                .and_then(|op| op.join().ok())
                .and_then(|p| p.Title().ok())
                .is_some_and(|t| !t.to_string().trim().is_empty());
            println!("  app={app:?} status={status:?} has_title={has_title}");
        }
        println!(
            "now_playing playing = {:?}",
            now_playing().map(|n| n.playing)
        );
    }
}

#[cfg(test)]
mod private_copy {
    use windows::Win32::{
        Foundation::HANDLE,
        System::{
            DataExchange::{EmptyClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GMEM_MOVEABLE},
        },
    };

    use super::*;

    const CF_UNICODETEXT: u32 = 13;

    /// Puts `text` on the clipboard the way a password manager does: together with the
    /// "ExcludeClipboardContentFromMonitorProcessing" marker.
    fn copy_privately(text: &str) {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: the clipboard is opened and closed here; each block is allocated movable,
        // filled while locked, and then owned by the clipboard (not freed by us).
        unsafe {
            OpenClipboard(None).unwrap();
            EmptyClipboard().unwrap();
            let bytes = wide.len() * 2;
            let block = GlobalAlloc(GMEM_MOVEABLE, bytes).unwrap();
            let ptr = GlobalLock(block) as *mut u16;
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(block);
            SetClipboardData(CF_UNICODETEXT, Some(HANDLE(block.0))).unwrap();

            let marker = GlobalAlloc(GMEM_MOVEABLE, 4).unwrap();
            let ptr = GlobalLock(marker) as *mut u32;
            ptr.write(0);
            let _ = GlobalUnlock(marker);
            SetClipboardData(
                format_id("ExcludeClipboardContentFromMonitorProcessing"),
                Some(HANDLE(marker.0)),
            )
            .unwrap();
            let _ = CloseClipboard();
        }
    }

    /// `cargo test --lib private_copy -- --ignored --nocapture`
    /// Replaces the clipboard for two seconds (the previous text is put back). Run it while
    /// Preku is running to also check that its history skipped the copy: the test prints a
    /// marker to look for in the database.
    #[test]
    #[ignore = "writes to the real clipboard"]
    fn a_copy_marked_private_is_detected() {
        let mut clipboard = arboard::Clipboard::new().unwrap();
        let before = clipboard.get_text().ok();
        if before.is_none() && clipboard.get_image().is_ok() {
            println!("the clipboard holds an image: not touching it");
            return;
        }
        let secret = format!("preku-private-copy-test-{}", uuid::Uuid::new_v4());
        copy_privately(&secret);
        assert!(
            clipboard_is_sensitive(),
            "the private marker was not detected"
        );
        println!("MARKER {secret}");
        // Long enough for a running Preku to notice the change (it polls twice a second).
        std::thread::sleep(std::time::Duration::from_secs(2));
        match before {
            Some(text) => clipboard.set_text(text).unwrap(),
            None => clipboard.clear().unwrap(),
        }
        // An ordinary copy isn't flagged.
        assert!(!clipboard_is_sensitive());
    }
}
