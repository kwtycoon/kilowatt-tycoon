//! Tests that the UI-scaling and camera-fit math produces correct values at
//! common mobile and tablet viewport sizes.  These run under `cargo test` with
//! no browser required.

use kilowatt_tycoon::resources::{GRID_HEIGHT, GRID_WIDTH, TILE_SIZE};
use kilowatt_tycoon::systems::{
    DESIGN_HEIGHT, DESIGN_WIDTH, compute_fit_scale, compute_ui_scale, ui_layout_constants,
};

// --------------------------------------------------------------------------
// Target device logical resolutions (landscape unless noted)
// --------------------------------------------------------------------------

const IPHONE_14_LANDSCAPE: (f32, f32) = (844.0, 390.0);
const IPHONE_14_PRO_MAX_LANDSCAPE: (f32, f32) = (932.0, 430.0);
const PIXEL_7_LANDSCAPE: (f32, f32) = (915.0, 412.0);
const IPAD_MINI_LANDSCAPE: (f32, f32) = (1024.0, 768.0);
const IPAD_AIR_LANDSCAPE: (f32, f32) = (1180.0, 820.0);

const IPHONE_14_PORTRAIT: (f32, f32) = (390.0, 844.0);

const DESKTOP_1080P: (f32, f32) = (1920.0, 1080.0);
const DESIGN_RES: (f32, f32) = (DESIGN_WIDTH, DESIGN_HEIGHT);

// --------------------------------------------------------------------------
// compute_ui_scale
// --------------------------------------------------------------------------

#[test]
fn ui_scale_at_design_resolution_is_one() {
    let scale = compute_ui_scale(DESIGN_RES.0, DESIGN_RES.1);
    assert!((scale - 1.0).abs() < 0.001);
}

#[test]
fn ui_scale_on_large_desktop_is_capped_at_one() {
    let scale = compute_ui_scale(DESKTOP_1080P.0, DESKTOP_1080P.1);
    assert!(
        (scale - 1.0).abs() < 0.001,
        "desktop bigger than design should still be 1.0, got {scale}"
    );
}

#[test]
fn ui_scale_on_tablets_is_below_one() {
    for (name, (w, h)) in [
        ("iPad Mini landscape", IPAD_MINI_LANDSCAPE),
        ("iPad Air landscape", IPAD_AIR_LANDSCAPE),
    ] {
        let scale = compute_ui_scale(w, h);
        assert!(scale < 1.0, "{name}: scale should be < 1.0, got {scale}");
        assert!(
            scale > 0.4,
            "{name}: scale should be > 0.4 (still usable), got {scale}"
        );
    }
}

#[test]
fn ui_scale_on_phones_is_below_tablets() {
    let tablet = compute_ui_scale(IPAD_AIR_LANDSCAPE.0, IPAD_AIR_LANDSCAPE.1);
    for (name, (w, h)) in [
        ("iPhone 14 landscape", IPHONE_14_LANDSCAPE),
        ("Pixel 7 landscape", PIXEL_7_LANDSCAPE),
    ] {
        let phone = compute_ui_scale(w, h);
        assert!(
            phone < tablet,
            "{name} scale ({phone}) should be smaller than iPad Air ({tablet})"
        );
        assert!(phone > 0.0, "{name}: scale must be positive, got {phone}");
    }
}

#[test]
fn ui_scale_portrait_phone_uses_narrow_dimension() {
    let portrait = compute_ui_scale(IPHONE_14_PORTRAIT.0, IPHONE_14_PORTRAIT.1);
    let landscape = compute_ui_scale(IPHONE_14_LANDSCAPE.0, IPHONE_14_LANDSCAPE.1);
    assert!(
        portrait < landscape,
        "portrait scale ({portrait}) should be smaller than landscape ({landscape})"
    );
}

#[test]
fn ui_scale_is_width_over_design_when_height_is_ample() {
    let (w, h) = IPAD_AIR_LANDSCAPE;
    let expected_w = w / DESIGN_WIDTH;
    let expected_h = h / DESIGN_HEIGHT;
    let scale = compute_ui_scale(w, h);
    let expected = expected_w.min(expected_h).min(1.0);
    assert!(
        (scale - expected).abs() < 0.001,
        "expected {expected}, got {scale}"
    );
}

// --------------------------------------------------------------------------
// compute_fit_scale (camera zoom to fit grid in viewport)
// --------------------------------------------------------------------------

fn default_grid_world_size() -> (f32, f32) {
    (
        GRID_WIDTH as f32 * TILE_SIZE,
        GRID_HEIGHT as f32 * TILE_SIZE,
    )
}

#[test]
fn fit_scale_at_design_resolution() {
    let (gw, gh) = default_grid_world_size();
    let (top_bar, top_nav, site_tabs, sidebar) = ui_layout_constants();
    let header = top_bar + top_nav + site_tabs;
    let vw = DESIGN_RES.0 - sidebar;
    let vh = DESIGN_RES.1 - header;
    let fit = compute_fit_scale(gw, gh, vw, vh);
    assert!(
        fit > 0.0,
        "fit scale must be positive at design res, got {fit}"
    );
}

