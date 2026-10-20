use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex, PoisonError,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};
use ts_rs::TS;

use super::{
    geometry::{self, MonitorRect},
    platform, ISLAND_WINDOW,
};
use crate::foundation::{
    errors::{PrekuError, Result},
    preferences::IslandPosition,
};

/// The island's visibility state (Architecture.md §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum IslandState {
    Hidden,
    Compact,
    Expanded,
}

/// Where on screen the island lives.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    pub position: IslandPosition,
    /// Monitor name; `None` = primary.
    pub monitor: Option<String>,
}

/// Event emitted to the frontend whenever the state changes.
pub const STATE_EVENT: &str = "island:state";
/// Emitted when the Windows desktop comes to the front or goes behind an app (bool).
pub const DESKTOP_EVENT: &str = "island:desktop";

/// Matches `--dur-base` (200 ms) plus a small buffer, so the CSS collapse animation
/// finishes before the window shrinks around it.
const SHRINK_DELAY: Duration = Duration::from_millis(220);

/// How often the watcher looks at the cursor while hidden, and at the mouse while shown.
const HIDDEN_POLL: Duration = Duration::from_millis(50);
/// The pointer must rest on the top edge this long before the island opens, so passing
/// over it or flicking up to a browser tab doesn't trigger it.
const EDGE_DWELL: Duration = Duration::from_millis(250);
/// Window geometry and "desktop in front" are re-read every this many hidden polls. Both go
/// through the UI thread, so reading them every 50 ms cost noticeable idle CPU.
const SLOW_CHECK_EVERY: u32 = 8;
const VISIBLE_POLL: Duration = Duration::from_millis(40);
/// A toggle right after a click-outside hid the island is that same click (e.g. on the tray
/// icon), so it must not reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(400);

/// Owns the island state. The frontend only renders what it is told (source of truth here).
pub struct IslandController {
    state: Mutex<IslandState>,
    placement: Mutex<Placement>,
    paused: AtomicBool,
    /// Bumped on every transition so a delayed shrink can tell it has been superseded.
    generation: AtomicU64,
    /// When a click outside last hid the island.
    auto_hidden_at: Mutex<Option<Instant>>,
    /// Expanded panel height (logical px), chosen by the frontend for the current view.
    panel_height: Mutex<f64>,
    /// Whether the desktop (no app) is in front; the hidden line is only drawn then.
    on_desktop: AtomicBool,
}

impl Default for IslandController {
    fn default() -> Self {
        Self {
            state: Mutex::new(IslandState::Hidden),
            placement: Mutex::new(Placement::default()),
            paused: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            auto_hidden_at: Mutex::new(None),
            panel_height: Mutex::new(geometry::DEFAULT_PANEL_HEIGHT),
            on_desktop: AtomicBool::new(true),
        }
    }
}

