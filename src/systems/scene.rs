//! Scene rendering from SiteGrid
//!
//! All rendering uses PNG assets (generated from SVG source files).
//! If assets are missing, the game will fail during the Loading state.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::components::charger::{Charger, ChargerSprite, ChargerState, ChargerTier, ChargerType};
use crate::components::power::Transformer;
use crate::resources::{
    AmenityType, ChargerPadType, GRID_HEIGHT, GRID_OFFSET_X, GRID_OFFSET_Y, GRID_WIDTH,
    ImageAssets, SiteGrid, SiteId, StructureSize, TILE_SIZE, TileContent,
};
use crate::systems::sprite::get_charger_image;

/// Resource tracking last processed grid revision per site for charger sync.
/// Systems use this to detect when the grid has changed and needs re-sync.
#[derive(Resource, Default)]
pub struct ChargerSyncRevision(pub HashMap<SiteId, u64>);

/// Resource tracking last processed grid revision per site for visual updates.
#[derive(Resource, Default)]
pub struct GridVisualRevision(pub HashMap<SiteId, u64>);

/// Marker for grid tile visuals
#[derive(Component)]
pub struct GridTileVisual {
    pub grid_x: i32,
    pub grid_y: i32,
}

// Note: GridChargerVisual marker was removed - charger visuals are now entity-driven
// via Charger entities + ChargerSprite (see sync_chargers_with_grid)

/// Marker for transformer visual
#[derive(Component)]
pub struct TransformerVisual;

/// Marker for transformer load gauge bar fill
#[derive(Component)]
pub struct TransformerLoadBar;

/// Marker for solar generation bar fill (world sprite)
#[derive(Component)]
pub struct SolarGenerationBar;

/// Marker for solar generation text label (world sprite)
#[derive(Component)]
pub struct SolarGenerationLabel;

/// Marker for battery SOC bar fill (world sprite)
#[derive(Component)]
pub struct BatterySOCBar;

/// Marker for battery SOC text label (world sprite)
#[derive(Component)]
pub struct BatterySOCLabel;

/// Marker for solar array visual
#[derive(Component)]
pub struct SolarArrayVisual;

/// Marker for battery storage visual
#[derive(Component)]
pub struct BatteryStorageVisual;

/// Marker for the static pole/base of the security system (parent entity).
#[derive(Component)]
pub struct SecuritySystemVisual;

/// Marker and animation state for the rotating camera head (child entity).
/// The camera swivels back and forth around its base heading.
#[derive(Component)]
pub struct SecurityCameraHead {
    /// Base rotation angle (radians) pointing toward lot center
    pub base_angle: f32,
    /// Animation timer (accumulates real time)
    pub timer: f32,
}

/// Blinking LED indicator on the security camera head.
/// Oscillates alpha at ~1 Hz to show the system is active.
#[derive(Component)]
pub struct SecurityCameraLed {
    /// Animation timer (accumulates real time)
    pub timer: f32,
}

/// Marker for amenity building visual
#[derive(Component)]
pub struct AmenityBuildingVisual {
    pub amenity_type: AmenityType,
}

/// Marker for entry/exit indicators
#[derive(Component)]
pub struct EntryExitMarker;

/// Tile scale constant - kept for backward compatibility with prop rendering
/// With PNG exports at native size (64px), TILE_SCALE = 64/64 = 1.0
/// New code should use sprite_metadata functions instead.
const TILE_SCALE: f32 = 1.0;

/// Spawn entry/exit arrow markers for all owned sites
///
/// NOTE: This is now optional - these arrows are just visual hints to show
/// where vehicles enter/exit. Could be removed or rendered in Tiled instead.
pub fn spawn_grid_background(
    mut commands: Commands,
    multi_site: Res<crate::resources::MultiSiteManager>,
    image_assets: Res<ImageAssets>,
) {
    // Spawn entry/exit markers for all owned sites
    for (site_id, site_state) in &multi_site.owned_sites {
        // Skip if site doesn't have a root entity yet
        let Some(root_entity) = site_state.root_entity else {
            continue;
        };

        commands.entity(root_entity).with_children(|parent| {
            // Entry point marker (rotated -90 degrees to point right/east along road)
            let entry_pos =
                SiteGrid::grid_to_world(site_state.grid.entry_pos.0, site_state.grid.entry_pos.1);
            parent.spawn((
                Sprite::from_image(image_assets.decal_arrow.clone()),
                Transform::from_xyz(entry_pos.x, entry_pos.y, 1.0)
                    .with_scale(Vec3::splat(TILE_SCALE * 0.2))
                    .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
                EntryExitMarker,
                crate::components::BelongsToSite::new(*site_id),
            ));

            // Exit point marker (rotated -90 degrees to point right/east along road)
            let exit_pos =
                SiteGrid::grid_to_world(site_state.grid.exit_pos.0, site_state.grid.exit_pos.1);
            parent.spawn((
                Sprite::from_image(image_assets.decal_arrow.clone()),
                Transform::from_xyz(exit_pos.x, exit_pos.y, 1.0)
                    .with_scale(Vec3::splat(TILE_SCALE * 0.2))
                    .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
                EntryExitMarker,
                crate::components::BelongsToSite::new(*site_id),
            ));
        });

        info!(
            "Grid background spawned for site {:?} ({}x{} tiles)",
            site_id, site_state.grid.width, site_state.grid.height
        );
    }
}

