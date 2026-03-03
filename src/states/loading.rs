//! Loading screen state systems.
//!
//! This module implements asset loading with visual feedback,
//! following the pattern from Bevy's loading_screen example.
//! If any asset fails to load, the game will panic with a clear error.

use bevy::asset::UntypedHandle;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;
use std::collections::HashMap;

use super::AppState;
use crate::data::LoadedChargers;
use crate::data::json_assets::{ChargersAsset, DriverScheduleAsset};
use crate::resources::{
    DriverSchedule, GameDataAssets, MultiSiteManager, SiteArchetype, SiteTemplateCache,
};

/// A tracked asset with its path for error reporting
#[derive(Debug, Clone)]
pub struct TrackedAsset {
    pub path: String,
    pub handle: UntypedHandle,
}

/// Resource to track loading progress
#[derive(Resource, Debug)]
pub struct LoadingData {
    /// Assets being loaded with their paths
    pub tracked_assets: Vec<TrackedAsset>,
    /// Number of confirmation frames before transitioning
    pub confirmation_frames_target: usize,
    /// Current count of confirmation frames
    pub confirmation_frames_count: usize,
    /// Progress message to display
    pub status_message: String,
}

impl Default for LoadingData {
    fn default() -> Self {
        Self {
            tracked_assets: Vec::new(),
            confirmation_frames_target: 5,
            confirmation_frames_count: 0,
            status_message: "Initializing...".to_string(),
        }
    }
}

impl LoadingData {
    pub fn new(confirmation_frames: usize) -> Self {
        Self {
            confirmation_frames_target: confirmation_frames,
            ..default()
        }
    }
}

/// Marker component for loading screen UI
#[derive(Component)]
pub struct LoadingScreenUI;

/// Marker for the progress bar fill
#[derive(Component)]
pub struct LoadingProgressBar;

/// Marker for the status text
#[derive(Component)]
pub struct LoadingStatusText;

/// Minimal loading indicator text
#[derive(Component)]
pub struct MinimalLoadingIndicator;

/// Setup the loading screen UI
pub fn setup_loading_screen(mut commands: Commands) {
    // Spawn UI overlay for title and loading indicator
    commands
        .spawn((
            LoadingScreenUI,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(40.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.07, 0.1)),
            GlobalZIndex(950),
        ))
        .with_children(|parent| {
            // Top section: Title
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|top| {
                    top.spawn((
                        Text::new("Kilowatt Tycoon"),
                        TextFont {
                            font_size: 80.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.2, 0.8, 0.4)),
                    ));
                    top.spawn((
                        Text::new("Your Charging Empire Awaits..."),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.6, 0.7)),
                    ));
                });

            // Middle section: spacer
            parent.spawn(Node {
                flex_grow: 1.0,
                ..default()
            });

            // Bottom section: Minimal loading indicator
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|bottom| {
                    bottom.spawn((
                        MinimalLoadingIndicator,
                        Text::new("Loading..."),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.6, 0.65, 0.7)),
                    ));
                });
        });
}

/// Update loading progress and check for failed assets (panic if any fail)
pub fn update_loading_progress(
    asset_server: Res<AssetServer>,
    mut loading_data: ResMut<LoadingData>,
    mut progress_query: Query<&mut Node, With<LoadingProgressBar>>,
    mut status_query: Query<&mut Text, With<LoadingStatusText>>,
) {
    let mut loaded = 0;
    let total = loading_data.tracked_assets.len();

    for tracked in &loading_data.tracked_assets {
        let load_state = asset_server.load_state(tracked.handle.id());

        // Check for failed assets and panic immediately
        if load_state.is_failed() {
            panic!(
                "FATAL: Failed to load required asset '{}'. \
                 The game cannot continue without this asset. \
                 Please ensure the asset file exists in the assets/ directory.",
                tracked.path
            );
        }

        if asset_server.is_loaded_with_dependencies(tracked.handle.id()) {
            loaded += 1;
        }
    }

    let progress = if total == 0 {
        1.0
    } else {
        loaded as f32 / total as f32
    };

    // Update progress bar
    for mut node in &mut progress_query {
        node.width = Val::Percent(progress * 100.0);
    }

    // Update status text
    loading_data.status_message = if total == 0 {
        "Initializing game systems...".to_string()
    } else {
        format!("Loading assets... ({loaded}/{total})")
    };

    for mut text in &mut status_query {
        *text = Text::new(&loading_data.status_message);
    }
}

/// Check if loading is complete and transition to playing
pub fn check_loading_complete(
    asset_server: Res<AssetServer>,
    mut loading_data: ResMut<LoadingData>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Check if all assets are loaded
    let all_loaded = loading_data
        .tracked_assets
        .iter()
        .all(|tracked| asset_server.is_loaded_with_dependencies(tracked.handle.id()));

    // If we have no assets to track, we're ready
    if loading_data.tracked_assets.is_empty() || all_loaded {
        // Wait for confirmation frames to prevent flicker
        loading_data.confirmation_frames_count += 1;

        if loading_data.confirmation_frames_count >= loading_data.confirmation_frames_target {
            info!("Loading complete, transitioning to Playing state");
            next_state.set(AppState::Playing);
        }
    } else {
        // Reset confirmation if assets aren't ready
        loading_data.confirmation_frames_count = 0;
    }
}

