//! Game state management for controlling game flow.
//!
//! This module implements a state machine for managing different phases of the game:
//! - `MainMenu`: Initial menu screen
//! - `Loading`: Asset loading with progress display
//! - `CharacterSetup`: Character selection and name input
//! - `Playing`: Active gameplay (time can be paused/running, building always allowed)
//! - `Paused`: Game paused (time frozen, UI visible)
//! - `DayEnd`: End of day summary screen
//! - `GameOver`: End screen (win or lose)

pub mod character_setup;
pub mod day_end;
pub mod game_over;
pub mod loading;
pub mod main_menu;
pub mod pause;
pub mod playing;

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

pub use character_setup::*;
use day_end::interactions::{
    auto_submit_score_on_day_end, day_end_system, kpi_toggle_system, linkedin_share_system,
    on_exit_day_end,
};
use day_end::report::prepare_day_end_report;
use day_end::ui::spawn_day_end_ui;
pub use game_over::*;
pub use loading::*;
pub use main_menu::*;
pub use pause::*;
pub use playing::*;

/// Primary application state for controlling game flow.
///
/// # State Transitions
/// ```text
/// MainMenu -> Loading -> CharacterSetup -> Playing <-> Paused
///                                             |
///                                             v
///                                          DayEnd -> Playing (next day)
///                                             |
///                                             v
///                                          GameOver -> MainMenu
/// ```
#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub enum AppState {
    /// Main menu
    MainMenu,
    /// Loading assets and preparing the scene
    #[default]
    Loading,
    /// Character selection and name input
    CharacterSetup,
    /// Active gameplay (time can be paused/running, building always allowed)
    Playing,
    /// Game paused (from Playing state only)
    Paused,
    /// End of day summary screen
    DayEnd,
    /// Game over screen (win or lose)
    GameOver,
}

impl AppState {
    /// Returns true if the game simulation should be running
    pub fn is_simulation_active(&self) -> bool {
        matches!(self, AppState::Playing)
    }

    /// Returns true if we're in a state where the game world is visible
    pub fn is_game_visible(&self) -> bool {
        matches!(
            self,
            AppState::CharacterSetup
                | AppState::Playing
                | AppState::Paused
                | AppState::DayEnd
                | AppState::GameOver
        )
    }

    /// Returns true if we're in a menu state
    pub fn is_menu(&self) -> bool {
        matches!(self, AppState::MainMenu | AppState::GameOver)
    }
}

/// Plugin that sets up the state machine and state-related systems
pub struct StatesPlugin;

impl Plugin for StatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            // Loading systems
            .add_systems(
                OnEnter(AppState::Loading),
                (setup_loading_screen, start_asset_loading),
            )
            .add_systems(OnExit(AppState::Loading), cleanup_loading_screen)
            .add_systems(
                Update,
                (
                    update_loading_progress,
                    populate_template_cache,
                    populate_available_sites,
                    populate_game_data_from_assets,
                    check_loading_complete,
                    update_minimal_loading_indicator,
                )
                    .run_if(in_state(AppState::Loading)),
            )
            // Character setup systems
            .add_systems(
                Update,
                (
                    handle_character_selection,
                    handle_character_card_hover,
                    handle_next_button,
                    handle_name_input,
                    animate_cursor,
                    cycle_placeholder_names,
                    handle_start_button,
                )
                    .run_if(in_state(AppState::Playing).and(resource_exists::<SetupStep>)),
            )
            // Playing state
            .add_systems(OnEnter(AppState::Playing), on_enter_playing)
            .add_systems(OnExit(AppState::Playing), on_exit_playing)
            // Paused state
            .add_systems(OnEnter(AppState::Paused), on_enter_paused)
            .add_systems(OnExit(AppState::Paused), on_exit_paused)
            .add_systems(Update, pause_menu_system.run_if(in_state(AppState::Paused)))
            // DayEnd state
            .add_systems(
                OnEnter(AppState::DayEnd),
                (
                    (
                        cleanup_entities_on_day_end,
                        cleanup_hacker_entities_on_day_end,
                        crate::systems::cleanup_technicians_on_day_end,
                    ),
                    prepare_day_end_report,
                    spawn_day_end_ui,
                    auto_submit_score_on_day_end,
                )
                    .chain(),
            )
            .add_systems(OnExit(AppState::DayEnd), on_exit_day_end)
            .add_systems(
                Update,
                (day_end_system, kpi_toggle_system, linkedin_share_system)
                    .run_if(in_state(AppState::DayEnd)),
            )
            // Global UI scroll
            .init_resource::<TouchScrollState>()
            .add_systems(Update, (ui_scroll_system, ui_touch_scroll_system))
            // GameOver state
            .add_systems(OnEnter(AppState::GameOver), setup_game_over)
            .add_systems(OnExit(AppState::GameOver), cleanup_game_over)
            .add_systems(
                Update,
                game_over_system.run_if(in_state(AppState::GameOver)),
            )
            // Pause toggle works in both Playing and Paused states
            .add_systems(
                Update,
                toggle_pause.run_if(in_state(AppState::Playing).or(in_state(AppState::Paused))),
            );
    }
}