/// Update infrastructure overlays when grid changes.
///
/// **IMPORTANT**: Base tiles (grass, road, parking bays, walls, etc.) are rendered by
/// bevy_ecs_tiled from TMX files. This system ONLY spawns entity overlays for
/// interactive infrastructure:
/// - Transformers (with gauge bars)
/// - Solar arrays (with generation bars)
/// - Batteries (with SOC bars)
/// - Amenities (with custom state)
///
/// Uses revision-based change detection for robustness.
pub fn update_grid_visuals(
    mut commands: Commands,
    multi_site: Res<crate::resources::MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    existing_transformer: Query<Entity, With<TransformerVisual>>,
    existing_solar: Query<Entity, With<SolarArrayVisual>>,
    existing_battery: Query<Entity, With<BatteryStorageVisual>>,
    existing_security: Query<Entity, With<SecuritySystemVisual>>,
    existing_amenity: Query<Entity, With<AmenityBuildingVisual>>,
    mut visual_revision: ResMut<GridVisualRevision>,
) {
    // Get active site grid or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let grid = &site.grid;
    let site_id = site.id;
    let current_revision = grid.revision;

    // Check if we've already processed this revision
    let last_revision = visual_revision.0.get(&site_id).copied().unwrap_or(0);
    if current_revision == last_revision {
        return; // No changes since last update
    }

    // Get site root entity - required for spawning children
    let Some(root_entity) = site.root_entity else {
        warn!(
            "Site {:?} has no root entity, skipping visual refresh",
            site_id
        );
        return;
    };

    // Clear existing infrastructure visuals
    for entity in &existing_transformer {
        commands.entity(entity).try_despawn();
    }
    for entity in &existing_solar {
        commands.entity(entity).try_despawn();
    }
    for entity in &existing_battery {
        commands.entity(entity).try_despawn();
    }
    for entity in &existing_security {
        commands.entity(entity).try_despawn();
    }
    for entity in &existing_amenity {
        commands.entity(entity).try_despawn();
    }

    // Spawn entity overlays ONLY for infrastructure with dynamic state
    // All base tiles (grass, road, parking, walls, decorations) are rendered by Tiled
    for ((x, y), tile) in grid.iter_tiles() {
        match tile.content {
            // Infrastructure with dynamic state - needs entity overlays
            TileContent::TransformerPad => {
                // Spawn transformer entity with gauge overlay
                let kva = grid
                    .get_transformer_at(x, y)
                    .map(|t| t.kva)
                    .unwrap_or(500.0);
                spawn_transformer(
                    &mut commands,
                    root_entity,
                    &image_assets,
                    x,
                    y,
                    kva,
                    site.id,
                );
            }
            TileContent::SolarPad => {
                // Spawn solar array entity with generation bar overlay
                spawn_solar_array(&mut commands, root_entity, &image_assets, x, y, site.id);
            }
            TileContent::BatteryPad => {
                // Spawn battery entity with SOC bar overlay
                spawn_battery_storage(&mut commands, root_entity, &image_assets, x, y, site.id);
            }
            TileContent::SecurityPad => {
                spawn_security_system(&mut commands, root_entity, &image_assets, x, y, site.id);
            }
            TileContent::AmenityWifiRestrooms => {
                spawn_amenity_building(
                    &mut commands,
                    root_entity,
                    &image_assets,
                    x,
                    y,
                    AmenityType::WifiRestrooms,
                    site.id,
                );
            }
            TileContent::AmenityLoungeSnacks => {
                spawn_amenity_building(
                    &mut commands,
                    root_entity,
                    &image_assets,
                    x,
                    y,
                    AmenityType::LoungeSnacks,
                    site.id,
                );
            }
            TileContent::AmenityRestaurant => {
                spawn_amenity_building(
                    &mut commands,
                    root_entity,
                    &image_assets,
                    x,
                    y,
                    AmenityType::Restaurant,
                    site.id,
                );
            }
            // All other tile types are rendered by Tiled - no entity spawning needed
            _ => {}
        }
    }

    // Update last processed revision
    visual_revision.0.insert(site_id, current_revision);
}

// ============================================================================
// NOTE: Base tile spawning functions removed - tiles are now rendered by Tiled
// ============================================================================
//
// The following tile types are now rendered by bevy_ecs_tiled from TMX files:
// - Base terrain: Grass, Road, Entry, Exit, Lot, Concrete
// - Parking: ParkingBayNorth, ParkingBaySouth (including stall lines, decals)
// - Gas station: StoreWall, StoreEntrance, Storefront, PumpIsland, Canopy, etc.
// - Mall/Garage: GarageFloor, GaragePillar, MallFacade
// - Transit: LoadingZone
// - Decorative: Planter, StreetTree, LightPole, Bollard, WheelStop, etc.
//
// Only infrastructure with dynamic state needs entity overlays (see functions below).
// ============================================================================