impl IslandController {
    fn panel_height(&self) -> f64 {
        *self
            .panel_height
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// The panel's height for the current view. While expanded the window follows: it grows
    /// at once and shrinks after the CSS animation, like state changes.
    pub fn set_panel_height(&self, app: &AppHandle, height: f64) -> Result<()> {
        let height = geometry::clamp_panel_height(height);
        let previous = std::mem::replace(
            &mut *self
                .panel_height
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
            height,
        );
        if (previous - height).abs() < 0.5 || self.state() != IslandState::Expanded {
            return Ok(());
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        if height > previous {
            apply_geometry(
                &island_window(app)?,
                IslandState::Expanded,
                &self.placement(),
                height,
            )
        } else {
            shrink_later(app.clone(), generation, IslandState::Expanded);
            Ok(())
        }
    }

    pub fn on_desktop(&self) -> bool {
        self.on_desktop.load(Ordering::SeqCst)
    }

    pub fn state(&self) -> IslandState {
        *self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn placement(&self) -> Placement {
        self.placement
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Positions the island at the top edge and shows it (unless paused). Call once at startup.
    pub fn init(&self, window: &WebviewWindow, placement: Placement, paused: bool) -> Result<()> {
        *self
            .placement
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = placement;
        self.paused.store(paused, Ordering::SeqCst);
        platform::set_activatable(window, false)?;
        apply_geometry(window, self.state(), &self.placement(), self.panel_height())?;
        if !paused {
            platform::show_inactive(window)?;
        }
        Ok(())
    }

    /// Moves the island to a new position/monitor right away.
    pub fn set_placement(&self, app: &AppHandle, placement: Placement) -> Result<()> {
        *self
            .placement
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = placement;
        apply_geometry(
            &island_window(app)?,
            self.state(),
            &self.placement(),
            self.panel_height(),
        )
    }

    /// Paused: island hidden completely and ignores hover, clicks and shortcuts.
    pub fn set_paused(&self, app: &AppHandle, paused: bool) -> Result<()> {
        if self.paused.swap(paused, Ordering::SeqCst) == paused {
            return Ok(());
        }
        let window = island_window(app)?;
        if paused {
            self.set_state_unchecked(app, IslandState::Hidden)?;
            window.hide().map_err(window_error)
        } else {
            apply_geometry(
                &window,
                self.state(),
                &self.placement(),
                self.panel_height(),
            )?;
            platform::show_inactive(&window)
        }
    }

    pub fn set_state(&self, app: &AppHandle, next: IslandState) -> Result<()> {
        if self.paused.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.set_state_unchecked(app, next)
    }

    fn set_state_unchecked(&self, app: &AppHandle, next: IslandState) -> Result<()> {
        let previous = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            std::mem::replace(&mut *state, next)
        };
        if previous == next {
            return Ok(());
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let window = island_window(app)?;

        let panel = self.panel_height();
        if geometry::area(next, panel) > geometry::area(previous, panel) {
            // Grow the window first so the CSS expand animation has room.
            apply_geometry(&window, next, &self.placement(), panel)?;
        } else {
            shrink_later(app.clone(), generation, next);
        }

        if next == IslandState::Expanded {
            platform::remember_foreground(&window)?;
            platform::set_activatable(&window, true)?;
            window.set_focus().map_err(window_error)?;
        } else if previous == IslandState::Expanded {
            platform::set_activatable(&window, false)?;
            platform::restore_foreground(&window)?;
        }

        app.emit(STATE_EVENT, next).map_err(window_error)?;
        tracing::debug!(?previous, ?next, "island state changed");
        Ok(())
    }

    /// Shortcut / tray behaviour: open the panel, or close it if it is already open.
    pub fn toggle(&self, app: &AppHandle) {
        let just_auto_hidden = self
            .auto_hidden_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some_and(|at| at.elapsed() < REOPEN_GUARD);
        if just_auto_hidden {
            return;
        }
        let next = if self.state() == IslandState::Expanded {
            IslandState::Hidden
        } else {
            IslandState::Expanded
        };
        if let Err(err) = self.set_state(app, next) {
            tracing::warn!(error = %err, "failed to toggle island");
        }
    }
}

/// Background watcher (Windows; on Linux the probes return defaults and hover works via the
/// window as before):
/// - while hidden the window is click-through, so it never blocks browser tabs or title bars;
///   instead the island opens when the pointer rests on the top screen edge over the line
///   (`EDGE_DWELL`) with no button held — user request 2026-10-09 ("too sensitive");
/// - tells the UI whether the desktop is in front, so the thin line is only drawn there;
/// - while shown, a mouse click anywhere outside the island hides it.
pub fn spawn_watcher(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("preku-island-watch".into())
        .spawn(move || watch(app));
    if let Err(err) = spawned {
        tracing::warn!(error = %err, "island watcher failed to start");
    }
}

fn watch(app: AppHandle) {
    let mut was_pressed = false;
    let edge_trigger = platform::cursor_position().is_some();
    let mut click_through: Option<bool> = None;
    let mut dwell_since: Option<Instant> = None;
    let mut edge_rect: Option<EdgeRect> = None;
    let mut tick: u32 = 0;
    loop {
        let controller = app.state::<IslandController>();
        let Ok(window) = island_window(&app) else {
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        if controller.paused.load(Ordering::SeqCst) {
            std::thread::sleep(HIDDEN_POLL);
            continue;
        }

        tick = tick.wrapping_add(1);
        let slow_check = tick % SLOW_CHECK_EVERY == 0;
        if slow_check {
            let on_desktop = platform::desktop_in_front(&window);
            if controller.on_desktop.swap(on_desktop, Ordering::SeqCst) != on_desktop {
                if let Err(err) = app.emit(DESKTOP_EVENT, on_desktop) {
                    tracing::warn!(error = %err, "failed to emit desktop state");
                }
            }
        }

        if controller.state() == IslandState::Hidden {
            if edge_trigger {
                if click_through != Some(true) && set_click_through(&window, true, false) {
                    click_through = Some(true);
                }
                if slow_check || edge_rect.is_none() {
                    edge_rect = EdgeRect::read(&window);
                }
                let pressed = platform::mouse_pressed();
                if !pressed && edge_rect.is_some_and(EdgeRect::under_cursor) {
                    let since = *dwell_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= EDGE_DWELL {
                        dwell_since = None;
                        if let Err(err) = controller.set_state(&app, IslandState::Compact) {
                            tracing::warn!(error = %err, "failed to open island from edge");
                        }
                    }
                } else {
                    dwell_since = None;
                }
            }
            // Remember a press already in progress, so it can't count as a fresh click later.
            was_pressed = platform::mouse_pressed();
            std::thread::sleep(HIDDEN_POLL);
            continue;
        }

        dwell_since = None;
        // The window moves/resizes while shown; read it fresh next time it's hidden.
        edge_rect = None;
        if click_through != Some(false) {
            let expanded = controller.state() == IslandState::Expanded;
            if set_click_through(&window, false, expanded) {
                click_through = Some(false);
            }
        }

        let pressed = platform::mouse_pressed();
        if pressed && !was_pressed && clicked_outside(&window) {
            *controller
                .auto_hidden_at
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
            if let Err(err) = controller.set_state(&app, IslandState::Hidden) {
                tracing::warn!(error = %err, "failed to hide island after outside click");
            }
        }
        was_pressed = pressed;
        std::thread::sleep(VISIBLE_POLL);
    }
}

/// Tauri rebuilds the window's styles when toggling click-through, which drops our "never take
/// focus" flag and hides the window, so both are restored right after. Returns whether it worked.
fn set_click_through(window: &WebviewWindow, ignore: bool, activatable: bool) -> bool {
    if let Err(err) = window.set_ignore_cursor_events(ignore) {
        tracing::warn!(error = %err, "failed to toggle click-through");
        return false;
    }
    if let Err(err) = platform::set_activatable(window, activatable) {
        tracing::warn!(error = %err, "failed to restore no-activate style");
    }
    // Tauri also re-applies its own idea of visibility, and it believes the window is hidden
    // (we show it with a raw no-activate call so it never steals focus). Show it again the
    // same way, or the island vanishes after the first toggle.
    if let Err(err) = platform::show_inactive(window) {
        tracing::warn!(error = %err, "failed to re-show island after click-through change");
    }
    true
}

/// The hidden window's placement, cached between polls (see `SLOW_CHECK_EVERY`).
#[derive(Clone, Copy)]
struct EdgeRect {
    x: i32,
    y: i32,
    width: u32,
    scale: f64,
}

impl EdgeRect {
    fn read(window: &WebviewWindow) -> Option<Self> {
        let pos = window.outer_position().ok()?;
        Some(Self {
            x: pos.x,
            y: pos.y,
            width: window.outer_size().ok()?.width,
            scale: window.scale_factor().ok()?,
        })
    }

    /// Whether the cursor touches the top edge over the hidden line.
    fn under_cursor(self) -> bool {
        platform::cursor_position().is_some_and(|(x, y)| {
            geometry::in_edge_zone(self.x, self.y, self.width, self.scale, x, y)
        })
    }
}

fn clicked_outside(window: &WebviewWindow) -> bool {
    let Some((x, y)) = platform::cursor_position() else {
        return false;
    };
    match (window.outer_position(), window.outer_size()) {
        (Ok(pos), Ok(size)) => !geometry::contains(pos.x, pos.y, size.width, size.height, x, y),
        _ => false,
    }
}

/// Shrinks the window to `state` once the CSS collapse animation has finished, unless
/// another transition happened meanwhile (generation changed).
fn shrink_later(app: AppHandle, generation: u64, state: IslandState) {
    std::thread::spawn(move || {
        std::thread::sleep(SHRINK_DELAY);
        let controller = app.state::<IslandController>();
        if controller.generation.load(Ordering::SeqCst) != generation {
            return;
        }
        let (placement, panel) = (controller.placement(), controller.panel_height());
        if let Err(err) =
            island_window(&app).and_then(|w| apply_geometry(&w, state, &placement, panel))
        {
            tracing::warn!(error = %err, "failed to shrink island");
        }
    });
}

fn island_window(app: &AppHandle) -> Result<WebviewWindow> {
    app.get_webview_window(ISLAND_WINDOW)
        .ok_or_else(|| PrekuError::Window("island window is missing".into()))
}

fn window_error(err: tauri::Error) -> PrekuError {
    PrekuError::Window(err.to_string())
}

/// The chosen monitor if it is still connected, else the primary one.
fn pick_monitor(window: &WebviewWindow, name: Option<&str>) -> Result<Monitor> {
    if let Some(name) = name {
        let found = window
            .available_monitors()
            .map_err(window_error)?
            .into_iter()
            .find(|m| m.name().is_some_and(|n| n == name));
        if let Some(monitor) = found {
            return Ok(monitor);
        }
    }
    match window.primary_monitor().map_err(window_error)? {
        Some(monitor) => Ok(monitor),
        None => window
            .current_monitor()
            .map_err(window_error)?
            .ok_or_else(|| PrekuError::Window("no monitor found".into())),
    }
}

/// Sizes the window for `state` and pins it to the top edge.
fn apply_geometry(
    window: &WebviewWindow,
    state: IslandState,
    placement: &Placement,
    panel_height: f64,
) -> Result<()> {
    let monitor = pick_monitor(window, placement.monitor.as_deref())?;
    let rect = MonitorRect {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        scale: monitor.scale_factor(),
    };
    let (width, height) = geometry::window_size(state, panel_height);
    let size = PhysicalSize::new(
        (width * rect.scale).round() as u32,
        (height * rect.scale).round() as u32,
    );
    let (x, y) = geometry::top_anchor(placement.position, rect, size.width);
    window.set_size(size).map_err(window_error)?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(window_error)
}