/// Update minimal loading indicator
pub fn update_minimal_loading_indicator(
    loading_data: Res<LoadingData>,
    mut indicator: Query<&mut Text, With<MinimalLoadingIndicator>>,
) {
    for mut text in &mut indicator {
        let loaded = loading_data
            .tracked_assets
            .iter()
            .filter(|_asset| {
                // This is approximate - we don't have asset_server here
                true // Just show the count
            })
            .count();
        let total = loading_data.tracked_assets.len();

        if total == 0 {
            *text = Text::new("Loading...");
        } else {
            let progress = (loaded as f32 / total as f32 * 100.0) as u32;
            *text = Text::new(format!("Loading... {progress}%"));
        }
    }
}

/// Cleanup loading screen UI
pub fn cleanup_loading_screen(mut commands: Commands, query: Query<Entity, With<LoadingScreenUI>>) {
    for entity in &query {
        commands.entity(entity).try_despawn();
    }
    commands.remove_resource::<LoadingData>();
}

/// Load all game assets and track them with paths for error reporting
pub fn start_asset_loading(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut game_data: ResMut<GameDataAssets>,
) {
    let mut loading = LoadingData::new(5);

    // List all required image assets
    let assets_to_load: Vec<&str> = vec![
        // Tiles
        "world/tiles/tile_grass.png",
        "world/tiles/tile_asphalt_clean.png",
        "world/tiles/tile_asphalt_lines.png",
        "world/tiles/tile_concrete.png",
        "world/tiles/tile_curb_asphalt_grass.png",
        "world/tiles/tile_curb_asphalt_concrete.png",
        // Vehicles
        "vehicles/vehicle_compact.png",
        "vehicles/vehicle_sedan.png",
        "vehicles/vehicle_suv.png",
        "vehicles/vehicle_crossover.png",
        "vehicles/vehicle_pickup.png",
        "vehicles/vehicle_firetruck.png",
        // Chargers - DCFC 50kW (compact, budget)
        "chargers/dcfc50/charger_dcfc50_available.png",
        "chargers/dcfc50/charger_dcfc50_charging.png",
        "chargers/dcfc50/charger_dcfc50_offline.png",
        "chargers/dcfc50/charger_dcfc50_warning.png",
        "chargers/dcfc50/charger_dcfc50_cable_stuck.png",
        // Chargers - DCFC 150kW (standard)
        "chargers/dcfc150/charger_dcfc150_available.png",
        "chargers/dcfc150/charger_dcfc150_charging.png",
        "chargers/dcfc150/charger_dcfc150_offline.png",
        "chargers/dcfc150/charger_dcfc150_warning.png",
        "chargers/dcfc150/charger_dcfc150_cable_stuck.png",
        // Chargers - DCFC 350kW (premium, flagship)
        "chargers/dcfc350/charger_dcfc350_available.png",
        "chargers/dcfc350/charger_dcfc350_charging.png",
        "chargers/dcfc350/charger_dcfc350_offline.png",
        "chargers/dcfc350/charger_dcfc350_warning.png",
        "chargers/dcfc350/charger_dcfc350_cable_stuck.png",
        // Chargers - L2
        "chargers/l2/charger_l2_available.png",
        "chargers/l2/charger_l2_charging.png",
        "chargers/l2/charger_l2_offline.png",
        "chargers/l2/charger_l2_warning.png",
        "chargers/l2/charger_l2_cable_stuck.png",
        // Props
        "props/prop_transformer.png",
        "props/prop_transformer_hot.png",
        "props/prop_transformer_critical.png",
        "props/prop_transformer_destroyed.png",
        "props/prop_solar_array_ground.png",
        "props/prop_battery_container.png",
        // Mood Icons (displayed on vehicles)
        "ui/icons/icon_mood_neutral.png",
        "ui/icons/icon_mood_happy.png",
        "ui/icons/icon_mood_impatient.png",
        "ui/icons/icon_mood_angry.png",
        // Decals
        "world/decals/decal_ev_parking.png",
        "world/decals/decal_arrow.png",
        "world/decals/decal_stall_lines.png",
        // VFX
        "vfx/vfx_selection.png",
        "vfx/vfx_placement_cursor.png",
        "vfx/vfx_float_money.png",
    ];

    // Load each image asset and track with path for error reporting
    for path in assets_to_load {
        let handle: Handle<Image> = asset_server.load(path);
        loading.tracked_assets.push(TrackedAsset {
            path: path.to_string(),
            handle: handle.untyped(),
        });
    }

    // Load JSON data assets
    // Chargers configuration
    let chargers_path = "data/chargers/mvp_chargers.chargers.json";
    let chargers_handle: Handle<ChargersAsset> = asset_server.load(chargers_path);
    loading.tracked_assets.push(TrackedAsset {
        path: chargers_path.to_string(),
        handle: chargers_handle.clone().untyped(),
    });
    game_data.chargers = chargers_handle;

    // Driver schedule (switchable via KWT_SCENARIO for quick content testing)
    let scenario_path = match std::env::var("KWT_SCENARIO").ok().as_deref() {
        Some("hcmc_scooters") => "data/scenarios/hcmc_scooters.scenario.json",
        _ => "data/scenarios/mvp_drivers.scenario.json",
    };
    let scenario_handle: Handle<DriverScheduleAsset> = asset_server.load(scenario_path);
    loading.tracked_assets.push(TrackedAsset {
        path: scenario_path.to_string(),
        handle: scenario_handle.clone().untyped(),
    });
    game_data.driver_schedule = scenario_handle;

    // Tiled maps - load TMX files (single source of truth for level data)
    let tiled_maps: Vec<(SiteArchetype, &str)> = vec![
        (SiteArchetype::ParkingLot, "maps/01_first_street.tmx"),
        (
            SiteArchetype::GasStation,
            "maps/02_quick_charge_express.tmx",
        ),
        (SiteArchetype::FleetDepot, "maps/03_central_fleet_plaza.tmx"),
        (SiteArchetype::ScooterHub, "maps/04_scooter_alley.tmx"),
    ];

    let mut tiled_handles = HashMap::new();
    for (archetype, path) in tiled_maps {
        let handle: Handle<TiledMapAsset> = asset_server.load(path);
        loading.tracked_assets.push(TrackedAsset {
            path: path.to_string(),
            handle: handle.clone().untyped(),
        });
        tiled_handles.insert(archetype, handle);
    }
    game_data.tiled_maps = tiled_handles;

    info!(
        "Starting to load {} assets ({} images, {} JSON, {} TMX)",
        loading.tracked_assets.len(),
        loading.tracked_assets.len() - 6, // 2 JSON + 4 TMX
        2,
        4
    );
    commands.insert_resource(loading);
}