/// Spawn a 2x2 transformer entity overlay (anchor at x,y which is bottom-left)
///
/// Base tiles are rendered by Tiled. This only spawns the transformer prop sprite
/// with gauge overlays for dynamic state visualization.
fn spawn_transformer(
    commands: &mut Commands,
    root_entity: Entity,
    assets: &ImageAssets,
    anchor_x: i32,
    anchor_y: i32,
    kva: f32,
    site_id: crate::resources::SiteId,
) {
    let size = StructureSize::TwoByTwo;

    commands.entity(root_entity).with_children(|parent| {
        // NOTE: Concrete pad tiles are rendered by Tiled, not spawned here

        // Transformer prop sprite centered on the 2x2 footprint
        let grid_center = SiteGrid::multi_tile_center(anchor_x, anchor_y, size);
        parent
            .spawn((
                Sprite::from_image(assets.prop_transformer.clone()),
                Transform::from_xyz(grid_center.x, grid_center.y, 2.0)
                    .with_scale(Vec3::splat(TILE_SCALE * 0.4)), // 2x2 footprint (256px PNG -> 128px)
                TransformerVisual,
                crate::components::BelongsToSite::new(site_id),
            ))
            .with_children(|sprite_parent| {
                // Calculate inverse scale to counter parent scaling
                // Parent is scaled by TILE_SCALE * 0.4, so we need to counter that
                let inverse_scale = 1.0 / (TILE_SCALE * 0.4);

                // kVA rating text overlay - shadow layer for visibility
                sprite_parent.spawn((
                    Text2d::new(format!("{} kVA", kva as i32)),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                    Transform::from_xyz(1.0, TRANSFORMER_LABEL_TEXT_Y - 1.0, 0.09)
                        .with_scale(Vec3::splat(inverse_scale)),
                ));

                // kVA rating text overlay - main text
                sprite_parent.spawn((
                    Text2d::new(format!("{} kVA", kva as i32)),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Transform::from_xyz(0.0, TRANSFORMER_LABEL_TEXT_Y, 0.1)
                        .with_scale(Vec3::splat(inverse_scale)),
                ));

                // Power load gauge bar - below the text, thicker + higher contrast
                let bar_width = TRANSFORMER_GAUGE_BAR_WIDTH;
                let bar_height = TRANSFORMER_GAUGE_BAR_HEIGHT;
                let bar_y = TRANSFORMER_GAUGE_BAR_Y;

                // Outline (subtle dark border for integrated look)
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgba(0.2, 0.2, 0.2, 0.8),
                        custom_size: Some(Vec2::new(bar_width + 4.0, bar_height + 4.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, bar_y, 0.105).with_scale(Vec3::splat(inverse_scale)),
                ));

                // Background
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgba(0.12, 0.12, 0.16, 0.95),
                        custom_size: Some(Vec2::new(bar_width, bar_height)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, bar_y, 0.11).with_scale(Vec3::splat(inverse_scale)),
                ));

                // Fill (blue, shows power load) - starts at 0 width, left-anchored
                // Position x is scaled by inverse_scale to compensate for parent scaling
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgba(0.15, 0.7, 1.0, 1.0),
                        custom_size: Some(Vec2::new(0.0, bar_height)),
                        ..default()
                    },
                    Transform::from_xyz(-bar_width / 2.0 * inverse_scale, bar_y, 0.12)
                        .with_scale(Vec3::splat(inverse_scale)),
                    Anchor::CENTER_LEFT,
                    TransformerLoadBar,
                ));
            });
    });
}

/// Transformer overlay layout constants
const TRANSFORMER_LABEL_TEXT_Y: f32 = 12.0; // Above center
const TRANSFORMER_GAUGE_BAR_Y: f32 = -32.0; // Below center, more separation
const TRANSFORMER_GAUGE_BAR_WIDTH: f32 = 60.0;
const TRANSFORMER_GAUGE_BAR_HEIGHT: f32 = 8.0;

/// Solar array overlay layout constants
const SOLAR_LABEL_TEXT_Y: f32 = 18.0; // Above center
const SOLAR_BAR_Y: f32 = -25.0; // Below center
const SOLAR_BAR_WIDTH: f32 = 70.0;
const SOLAR_BAR_HEIGHT: f32 = 8.0;

/// Battery storage overlay layout constants
const BATTERY_LABEL_TEXT_Y: f32 = 18.0; // Above center
const BATTERY_BAR_Y: f32 = -25.0; // Below center
const BATTERY_BAR_WIDTH: f32 = 60.0;
const BATTERY_BAR_HEIGHT: f32 = 8.0;

