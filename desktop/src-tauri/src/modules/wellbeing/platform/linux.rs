//! Linux via X11 (native or XWayland). Unverified on real hardware — see Memory.md.
//! Native Wayland sessions don't expose the focused app to normal clients, so per-app screen
//! time is reported as unsupported there.

use std::fs;

use x11rb::{
    connection::Connection,
    protocol::{
        screensaver::ConnectionExt as _,
        xproto::{AtomEnum, ConnectionExt as _, Window},
    },
};

use super::Activity;

fn wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland"))
}

pub fn sample() -> Activity {
    if wayland_session() {
        return Activity {
            supported: false,
            ..Activity::default()
        };
    }
    x11_sample().unwrap_or(Activity {
        supported: false,
        ..Activity::default()
    })
}

fn atom(conn: &impl Connection, name: &str) -> Option<u32> {
    Some(
        conn.intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom,
    )
}

fn x11_sample() -> Option<Activity> {
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root: Window = conn.setup().roots.get(screen)?.root;

    let idle_ms = conn
        .screensaver_query_info(root)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|info| u64::from(info.ms_since_user_input));

    let active = atom(&conn, "_NET_ACTIVE_WINDOW")?;
    let window = conn
        .get_property(false, root, active, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?
        .value32()?
        .next()
        .filter(|w| *w != 0);

    let (app, fullscreen) = match window {
        Some(window) => {
            let pid_atom = atom(&conn, "_NET_WM_PID")?;
            let app = conn
                .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)
                .ok()
                .and_then(|c| c.reply().ok())
                .and_then(|r| r.value32().and_then(|mut v| v.next()))
                .and_then(|pid| fs::read_to_string(format!("/proc/{pid}/comm")).ok())
                .map(|name| name.trim().to_owned());

            let state = atom(&conn, "_NET_WM_STATE")?;
            let fullscreen_atom = atom(&conn, "_NET_WM_STATE_FULLSCREEN")?;
            let fullscreen = conn
                .get_property(false, window, state, AtomEnum::ATOM, 0, 32)
                .ok()
                .and_then(|c| c.reply().ok())
                .and_then(|r| r.value32().map(|mut v| v.any(|a| a == fullscreen_atom)))
                .unwrap_or(false);
            (app, fullscreen)
        }
        None => (None, false),
    };

    Some(Activity {
        app,
        idle_ms,
        fullscreen,
        supported: true,
    })
}
