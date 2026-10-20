use std::sync::atomic::{AtomicIsize, Ordering};

use tauri::WebviewWindow;
use windows::Win32::{
    Foundation::{HWND, POINT},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{
            GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW,
            SetForegroundWindow, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SW_SHOWNOACTIVATE,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        },
    },
};

use crate::foundation::errors::{PrekuError, Result};

/// The window the user was in before the panel opened, so focus can go back to it.
static PREVIOUS_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

fn hwnd(window: &WebviewWindow) -> Result<HWND> {
    let raw = window
        .hwnd()
        .map_err(|e| PrekuError::Window(e.to_string()))?;
    Ok(HWND(raw.0))
}

/// Nothing to configure before startup on Windows.
pub fn configure_backend() {}

/// True when the Windows desktop (no app) is in front — or Preku itself is. The desktop
/// is owned by Explorer's `Progman`, or a `WorkerW` window after Win+D / wallpaper changes.
pub fn desktop_in_front(window: &WebviewWindow) -> bool {
    // SAFETY: no preconditions.
    let front = unsafe { GetForegroundWindow() };
    if front.0.is_null() || hwnd(window).is_ok_and(|ours| ours == front) {
        return true;
    }
    let mut class = [0u16; 64];
    // SAFETY: `class` is a valid buffer; the call writes at most its length.
    let len = unsafe { GetClassNameW(front, &mut class) };
    let class = String::from_utf16_lossy(&class[..len.max(0) as usize]);
    matches!(class.as_str(), "Progman" | "WorkerW")
}

/// Whether any mouse button is held down right now (anywhere on screen).
pub fn mouse_pressed() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON].iter().any(|key| {
        // SAFETY: no preconditions. The high bit (negative value) means "currently down".
        let state = unsafe { GetAsyncKeyState(i32::from(key.0)) };
        state < 0
    })
}

/// Cursor position in physical screen pixels.
pub fn cursor_position() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: `point` is a valid out pointer.
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some((point.x, point.y))
}

/// `activatable = false` adds `WS_EX_NOACTIVATE` so clicking the island never steals keyboard
/// focus from the app the user is typing in. `WS_EX_TOOLWINDOW` keeps it out of Alt+Tab.
pub fn set_activatable(window: &WebviewWindow, activatable: bool) -> Result<()> {
    let hwnd = hwnd(window)?;
    let no_activate = WS_EX_NOACTIVATE.0 as isize;
    let tool_window = WS_EX_TOOLWINDOW.0 as isize;

    // SAFETY: `hwnd` is the live top-level window owned by this process; reading and writing
    // its extended style has no other preconditions.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) | tool_window;
        let style = if activatable {
            style & !no_activate
        } else {
            style | no_activate
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
    }
    Ok(())
}

/// Shows the window without activating it (Tauri's `show()` would steal focus).
pub fn show_inactive(window: &WebviewWindow) -> Result<()> {
    let hwnd = hwnd(window)?;
    // SAFETY: valid window handle owned by this process.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    Ok(())
}

/// Call just before the panel takes focus.
pub fn remember_foreground(window: &WebviewWindow) -> Result<()> {
    let ours = hwnd(window)?;
    // SAFETY: GetForegroundWindow has no preconditions.
    let current = unsafe { GetForegroundWindow() };
    if current != ours && !current.0.is_null() {
        PREVIOUS_FOREGROUND.store(current.0 as isize, Ordering::SeqCst);
    }
    Ok(())
}

/// Call when the panel closes. Only hands focus back if Preku still has it — if the user
/// clicked another app, that app already has focus and must keep it.
pub fn restore_foreground(window: &WebviewWindow) -> Result<()> {
    let ours = hwnd(window)?;
    let previous = PREVIOUS_FOREGROUND.swap(0, Ordering::SeqCst);
    // SAFETY: GetForegroundWindow has no preconditions; SetForegroundWindow tolerates stale
    // handles (it just fails), and we are the foreground process so it is allowed to switch.
    unsafe {
        if previous != 0 && GetForegroundWindow() == ours {
            let _ = SetForegroundWindow(HWND(previous as *mut _));
        }
    }
    Ok(())
}