/// Update transformer load gauge bar and sprite state based on current site state
pub fn update_transformer_gauges(
    multi_site: Res<crate::resources::MultiSiteManager>,
    assets: Res<ImageAssets>,
    mut load_bars: Query<
        (&mut Sprite, &mut Transform),
        (With<TransformerLoadBar>, Without<TransformerVisual>),
    >,
    mut visuals: Query<&mut Sprite, (With<TransformerVisual>, Without<TransformerLoadBar>)>,
    transformers: Query<&Transformer>,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    // Calculate load percentage
    let total_load = site_state.phase_loads.total_load();
    let capacity = site_state.effective_capacity_kva();
    let load_pct = if capacity > 0.0 {
        (total_load / capacity).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let bar_width = TRANSFORMER_GAUGE_BAR_WIDTH;

    // Update load bar (0% = no fill visible)
    // Fill sprite is left-anchored, so we only need to update width
    for (mut sprite, _transform) in &mut load_bars {
        let fill_width = bar_width * load_pct;
        sprite.custom_size = Some(Vec2::new(fill_width, TRANSFORMER_GAUGE_BAR_HEIGHT));
    }

    // Update main transformer sprite based on temperature
    // Since there's only one transformer per site, we can use the first one found
    if let Some(transformer) = transformers.iter().next() {
        let new_image = if transformer.is_critical() {
            assets.prop_transformer_critical.clone()
        } else if transformer.is_warning() {
            assets.prop_transformer_hot.clone()
        } else {
            assets.prop_transformer.clone()
        };

        for mut sprite in &mut visuals {
            if sprite.image != new_image {
                sprite.image = new_image.clone();
            }
        }
    }
}

/// Update solar generation bar and label based on current site state
pub fn update_solar_generation_bar(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut bars: Query<&mut Sprite, (With<SolarGenerationBar>, Without<SolarGenerationLabel>)>,
    mut labels: Query<(&mut Text2d, &mut TextColor), With<SolarGenerationLabel>>,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    // Get solar state
    let peak_kw = site_state.grid.total_solar_kw;
    if peak_kw <= 0.0 {
        // No solar installed - hide bar
        for mut sprite in &mut bars {
            sprite.custom_size = Some(Vec2::new(0.0, SOLAR_BAR_HEIGHT));
        }
        return;
    }

    let current_kw = site_state.solar_state.current_generation_kw;
    let generation_pct = (current_kw / peak_kw).clamp(0.0, 1.0);

    // Update bar fill
    let fill_width = SOLAR_BAR_WIDTH * generation_pct;
    let color = if generation_pct > 0.5 {
        Color::srgb(1.0, 0.85, 0.1) // Bright yellow
    } else if generation_pct > 0.1 {
        Color::srgb(1.0, 0.7, 0.2) // Orange-yellow
    } else {
        Color::srgb(0.5, 0.4, 0.2) // Dim (night/low light)
    };

    for mut sprite in &mut bars {
        sprite.custom_size = Some(Vec2::new(fill_width, SOLAR_BAR_HEIGHT));
        sprite.color = color;
    }

    // Update label text
    for (mut text, mut text_color) in &mut labels {
        **text = format!("{current_kw:.0} kW");
        *text_color = TextColor(color);
    }
}

/// Update battery SOC bar and label based on current site state
pub fn update_battery_soc_bar(
    multi_site: Res<crate::resources::MultiSiteManager>,
    mut bars: Query<&mut Sprite, (With<BatterySOCBar>, Without<BatterySOCLabel>)>,
    mut labels: Query<(&mut Text2d, &mut TextColor), With<BatterySOCLabel>>,
) {
    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    // Get battery state
    let capacity_kwh = site_state.bess_state.capacity_kwh;
    if capacity_kwh <= 0.0 {
        // No battery installed - hide bar
        for mut sprite in &mut bars {
            sprite.custom_size = Some(Vec2::new(0.0, BATTERY_BAR_HEIGHT));
        }
        return;
    }

    let soc_percent = site_state.bess_state.soc_percent();
    let power = site_state.bess_state.current_power_kw;

    // Update bar fill
    let fill_width = BATTERY_BAR_WIDTH * (soc_percent / 100.0);

    // Color based on state
    let color = if soc_percent < 20.0 {
        Color::srgb(0.9, 0.3, 0.3) // Red - low battery
    } else if power < -0.1 {
        Color::srgb(0.3, 0.6, 0.9) // Blue - charging
    } else if power > 0.1 {
        Color::srgb(0.3, 0.9, 0.5) // Green - discharging
    } else {
        Color::srgb(0.4, 0.7, 0.8) // Cyan - standby
    };

    for mut sprite in &mut bars {
        sprite.custom_size = Some(Vec2::new(fill_width, BATTERY_BAR_HEIGHT));
        sprite.color = color;
    }

    // Update label text - show percentage and rate indicator
    let rate_str = if power > 0.1 {
        format!(" -{power:.0}")
    } else if power < -0.1 {
        format!(" +{:.0}", power.abs())
    } else {
        String::new()
    };

    for (mut text, mut text_color) in &mut labels {
        **text = format!("{soc_percent:.0}%{rate_str}");
        *text_color = TextColor(color);
    }
}

/// Spawn a 3x2 solar array entity overlay (anchor at x,y which is bottom-left)
///
/// Base tiles are rendered by Tiled. This only spawns the solar array prop sprite
/// with generation bar overlays for dynamic state visualization.
fn spawn_solar_array(
    commands: &mut Commands,
    root_entity: Entity,
    assets: &ImageAssets,
    anchor_x: i32,
    anchor_y: i32,
    site_id: crate::resources::SiteId,
) {
    let size = StructureSize::ThreeByTwo;

    commands.entity(root_entity).with_children(|parent| {
        // NOTE: Grass pad tiles are rendered by Tiled, not spawned here

        // Solar array prop sprite centered on the 3x2 footprint
        let grid_center = SiteGrid::multi_tile_center(anchor_x, anchor_y, size);
        let solar_scale = TILE_SCALE * 0.625; // 3x2 footprint
        parent
            .spawn((
                Sprite::from_image(assets.prop_solar_array_ground.clone()),
                Transform::from_xyz(grid_center.x, grid_center.y, 2.0)
                    .with_scale(Vec3::splat(solar_scale)), // Scale for 3x2 (wider)
                SolarArrayVisual,
                crate::components::BelongsToSite::new(site_id),
            ))
            .with_children(|sprite_parent| {
                // Calculate inverse scale to counter parent scaling
                let inverse_scale = 1.0 / solar_scale;

                // Generation text label - shadow layer
                sprite_parent.spawn((
                    Text2d::new("0 kW"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                    Transform::from_xyz(1.0, SOLAR_LABEL_TEXT_Y - 1.0, 0.09)
                        .with_scale(Vec3::splat(inverse_scale)),
                ));

                // Generation text label - main text
                sprite_parent.spawn((
                    Text2d::new("0 kW"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.9, 0.3)), // Yellow for solar
                    Transform::from_xyz(0.0, SOLAR_LABEL_TEXT_Y, 0.1)
                        .with_scale(Vec3::splat(inverse_scale)),
                    SolarGenerationLabel,
                ));

                // Generation bar
                let bar_width = SOLAR_BAR_WIDTH;
                let bar_height = SOLAR_BAR_HEIGHT;
                let bar_y = SOLAR_BAR_Y;

                // Outline (subtle dark border)
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgba(0.2, 0.2, 0.2, 0.8),
                        custom_size: Some(Vec2::new(bar_width + 4.0, bar_height + 4.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, bar_y, 0.105).with_scale(Vec3::splat(inverse_scale)),
                ));

                // Background
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgba(0.12, 0.12, 0.16, 0.95),
                        custom_size: Some(Vec2::new(bar_width, bar_height)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, bar_y, 0.11).with_scale(Vec3::splat(inverse_scale)),
                ));

                // Fill (yellow/orange for solar) - starts at 0 width, left-anchored
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgb(1.0, 0.8, 0.2),
                        custom_size: Some(Vec2::new(0.0, bar_height)),
                        ..default()
                    },
                    Transform::from_xyz(-bar_width / 2.0 * inverse_scale, bar_y, 0.12)
                        .with_scale(Vec3::splat(inverse_scale)),
                    Anchor::CENTER_LEFT,
                    SolarGenerationBar,
                ));
            });
    });
}