/// Marker component for main menu UI elements
#[derive(Component)]
pub struct MainMenuUI;

/// Marker component for character setup UI elements
#[derive(Component)]
pub struct CharacterSetupUI;

/// Marker component for day end UI elements
#[derive(Component)]
pub struct DayEndUI;

/// Continue to next day button marker for day end modal
#[derive(Component, Debug, Clone, Copy)]
pub struct DayEndContinueButton;

/// Toggle button between collapsed and expanded KPI views
#[derive(Component, Debug, Clone, Copy)]
pub struct KpiToggleButton;

/// Marker for the scrollable body of the day-end modal (used by test bridge).
#[derive(Component, Debug, Clone, Copy)]
pub struct DayEndScrollBody;

// ---------------------------------------------------------------------------
// Day-end cleanup systems (remain here since they don't belong to a specific
// sub-feature -- they clean up entities from many different systems)
// ---------------------------------------------------------------------------

/// Despawn all vehicles, robbers, and sprites when entering DayEnd.
fn cleanup_entities_on_day_end(
    mut commands: Commands,
    drivers: Query<Entity, With<crate::components::driver::Driver>>,
    ambient_vehicles: Query<Entity, With<crate::systems::ambient_traffic::AmbientVehicle>>,
    vehicle_sprites: Query<Entity, With<crate::systems::sprite::VehicleSprite>>,
    character_sprites: Query<Entity, With<crate::systems::sprite::DriverCharacterSprite>>,
    robbers: Query<Entity, With<crate::components::robber::Robber>>,
    theft_alarms: Query<Entity, With<crate::systems::sprite::TheftAlarmVfx>>,
    stealing_sparks: Query<Entity, With<crate::systems::sprite::StealingSparkVfx>>,
    loot_bubbles: Query<Entity, With<crate::systems::sprite::RobberLootBubble>>,
    stolen_cables: Query<Entity, With<crate::systems::sprite::StolenCableSprite>>,
    firetrucks: Query<Entity, With<crate::systems::power::EmergencyFiretruck>>,
    fire_vfx: Query<Entity, With<crate::systems::power::PixelFire>>,
    water_vfx: Query<Entity, With<crate::systems::power::TransformerWaterVfx>>,
) {
    for entity in &drivers {
        commands.entity(entity).try_despawn();
    }
    for entity in &ambient_vehicles {
        commands.entity(entity).try_despawn();
    }
    for entity in &vehicle_sprites {
        commands.entity(entity).try_despawn();
    }
    for entity in &character_sprites {
        commands.entity(entity).try_despawn();
    }
    for entity in &robbers {
        commands.entity(entity).try_despawn();
    }
    for entity in &theft_alarms {
        commands.entity(entity).try_despawn();
    }
    for entity in &stealing_sparks {
        commands.entity(entity).try_despawn();
    }
    for entity in &loot_bubbles {
        commands.entity(entity).try_despawn();
    }
    for entity in &stolen_cables {
        commands.entity(entity).try_despawn();
    }
    for entity in &firetrucks {
        commands.entity(entity).try_despawn();
    }
    for entity in &fire_vfx {
        commands.entity(entity).try_despawn();
    }
    for entity in &water_vfx {
        commands.entity(entity).try_despawn();
    }
}

/// Despawn hacker entities and cancel active hacker effects at day end.
fn cleanup_hacker_entities_on_day_end(
    mut commands: Commands,
    hackers: Query<Entity, With<crate::components::hacker::Hacker>>,
    hacker_glitch_vfx: Query<Entity, With<crate::systems::sprite::HackingGlitchVfx>>,
    hacker_loot_bubbles: Query<Entity, With<crate::systems::sprite::HackerLootBubble>>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
) {
    for entity in &hackers {
        commands.entity(entity).try_despawn();
    }
    for entity in &hacker_glitch_vfx {
        commands.entity(entity).try_despawn();
    }
    for entity in &hacker_loot_bubbles {
        commands.entity(entity).try_despawn();
    }

    for (_site_id, site_state) in multi_site.owned_sites.iter_mut() {
        site_state.hacker_overload_remaining_secs = 0.0;
        site_state.service_strategy.pricing.hacker_price_override = None;
        site_state
            .service_strategy
            .pricing
            .hacker_price_override_remaining_secs = 0.0;
    }
}

