use super::controller::IslandState;
use crate::foundation::preferences::IslandPosition;

/// Transparent margin around the visible island so its drop shadow isn't clipped (logical px).
const SHADOW_MARGIN: f64 = 12.0;
/// Gap from the screen's side edge when the island sits left or right (logical px).
const SIDE_GAP: f64 = 16.0;

/// Expanded panel height: the frontend picks it per view (short home row, taller module
/// views); clamped so a bad value can't make a giant or useless window.
pub const DEFAULT_PANEL_HEIGHT: f64 = 360.0;
pub const MIN_PANEL_HEIGHT: f64 = 160.0;
pub const MAX_PANEL_HEIGHT: f64 = 520.0;

pub fn clamp_panel_height(height: f64) -> f64 {
    if height.is_finite() {
        height.clamp(MIN_PANEL_HEIGHT, MAX_PANEL_HEIGHT)
    } else {
        DEFAULT_PANEL_HEIGHT
    }
}

/// Window size in logical pixels for each state. Visible sizes come from Design.md §5;
/// the window adds a shadow margin on the sides and bottom (the top is flush with the edge).
pub fn window_size(state: IslandState, panel_height: f64) -> (f64, f64) {
    let (w, h) = match state {
        // 180 × 4 visible line, but a 24 px tall strip so the edge is easy to hover.
        IslandState::Hidden => return (200.0, 24.0),
        IslandState::Compact => (300.0, 36.0),
        IslandState::Expanded => (480.0, clamp_panel_height(panel_height)),
    };
    (w + SHADOW_MARGIN * 2.0, h + SHADOW_MARGIN)
}

pub fn area(state: IslandState, panel_height: f64) -> f64 {
    let (w, h) = window_size(state, panel_height);
    w * h
}

/// Monitor rectangle in physical pixels.
#[derive(Debug, Clone, Copy)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub scale: f64,
}

/// Top-edge position (physical px) for a window `width_px` wide.
pub fn top_anchor(position: IslandPosition, monitor: MonitorRect, width_px: u32) -> (i32, i32) {
    let gap = (SIDE_GAP * monitor.scale).round() as i64;
    let free = i64::from(monitor.width) - i64::from(width_px);
    let offset = match position {
        IslandPosition::Left => gap,
        IslandPosition::Center => free / 2,
        IslandPosition::Right => free - gap,
    };
    (monitor.x + offset as i32, monitor.y)
}

/// The edge trigger: the top `EDGE_PX` logical pixels of the screen, across the hidden line's
/// width plus a little slack on each side.
pub const EDGE_PX: f64 = 2.0;
pub const EDGE_SLACK_PX: f64 = 40.0;

/// Whether the cursor touches the top edge where the hidden line is. `window_*` is the hidden
/// island window in physical pixels (its top is the monitor's top edge).
pub fn in_edge_zone(
    window_x: i32,
    window_y: i32,
    window_width: u32,
    scale: f64,
    x: i32,
    y: i32,
) -> bool {
    let slack = (EDGE_SLACK_PX * scale).round() as i64;
    let depth = (EDGE_PX * scale).round().max(1.0) as i64;
    let (x, y) = (i64::from(x), i64::from(y));
    let left = i64::from(window_x) - slack;
    let right = i64::from(window_x) + i64::from(window_width) + slack;
    let top = i64::from(window_y);
    x >= left && x < right && y >= top && y < top + depth
}

/// Whether point (x, y) lies inside the rectangle at (left, top) of the given size.
pub fn contains(left: i32, top: i32, width: u32, height: u32, x: i32, y: i32) -> bool {
    let right = i64::from(left) + i64::from(width);
    let bottom = i64::from(top) + i64::from(height);
    x >= left && y >= top && i64::from(x) < right && i64::from(y) < bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_zone_is_only_the_top_pixels_over_the_line() {
        // Hidden window: 250 px wide at x = 835 on a 125% monitor starting at y = 0.
        let zone = |x, y| in_edge_zone(835, 0, 250, 1.25, x, y);
        assert!(zone(900, 0));
        assert!(zone(900, 2), "2 logical px at 125% = 3 physical px (0..2)");
        assert!(
            !zone(900, 3),
            "a browser tab row below the edge does not trigger"
        );
        assert!(!zone(900, 20));
        assert!(zone(835 - 50, 0), "slack of 40 logical = 50 physical");
        assert!(!zone(835 - 51, 0));
        assert!(!zone(2000, 0), "far away on the same edge");
    }

    #[test]
    fn point_in_rect() {
        assert!(contains(100, 0, 50, 20, 100, 0));
        assert!(contains(100, 0, 50, 20, 149, 19));
        assert!(!contains(100, 0, 50, 20, 150, 10), "right edge is outside");
        assert!(!contains(100, 0, 50, 20, 99, 10));
        assert!(
            contains(-1280, -1024, 100, 100, -1200, -1000),
            "negative monitor coords"
        );
    }

    fn monitor(x: i32, y: i32, width: u32, scale: f64) -> MonitorRect {
        MonitorRect { x, y, width, scale }
    }

    #[test]
    fn states_grow_in_order() {
        let h = MIN_PANEL_HEIGHT;
        assert!(area(IslandState::Hidden, h) < area(IslandState::Compact, h));
        assert!(area(IslandState::Compact, h) < area(IslandState::Expanded, h));
    }

    #[test]
    fn panel_height_is_clamped() {
        assert_eq!(clamp_panel_height(204.0), 204.0);
        assert_eq!(clamp_panel_height(10.0), MIN_PANEL_HEIGHT);
        assert_eq!(clamp_panel_height(9_999.0), MAX_PANEL_HEIGHT);
        assert_eq!(clamp_panel_height(f64::NAN), DEFAULT_PANEL_HEIGHT);
        assert_eq!(window_size(IslandState::Expanded, 204.0), (504.0, 216.0));
    }

    #[test]
    fn centers_on_primary_and_secondary_monitors() {
        let center = IslandPosition::Center;
        assert_eq!(top_anchor(center, monitor(0, 0, 1920, 1.0), 200), (860, 0));
        assert_eq!(
            top_anchor(center, monitor(1920, 0, 2560, 1.0), 504),
            (1920 + 1028, 0)
        );
        assert_eq!(
            top_anchor(center, monitor(-1280, -1024, 1280, 1.0), 280),
            (-1280 + 500, -1024)
        );
    }

    #[test]
    fn left_and_right_keep_a_scaled_gap() {
        let m = monitor(0, 0, 1920, 1.25);
        assert_eq!(top_anchor(IslandPosition::Left, m, 300), (20, 0));
        assert_eq!(
            top_anchor(IslandPosition::Right, m, 300),
            (1920 - 300 - 20, 0)
        );
    }
}