/// Spawn a 2x2 battery storage entity overlay (anchor at x,y which is bottom-left)
///
/// Base tiles are rendered by Tiled. This only spawns the battery container prop sprite
/// with SOC bar overlays for dynamic state visualization.
fn spawn_battery_storage(
    commands: &mut Commands,
    root_entity: Entity,
    assets: &ImageAssets,
    anchor_x: i32,
    anchor_y: i32,
    site_id: crate::resources::SiteId,
) {
    let size = StructureSize::TwoByTwo;

    commands.entity(root_entity).with_children(|parent| {
        // NOTE: Concrete pad tiles are rendered by Tiled, not spawned here

        // Battery container prop sprite centered on the 2x2 footprint
        let grid_center = SiteGrid::multi_tile_center(anchor_x, anchor_y, size);
        let battery_scale = TILE_SCALE * 0.375; // 2x2 footprint
        parent
            .spawn((
                Sprite::from_image(assets.prop_battery_container.clone()),
                Transform::from_xyz(grid_center.x, grid_center.y, 2.0)
                    .with_scale(Vec3::splat(battery_scale)), // Scale up for 2x2
                BatteryStorageVisual,
                crate::components::BelongsToSite::new(site_id),
            ))
            .with_children(|sprite_parent| {
                // Calculate inverse scale to counter parent scaling
                let inverse_scale = 1.0 / battery_scale;

                // SOC text label - shadow layer
                sprite_parent.spawn((
                    Text2d::new("0%"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                    Transform::from_xyz(1.0, BATTERY_LABEL_TEXT_Y - 1.0, 0.09)
                        .with_scale(Vec3::splat(inverse_scale)),
                ));

                // SOC text label - main text
                sprite_parent.spawn((
                    Text2d::new("0%"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.4, 0.8, 1.0)), // Cyan/blue for battery
                    Transform::from_xyz(0.0, BATTERY_LABEL_TEXT_Y, 0.1)
                        .with_scale(Vec3::splat(inverse_scale)),
                    BatterySOCLabel,
                ));

                // SOC bar
                let bar_width = BATTERY_BAR_WIDTH;
                let bar_height = BATTERY_BAR_HEIGHT;
                let bar_y = BATTERY_BAR_Y;

                // Outline (subtle dark border)
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgba(0.2, 0.2, 0.2, 0.8),
                        custom_size: Some(Vec2::new(bar_width + 4.0, bar_height + 4.0)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, bar_y, 0.105).with_scale(Vec3::splat(inverse_scale)),
                ));

                // Background
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgba(0.12, 0.12, 0.16, 0.95),
                        custom_size: Some(Vec2::new(bar_width, bar_height)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, bar_y, 0.11).with_scale(Vec3::splat(inverse_scale)),
                ));

                // Fill (blue/cyan for battery) - starts at 50% width (initial SOC), left-anchored
                sprite_parent.spawn((
                    Sprite {
                        color: Color::srgb(0.3, 0.7, 0.9),
                        custom_size: Some(Vec2::new(bar_width * 0.5, bar_height)), // Start at 50%
                        ..default()
                    },
                    Transform::from_xyz(-bar_width / 2.0 * inverse_scale, bar_y, 0.12)
                        .with_scale(Vec3::splat(inverse_scale)),
                    Anchor::CENTER_LEFT,
                    BatterySOCBar,
                ));
            });
    });
}

