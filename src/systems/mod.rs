//! ECS Systems for game logic
//!
//! Systems are organized into sets that run in a specific order during the Playing state.
//! Most game systems are gated by `in_state(AppState::Playing)` to ensure they only run
//! during active gameplay.

pub mod achievements;
pub mod actions;
pub mod ambient_traffic;
pub mod build_input;
pub mod charger;
pub mod demand_warnings;
pub mod driver;
pub mod emotion;
pub mod environment;
pub mod fleet;
pub mod gameplay_tips;
pub mod hacker;
pub mod interaction;
pub mod northstar_movement;
pub mod power;
pub mod power_dispatch;
pub mod robber;
pub mod scenario_events;
pub mod scene;
pub mod screenshot;
pub mod site_roots;
pub mod site_switching;
pub mod site_visibility;
pub mod sprite;
pub mod technician;
pub mod ticket;
pub mod tiled_maps;
pub mod time;
pub mod utility_billing;
pub mod win_lose;

#[cfg(target_arch = "wasm32")]
pub mod test_bridge;

use bevy::camera;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::helpers::CameraController;
use crate::resources::{
    GRID_HEIGHT, GRID_OFFSET_X, GRID_OFFSET_Y, GRID_WIDTH, GameClock, TILE_SIZE,
};
use crate::states::{
    AppState, check_game_over_transition, is_game_visible, is_playing, is_station_open,
};

pub use achievements::*;
pub use actions::*;
pub use ambient_traffic::*;
pub use build_input::*;
pub use charger::*;
pub use driver::*;
pub use emotion::*;
pub use environment::*;
pub use fleet::*;
pub use hacker::*;
pub use interaction::*;
pub use northstar_movement::*;
pub use power::*;
pub use power_dispatch::*;
pub use robber::*;
pub use scenario_events::*;
pub use scene::*;
pub use screenshot::*;
pub use site_roots::*;
pub use site_switching::*;
pub use site_visibility::*;
pub use sprite::*;
pub use technician::*;
pub use ticket::*;
pub use tiled_maps::*;
pub use time::*;
pub use utility_billing::*;
pub use win_lose::*;

/// System sets for ordering
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSystemSet {
    TimeUpdate,
    Environment,
    BuildInput,
    Input,
    DriverSpawn,
    /// Vehicle movement using bevy_northstar pathfinding
    MovementUpdate,
    ChargerUpdate,
    PowerDispatch,
    ChargingUpdate,
    PatienceUpdate,
    TicketUpdate,
    PowerUpdate,
    UtilityBilling,
    ActionExecution,
    WinLoseCheck,
    SpriteUpdate,
    UiUpdate,
}

/// Plugin that registers all game systems
pub struct SystemsPlugin;

