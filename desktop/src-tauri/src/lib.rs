mod actions;
pub mod commands;
pub mod foundation;
pub mod island;
pub mod live;
pub mod modules;
pub mod pip;
pub mod protocol;
mod runtime;
mod tray;

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    foundation::{
        errors::{PrekuError, Result},
        event_bus::EventBus,
        logging, preferences,
        state::AppState,
        storage::Storage,
    },
    island::{
        controller::{IslandController, Placement},
        platform, ISLAND_WINDOW,
    },
    modules::{
        bridge::BridgeModule, monitor::MonitorModule, productivity::ProductivityModule,
        weather::WeatherModule, wellbeing::WellbeingModule, Module, ModuleContext, ModuleRegistry,
    },
    runtime::{LiveState, MoodState},
};

/// Global shortcuts to open/close the island (PRD C6), tried in order until one is free.
/// Users will be able to pick their own in the settings window.
const TOGGLE_SHORTCUTS: &[&str] = &["ctrl+alt+space", "ctrl+alt+p", "alt+shift+p"];

/// The toggle shortcut that is actually registered, shown in the UI.
pub struct ActiveShortcut(pub Option<&'static str>);

/// Quick capture (PRD A13): opens the notch on Tasks with the cursor in "add a task". Tried
/// in order until one is free, so it never takes a shortcut another app (or the toggle) uses.
const CAPTURE_SHORTCUTS: &[&str] = &["ctrl+alt+n", "ctrl+alt+t", "alt+shift+n"];
pub const CAPTURE_EVENT: &str = "island:capture";

/// The quick-capture shortcut that is registered (its name for the UI, and its id).
pub struct ActiveCapture(pub Option<(&'static str, u32)>);

/// Builds and runs the Tauri application.
pub fn run() {
    platform::configure_backend();

    let result = tauri::Builder::default()
        // Must be registered first: a second launch just opens the existing island.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app.state::<IslandController>().toggle(app);
        }))
        // Native file picker for "Send files to phone" (used from Rust only).
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let capture = app
                        .try_state::<ActiveCapture>()
                        .and_then(|active| active.0)
                        .is_some_and(|(_, id)| id == shortcut.id());
                    if capture {
                        // Open (never close) and put the cursor in "add a task".
                        let controller = app.state::<IslandController>();
                        if let Err(err) =
                            controller.set_state(app, island::controller::IslandState::Expanded)
                        {
                            tracing::warn!(error = %err, "failed to open the island for capture");
                        }
                        let _ = tauri::Emitter::emit(app, CAPTURE_EVENT, ());
                    } else {
                        app.state::<IslandController>().toggle(app);
                    }
                })
                .build(),
        )
        .manage(IslandController::default())
        .manage(LiveState::default())
        .manage(MoodState::default())
        .setup(|app| {
            setup(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::island::island_get_state,
            commands::island::island_on_desktop,
            commands::island::island_set_panel_height,
            commands::island::island_set_state,
            commands::live::live_get_items,
            commands::live::pip_get_mood,
            commands::privacy::privacy_get,
            commands::privacy::privacy_set,
            commands::privacy::privacy_export,
            commands::privacy::privacy_delete_all,
            commands::quiet::quiet_get,
            commands::quiet::quiet_pause,
            commands::quiet::quiet_hold_in_focus,
            commands::quiet::quiet_hold_fullscreen_apps,
            commands::preferences::preferences_get,
            commands::preferences::preferences_set,
            commands::preferences::monitors_list,
            commands::preferences::modules_list,
            commands::preferences::settings_open,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::productivity::timer_get,
            commands::productivity::timer_start,
            commands::productivity::timer_dismiss_finished,
            commands::productivity::task_set_priority,
            commands::productivity::today_summary,
            commands::productivity::task_set_due,
            commands::wellbeing::habit_update,
            commands::productivity::calendar_get,
            commands::productivity::calendar_add_link,
            commands::productivity::calendar_add_file,
            commands::productivity::calendar_remove,
            commands::productivity::calendar_set_enabled,
            commands::productivity::calendar_set_options,
            commands::productivity::calendar_refresh,
            commands::productivity::timer_pause,
            commands::productivity::timer_resume,
            commands::productivity::timer_stop,
            commands::productivity::tasks_list,
            commands::productivity::task_add,
            commands::productivity::task_rename,
            commands::productivity::task_set_done,
            commands::productivity::task_delete,
            commands::productivity::tasks_reorder,
            commands::productivity::clipboard_list,
            commands::productivity::clipboard_copy,
            commands::productivity::clipboard_set_pinned,
            commands::productivity::clipboard_delete,
            commands::productivity::clipboard_clear,
            commands::productivity::clipboard_image,
            commands::productivity::media_get,
            commands::productivity::media_command,
            commands::monitor::monitor_get,
            commands::monitor::monitor_set_view,
            commands::monitor::process_kill,
            commands::monitor::alert_rule_save,
            commands::monitor::alert_repeat_set,
            commands::monitor::alert_rule_delete,
            commands::monitor::docker_action,
            commands::monitor::github_token_set,
            commands::monitor::target_add,
            commands::monitor::target_delete,
            commands::wellbeing::wellbeing_get,
            commands::wellbeing::water_add,
            commands::wellbeing::water_undo,
            commands::wellbeing::habit_add,
            commands::wellbeing::habit_delete,
            commands::wellbeing::habit_log,
            commands::wellbeing::wellbeing_settings_set,
            commands::wellbeing::wellbeing_prompt_later,
            commands::wellbeing::wellbeing_break_done,
            commands::wellbeing::mood_log,
            commands::bridge::bridge_get,
            commands::bridge::bridge_pair_start,
            commands::bridge::bridge_pair_cancel,
            commands::bridge::bridge_unpair,
            commands::bridge::bridge_send_files,
            commands::bridge::bridge_pick_and_send,
            commands::bridge::bridge_cancel_transfer,
            commands::bridge::bridge_reveal,
            commands::bridge::bridge_send_text,
            commands::bridge::bridge_clear_texts,
            commands::bridge::bridge_open_link,
            commands::weather::weather_get,
            commands::weather::weather_search,
            commands::weather::weather_refresh,
            commands::skills::skills_get,
            commands::skills::skill_install,
            commands::skills::skill_set_enabled,
            commands::skills::skill_remove,
            commands::skills::skill_remove_broken,
        ])
        .run(tauri::generate_context!());

    if let Err(err) = result {
        eprintln!("Preku failed to start: {err}");
        std::process::exit(1);
    }
}