/// Spawn a security system visual entity overlay (2x2 footprint)
fn spawn_security_system(
    commands: &mut Commands,
    root_entity: Entity,
    assets: &ImageAssets,
    anchor_x: i32,
    anchor_y: i32,
    site_id: crate::resources::SiteId,
) {
    let size = StructureSize::TwoByTwo;

    commands.entity(root_entity).with_children(|parent| {
        let world_pos = SiteGrid::multi_tile_center(anchor_x, anchor_y, size);
        let pole_scale = TILE_SCALE * 0.75; // 2x2 footprint, larger visual

        // Compute rotation so the camera lens faces toward the parking lot center.
        let lot_center = Vec2::new(
            GRID_OFFSET_X + (GRID_WIDTH as f32 / 2.0) * TILE_SIZE,
            GRID_OFFSET_Y + (GRID_HEIGHT as f32 / 2.0) * TILE_SIZE,
        );
        let to_center = lot_center - world_pos;
        let desired_angle = bevy::math::ops::atan2(to_center.y, to_center.x);
        // The camera head SVG has the arm+lens pointing upper-left at ~135° (3π/4)
        // in Bevy's coordinate system (Y-up). Subtract that so the lens aims at desired_angle.
        let camera_natural_angle = 3.0 * std::f32::consts::FRAC_PI_4; // 135° = 3π/4
        let angle_to_center = desired_angle - camera_natural_angle;

        // Spawn static pole/base (no rotation)
        parent
            .spawn((
                Sprite::from_image(assets.prop_security_pole.clone()),
                Transform::from_xyz(world_pos.x, world_pos.y, 2.0)
                    .with_scale(Vec3::new(pole_scale, pole_scale, 1.0)),
                SecuritySystemVisual,
                crate::components::BelongsToSite::new(site_id),
            ))
            .with_children(|pole| {
                // Camera head as child, positioned at pole top (local coords).
                // Pole SVG is 128x128, center at (64,64). Pole cap top is at SVG y=55.
                // Bevy Y is up, SVG Y is down, so offset = 64 - 55 = +9 image pixels.
                // As a child of a scaled sprite, local units = image pixels.
                let camera_scale = 1.0; // Camera head PNG is 64px, pole PNG is 128px
                let pole_top_y = 9.0;

                pole.spawn((
                    Sprite::from_image(assets.prop_security_camera_head.clone()),
                    Transform::from_xyz(0.0, pole_top_y, 0.1)
                        .with_scale(Vec3::new(camera_scale, camera_scale, 1.0))
                        .with_rotation(Quat::from_rotation_z(angle_to_center)),
                    SecurityCameraHead {
                        base_angle: angle_to_center,
                        timer: 0.0,
                    },
                ))
                .with_children(|cam| {
                    // Green active-recording LED on the camera head.
                    // Camera head SVG is 64x64, center at (32,32).
                    // Recording LED in SVG is at (26, 8).
                    // Local offset from center: x = 26-32 = -6, y = 32-8 = +24 (Y flipped).
                    cam.spawn((
                        Sprite {
                            color: Color::srgba(0.2, 1.0, 0.3, 0.8),
                            custom_size: Some(Vec2::new(5.0, 5.0)),
                            ..default()
                        },
                        Transform::from_xyz(-6.0, 24.0, 0.2),
                        SecurityCameraLed { timer: 0.0 },
                    ));
                });
            });
    });
}

/// Spawn an amenity building entity overlay (size depends on type, anchor at x,y which is bottom-left)
///
/// Base tiles are rendered by Tiled. This only spawns the amenity building prop sprite
/// for visual representation (amenities don't have dynamic gauges yet).
fn spawn_amenity_building(
    commands: &mut Commands,
    root_entity: Entity,
    assets: &ImageAssets,
    anchor_x: i32,
    anchor_y: i32,
    amenity_type: AmenityType,
    site_id: crate::resources::SiteId,
) {
    let size = amenity_type.size();

    commands.entity(root_entity).with_children(|parent| {
        // NOTE: Concrete pad tiles are rendered by Tiled, not spawned here

        // Get the appropriate sprite and scale for this amenity type
        let (sprite, scale) = match amenity_type {
            AmenityType::WifiRestrooms => (
                assets.prop_amenity_wifi_restrooms.clone(),
                TILE_SCALE * 0.3625, // 3x3 footprint (512px PNG -> 192px)
            ),
            AmenityType::LoungeSnacks => (
                assets.prop_amenity_lounge_snacks.clone(),
                TILE_SCALE * 0.325, // 4x4 footprint (768x512 PNG -> 256x256)
            ),
            AmenityType::Restaurant => (
                assets.prop_amenity_restaurant_premium.clone(),
                TILE_SCALE * 0.305, // 5x4 footprint (1024x768 PNG -> 320x256)
            ),
        };

        // Amenity building sprite centered on the footprint
        let grid_center = SiteGrid::multi_tile_center(anchor_x, anchor_y, size);
        parent.spawn((
            Sprite::from_image(sprite),
            Transform::from_xyz(grid_center.x, grid_center.y, 2.0).with_scale(Vec3::splat(scale)),
            AmenityBuildingVisual { amenity_type },
            crate::components::BelongsToSite::new(site_id),
        ));
    });
}

/// Mark grid as not needing refresh after visuals update
pub fn clear_grid_refresh_flag(mut multi_site: ResMut<crate::resources::MultiSiteManager>) {
    // Clear refresh flag for active site
    if let Some(site) = multi_site.active_site_mut() {
        site.grid.needs_visual_refresh = false;
    }
}

/// Legacy function - charger entity management is now handled by sync_chargers_with_grid.
/// Kept as no-op for backwards compatibility with system registration.
#[allow(dead_code)]
pub fn spawn_chargers_from_grid(
    _commands: Commands,
    _build_state: Res<crate::resources::BuildState>,
    _multi_site: Res<crate::resources::MultiSiteManager>,
    _existing_chargers: Query<(&Charger, &crate::components::BelongsToSite)>,
) {
    // No-op: charger entities are now managed by sync_chargers_with_grid
    // which runs during Playing state regardless of is_open
}