impl Plugin for SystemsPlugin {
    fn build(&self, app: &mut App) {
        // Initialize revision tracking resources for grid change detection
        app.init_resource::<scene::ChargerSyncRevision>()
            .init_resource::<scene::GridVisualRevision>()
            .init_resource::<scene::TransformerSyncRevision>()
            .init_resource::<site_roots::ActivePathfindingGrid>();

        // Configure system set ordering
        // Simulation sets that require station to be open (time advances, drivers spawn, etc.)
        // These only run when in Playing state AND station is open for business
        app.configure_sets(
            Update,
            (
                GameSystemSet::TimeUpdate,
                GameSystemSet::Environment,
                GameSystemSet::DriverSpawn,
                GameSystemSet::ChargerUpdate,
                GameSystemSet::PowerDispatch,
                GameSystemSet::ChargingUpdate,
                GameSystemSet::PatienceUpdate,
                GameSystemSet::TicketUpdate,
                GameSystemSet::PowerUpdate,
                GameSystemSet::UtilityBilling,
                GameSystemSet::ActionExecution,
                GameSystemSet::WinLoseCheck,
            )
                .chain()
                .run_if(in_state(AppState::Playing).and(is_station_open)),
        );

        // Build phase systems - run during Playing state even when station is not yet open
        // This allows players to build and configure before opening
        app.configure_sets(
            Update,
            (GameSystemSet::BuildInput, GameSystemSet::MovementUpdate)
                .chain()
                .after(GameSystemSet::Environment)
                .run_if(in_state(AppState::Playing)),
        );

        // Input, Sprite, and UI sets run when game is visible (Playing, Paused, or GameOver)
        // This allows speed buttons and pause controls to work during pause
        app.configure_sets(
            Update,
            (
                GameSystemSet::Input,
                GameSystemSet::SpriteUpdate,
                GameSystemSet::UiUpdate,
            )
                .chain()
                .after(GameSystemSet::WinLoseCheck)
                .run_if(is_game_visible),
        );

        // Add systems to their sets (all gated by Playing state via set config)
        app.add_systems(
            Update,
            (time_system, tick_demand_boosts).in_set(GameSystemSet::TimeUpdate),
        );

        // Environment systems
        app.add_systems(
            Update,
            (
                environment_system,
                log_environment_changes,
                rf_environment_system,
            )
                .in_set(GameSystemSet::Environment),
        );

        // Build mode input systems
        app.add_systems(
            Update,
            (
                build_placement_system,
                update_placement_cursor,
                update_sell_cursor,
                build_keyboard_shortcuts,
                crate::resources::strategy::sync_amenity_from_grid,
            )
                .in_set(GameSystemSet::BuildInput),
        );

        // Input systems
        app.add_systems(
            Update,
            (click_to_select_charger, keyboard_shortcuts).in_set(GameSystemSet::Input),
        );

        // Spawn systems (driver/transformer spawning requires station to be open)
        app.add_systems(
            Update,
            (
                driver_spawn_system,
                driver_arrival_system,
                ambient_to_customer_system,
                fleet::fleet_spawn_system,
            )
                .in_set(GameSystemSet::DriverSpawn),
        );

        // Charger entity sync - runs during Playing state regardless of is_station_open
        // This ensures chargers exist as entities immediately when placed (build mode or during day)
        // IMPORTANT: Must run AFTER build_placement_system to see the updated grid state
        app.add_systems(
            Update,
            sync_chargers_with_grid
                .in_set(GameSystemSet::BuildInput)
                .after(build_placement_system)
                .before(GameSystemSet::SpriteUpdate),
        );

        // Transformer entity sync - same pattern as charger sync above
        app.add_systems(
            Update,
            sync_transformers_with_grid
                .in_set(GameSystemSet::BuildInput)
                .after(build_placement_system)
                .before(GameSystemSet::SpriteUpdate),
        );

        // Clean up stale references (selection, technician jobs, dispatch queue) when a
        // charger entity is despawned (e.g. sold). Runs after charger sync so it sees the
        // removed entities and before movement/action systems consume the references.
        app.add_systems(
            Update,
            cleanup_sold_charger_references
                .in_set(GameSystemSet::BuildInput)
                .after(sync_chargers_with_grid),
        );

        // Vehicle and technician movement using bevy_northstar pathfinding
        // NorthstarPlugin handles pathfinding, these systems handle:
        // - Smooth visual movement between grid positions
        // - Arrival/departure state transitions
        // - Vehicle and technician cleanup
        // - Reroute failure handling with cooldown
        app.add_systems(
            Update,
            (
                northstar_movement::northstar_move_vehicles,
                northstar_movement::northstar_arrival_detection,
                northstar_movement::northstar_trigger_departure,
                northstar_movement::northstar_cleanup_exited,
                northstar_movement::northstar_cleanup_ambient,
                northstar_movement::northstar_handle_pathfinding_failed,
                northstar_movement::northstar_handle_reroute_failed,
                northstar_movement::northstar_clear_cooldown_on_success,
                technician_movement_system,
                technician_arrival_detection,
                cleanup_exited_technicians,
            )
                .in_set(GameSystemSet::MovementUpdate)
                .after(GameSystemSet::DriverSpawn),
        );

        // Robber movement and lifecycle — runs during Playing state
        // Robbers walk in straight lines (no pathfinding), so they don't need MovementUpdate
        app.add_systems(
            Update,
            (
                robber::robber_movement_system,
                robber::robber_arrival_system,
                robber::robber_stealing_system,
                robber::robber_flee_arrival_system,
                robber::robber_cleanup_system,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );

        // Hacker movement and lifecycle — mirrors robber lifecycle
        app.add_systems(
            Update,
            (
                hacker::hacker_movement_system,
                hacker::hacker_arrival_system,
                hacker::hacker_attack_system,
                hacker::hacker_flee_arrival_system,
                hacker::hacker_cleanup_system,
            )
                .chain()
                .run_if(in_state(AppState::Playing)),
        );

        // Ambient traffic systems - only run when day is active
        // Note: cleanup_ambient_sprites was removed - cleanup is now unified in cleanup_driver_sprites
        app.add_systems(
            Update,
            (
                spawn_ambient_traffic,
                update_interested_vehicles,
                update_ambient_sprite_positions,
            )
                .run_if(in_state(AppState::Playing).and(is_station_open)),
        );

        // Fleet contract offer banner (non-blocking accept/decline prompt)
        app.add_systems(
            Update,
            (
                fleet::spawn_fleet_offer_banner,
                fleet::fleet_offer_interaction_system,
            )
                .run_if(in_state(AppState::Playing)),
        );

        // Scenario scripted events (monsoon floods, brownouts, swap competitor, etc.)
        app.add_systems(
            Update,
            scenario_events::scenario_event_system.in_set(GameSystemSet::ChargerUpdate),
        );

        // Core game systems
        app.add_systems(
            Update,
            (
                charger_state_system,
                charger_cooldown_system,
                scripted_fault_system,
                stochastic_fault_system,
                guaranteed_day1_technician_fault_system,
                // Cable theft system - random robber spawns (higher at night)
                robber::cable_theft_system,
                // Hacker spawn system - random cyber-attacks (higher at night)
                hacker::hacker_spawn_system,
                hacker::debug_spawn_hacker,
                // Hacker effect tick - countdown overload and price slash timers
                hacker::hacker_effect_tick_system,
                // Fault detection runs after faults are injected - handles detection delay
                // and auto-remediation based on OEM tier
                fault_detection_system,
                kick_drivers_from_faulted_chargers,
                // Reliability degradation runs every frame while chargers are faulted
                reliability_degradation_system,
            )
                .chain()
                .in_set(GameSystemSet::ChargerUpdate),
        );
        app.add_systems(
            Update,
            power_dispatch_system.in_set(GameSystemSet::PowerDispatch),
        );
        app.add_systems(
            Update,
            (charging_system, queue_assignment_system).in_set(GameSystemSet::ChargingUpdate),
        );
        app.add_systems(
            Update,
            (
                patience_system,
                frustrated_driver_system,
                init_driver_emotions,
                evaluate_driver_emotions,
                sync_mood_with_emotion,
                fleet::fleet_sla_system,
            )
                .in_set(GameSystemSet::PatienceUpdate),
        );
        app.add_systems(
            Update,
            ticket_sla_system.in_set(GameSystemSet::TicketUpdate),
        );
        app.add_systems(
            Update,
            (
                power_system,
                update_transformer_overload_fire_state,
                dispatch_firetrucks_to_transformer_fires,
                update_emergency_firetruck_response,
                sync_transformer_fire_vfx,
                animate_transformer_fire_vfx,
                update_water_spray_vfx,
            )
                .chain()
                .in_set(GameSystemSet::PowerUpdate),
        );
        app.add_systems(
            Update,
            (
                grid_event_system,
                utility_billing_system,
                demand_warnings::monitor_demand_warnings,
            )
                .chain()
                .in_set(GameSystemSet::UtilityBilling),
        );
        app.add_systems(
            Update,
            (
                action_system,
                handle_oem_upgrade_existing_faults,
                om_auto_dispatch_system,
                dispatch_technician_system,
                technician_travel_system,
                technician_repair_system,
            )
                .in_set(GameSystemSet::ActionExecution),
        );
        app.add_systems(Update, win_lose_system.in_set(GameSystemSet::WinLoseCheck));
        app.add_systems(
            Update,
            check_achievements.in_set(GameSystemSet::WinLoseCheck),
        );

        // Day ending wind-down system: runs after all simulation sets to monitor
        // whether drivers have finished and exited, then triggers DayEnd transition.
        app.add_systems(
            Update,
            day_ending_system
                .after(GameSystemSet::WinLoseCheck)
                .run_if(in_state(AppState::Playing).and(is_station_open)),
        );

        // Sprite systems - run when game is visible (Playing, Paused, or GameOver)
        // All rendering uses SVG assets - no fallback sprites
        // Note: run_if(is_game_visible) is set at the set level, not here
        // Note: spawn_charger_sprites was removed - ChargerSprite is now spawned
        // in sync_chargers_with_grid (scene.rs) to avoid deferred command timing issues
        app.add_systems(
            Update,
            (
                (
                    spawn_vehicle_sprites,
                    update_charger_sprites,
                    update_vehicle_positions,
                    update_driver_mood_sprites,
                    update_soc_indicators,
                    cleanup_driver_sprites,
                    update_floating_money,
                    update_floating_wrench,
                    spawn_fault_pulse_vfx,
                    spawn_fault_pulse_on_fault,
                    update_fault_pulse_vfx,
                ),
                (
                    spawn_broken_charger_icons,
                    update_broken_charger_icon_positions,
                    spawn_frustration_indicators,
                    update_frustration_indicator_positions,
                    spawn_technician_sprite,
                    update_technician_sprite,
                    animate_technician_working,
                    spawn_power_throttle_icons,
                    update_power_throttle_icon_positions,
                    sync_anti_theft_shield_indicators,
                    sync_ad_screen_overlays,
                    update_world_gif_animations,
                    animate_security_camera_swivel,
                    animate_security_camera_led,
                    update_security_alert_bubble,
                ),
                (
                    update_theft_alarm_vfx,
                    update_stealing_spark_vfx,
                    update_robber_loot_bubble,
                    update_stolen_cable_sprite,
                    update_hacking_glitch_vfx,
                    update_hacker_loot_bubble,
                    fleet::fleet_visual_system,
                    fleet::toggle_fleet_debug,
                    fleet::fleet_debug_label_sync,
                ),
                update_grid_visuals,
                // Update infrastructure gauge bars
                update_transformer_gauges,
                update_solar_generation_bar,
                update_battery_soc_bar,
            )
                .in_set(GameSystemSet::SpriteUpdate),
        );

        // Site root spawning - runs when game is visible to create root entities for new sites
        // Must run BEFORE DriverSpawn and other systems that need site root entities
        app.add_systems(
            Update,
            spawn_missing_site_roots
                .run_if(is_game_visible)
                .before(GameSystemSet::TimeUpdate),
        );

        // Rebuild pathfinding grids when SiteGrid changes (e.g., chargers placed)
        // Must run BEFORE bevy_northstar's PathingSet so paths use up-to-date nav data
        app.add_systems(
            Update,
            rebuild_site_pathfinding_grids
                .run_if(is_game_visible)
                .after(spawn_missing_site_roots)
                .before(bevy_northstar::prelude::PathingSet),
        );

        // Site roots are now spawned in OnEnter(CharacterSetup) to make map visible during character selection
        // app.add_systems(OnEnter(AppState::Playing), spawn_missing_site_roots);

        // Tiled map spawning - spawns Tiled maps for sites after site roots exist
        app.add_systems(
            Update,
            (
                tiled_maps::spawn_site_tiled_maps,
                tiled_maps::sync_tiled_map_visibility,
                tiled_maps::despawn_sold_site_tiled_maps,
            )
                .chain()
                .run_if(is_game_visible)
                .after(spawn_missing_site_roots),
        );

        // Site switching and visibility systems - run when game is visible
        app.add_systems(
            Update,
            (
                handle_site_switch,
                cleanup_sold_site,
                update_site_entity_visibility,
                update_camera_for_site,
            )
                .chain()
                .in_set(GameSystemSet::Input)
                .run_if(is_game_visible),
        );

        // Transfer pathfinding grid when switching sites
        // Must run after handle_site_switch updates the viewed site, before pathfinding
        app.add_systems(
            Update,
            transfer_pathfinding_grid_on_site_switch
                .run_if(is_game_visible)
                .after(handle_site_switch)
                .before(bevy_northstar::prelude::PathingSet),
        );

        // Game over transition check - runs during Playing state
        app.add_systems(Update, check_game_over_transition.run_if(is_playing));

        // Startup systems - setup_camera first, spawn_grid_background after svg assets loaded
        app.add_systems(Startup, setup_camera);
        app.add_systems(
            Startup,
            spawn_grid_background.after(crate::resources::load_image_assets),
        );

        // Scale all Bevy UI proportionally when the window is smaller than the 1600x900 design
        // resolution (e.g. tablets). Must run before update_world_camera_layout so the camera
        // viewport uses the already-updated scale.
        app.add_systems(Update, update_ui_scale);

        // Keep world camera viewport + zoom-out limit synced with window size/state.
        // Important: this must run in *all* states so the menu/splash camera isn't broken.
        app.add_systems(Update, update_world_camera_layout.after(update_ui_scale));

        // Day/night cycle: set world camera clear color from game time (6am–6pm day, else night).
        app.add_systems(Update, day_night_clear_color_system.run_if(is_game_visible));

        // Screenshot automation systems
        app.init_resource::<ScreenshotMode>();
        app.add_systems(
            Update,
            screenshot_skip_menu_system.run_if(screenshot_mode_enabled),
        );
        app.add_systems(
            OnEnter(AppState::Loading),
            screenshot_skip_character_setup.run_if(screenshot_mode_enabled),
        );
        app.add_systems(
            OnEnter(AppState::Playing),
            screenshot_init_system.run_if(screenshot_mode_enabled),
        );
        app.add_systems(
            Update,
            screenshot_automation_system
                .run_if(screenshot_mode_enabled)
                .run_if(in_state(AppState::Playing)),
        );

        // Test bridge: exposes game state + element positions to JS for Playwright.
        #[cfg(target_arch = "wasm32")]
        {
            app.init_resource::<test_bridge::TestBridgeState>();
            app.add_systems(PostUpdate, test_bridge::update_test_bridge);
        }
    }
}

/// Marker for the camera that renders the world/grid.
#[derive(Component)]
pub struct WorldCamera;

/// Clear color for day (6:00–17:59). Matches green used elsewhere (e.g. states/mod.rs).
const DAY_CLEAR_COLOR: Color = Color::srgb(0.2, 0.6, 0.2);
/// Clear color for night (18:00–5:59). Indigo #4B0082.
const NIGHT_CLEAR_COLOR: Color = Color::srgb(75.0 / 255.0, 0.0, 130.0 / 255.0);

/// Updates the world camera clear color based on game time: day (6am–6pm) green, night purple.
fn day_night_clear_color_system(
    game_clock: Res<GameClock>,
    mut cameras: Query<&mut Camera, With<WorldCamera>>,
) {
    let h = game_clock.hour();
    let color = if (6..18).contains(&h) {
        DAY_CLEAR_COLOR
    } else {
        NIGHT_CLEAR_COLOR
    };
    for mut camera in &mut cameras {
        camera.clear_color = ClearColorConfig::Custom(color);
    }
}

/// Design resolution the UI was authored for. UiScale is computed relative to this.
pub const DESIGN_WIDTH: f32 = 1600.0;
pub const DESIGN_HEIGHT: f32 = 900.0;

/// Pure math for UI scale: shrinks proportionally on screens smaller than the design
/// resolution, capped at 1.0 so desktop users see no upscale.
pub fn compute_ui_scale(window_width: f32, window_height: f32) -> f32 {
    (window_width / DESIGN_WIDTH)
        .min(window_height / DESIGN_HEIGHT)
        .min(1.0)
}

/// Keeps `UiScale` in sync with the window size so all Bevy UI shrinks proportionally
/// on screens smaller than the 1600x900 design resolution (e.g. tablets).
/// Never upscales beyond 1.0 so desktop users see no change.
fn update_ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut ui_scale: ResMut<UiScale>) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let w = window.resolution.width();
    let h = window.resolution.height();
    let scale = compute_ui_scale(w, h);
    if (ui_scale.0 - scale).abs() > 0.001 {
        ui_scale.0 = scale;
    }
}

