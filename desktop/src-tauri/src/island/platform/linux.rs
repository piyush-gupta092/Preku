//! Linux (GTK). Not yet verified on a real Linux desktop — see Memory.md.
//!
//! Preku runs through X11 (native, or XWayland on Wayland sessions): plain Wayland does not
//! let a normal window place itself at the top edge or stay on top. A native Wayland
//! layer-shell surface (gtk-layer-shell) is planned as a follow-up.

use gtk::prelude::*;
use tauri::WebviewWindow;

use crate::foundation::errors::{PrekuError, Result};

/// Must run before Tauri/GTK starts. Respects an explicit `GDK_BACKEND` from the user.
pub fn configure_backend() {
    if std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
}

fn gtk_window(window: &WebviewWindow) -> Result<gtk::ApplicationWindow> {
    window
        .gtk_window()
        .map_err(|e| PrekuError::Window(e.to_string()))
}

/// `activatable = false`: the window manager never gives the island keyboard focus.
pub fn set_activatable(window: &WebviewWindow, activatable: bool) -> Result<()> {
    gtk_window(window)?.set_accept_focus(activatable);
    Ok(())
}

pub fn show_inactive(window: &WebviewWindow) -> Result<()> {
    gtk_window(window)?.set_focus_on_map(false);
    window.show().map_err(|e| PrekuError::Window(e.to_string()))
}

/// X11 window managers hand focus back to the previous window on their own.
/// Not detected on Linux yet: hovering works everywhere, as before (see Memory.md).
pub fn desktop_in_front(_window: &WebviewWindow) -> bool {
    true
}

/// Not available without a global input hook; Linux relies on focus loss instead.
pub fn mouse_pressed() -> bool {
    false
}

pub fn cursor_position() -> Option<(i32, i32)> {
    None
}

pub fn remember_foreground(_window: &WebviewWindow) -> Result<()> {
    Ok(())
}

pub fn restore_foreground(_window: &WebviewWindow) -> Result<()> {
    Ok(())
}
