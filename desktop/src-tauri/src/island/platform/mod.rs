//! OS-specific window tweaks. Business logic never uses `cfg` directly (Rules.md §7).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{
    configure_backend, cursor_position, desktop_in_front, mouse_pressed, remember_foreground,
    restore_foreground, set_activatable, show_inactive,
};
#[cfg(windows)]
pub use windows::{
    configure_backend, cursor_position, desktop_in_front, mouse_pressed, remember_foreground,
    restore_foreground, set_activatable, show_inactive,
};