pub fn ui_layout_constants() -> (f32, f32, f32, f32) {
    // These match the HUD layout (logical pixels):
    // - Top bar: 42px (`src/ui/hud.rs`)
    // - Top nav: 50px (`src/ui/top_nav.rs`)
    // - Site tabs: fixed height (`src/ui/site_tabs.rs`)
    // - Sidebar: 340px wide (`src/ui/sidebar/mod.rs`)
    const TOP_BAR_HEIGHT: f32 = 42.0;
    const TOP_NAV_HEIGHT: f32 = 50.0;
    const SITE_TABS_HEIGHT: f32 = 52.0;
    const SIDEBAR_WIDTH: f32 = 340.0;
    (
        TOP_BAR_HEIGHT,
        TOP_NAV_HEIGHT,
        SITE_TABS_HEIGHT,
        SIDEBAR_WIDTH,
    )
}

pub fn compute_fit_scale(
    grid_width: f32,
    grid_height: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> f32 {
    // With our CameraController:
    // - Larger `Transform.scale` == zoom out (see more world)
    // - Smaller `Transform.scale` == zoom in
    //
    // Approximate visible world size:
    //   visible_world_width  ≈ viewport_width  * scale
    //   visible_world_height ≈ viewport_height * scale
    //
    // To fit the full grid, we need scale large enough:
    //   scale >= grid_width  / viewport_width
    //   scale >= grid_height / viewport_height
    let scale_for_width = if viewport_width > 0.0 {
        grid_width / viewport_width
    } else {
        1.0
    };
    let scale_for_height = if viewport_height > 0.0 {
        grid_height / viewport_height
    } else {
        1.0
    };
    scale_for_width.max(scale_for_height).max(0.01)
}

fn update_world_camera_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    app_state: Res<State<AppState>>,
    mut cameras: Query<(&mut Camera, &mut Transform, &mut CameraController), With<WorldCamera>>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    ui_scale: Res<UiScale>,
    mut last_state: Local<Option<AppState>>,
    mut last_fit_scale: Local<Option<f32>>,
    mut last_site_id: Local<Option<crate::resources::SiteId>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };

    let state = *app_state.get();
    let is_gameplay_view = state.is_game_visible();

    // UI chrome only exists during gameplay-visible states.
    // Multiply by UiScale so the camera viewport matches the scaled-down UI on small screens.
    let scale = ui_scale.0;
    let (top_bar_h, top_nav_h, site_tabs_h, sidebar_w) = if is_gameplay_view {
        let (a, b, c, d) = ui_layout_constants();
        (a * scale, b * scale, c * scale, d * scale)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };
    let header_h = top_bar_h + top_nav_h + site_tabs_h;

    // Logical sizes for zoom math
    let window_w = window.resolution.width();
    let window_h = window.resolution.height();
    let viewport_w = (window_w - sidebar_w).max(1.0);
    let viewport_h = (window_h - header_h).max(1.0);

    // Physical sizes for Camera viewport
    let sf = window.scale_factor();
    let sidebar_px = (sidebar_w * sf).round().max(0.0) as u32;
    let header_px = (header_h * sf).round().max(0.0) as u32;
    let viewport_px_w = ((viewport_w * sf).round().max(1.0)) as u32;
    let viewport_px_h = ((viewport_h * sf).round().max(1.0)) as u32;

    // Grid dimensions in world space (use active site's dimensions, fallback to default)
    let active_site_id = multi_site.viewed_site_id;
    let (gw, gh) = multi_site
        .active_site()
        .map(|s| (s.grid.width, s.grid.height))
        .unwrap_or((GRID_WIDTH, GRID_HEIGHT));
    let grid_w = (gw as f32) * TILE_SIZE;
    let grid_h = (gh as f32) * TILE_SIZE;

    // Gameplay: zoom out just enough to fully fit the grid in the viewport.
    // Menu/Loading: keep a neutral scale and no viewport cropping.
    let fit_scale = if is_gameplay_view {
        compute_fit_scale(grid_w, grid_h, viewport_w, viewport_h)
    } else {
        1.0
    };

    // Allow zooming in (smaller scale), but never zoom out beyond "fit" in gameplay.
    let (min_zoom, max_zoom) = if is_gameplay_view {
        ((fit_scale * 0.25).max(0.05), fit_scale)
    } else {
        // Defaults from CameraController::default()
        (0.5, 2.0)
    };

    // Trigger a re-center and re-zoom on state transitions OR when the active site changes.
    let state_changed = last_state.map(|s| s != state).unwrap_or(true);
    let site_changed = *last_site_id != active_site_id;
    let should_reset = state_changed || site_changed;

    for (mut camera, mut transform, mut controller) in &mut cameras {
        // Apply viewport cropping only during gameplay states.
        camera.viewport = if is_gameplay_view {
            Some(camera::Viewport {
                physical_position: UVec2::new(sidebar_px, header_px),
                physical_size: UVec2::new(viewport_px_w, viewport_px_h),
                ..default()
            })
        } else {
            None
        };

        controller.min_zoom = min_zoom;
        controller.max_zoom = max_zoom;

        // On state transitions or site switches, re-center and set fit zoom.
        // Otherwise, leave camera position/zoom alone so player pan/zoom isn't fighting this system.
        if should_reset {
            if is_gameplay_view {
                // Grid center in world space (including offsets)
                let grid_center_x = GRID_OFFSET_X + grid_w / 2.0;
                let grid_center_y = GRID_OFFSET_Y + grid_h / 2.0;
                transform.translation.x = grid_center_x;
                transform.translation.y = grid_center_y;

                controller.current_scale = fit_scale;
                transform.scale = Vec3::splat(controller.current_scale);
            } else {
                // Center on the menu background's intended anchor (see `main_menu.rs` / loading splash).
                // These backgrounds spawn around x=450, y=300 in world pixels.
                transform.translation.x = 450.0;
                transform.translation.y = 300.0;

                controller.current_scale = 1.0;
                transform.scale = Vec3::splat(controller.current_scale);
            }
            *last_state = Some(state);
            *last_site_id = active_site_id;
            *last_fit_scale = Some(fit_scale);
            continue;
        }

        // If the window resized and reduced the max zoom-out, clamp current zoom to the new max.
        if let Some(prev_fit) = *last_fit_scale {
            if is_gameplay_view && (prev_fit - fit_scale).abs() > 0.001 {
                *last_fit_scale = Some(fit_scale);
            }
        } else {
            *last_fit_scale = Some(fit_scale);
        }

        if controller.current_scale > controller.max_zoom {
            controller.current_scale = controller.max_zoom;
            transform.scale = Vec3::splat(controller.current_scale);
        }
    }
}

/// Set up the 2D camera with controller for pan/zoom
fn setup_camera(mut commands: Commands) {
    // World camera: renders ONLY into the non-UI viewport.
    // The viewport, position, and fit zoom are computed/updated by `update_world_camera_layout`.
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        Tonemapping::None,
        WorldCamera,
        // World renders on layer 0 (default). This prevents the UI camera from also rendering the world.
        RenderLayers::layer(0),
        Transform::default(),
        CameraController {
            // Placeholder; first `update_world_camera_layout` will set these correctly.
            current_scale: 1.0,
            ..default()
        },
    ));

    // UI camera: renders UI across the full window, on top of the world.
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::None,
        // UI camera should not render world sprites; it exists to render bevy_ui on top.
        RenderLayers::layer(1),
        IsDefaultUiCamera,
    ));
}
