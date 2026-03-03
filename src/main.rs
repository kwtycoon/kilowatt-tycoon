//! Kilowatt Tycoon - Main entry point
//!
//! A 2D top-down simulation/tycoon game where the player operates
//! an EV charging network.
//!
//! ## Command-line Options
//!
//! - `--screenshot`: Capture screenshots of all levels to `spec/levels/`

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::winit::WinitSettings;
use kilowatt_tycoon::ChargeOpsPlugin;
use kilowatt_tycoon::systems::ScreenshotMode;

fn main() {
    // WASM: install panic hook so panics show in the browser console
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let screenshot_mode = args.iter().any(|arg| arg == "--screenshot");

    if screenshot_mode {
        println!("Screenshot mode enabled - will capture all levels");
    }

    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Kilowatt Tycoon".to_string(),
                    resolution: (1600_u32, 900_u32).into(),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            }),
    )
    .add_plugins(ChargeOpsPlugin);

    // WASM: use reactive update mode to avoid spinning the event loop far beyond
    // the browser's ~60 Hz refresh rate, which wastes CPU for no visual benefit.
    // Native: use the default game() preset (continuous when focused, reactive when not).
    #[cfg(target_arch = "wasm32")]
    app.insert_resource(WinitSettings {
        focused_mode: bevy::winit::UpdateMode::reactive(std::time::Duration::from_secs_f32(
            1.0 / 60.0,
        )),
        unfocused_mode: bevy::winit::UpdateMode::reactive(std::time::Duration::from_secs_f32(
            1.0 / 30.0,
        )),
    });
    #[cfg(not(target_arch = "wasm32"))]
    app.insert_resource(WinitSettings::game());

    // Enable screenshot mode if requested
    if screenshot_mode {
        app.insert_resource(ScreenshotMode {
            enabled: true,
            ..default()
        });
    }

    app.run();
}