// ---------------------------------------------------------------------------
// Global UI scroll systems
// ---------------------------------------------------------------------------

const SCROLL_LINE_HEIGHT: f32 = 21.0;

fn ui_scroll_system(
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut scrollable: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
) {
    let mut total_dy = 0.0_f32;
    for ev in mouse_wheel.read() {
        let dy = match ev.unit {
            MouseScrollUnit::Line => -ev.y * SCROLL_LINE_HEIGHT,
            MouseScrollUnit::Pixel => -ev.y,
        };
        total_dy += dy;
    }

    if total_dy == 0.0 {
        return;
    }

    for (mut scroll_pos, node, computed) in scrollable.iter_mut() {
        if node.overflow.y != OverflowAxis::Scroll {
            continue;
        }
        let content_h = computed.content_size().y;
        let visible_h = computed.size().y;
        let scale = computed.inverse_scale_factor();
        let max_scroll = ((content_h - visible_h) * scale).max(0.0);
        scroll_pos.y = (scroll_pos.y + total_dy).clamp(0.0, max_scroll);
    }
}

/// Tracks pointer-drag scroll gestures over UI scroll containers.
#[derive(Resource, Default)]
pub struct TouchScrollState {
    prev_y: Option<f32>,
    active_entity: Option<Entity>,
    cumulative_drag: f32,
    pub scrolling: bool,
}

const TOUCH_SCROLL_DEAD_ZONE: f32 = 8.0;

fn ui_touch_scroll_system(
    mut pointer: ResMut<crate::helpers::pointer::GamePointer>,
    mut state: ResMut<TouchScrollState>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut scrollable: Query<(
        Entity,
        &mut ScrollPosition,
        &Node,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
) {
    if !pointer.pressed && !pointer.just_pressed {
        if state.scrolling {
            pointer.just_released = false;
        }
        *state = TouchScrollState::default();
        return;
    }

    let Some(pos) = pointer.screen_position else {
        *state = TouchScrollState::default();
        return;
    };

    let dpr = windows
        .single()
        .map(|w: &Window| w.scale_factor())
        .unwrap_or(1.0);

    if pointer.just_pressed {
        let mut best: Option<(Entity, f32)> = None;
        for (entity, _scroll, node, computed, ugt) in scrollable.iter() {
            if node.overflow.y != OverflowAxis::Scroll {
                continue;
            }
            let size = computed.size() / dpr;
            let center = ugt.translation / dpr;
            let half = size * 0.5;
            let min = center - half;
            let max = center + half;

            if pos.x >= min.x && pos.x <= max.x && pos.y >= min.y && pos.y <= max.y {
                let area = size.x * size.y;
                if best.is_none_or(|(_, a)| area < a) {
                    best = Some((entity, area));
                }
            }
        }

        *state = TouchScrollState {
            prev_y: Some(pos.y),
            active_entity: best.map(|(e, _)| e),
            cumulative_drag: 0.0,
            scrolling: false,
        };
        return;
    }

    if state.scrolling {
        pointer.just_pressed = false;
        pointer.just_released = false;
    }

    let Some(prev_y) = state.prev_y else {
        return;
    };
    let Some(target) = state.active_entity else {
        state.prev_y = Some(pos.y);
        return;
    };

    let raw_dy = pos.y - prev_y;
    state.cumulative_drag += raw_dy;
    state.prev_y = Some(pos.y);

    if !state.scrolling && state.cumulative_drag.abs() < TOUCH_SCROLL_DEAD_ZONE {
        return;
    }
    state.scrolling = true;

    if let Ok((_entity, mut scroll_pos, _node, computed, _ugt)) = scrollable.get_mut(target) {
        let isf = computed.inverse_scale_factor();
        let scroll_dy = -raw_dy * dpr * isf;

        let content_h = computed.content_size().y;
        let visible_h = computed.size().y;
        let max_scroll = ((content_h - visible_h) * isf).max(0.0);
        scroll_pos.y = (scroll_pos.y + scroll_dy).clamp(0.0, max_scroll);
    }
}