fn setup(app: &AppHandle) -> Result<()> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| PrekuError::Window(e.to_string()))?;
    // Testing aid: PREKU_DATA_DIR runs Preku as a brand-new user in an empty folder (its
    // secrets then go to a separate keychain entry, see foundation/secrets.rs).
    let data_dir = std::env::var_os(foundation::secrets::TEST_PROFILE_ENV)
        .map(std::path::PathBuf::from)
        .unwrap_or(data_dir);
    app.manage(logging::init(&data_dir.join("logs"))?);

    let storage = Arc::new(Storage::open(&data_dir.join("preku.db"))?);
    let prefs = preferences::load(&storage)?;
    app.manage(AppState {
        storage: storage.clone(),
        preferences: Mutex::new(prefs.clone()),
    });

    // Subscribe before any module starts so no event is lost.
    let bus = EventBus::default();
    runtime::spawn(app.clone(), bus.subscribe());

    let window = app
        .get_webview_window(ISLAND_WINDOW)
        .ok_or_else(|| PrekuError::Window("island window missing from config".into()))?;
    let placement = Placement {
        position: prefs.island_position,
        monitor: prefs.monitor.clone(),
    };
    app.state::<IslandController>()
        .init(&window, placement, prefs.paused)?;
    island::controller::spawn_watcher(app.clone());

    tray::init(app, prefs.paused)?;
    let shortcut = register_shortcut(app);
    app.manage(ActiveShortcut(shortcut));
    app.manage(ActiveCapture(register_capture_shortcut(app)));

    let productivity = Arc::new(ProductivityModule::default());
    app.manage(productivity.clone());
    let monitor = Arc::new(MonitorModule::default());
    app.manage(monitor.clone());
    let wellbeing = Arc::new(WellbeingModule::default());
    app.manage(wellbeing.clone());
    let weather = Arc::new(WeatherModule::default());
    app.manage(weather.clone());
    let bridge = Arc::new(BridgeModule::default());
    app.manage(bridge.clone());
    #[allow(unused_mut)]
    let mut modules: Vec<Arc<dyn Module>> = vec![productivity, monitor, wellbeing, weather, bridge];
    // The demo module (sample live items) is a development aid: it isn't part of builds
    // that are shared with people.
    #[cfg(debug_assertions)]
    modules.push(Arc::new(modules::demo::DemoModule::default()));
    let quiet = foundation::quiet::Quiet::load(&storage);
    app.manage(quiet.clone());
    commands::quiet::spawn_screen_watcher(app.clone(), quiet.clone());
    app.manage(storage.clone());
    let privacy = foundation::privacy::Privacy::load(&storage);
    app.manage(privacy.clone());
    let image_dir = commands::privacy::ImageDir(data_dir.join("clipboard"));
    commands::privacy::spawn_pruner(storage.clone(), privacy.clone(), image_dir.0.clone());
    app.manage(image_dir);
    let ctx = ModuleContext {
        bus,
        storage,
        data_dir,
        quiet,
        privacy,
    };
    // Skills have a switch each instead of one for the module, so they live beside the registry.
    let skills = Arc::new(modules::skills::SkillsService::new(ctx.clone()));
    skills.start();
    app.manage(skills);
    let registry = ModuleRegistry::new(ctx, modules);
    registry.apply(&prefs.modules, prefs.paused);
    app.manage(registry);

    actions::sync_autostart(app, prefs.launch_at_login);
    // First run: a short, skippable welcome in the Settings window (usability U3).
    if !prefs.welcomed {
        if let Err(err) = actions::open_settings(app) {
            tracing::warn!(error = %err, "couldn't open the welcome");
        }
    }

    tracing::info!(version = %app.package_info().version, "Preku started");
    Ok(())
}

/// Registers the first free quick-capture shortcut; none free is fine (the feature is then
/// simply not offered).
fn register_capture_shortcut(app: &AppHandle) -> Option<(&'static str, u32)> {
    use std::str::FromStr;
    for name in CAPTURE_SHORTCUTS {
        let Ok(shortcut) = tauri_plugin_global_shortcut::Shortcut::from_str(name) else {
            continue;
        };
        match app.global_shortcut().register(shortcut) {
            Ok(()) => {
                tracing::info!(shortcut = name, "quick-capture shortcut registered");
                return Some((name, shortcut.id()));
            }
            Err(err) => {
                tracing::debug!(error = %err, shortcut = name, "capture shortcut unavailable")
            }
        }
    }
    None
}

/// A shortcut already taken by another app must not stop Preku from starting.
/// Returns the shortcut that was registered, if any.
fn register_shortcut(app: &AppHandle) -> Option<&'static str> {
    for shortcut in TOGGLE_SHORTCUTS {
        match app.global_shortcut().register(*shortcut) {
            Ok(()) => {
                tracing::info!(shortcut, "global shortcut registered");
                return Some(shortcut);
            }
            Err(err) => tracing::warn!(error = %err, shortcut, "global shortcut unavailable"),
        }
    }
    None
}
