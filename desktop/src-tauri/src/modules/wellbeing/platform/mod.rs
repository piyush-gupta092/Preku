//! OS integrations for the wellbeing module (Rules.md §7).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::sample;
#[cfg(windows)]
pub use windows::{sample, screen};

/// X11 only tells us "full screen", not what kind.
#[cfg(target_os = "linux")]
pub fn screen() -> crate::foundation::quiet::Screen {
    if sample().fullscreen {
        crate::foundation::quiet::Screen::FullscreenApp
    } else {
        crate::foundation::quiet::Screen::Normal
    }
}

/// One look at what the user is doing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Activity {
    /// Executable name of the foreground app (e.g. `Code.exe`), if known.
    pub app: Option<String>,
    /// Milliseconds since the last keyboard/mouse input; `None` if unknown.
    pub idle_ms: Option<u64>,
    /// A fullscreen app, game or presentation is running → Do-Not-Disturb.
    pub fullscreen: bool,
    /// False where per-app tracking isn't possible (native Wayland sessions).
    pub supported: bool,
}
