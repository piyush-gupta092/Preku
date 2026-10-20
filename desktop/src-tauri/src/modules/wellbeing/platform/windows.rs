use windows::{
    core::PWSTR,
    Win32::{
        Foundation::CloseHandle,
        System::{
            SystemInformation::GetTickCount,
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
            Shell::{
                SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
                QUNS_RUNNING_D3D_FULL_SCREEN,
            },
            WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        },
    },
};

use super::Activity;

pub fn sample() -> Activity {
    Activity {
        app: foreground_app(),
        idle_ms: idle_ms(),
        fullscreen: fullscreen_busy(),
        supported: true,
    }
}

fn idle_ms() -> Option<u64> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a valid, correctly sized struct for the duration of the call.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }
    // SAFETY: no preconditions. Both values wrap every ~49 days; wrapping_sub handles it.
    let now = unsafe { GetTickCount() };
    Some(u64::from(now.wrapping_sub(info.dwTime)))
}

fn foreground_app() -> Option<String> {
    // SAFETY: no preconditions.
    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return None;
    }
    let mut pid = 0u32;
    // SAFETY: `window` came from the OS just now; `pid` is a valid out pointer.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    // SAFETY: limited-information access needs no special rights; the handle is closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = [0u16; 1024];
    let mut len = buffer.len() as u32;
    // SAFETY: `buffer` holds `len` u16s; the call writes at most that many and updates `len`.
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
    };
    // SAFETY: `process` is an open handle we own.
    unsafe {
        let _ = CloseHandle(process);
    }
    result.ok()?;
    let path = String::from_utf16_lossy(&buffer[..len as usize]);
    path.rsplit('\\').next().map(str::to_owned)
}

/// What is in front, for quiet mode: an ordinary full-screen app is not the same as a game
/// or a presentation.
pub fn screen() -> crate::foundation::quiet::Screen {
    use crate::foundation::quiet::Screen;
    // SAFETY: no preconditions.
    match unsafe { SHQueryUserNotificationState() } {
        Ok(QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE) => Screen::GameOrPresentation,
        Ok(QUNS_BUSY) => Screen::FullscreenApp,
        _ => Screen::Normal,
    }
}

/// Windows' own "don't disturb" signal: a fullscreen app, a D3D game or presentation mode.
fn fullscreen_busy() -> bool {
    // SAFETY: no preconditions.
    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => matches!(
            state,
            QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
        ),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_works_on_this_machine() {
        let activity = sample();
        assert!(activity.supported);
        assert!(activity.idle_ms.is_some());
    }
}
