//! OS integrations for the productivity module (Rules.md §7: no `cfg` in business logic).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{clipboard_change_marker, clipboard_is_sensitive, media_command, now_playing};
#[cfg(windows)]
pub use windows::{clipboard_change_marker, clipboard_is_sensitive, media_command, now_playing};