/// Result of computing charger diff between grid and existing entities.
/// This is a pure data structure for testability.
#[derive(Debug, Default)]
pub struct ChargerDiff {
    /// Positions where chargers should be despawned (no longer in grid)
    pub to_despawn: Vec<((i32, i32), Entity)>,
    /// Positions where chargers should be spawned (new in grid)
    pub to_spawn: Vec<(i32, i32, ChargerPadType)>,
}

/// Compute the diff between desired charger positions (from grid) and existing charger entities.
/// Pure function - no ECS side effects, can be unit tested.
pub fn compute_charger_diff(
    grid: &SiteGrid,
    existing_positions: &HashMap<(i32, i32), Entity>,
) -> ChargerDiff {
    use std::collections::HashSet;

    // Collect desired charger positions from grid
    let desired_chargers: HashSet<(i32, i32)> = grid
        .iter_tiles()
        .filter_map(|((x, y), tile)| {
            if tile.content == TileContent::ChargerPad && tile.charger_type.is_some() {
                Some((x, y))
            } else {
                None
            }
        })
        .collect();

    let mut diff = ChargerDiff::default();

    // Find chargers to despawn (exist but no longer in grid)
    for (&pos, &entity) in existing_positions {
        if !desired_chargers.contains(&pos) {
            diff.to_despawn.push((pos, entity));
        }
    }

    // Find chargers to spawn (in grid but don't exist)
    for &(x, y) in &desired_chargers {
        if !existing_positions.contains_key(&(x, y))
            && let Some(tile) = grid.get_tile(x, y)
            && let Some(charger_type) = tile.charger_type
        {
            diff.to_spawn.push((x, y, charger_type));
        }
    }

    diff
}

