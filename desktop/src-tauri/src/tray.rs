use tauri::{
    menu::{MenuBuilder, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

use crate::{
    actions,
    foundation::{
        errors::{PrekuError, Result},
        preferences::Preferences,
    },
    island::controller::IslandController,
};

const MENU_TOGGLE: &str = "toggle";
const MENU_SETTINGS: &str = "settings";
const MENU_PAUSE: &str = "pause";
const MENU_QUIT: &str = "quit";

// Tray labels are English-only until the Rust side gets a string table (Phase 10, Memory.md).
fn pause_label(paused: bool) -> &'static str {
    if paused {
        "Resume Preku"
    } else {
        "Pause Preku"
    }
}

/// Handle to the menu item whose label changes with the pause state.
struct PauseItem(MenuItem<Wry>);

/// System tray icon: left click toggles the panel; right click opens the menu.
pub fn init(app: &AppHandle, paused: bool) -> Result<()> {
    let to_err = |e: tauri::Error| PrekuError::Window(e.to_string());

    let pause = MenuItem::with_id(app, MENU_PAUSE, pause_label(paused), true, None::<&str>)
        .map_err(to_err)?;
    let menu = MenuBuilder::new(app)
        .text(MENU_TOGGLE, "Open / close Preku")
        .text(MENU_SETTINGS, "Settings…")
        .item(&pause)
        .separator()
        .text(MENU_QUIT, "Quit Preku")
        .build()
        .map_err(to_err)?;
    app.manage(PauseItem(pause));

    let mut builder = TrayIconBuilder::with_id("preku")
        .tooltip("Preku")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_TOGGLE => app.state::<IslandController>().toggle(app),
            MENU_SETTINGS => {
                if let Err(err) = actions::open_settings(app) {
                    tracing::warn!(error = %err, "failed to open settings");
                }
            }
            MENU_PAUSE => actions::toggle_pause(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                app.state::<IslandController>().toggle(app);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app).map_err(to_err)?;
    Ok(())
}

/// Keeps menu labels in step with preferences.
pub fn sync(app: &AppHandle, preferences: &Preferences) {
    if let Some(item) = app.try_state::<PauseItem>() {
        if let Err(err) = item.0.set_text(pause_label(preferences.paused)) {
            tracing::warn!(error = %err, "failed to update tray menu");
        }
    }
}