/// System to populate SiteTemplateCache from TMX assets
///
/// TMX is the single source of truth for level data. This system extracts gameplay
/// configuration from TMX map properties and tile/object layers.
pub fn populate_template_cache(
    asset_server: Res<AssetServer>,
    game_data: Res<GameDataAssets>,
    tiled_assets: Res<Assets<TiledMapAsset>>,
    mut template_cache: ResMut<SiteTemplateCache>,
) {
    // Skip if already loaded
    if template_cache.loaded {
        return;
    }

    // Check if all TMX maps are loaded
    let all_tmx_loaded = game_data
        .tiled_maps
        .values()
        .all(|handle| asset_server.is_loaded_with_dependencies(handle.id()));

    if !all_tmx_loaded {
        return;
    }

    // Populate cache from TMX assets
    let mut loaded_count = 0;

    for (archetype, tmx_handle) in &game_data.tiled_maps {
        if let Some(tmx_asset) = tiled_assets.get(tmx_handle) {
            // Extract template data from TMX map properties
            if let Some(template) =
                crate::data::tiled_loader::extract_template_from_map(&tmx_asset.map)
            {
                template_cache.templates.insert(*archetype, template);
                loaded_count += 1;
            } else {
                warn!("Failed to extract template from TMX for {:?}", archetype);
            }
        }
    }

    template_cache.loaded = true;
    info!(
        "SiteTemplateCache populated: {} templates from TMX",
        loaded_count
    );
}

/// System to populate MultiSiteManager available sites from template cache
pub fn populate_available_sites(
    template_cache: Res<SiteTemplateCache>,
    mut multi_site: ResMut<MultiSiteManager>,
) {
    // Skip if cache not loaded yet or sites already populated
    if !template_cache.loaded || multi_site.is_populated() {
        return;
    }

    multi_site.populate_from_cache(&template_cache);
}

/// System to populate game resources from loaded JSON assets
pub fn populate_game_data_from_assets(
    asset_server: Res<AssetServer>,
    game_data: Res<GameDataAssets>,
    chargers_assets: Res<Assets<ChargersAsset>>,
    scenario_assets: Res<Assets<DriverScheduleAsset>>,
    mut commands: Commands,
    mut driver_schedule: ResMut<DriverSchedule>,
    loaded_chargers: Option<Res<LoadedChargers>>,
) {
    // Skip if already populated
    if loaded_chargers.is_some() {
        return;
    }

    // Check if chargers and scenario are loaded
    let chargers_loaded = asset_server.is_loaded_with_dependencies(game_data.chargers.id());
    let scenario_loaded = asset_server.is_loaded_with_dependencies(game_data.driver_schedule.id());

    if !chargers_loaded || !scenario_loaded {
        return;
    }

    // Populate chargers
    if let Some(chargers_asset) = chargers_assets.get(&game_data.chargers) {
        info!("Loaded {} chargers from asset", chargers_asset.0.len());
        commands.insert_resource(LoadedChargers(chargers_asset.0.clone()));
    }

    // Populate driver schedule
    if let Some(scenario_asset) = scenario_assets.get(&game_data.driver_schedule) {
        info!(
            "Loaded driver schedule '{}' with {} drivers",
            scenario_asset.0.name,
            scenario_asset.0.drivers.len()
        );
        *driver_schedule = scenario_asset.0.clone();
    }
}