/// Sync charger entities with the grid.
/// Runs in Playing state regardless of is_open, ensuring chargers exist as entities
/// whenever they are placed on the grid (both build mode and during the day).
///
/// Uses revision-based change detection: only runs when grid.revision changes,
/// eliminating race conditions from frame ordering.
///
/// Spawns both the Charger entity and its ChargerSprite in the same command chain.
pub fn sync_chargers_with_grid(
    mut commands: Commands,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    existing_chargers: Query<(Entity, &Charger, &crate::components::BelongsToSite)>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    mut sync_revision: ResMut<ChargerSyncRevision>,
) {
    // Get active site info (read-only first pass)
    let (
        site_id,
        current_revision,
        root_entity,
        diff,
        advertiser_probability,
        existing_charger_count,
    ) = {
        let Some(site) = multi_site.active_site() else {
            return;
        };

        let grid = &site.grid;
        let site_id = site.id;
        let current_revision = grid.revision;
        // Get advertiser interest probability for video ad rolls
        let advertiser_probability = site.service_strategy.advertiser_interest_probability();

        // Check if we've already processed this revision
        let last_revision = sync_revision.0.get(&site_id).copied().unwrap_or(0);
        if current_revision == last_revision {
            return; // No changes since last sync
        }

        // Get site root entity
        let Some(root_entity) = site.root_entity else {
            warn!(
                "Site {:?} has no root entity, cannot sync chargers",
                site_id
            );
            return;
        };

        // Collect existing charger positions for this site
        let mut existing_positions: HashMap<(i32, i32), Entity> = HashMap::new();
        for (entity, charger, belongs) in &existing_chargers {
            if belongs.site_id == site_id
                && let Some(pos) = charger.grid_position
            {
                existing_positions.insert(pos, entity);
            }
        }

        // Compute diff using pure helper
        let diff = compute_charger_diff(grid, &existing_positions);

        let existing_charger_count = existing_positions.len();

        (
            site_id,
            current_revision,
            root_entity,
            diff,
            advertiser_probability,
            existing_charger_count,
        )
    };

    // Despawn chargers that are no longer in the grid
    for ((x, y), entity) in diff.to_despawn.iter() {
        commands.entity(*entity).try_despawn();
        info!(
            "Despawned charger at ({}, {}) for site {:?} - removed from grid",
            x, y, site_id
        );
    }

    // Track how many existing chargers for numbering
    // Count chargers that will remain (existing minus those being despawned)
    let remaining_count = existing_charger_count - diff.to_despawn.len();
    let mut next_charger_num = remaining_count + 1;

    // Collect positions to spawn so we can check pending_video_ad_chargers
    let spawn_positions: Vec<_> = diff.to_spawn.iter().map(|(x, y, _)| (*x, *y)).collect();

    // Get pending video ad chargers for this site (mutable access)
    let pending_video_ad: std::collections::HashSet<(i32, i32)> = {
        if let Some(site) = multi_site.active_site_mut() {
            // Take the positions that match what we're spawning
            let matched: std::collections::HashSet<_> = spawn_positions
                .iter()
                .filter(|pos| site.pending_video_ad_chargers.contains(pos))
                .copied()
                .collect();
            // Remove them from pending
            for pos in &matched {
                site.pending_video_ad_chargers.remove(pos);
            }
            matched
        } else {
            std::collections::HashSet::new()
        }
    };

    for (x, y, charger_pad_type) in diff.to_spawn.iter() {
        let grid_pos = SiteGrid::grid_to_world(*x, *y);

        let (bevy_charger_type, power_kw, tier) = match charger_pad_type {
            ChargerPadType::L2 => (ChargerType::AcLevel2, 7.0, ChargerTier::Standard),
            ChargerPadType::DCFC50 => (ChargerType::DcFast, 50.0, ChargerTier::Value),
            ChargerPadType::DCFC100 => (ChargerType::DcFast, 100.0, ChargerTier::Standard),
            ChargerPadType::DCFC150 => (ChargerType::DcFast, 150.0, ChargerTier::Standard),
            ChargerPadType::DCFC350 => (ChargerType::DcFast, 350.0, ChargerTier::Premium),
        };

        // Check if this charger should have video ads enabled
        // Player selected video ads for this charger AND an advertiser bought the space
        let has_video_ad_selected = pending_video_ad.contains(&(*x, *y));
        let video_ad_enabled = if has_video_ad_selected {
            // Roll probability to see if an advertiser buys the space
            let roll: f32 = rand::random();
            roll < advertiser_probability
        } else {
            false
        };

        // Get image handle and calculate scale from actual PNG dimensions
        let charger_image = get_charger_image(
            &image_assets,
            bevy_charger_type,
            power_kw,
            ChargerState::Available,
        );

        // Calculate scale using intended world size and actual PNG dimensions
        let intended_size =
            crate::resources::sprite_metadata::charger_world_size(bevy_charger_type, power_kw);
        let charger_scale = if let Some(image) = images.get(&charger_image) {
            intended_size.scale_for_image(image)
        } else {
            warn!(
                "Charger image not loaded yet for {:?} at ({}, {}), using fallback scale",
                charger_pad_type, x, y
            );
            // Fallback: use a reasonable default scale
            0.125 // This matches 32px / 256px for typical charger
        };

        let charger = Charger {
            id: format!("chg_{next_charger_num:02}"),
            name: format!("Charger {next_charger_num}"),
            charger_type: bevy_charger_type,
            max_power_kw: power_kw,
            rated_power_kw: power_kw,
            grid_position: Some((*x, *y)),
            tier,
            connector_jam_chance: 0.02, // 2% base chance
            video_ad_enabled,
            ..default()
        };

        // Spawn Charger with ChargerSprite as child (so sprite is despawned with charger)
        // For DCFC100 with video ads: the charger is offset left and a screen panel is on the right
        let has_ad_screen = matches!(charger_pad_type, ChargerPadType::DCFC100);

        // Calculate the horizontal offset for DCFC100 ad-screen layout
        let charger_x_offset = if has_ad_screen {
            -(intended_size.width / 2.0)
        } else {
            0.0
        };

        commands.entity(root_entity).with_children(|site_children| {
            // Spawn Charger and get entity commands
            let mut charger_commands = site_children.spawn((
                charger,
                Transform::from_xyz(grid_pos.x, grid_pos.y, 5.0),
                Visibility::default(),
                crate::components::BelongsToSite::new(site_id),
            ));
            let charger_entity = charger_commands.id();

            // Spawn ChargerSprite as child (local coordinates relative to parent)
            charger_commands.with_children(|charger_children| {
                charger_children.spawn((
                    Sprite::from_image(charger_image),
                    Transform::from_xyz(charger_x_offset, 0.0, 0.1)
                        .with_scale(Vec3::splat(charger_scale)),
                    ChargerSprite { charger_entity },
                ));

                // For DCFC100 with ads: spawn the physical screen panel to the right
                if has_ad_screen {
                    let screen_image = image_assets.charger_dcfc100_screen.clone();
                    // Screen panel uses the same scale as the charger so they match in size
                    charger_children.spawn((
                        Sprite::from_image(screen_image),
                        Transform::from_xyz(intended_size.width / 2.0, 0.0, 0.1)
                            .with_scale(Vec3::splat(charger_scale)),
                        crate::systems::sprite::AdScreenPanel { charger_entity },
                    ));
                }
            });
        });

        if has_video_ad_selected {
            if video_ad_enabled {
                info!(
                    "Synced charger with video ads (sold to advertiser) at ChargerPad ({}, {}) for site {:?}",
                    x, y, site_id
                );
            } else {
                info!(
                    "Synced charger with video ads (no advertiser at this price) at ChargerPad ({}, {}) for site {:?}",
                    x, y, site_id
                );
            }
        } else {
            info!(
                "Synced charger at ChargerPad ({}, {}) for site {:?}",
                x, y, site_id
            );
        }
        next_charger_num += 1;
    }

    // Update last processed revision
    sync_revision.0.insert(site_id, current_revision);
}

/// Spawn transformer logic entities when station opens (one per transformer on grid)
pub fn spawn_transformer_from_grid(
    mut commands: Commands,
    multi_site: Res<crate::resources::MultiSiteManager>,
    build_state: Res<crate::resources::BuildState>,
    existing_transformers: Query<Entity, With<Transformer>>,
) {
    // Only run when station just opened and no transformer entities exist
    if !build_state.is_open {
        return;
    }
    if !existing_transformers.is_empty() {
        return;
    }

    // Get active site grid or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let grid = &site.grid;

    // Only spawn if there are transformers on the grid
    if !grid.has_transformer() {
        return;
    }

    // Create a transformer logic entity for each transformer on the grid
    for transformer_placement in &grid.transformers {
        let transformer = Transformer {
            site_id: site.id,
            grid_pos: transformer_placement.pos,
            rating_kva: transformer_placement.kva,
            thermal_limit_c: 110.0,
            ..default()
        };

        commands.spawn(transformer);
        info!(
            "Spawned transformer logic entity at {:?} with {} kVA capacity",
            transformer_placement.pos, transformer_placement.kva
        );
    }

    info!(
        "Spawned {} transformer(s) with total {} kVA capacity",
        grid.transformer_count(),
        grid.total_transformer_capacity()
    );
}