#[test]
fn fit_scale_grows_for_smaller_viewports() {
    let (gw, gh) = default_grid_world_size();

    let fit_large = compute_fit_scale(gw, gh, 1200.0, 750.0);
    let fit_small = compute_fit_scale(gw, gh, 600.0, 350.0);
    assert!(
        fit_small > fit_large,
        "smaller viewport should need a larger fit-scale (more zoom out); small={fit_small}, large={fit_large}"
    );
}

#[test]
fn fit_scale_never_below_floor() {
    let fit = compute_fit_scale(1.0, 1.0, 10_000.0, 10_000.0);
    assert!(fit >= 0.01, "fit scale floor is 0.01, got {fit}");
}

#[test]
fn fit_scale_handles_zero_viewport_gracefully() {
    let fit = compute_fit_scale(500.0, 500.0, 0.0, 0.0);
    assert!(fit >= 0.01, "zero viewport should not panic, got {fit}");
}

// --------------------------------------------------------------------------
// Camera viewport math at mobile resolutions
// --------------------------------------------------------------------------

/// Simulates the viewport calculation from `update_world_camera_layout` for a
/// given window size and UI scale.  Returns (viewport_w, viewport_h, fit_scale).
fn simulate_camera_viewport(window_w: f32, window_h: f32) -> (f32, f32, f32) {
    let scale = compute_ui_scale(window_w, window_h);
    let (top_bar, top_nav, site_tabs, sidebar) = ui_layout_constants();
    let header = (top_bar + top_nav + site_tabs) * scale;
    let sidebar_scaled = sidebar * scale;

    let vw = (window_w - sidebar_scaled).max(1.0);
    let vh = (window_h - header).max(1.0);

    let (gw, gh) = default_grid_world_size();
    let fit = compute_fit_scale(gw, gh, vw, vh);

    (vw, vh, fit)
}

#[test]
fn viewport_has_positive_area_on_all_target_devices() {
    let devices = [
        ("iPhone 14 landscape", IPHONE_14_LANDSCAPE),
        ("iPhone 14 Pro Max landscape", IPHONE_14_PRO_MAX_LANDSCAPE),
        ("Pixel 7 landscape", PIXEL_7_LANDSCAPE),
        ("iPad Mini landscape", IPAD_MINI_LANDSCAPE),
        ("iPad Air landscape", IPAD_AIR_LANDSCAPE),
        ("iPhone 14 portrait", IPHONE_14_PORTRAIT),
        ("Desktop 1080p", DESKTOP_1080P),
        ("Design resolution", DESIGN_RES),
    ];
    for (name, (w, h)) in devices {
        let (vw, vh, fit) = simulate_camera_viewport(w, h);
        assert!(
            vw > 0.0 && vh > 0.0,
            "{name}: viewport must be positive, got {vw}x{vh}"
        );
        assert!(fit > 0.0, "{name}: fit scale must be positive, got {fit}");
    }
}

#[test]
fn sidebar_does_not_consume_entire_width_on_phones() {
    for (name, (w, h)) in [
        ("iPhone 14 landscape", IPHONE_14_LANDSCAPE),
        ("Pixel 7 landscape", PIXEL_7_LANDSCAPE),
    ] {
        let (vw, _vh, _) = simulate_camera_viewport(w, h);
        let game_ratio = vw / w;
        assert!(
            game_ratio > 0.4,
            "{name}: game viewport should be > 40% of screen width, got {:.0}% ({vw:.0}/{w:.0})",
            game_ratio * 100.0
        );
    }
}

#[test]
fn header_does_not_consume_entire_height_on_phones() {
    for (name, (w, h)) in [
        ("iPhone 14 landscape", IPHONE_14_LANDSCAPE),
        ("Pixel 7 landscape", PIXEL_7_LANDSCAPE),
    ] {
        let (_vw, vh, _) = simulate_camera_viewport(w, h);
        let game_ratio = vh / h;
        assert!(
            game_ratio > 0.5,
            "{name}: game viewport should be > 50% of screen height, got {:.0}% ({vh:.0}/{h:.0})",
            game_ratio * 100.0
        );
    }
}

#[test]
fn tablet_viewport_is_larger_than_phone_viewport() {
    let (phone_vw, phone_vh, _) =
        simulate_camera_viewport(IPHONE_14_LANDSCAPE.0, IPHONE_14_LANDSCAPE.1);
    let (tab_vw, tab_vh, _) = simulate_camera_viewport(IPAD_AIR_LANDSCAPE.0, IPAD_AIR_LANDSCAPE.1);
    assert!(
        tab_vw > phone_vw,
        "tablet viewport width should exceed phone"
    );
    assert!(
        tab_vh > phone_vh,
        "tablet viewport height should exceed phone"
    );
}
