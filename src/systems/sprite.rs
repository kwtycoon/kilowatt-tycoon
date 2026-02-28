//! Sprite management and visual updates
//!
//! All rendering uses PNG assets (generated from SVG source files).
//! If assets are missing, the game will fail during the Loading state.
//!
//! ## Transform Hierarchy Convention
//!
//! Driver entities are children of site roots. All visual sprites are children of drivers.
//! Child sprites use LOCAL coordinates (offsets from parent), NOT world coordinates.
//!
//! ```text
//! Site Root (world position)
//!   └─ Driver (local position from VehicleMovement)
//!        ├─ VehicleSprite (local: 0,0 + rotation)
//!        ├─ DriverCharacterSprite (local: offset from center)
//!        └─ SoC bars (local: offset from center)
//! ```
//!
//! Only update Driver.Transform for position. Children inherit automatically.

use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::{
    AntiTheftShieldIndicator, Charger, ChargerSprite, ChargerState, ChargerType,
};
use crate::components::driver::{Driver, DriverMood, DriverState, VehicleMovement, VehicleType};
use crate::resources::{
    ImageAssets, MultiSiteManager,
    sprite_metadata::{self, VfxType},
};

// ============ Colors (kept for reference/future use) ============

/// Colors from style guide (kept for non-sprite UI elements)
pub mod colors {
    use bevy::prelude::Color;

    // State indicator colors (for UI, not sprites)
    pub const AVAILABLE: Color = Color::srgb(0.2, 0.8, 0.2); // Green
    pub const CHARGING: Color = Color::srgb(0.2, 0.5, 1.0); // Blue
    pub const WARNING: Color = Color::srgb(1.0, 0.8, 0.2); // Yellow
    pub const OFFLINE: Color = Color::srgb(0.8, 0.2, 0.2); // Red
    pub const DISABLED: Color = Color::srgb(0.4, 0.4, 0.4); // Gray

    // Environment colors (for reference)
    pub const ASPHALT: Color = Color::srgb(0.15, 0.17, 0.2); // Dark blue-gray
    pub const CONCRETE: Color = Color::srgb(0.7, 0.68, 0.65); // Warm light gray
    pub const GRASS: Color = Color::srgb(0.3, 0.5, 0.3); // Muted green
    pub const PARKING_LINE: Color = Color::srgb(0.9, 0.9, 0.9); // White lines

    // Charger body colors (for reference)
    pub const CHARGER_DCFC: Color = Color::srgb(0.25, 0.25, 0.3); // Dark gray body
    pub const CHARGER_L2: Color = Color::srgb(0.35, 0.35, 0.4); // Lighter gray body

    // Vehicle colors (for reference)
    pub const VEHICLE_1: Color = Color::srgb(0.8, 0.1, 0.1); // Red
    pub const VEHICLE_2: Color = Color::srgb(0.1, 0.3, 0.8); // Blue
    pub const VEHICLE_3: Color = Color::srgb(0.9, 0.9, 0.9); // White
    pub const VEHICLE_4: Color = Color::srgb(0.2, 0.2, 0.2); // Black
    pub const VEHICLE_5: Color = Color::srgb(0.6, 0.6, 0.6); // Silver

    // Driver mood colors (for reference)
    pub const MOOD_NEUTRAL: Color = Color::srgb(0.4, 0.7, 0.4);
    pub const MOOD_IMPATIENT: Color = Color::srgb(0.9, 0.7, 0.2);
    pub const MOOD_ANGRY: Color = Color::srgb(0.9, 0.3, 0.2);
    pub const MOOD_HAPPY: Color = Color::srgb(0.3, 0.9, 0.4);

    // Selection
    pub const SELECTION_HIGHLIGHT: Color = Color::srgb(0.0, 0.8, 1.0);
}

// ============ Marker Components ============

// Note: ChargerSprite is defined in components/charger.rs as the canonical location.
// ChargerSprite spawning is now done in sync_chargers_with_grid (scene.rs).

/// Marker for vehicle sprite entities
#[derive(Component)]
pub struct VehicleSprite {
    pub driver_entity: Entity,
}

/// Marker for driver character sprite
#[derive(Component)]
pub struct DriverCharacterSprite {
    pub driver_entity: Entity,
}

/// Marker for parking lot background elements
#[derive(Component)]
pub struct ParkingLotElement;

/// Marker for vehicle SoC bar background
#[derive(Component)]
pub struct VehicleSoCBackground {
    pub driver_entity: Entity,
}

/// Marker for vehicle SoC bar fill
#[derive(Component)]
pub struct VehicleSoCBar {
    pub driver_entity: Entity,
}

/// Marker for vehicle SoC bar border (outer outline)
#[derive(Component)]
pub struct VehicleSoCBorder {
    pub driver_entity: Entity,
}

/// SoC bar constants
const SOC_BAR_WIDTH: f32 = 50.0;
const SOC_BAR_HEIGHT: f32 = 10.0;
const SOC_BAR_OFFSET_Y: f32 = 60.0; // Raised to avoid overlap with chargers

/// Floating money VFX component
#[derive(Component)]
pub struct FloatingMoney {
    /// How long the VFX has been alive
    pub lifetime: f32,
    /// Total duration before despawn
    pub max_lifetime: f32,
    /// Amount to display (for future text rendering)
    pub amount: f32,
}

impl FloatingMoney {
    pub fn new(amount: f32) -> Self {
        Self {
            lifetime: 0.0,
            max_lifetime: 1.6, // Extended for better visibility
            amount,
        }
    }
}

/// Pulsing fault indicator VFX component
#[derive(Component)]
pub struct FaultPulseVfx {
    pub charger_entity: Entity,
    pub pulse_time: f32,
}

/// Marker component for floating money text
#[derive(Component)]
pub struct FloatingMoneyText;

/// Marker for technician sprite
#[derive(Component)]
pub struct TechnicianSprite {
    pub technician_entity: Entity,
}

/// Floating loot speech bubble above a fleeing robber (world-space text)
#[derive(Component)]
pub struct RobberLootBubble {
    pub robber_entity: Entity,
    pub lifetime: f32,
}

/// Stolen cable sprite that follows a fleeing robber (world-space sprite)
#[derive(Component)]
pub struct StolenCableSprite {
    pub robber_entity: Entity,
}

/// Marker for technician travel progress indicator
#[derive(Component)]
pub struct TechnicianTravelIndicator;

/// Floating wrench VFX component
#[derive(Component)]
pub struct FloatingWrench {
    /// How long the VFX has been alive
    pub lifetime: f32,
    /// Total duration before despawn
    pub max_lifetime: f32,
    /// Horizontal velocity for burst spread
    pub velocity_x: f32,
    /// Initial rotation for visual variety
    pub rotation_speed: f32,
}

impl Default for FloatingWrench {
    fn default() -> Self {
        Self::new()
    }
}

impl FloatingWrench {
    pub fn new() -> Self {
        Self {
            lifetime: 0.0,
            max_lifetime: 1.2,
            velocity_x: 0.0,
            rotation_speed: 0.0,
        }
    }

    /// Create with velocity for burst effect
    pub fn with_velocity(velocity_x: f32, rotation_speed: f32) -> Self {
        Self {
            lifetime: 0.0,
            max_lifetime: 1.5,
            velocity_x,
            rotation_speed,
        }
    }
}

/// Marker component for floating wrench text (if needed)
#[derive(Component)]
pub struct FloatingWrenchText;

// ============ Charger Sprite Systems ============

/// Get charger image handle based on type, power rating, and state
pub fn get_charger_image(
    assets: &ImageAssets,
    charger_type: ChargerType,
    rated_power_kw: f32,
    state: ChargerState,
) -> Handle<Image> {
    match charger_type {
        ChargerType::DcFast => {
            // Select DCFC variant based on power rating
            if rated_power_kw <= 75.0 {
                // 50kW compact DCFC
                match state {
                    ChargerState::Available => assets.charger_dcfc50_available.clone(),
                    ChargerState::Charging => assets.charger_dcfc50_charging.clone(),
                    ChargerState::Warning => assets.charger_dcfc50_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        assets.charger_dcfc50_offline.clone()
                    }
                }
            } else if rated_power_kw <= 125.0 {
                // 100kW standard DCFC (built-in ad screen)
                match state {
                    ChargerState::Available => assets.charger_dcfc100_available.clone(),
                    ChargerState::Charging => assets.charger_dcfc100_charging.clone(),
                    ChargerState::Warning => assets.charger_dcfc100_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        assets.charger_dcfc100_offline.clone()
                    }
                }
            } else if rated_power_kw <= 200.0 {
                // 150kW standard DCFC
                match state {
                    ChargerState::Available => assets.charger_dcfc150_available.clone(),
                    ChargerState::Charging => assets.charger_dcfc150_charging.clone(),
                    ChargerState::Warning => assets.charger_dcfc150_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        assets.charger_dcfc150_offline.clone()
                    }
                }
            } else {
                // 350kW premium DCFC
                match state {
                    ChargerState::Available => assets.charger_dcfc350_available.clone(),
                    ChargerState::Charging => assets.charger_dcfc350_charging.clone(),
                    ChargerState::Warning => assets.charger_dcfc350_warning.clone(),
                    ChargerState::Offline | ChargerState::Disabled => {
                        assets.charger_dcfc350_offline.clone()
                    }
                }
            }
        }
        ChargerType::AcLevel2 => match state {
            ChargerState::Available => assets.charger_l2_available.clone(),
            ChargerState::Charging => assets.charger_l2_charging.clone(),
            ChargerState::Warning => assets.charger_l2_warning.clone(),
            ChargerState::Offline | ChargerState::Disabled => assets.charger_l2_offline.clone(),
        },
    }
}

// Note: spawn_charger_sprites was removed - ChargerSprite is now spawned
// in sync_chargers_with_grid (scene.rs) in the same command chain as the Charger entity.
// This eliminates deferred command timing issues.

/// Update charger sprites based on state changes
pub fn update_charger_sprites(
    chargers: Query<(Entity, &Charger), Changed<Charger>>,
    mut charger_sprites: Query<(&ChargerSprite, &mut Sprite)>,
    image_assets: Res<ImageAssets>,
) {
    for (charger_sprite, mut sprite_component) in &mut charger_sprites {
        if let Ok((_, charger)) = chargers.get(charger_sprite.charger_entity) {
            let new_image = get_charger_image(
                &image_assets,
                charger.charger_type,
                charger.rated_power_kw,
                charger.state(),
            );
            sprite_component.image = new_image;
        }
    }
}

// ============ Vehicle Sprite Systems ============

/// Get vehicle dimensions based on type
pub fn vehicle_dimensions(vehicle_type: VehicleType) -> Vec2 {
    match vehicle_type {
        VehicleType::Compact => Vec2::new(35.0, 60.0),
        VehicleType::Sedan => Vec2::new(40.0, 75.0),
        VehicleType::Suv => Vec2::new(45.0, 85.0),
        VehicleType::Crossover => Vec2::new(42.0, 78.0),
        VehicleType::Pickup => Vec2::new(48.0, 95.0),
        VehicleType::Bus => Vec2::new(55.0, 110.0),
        VehicleType::Semi => Vec2::new(58.0, 140.0),
        VehicleType::Tractor => Vec2::new(50.0, 80.0),
        VehicleType::Scooter => Vec2::new(22.0, 45.0),
        VehicleType::Motorcycle => Vec2::new(26.0, 65.0),
    }
}

/// Get vehicle image handle based on type
fn get_vehicle_image(assets: &ImageAssets, vehicle_type: VehicleType) -> Handle<Image> {
    match vehicle_type {
        VehicleType::Compact => assets.vehicle_compact.clone(),
        VehicleType::Sedan => assets.vehicle_sedan.clone(),
        VehicleType::Suv => assets.vehicle_suv.clone(),
        VehicleType::Crossover => assets.vehicle_crossover.clone(),
        VehicleType::Pickup => assets.vehicle_pickup.clone(),
        VehicleType::Bus => assets.vehicle_bus.clone(),
        VehicleType::Semi => assets.vehicle_semi.clone(),
        VehicleType::Tractor => assets.vehicle_tractor.clone(),
        VehicleType::Scooter => assets.vehicle_scooter.clone(),
        VehicleType::Motorcycle => assets.vehicle_motorcycle.clone(),
    }
}

/// Get mood icon image handle
pub fn get_mood_image(assets: &ImageAssets, mood: DriverMood) -> Handle<Image> {
    match mood {
        DriverMood::Neutral => assets.icon_mood_neutral.clone(),
        DriverMood::Impatient => assets.icon_mood_impatient.clone(),
        DriverMood::Angry => assets.icon_mood_angry.clone(),
        DriverMood::Happy => assets.icon_mood_happy.clone(),
    }
}

/// Spawn vehicle sprites for drivers
///
/// NOTE: Movement is created by driver_spawn_system using grid pathfinding.
/// This system only spawns visual sprites; it does NOT create movement.
///
/// If the driver was converted from an ambient vehicle, it already has a VehicleSprite.
/// In that case, we skip spawning the vehicle sprite but still spawn driver-specific
/// elements (character sprite, SoC bars).
pub fn spawn_vehicle_sprites(
    mut commands: Commands,
    drivers: Query<
        (
            Entity,
            &Driver,
            Option<&VehicleMovement>,
            &crate::components::BelongsToSite,
        ),
        Added<Driver>,
    >,
    existing_vehicle_sprites: Query<&VehicleSprite>,
    multi_site: Res<MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    // Return early if no active site
    if multi_site.active_site().is_none() {
        return;
    }

    for (driver_entity, driver, _movement_opt, belongs_to_site) in &drivers {
        // Check if this driver already has a vehicle sprite (e.g., converted from ambient traffic)
        let has_existing_sprite = existing_vehicle_sprites
            .iter()
            .any(|vs| vs.driver_entity == driver_entity);

        // Get vehicle image handle
        let vehicle_image = get_vehicle_image(&image_assets, driver.vehicle_type);

        // Calculate scale using intended world size and actual PNG dimensions
        let intended_size =
            crate::resources::sprite_metadata::vehicle_world_size(driver.vehicle_type);
        let vehicle_scale = if let Some(image) = images.get(&vehicle_image) {
            intended_size.scale_for_image(image)
        } else {
            // Fallback if image not loaded yet
            0.2 // Reasonable default for most vehicles
        };

        // Spawn all sprites as children of the driver entity (which is already a child of site root)
        // All child sprites use LOCAL coordinates (offsets from driver), NOT world coordinates.
        // The driver entity's position is updated by update_vehicle_positions; children inherit automatically.
        commands.entity(driver_entity).with_children(|parent| {
            // Only spawn vehicle sprite if one doesn't already exist
            // (ambient vehicles converted to drivers already have their vehicle sprite)
            if !has_existing_sprite {
                info!(
                    "Spawning vehicle sprite for {}: type={:?}, scale={:.3}, local=(0, 0, 2)",
                    driver.id, driver.vehicle_type, vehicle_scale
                );

                // Vehicle body sprite - Z = 2.0 to render above parking lines/decals (Z ~1.3)
                parent.spawn((
                    Sprite::from_image(vehicle_image),
                    Transform::from_xyz(0.0, 0.0, 2.0).with_scale(Vec3::splat(vehicle_scale)),
                    VehicleSprite { driver_entity },
                    *belongs_to_site,
                ));
            } else {
                info!(
                    "Driver {} already has vehicle sprite (converted from ambient)",
                    driver.id
                );
            }

            // Always spawn driver-specific elements (mood icon, SoC bars)
            // All use LOCAL offsets from driver center

            // Mood icon - centered on vehicle to show driver emotion
            // Z = 3.0 to render just above vehicle sprite (Z = 2.0)
            let mood_image = get_mood_image(&image_assets, driver.mood);
            let icon_size = sprite_metadata::icon_world_size();
            let icon_scale = if let Some(image) = images.get(&mood_image) {
                icon_size.scale_for_image(image)
            } else {
                0.3 // Fallback
            };
            parent.spawn((
                Sprite::from_image(mood_image),
                Transform::from_xyz(0.0, 0.0, 3.0).with_scale(Vec3::splat(icon_scale)),
                DriverCharacterSprite { driver_entity },
                *belongs_to_site,
            ));

            // SoC bar border (light gray outline for visibility) - above vehicle center
            // Z = 11.0 to render above chargers (which are at Z ~5.1)
            parent.spawn((
                Sprite {
                    color: Color::srgba(0.7, 0.7, 0.7, 0.95),
                    custom_size: Some(Vec2::new(SOC_BAR_WIDTH + 6.0, SOC_BAR_HEIGHT + 6.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, SOC_BAR_OFFSET_Y, 11.0),
                VehicleSoCBorder { driver_entity },
                *belongs_to_site,
            ));

            // SoC bar background (dark inner area) - above vehicle center
            // Z = 11.5 to render above chargers (which are at Z ~5.1)
            parent.spawn((
                Sprite {
                    color: Color::srgba(0.15, 0.15, 0.18, 1.0),
                    custom_size: Some(Vec2::new(SOC_BAR_WIDTH + 2.0, SOC_BAR_HEIGHT + 2.0)),
                    ..default()
                },
                Transform::from_xyz(0.0, SOC_BAR_OFFSET_Y, 11.5),
                VehicleSoCBackground { driver_entity },
                *belongs_to_site,
            ));

            // SoC bar fill (starts with minimum visible width)
            // Position adjusted for left-anchor effect using local offset
            // Z = 12.0 to render above chargers (which are at Z ~5.1)
            let initial_progress = driver.charge_progress();
            let fill_color = soc_color(initial_progress);
            let fill_width = (SOC_BAR_WIDTH * initial_progress).max(4.0);
            parent.spawn((
                Sprite {
                    color: fill_color,
                    custom_size: Some(Vec2::new(fill_width, SOC_BAR_HEIGHT)),
                    ..default()
                },
                Transform::from_xyz(
                    -SOC_BAR_WIDTH / 2.0 + fill_width / 2.0,
                    SOC_BAR_OFFSET_Y,
                    12.0,
                ),
                VehicleSoCBar { driver_entity },
                *belongs_to_site,
            ));
        });
    }
}

/// Get color for SoC bar based on charge progress (more vibrant colors)
fn soc_color(progress: f32) -> Color {
    if progress < 0.3 {
        Color::srgb(1.0, 0.2, 0.2) // Bright red
    } else if progress < 0.7 {
        Color::srgb(1.0, 0.85, 0.1) // Bright yellow
    } else {
        Color::srgb(0.1, 0.95, 0.3) // Bright green
    }
}

/// Update vehicle positions based on movement or driver state
///
/// Only updates:
/// 1. Driver entity's position (from VehicleMovement) - children inherit this automatically
/// 2. Vehicle sprite's rotation (smooth interpolation toward movement direction)
/// 3. SoC bar fill x-offset (for left-anchor effect based on charge progress)
///
/// Character sprites, SoC borders, and SoC backgrounds use fixed local offsets
/// and don't need per-frame position updates - they inherit from the driver parent.
pub fn update_vehicle_positions(
    mut drivers: Query<(
        Entity,
        &Driver,
        Option<&VehicleMovement>,
        &mut Transform,
        Option<&bevy_northstar::prelude::AgentPos>,
    )>,
    mut vehicles: Query<
        (&VehicleSprite, &mut Transform),
        (Without<Driver>, Without<VehicleSoCBar>),
    >,
    mut soc_bars: Query<(&VehicleSoCBar, &mut Transform), Without<Driver>>,
) {
    for (driver_entity, driver, movement_opt, mut driver_transform, agent_pos_opt) in &mut drivers {
        if let Some(movement) = movement_opt {
            // Update Driver entity's transform to match current position
            // Position is relative to site root - children (vehicle, character, SoC) inherit automatically
            //
            // IMPORTANT: Skip position update for northstar-controlled entities (those with AgentPos).
            // Their transform is managed by northstar_move_vehicles system.
            if agent_pos_opt.is_none()
                && let Some(pos) = movement.current_position()
            {
                driver_transform.translation.x = pos.x;
                driver_transform.translation.y = pos.y;
            }

            // Update vehicle sprite ROTATION only (position inherited from driver parent)
            for (vehicle, mut transform) in &mut vehicles {
                if vehicle.driver_entity == driver_entity {
                    // Smooth rotation toward target (0.0 means align with parent orientation)
                    let target_rot = 0.0;
                    let current_rot = transform.rotation.to_euler(EulerRot::ZYX).0;
                    let diff = (target_rot - current_rot).rem_euclid(std::f32::consts::TAU);
                    let shortest = if diff > std::f32::consts::PI {
                        diff - std::f32::consts::TAU
                    } else {
                        diff
                    };
                    let new_rot = current_rot + shortest * 0.12;
                    transform.rotation = Quat::from_rotation_z(new_rot);
                }
            }
        }

        // Update SoC bar fill x-position for left-anchor effect (local offset based on progress)
        let progress = driver.charge_progress();
        let fill_width = SOC_BAR_WIDTH * progress;
        for (soc_bar, mut transform) in &mut soc_bars {
            if soc_bar.driver_entity == driver_entity {
                // Adjust local x-offset to simulate left-anchored bar
                transform.translation.x = -SOC_BAR_WIDTH / 2.0 + fill_width / 2.0;
            }
        }
    }
}

/// Update driver character sprite when mood changes
pub fn update_driver_mood_sprites(
    drivers: Query<(Entity, &Driver), Changed<Driver>>,
    mut character_sprites: Query<(&DriverCharacterSprite, &mut Sprite)>,
    image_assets: Res<ImageAssets>,
) {
    for (driver_entity, driver) in &drivers {
        for (character, mut sprite) in &mut character_sprites {
            if character.driver_entity == driver_entity {
                let new_image = get_mood_image(&image_assets, driver.mood);
                // Only update if the image handle actually changed to avoid triggering
                // unnecessary change detection and potential rendering glitches
                if sprite.image != new_image {
                    sprite.image = new_image;
                }
            }
        }
    }
}

/// Update SoC indicators based on driver charge progress
pub fn update_soc_indicators(
    drivers: Query<(Entity, &Driver)>,
    mut soc_bars: Query<(&VehicleSoCBar, &mut Sprite)>,
) {
    for (driver_entity, driver) in &drivers {
        let progress = driver.charge_progress();
        let fill_width = SOC_BAR_WIDTH * progress;

        for (soc_bar, mut sprite) in &mut soc_bars {
            if soc_bar.driver_entity == driver_entity {
                // Update bar width based on progress (minimum 4px for visibility)
                // Position is updated in update_vehicle_positions
                sprite.custom_size = Some(Vec2::new(fill_width.max(4.0), SOC_BAR_HEIGHT));

                // Update color based on progress
                sprite.color = soc_color(progress);
            }
        }
    }
}

/// Cleanup sprites for despawned drivers
///
/// NOTE: This system checks for both Driver AND AmbientVehicle parents.
/// Ambient vehicles use VehicleSprite and DriverCharacterSprite but don't have Driver components.
/// We only despawn sprites when NEITHER a Driver nor an AmbientVehicle exists for the parent entity.
pub fn cleanup_driver_sprites(
    mut commands: Commands,
    drivers: Query<Entity, With<Driver>>,
    ambient_vehicles: Query<Entity, With<crate::systems::ambient_traffic::AmbientVehicle>>,
    vehicles: Query<(Entity, &VehicleSprite)>,
    characters: Query<(Entity, &DriverCharacterSprite)>,
    soc_borders: Query<(Entity, &VehicleSoCBorder)>,
    soc_backgrounds: Query<(Entity, &VehicleSoCBackground)>,
    soc_bars: Query<(Entity, &VehicleSoCBar)>,
) {
    // Find vehicles whose parent (driver or ambient vehicle) no longer exists
    for (entity, vehicle) in &vehicles {
        let parent_entity = vehicle.driver_entity;
        let is_driver = drivers.get(parent_entity).is_ok();
        let is_ambient = ambient_vehicles.get(parent_entity).is_ok();
        if !is_driver && !is_ambient {
            commands.entity(entity).try_despawn();
        }
    }

    // Find SoC borders whose drivers no longer exist
    // Note: SoC bars are only for Driver entities, not ambient vehicles
    for (entity, soc_border) in &soc_borders {
        if drivers.get(soc_border.driver_entity).is_err() {
            commands.entity(entity).try_despawn();
        }
    }

    // Find characters whose parent (driver or ambient vehicle) no longer exists
    for (entity, character) in &characters {
        let parent_entity = character.driver_entity;
        let is_driver = drivers.get(parent_entity).is_ok();
        let is_ambient = ambient_vehicles.get(parent_entity).is_ok();
        if !is_driver && !is_ambient {
            commands.entity(entity).try_despawn();
        }
    }

    // Find SoC backgrounds whose drivers no longer exist
    for (entity, soc_bg) in &soc_backgrounds {
        if drivers.get(soc_bg.driver_entity).is_err() {
            commands.entity(entity).try_despawn();
        }
    }

    // Find SoC bars whose drivers no longer exist
    for (entity, soc_bar) in &soc_bars {
        if drivers.get(soc_bar.driver_entity).is_err() {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Spawn a floating money VFX at the given position
pub fn spawn_floating_money(
    commands: &mut Commands,
    image_assets: &ImageAssets,
    images: &Assets<Image>,
    position: Vec3,
    amount: f32,
) {
    let vfx_image = image_assets.vfx_float_money.clone();
    let vfx_size = sprite_metadata::vfx_world_size(VfxType::FloatingMoney);
    let vfx_scale = if let Some(image) = images.get(&vfx_image) {
        vfx_size.scale_for_image(image)
    } else {
        0.8 // Fallback
    };

    commands
        .spawn((
            Sprite::from_image(vfx_image),
            Transform::from_xyz(position.x, position.y + 20.0, 10.0)
                .with_scale(Vec3::splat(vfx_scale)),
            FloatingMoney::new(amount),
        ))
        .with_children(|parent| {
            // Spawn text showing the dollar amount next to the icon
            // Gold color matches HUD cash display for visual consistency
            parent.spawn((
                Text2d::new(format!("+${amount:.2}")),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.2)),
                Transform::from_xyz(30.0, 0.0, 0.1),
                FloatingMoneyText,
            ));
        });
}

/// Update floating money VFX - animate upward and fade out
pub fn update_floating_money(
    mut commands: Commands,
    time: Res<Time>,
    images: Res<Assets<Image>>,
    mut money_query: Query<(
        Entity,
        &mut FloatingMoney,
        &mut Transform,
        &mut Sprite,
        &Children,
    )>,
    mut text_query: Query<&mut TextColor, With<FloatingMoneyText>>,
) {
    for (entity, mut money, mut transform, mut sprite, children) in &mut money_query {
        money.lifetime += time.delta_secs();

        // Float upward
        transform.translation.y += 40.0 * time.delta_secs();

        // Fade out
        let progress = money.lifetime / money.max_lifetime;
        let alpha = (1.0 - progress).clamp(0.0, 1.0);
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);

        // Fade text color to match sprite (gold color)
        for child in children.iter() {
            if let Ok(mut text_color) = text_query.get_mut(child) {
                text_color.0 = Color::srgba(1.0, 0.85, 0.2, alpha);
            }
        }

        // Pop animation: start big, quickly settle, then slowly shrink
        // Creates a satisfying "punch" effect when money appears
        let scale_multiplier = if progress < 0.1 {
            // Quick pop: 1.3 -> 1.0 in first 10%
            1.3 - progress * 3.0
        } else if progress < 0.3 {
            // Hold at full size
            1.0
        } else {
            // Slow shrink over remaining time
            1.0 - (progress - 0.3) * 0.4
        };

        // Calculate base scale from image dimensions
        let vfx_size = sprite_metadata::vfx_world_size(VfxType::FloatingMoney);
        let base_scale = if let Some(image) = images.get(&sprite.image) {
            vfx_size.scale_for_image(image)
        } else {
            0.8 // Fallback
        };
        transform.scale = Vec3::splat(base_scale * scale_multiplier);

        // Despawn when done (despawn() automatically handles children in Bevy 0.17+)
        if money.lifetime >= money.max_lifetime {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Spawn a floating wrench VFX at the given position (repair completion effect)
pub fn spawn_floating_wrench(
    commands: &mut Commands,
    image_assets: &ImageAssets,
    images: &Assets<Image>,
    position: Vec3,
) {
    let vfx_image = image_assets.vfx_float_wrench.clone();
    let vfx_size = sprite_metadata::vfx_world_size(VfxType::FloatingWrench);
    let vfx_scale = if let Some(image) = images.get(&vfx_image) {
        vfx_size.scale_for_image(image)
    } else {
        0.8 // Fallback
    };

    commands.spawn((
        Sprite::from_image(vfx_image),
        Transform::from_xyz(position.x, position.y + 20.0, 10.0).with_scale(Vec3::splat(vfx_scale)),
        FloatingWrench::new(),
    ));
}

/// Spawn a burst of floating wrenches for a celebratory repair completion effect
pub fn spawn_wrench_burst(
    commands: &mut Commands,
    image_assets: &ImageAssets,
    images: &Assets<Image>,
    position: Vec3,
) {
    let icon_image = image_assets.icon_technician.clone();
    let vfx_size = sprite_metadata::vfx_world_size(VfxType::FaultPulse);
    let vfx_scale = if let Some(image) = images.get(&icon_image) {
        vfx_size.scale_for_image(image)
    } else {
        0.4 // Fallback
    };

    // Spawn technician icons spreading outward for celebration effect
    let burst_configs = [
        (-25.0, 2.0), // Left, spinning clockwise
        (0.0, -1.5),  // Center, spinning counter-clockwise
        (25.0, 2.0),  // Right, spinning clockwise
    ];

    for (velocity_x, rotation_speed) in burst_configs {
        commands.spawn((
            Sprite::from_image(icon_image.clone()),
            Transform::from_xyz(position.x, position.y + 15.0, 10.0)
                .with_scale(Vec3::splat(vfx_scale)),
            FloatingWrench::with_velocity(velocity_x, rotation_speed),
        ));
    }
}

/// Update floating wrench VFX - animate upward and fade out
pub fn update_floating_wrench(
    mut commands: Commands,
    time: Res<Time>,
    images: Res<Assets<Image>>,
    mut wrench_query: Query<(Entity, &mut FloatingWrench, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut wrench, mut transform, mut sprite) in &mut wrench_query {
        let dt = time.delta_secs();
        wrench.lifetime += dt;

        // Float upward
        transform.translation.y += 50.0 * dt;

        // Apply horizontal velocity (with decay for arc effect)
        let decay = 1.0 - (wrench.lifetime / wrench.max_lifetime).min(1.0);
        transform.translation.x += wrench.velocity_x * dt * decay;

        // Apply rotation
        transform.rotation *= Quat::from_rotation_z(wrench.rotation_speed * dt);

        // Fade out
        let progress = wrench.lifetime / wrench.max_lifetime;
        let alpha = (1.0 - progress).clamp(0.0, 1.0);
        sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha);

        // Scale up slightly at start, then shrink
        let scale_multiplier = if progress < 0.2 {
            0.8 + progress * 1.0
        } else {
            1.0 - (progress - 0.2) * 0.3
        };

        // Calculate base scale from image dimensions
        let vfx_size = sprite_metadata::vfx_world_size(VfxType::FloatingWrench);
        let base_scale = if let Some(image) = images.get(&sprite.image) {
            vfx_size.scale_for_image(image)
        } else {
            0.8 // Fallback
        };
        transform.scale = Vec3::splat(base_scale * scale_multiplier);

        // Despawn when done
        if wrench.lifetime >= wrench.max_lifetime {
            commands.entity(entity).try_despawn();
        }
    }
}

// ============ Fault Pulse VFX ============

/// Spawn pulsing VFX overlays for chargers with faults
pub fn spawn_fault_pulse_vfx(
    mut commands: Commands,
    chargers: Query<(Entity, &Charger, &Transform), Added<Charger>>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    let vfx_image = image_assets.vfx_urgent_pulse.clone();
    let vfx_size = sprite_metadata::vfx_world_size(VfxType::FaultPulse);
    let base_scale = if let Some(image) = images.get(&vfx_image) {
        vfx_size.scale_for_image(image)
    } else {
        0.4 // Fallback
    };

    for (entity, charger, transform) in &chargers {
        if charger.current_fault.is_some() {
            let pos = transform.translation;

            commands.spawn((
                Sprite::from_image(vfx_image.clone()),
                Transform::from_xyz(pos.x, pos.y, 10.0).with_scale(Vec3::splat(base_scale)),
                FaultPulseVfx {
                    charger_entity: entity,
                    pulse_time: 0.0,
                },
            ));
        }
    }
}

/// Update fault pulse VFX (pulsing animation and position sync)
pub fn update_fault_pulse_vfx(
    mut commands: Commands,
    mut vfx_query: Query<(Entity, &mut FaultPulseVfx, &mut Transform, &mut Sprite)>,
    chargers: Query<(&Charger, &Transform), Without<FaultPulseVfx>>,
    time: Res<Time>,
    images: Res<Assets<Image>>,
    _image_assets: Res<ImageAssets>,
) {
    for (vfx_entity, mut pulse, mut vfx_transform, mut sprite) in &mut vfx_query {
        // Check if charger still has fault
        let Ok((charger, charger_transform)) = chargers.get(pulse.charger_entity) else {
            // Charger no longer exists, despawn VFX
            commands.entity(vfx_entity).try_despawn();
            continue;
        };

        if charger.current_fault.is_none() {
            // Fault cleared, despawn VFX
            commands.entity(vfx_entity).try_despawn();
            continue;
        }

        // Update position to match charger
        vfx_transform.translation.x = charger_transform.translation.x;
        vfx_transform.translation.y = charger_transform.translation.y;

        // Pulsing animation
        pulse.pulse_time += time.delta_secs() * 2.0; // 2x speed
        let pulse_value = pulse.pulse_time.sin() * 0.5 + 0.5; // 0.0 to 1.0

        // Pulsing scale
        let vfx_size = sprite_metadata::vfx_world_size(VfxType::FaultPulse);
        let base_scale = if let Some(image) = images.get(&sprite.image) {
            vfx_size.scale_for_image(image)
        } else {
            0.4 // Fallback
        };
        let scale_variation = base_scale * 0.25; // 25% variation
        let scale = base_scale + pulse_value * scale_variation;
        vfx_transform.scale = Vec3::splat(scale);

        // Pulsing alpha
        let alpha = 0.4 + pulse_value * 0.4; // 0.4 to 0.8
        sprite.color = Color::srgba(1.0, 0.2, 0.2, alpha); // Red pulse
    }
}

/// Spawn fault pulse VFX when chargers develop faults
pub fn spawn_fault_pulse_on_fault(
    mut commands: Commands,
    chargers: Query<(Entity, &Charger, &Transform), Changed<Charger>>,
    existing_vfx: Query<&FaultPulseVfx>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    let vfx_image = image_assets.vfx_urgent_pulse.clone();
    let vfx_size = sprite_metadata::vfx_world_size(VfxType::FaultPulse);
    let base_scale = if let Some(image) = images.get(&vfx_image) {
        vfx_size.scale_for_image(image)
    } else {
        0.4 // Fallback
    };

    for (entity, charger, transform) in &chargers {
        // Check if charger has a fault
        if charger.current_fault.is_none() {
            continue;
        }

        // Check if VFX already exists for this charger
        let has_vfx = existing_vfx.iter().any(|vfx| vfx.charger_entity == entity);

        if !has_vfx {
            let pos = transform.translation;

            commands.spawn((
                Sprite::from_image(vfx_image.clone()),
                Transform::from_xyz(pos.x, pos.y, 10.0).with_scale(Vec3::splat(base_scale)),
                FaultPulseVfx {
                    charger_entity: entity,
                    pulse_time: 0.0,
                },
            ));
        }
    }
}

// ============ Theft Alarm VFX ============

/// Fast-flashing red alarm VFX component for cable theft events.
/// Flashes at ~4Hz (much faster than the fault pulse) with bright red.
#[derive(Component)]
pub struct TheftAlarmVfx {
    pub charger_entity: Entity,
    pub flash_time: f32,
}

/// Update theft alarm VFX (rapid red flashing animation synced to charger position)
pub fn update_theft_alarm_vfx(
    mut commands: Commands,
    mut vfx_query: Query<(Entity, &mut TheftAlarmVfx, &mut Transform, &mut Sprite)>,
    chargers: Query<&GlobalTransform, (Without<TheftAlarmVfx>, With<crate::components::Charger>)>,
    time: Res<Time>,
) {
    for (vfx_entity, mut alarm, mut vfx_transform, mut sprite) in &mut vfx_query {
        // Sync position with charger (use GlobalTransform for correct world position)
        let Ok(charger_gt) = chargers.get(alarm.charger_entity) else {
            commands.entity(vfx_entity).try_despawn();
            continue;
        };

        let charger_pos = charger_gt.translation();
        vfx_transform.translation.x = charger_pos.x;
        vfx_transform.translation.y = charger_pos.y;

        // Rapid flash animation at ~4Hz (fast alarm effect)
        alarm.flash_time += time.delta_secs() * 8.0 * std::f32::consts::PI; // 4Hz cycle
        let flash_value = (alarm.flash_time.sin() * 0.5 + 0.5).powi(2); // sharper flash

        // Bright red flash cycling alpha from 0.1 to 0.9
        let alpha = 0.1 + flash_value * 0.8;
        sprite.color = Color::srgba(1.0, 0.0, 0.0, alpha);

        // Slight scale pulsing for emphasis
        let scale_base = vfx_transform.scale.x; // keep roughly at base
        let scale_pulse = 1.0 + flash_value * 0.15;
        vfx_transform.scale = Vec3::splat(scale_base.min(1.0) * scale_pulse);
    }
}

// ============ Stealing Spark VFX ============

/// Spark VFX that flashes on the charger while a robber is cutting the cable.
/// Uses the yellow pulse image with rapid randomized flicker.
#[derive(Component)]
pub struct StealingSparkVfx {
    pub charger_entity: Entity,
    pub spark_time: f32,
}

/// Update stealing spark VFX (rapid yellow spark flicker synced to charger position)
pub fn update_stealing_spark_vfx(
    mut commands: Commands,
    mut vfx_query: Query<(Entity, &mut StealingSparkVfx, &mut Transform, &mut Sprite)>,
    chargers: Query<
        &GlobalTransform,
        (Without<StealingSparkVfx>, With<crate::components::Charger>),
    >,
    time: Res<Time>,
) {
    for (vfx_entity, mut spark, mut vfx_transform, mut sprite) in &mut vfx_query {
        // Sync position with charger (use GlobalTransform for correct world position)
        let Ok(charger_gt) = chargers.get(spark.charger_entity) else {
            commands.entity(vfx_entity).try_despawn();
            continue;
        };

        let charger_pos = charger_gt.translation();
        vfx_transform.translation.x = charger_pos.x;
        vfx_transform.translation.y = charger_pos.y + 20.0;

        // Sharp staccato spark flicker — snappy on/off flashes
        spark.spark_time += time.delta_secs() * 24.0;
        let raw_a = (spark.spark_time * 9.7).sin();
        let raw_b = (spark.spark_time * 13.3).sin();
        // Sharp threshold — sparks pop in and out crisply
        let flash = if raw_a > 0.6 || raw_b > 0.7 { 1.0 } else { 0.0 };

        // Bright yellow-white when on, invisible when off
        let alpha = flash * 0.95;
        sprite.color = Color::srgba(1.0, 0.95, 0.3, alpha);

        // Smaller, tighter sparks
        let scale = 0.3 + flash * 0.25;
        vfx_transform.scale = Vec3::splat(scale);
    }
}

// ============ Robber Loot Bubble ============

/// Update robber loot bubble — follows the robber with a slight bob, despawns when robber is gone.
pub fn update_robber_loot_bubble(
    mut commands: Commands,
    time: Res<Time>,
    robbers: Query<(&crate::components::robber::Robber, &GlobalTransform)>,
    mut bubbles: Query<(Entity, &mut RobberLootBubble, &mut Transform)>,
) {
    for (entity, mut bubble, mut transform) in bubbles.iter_mut() {
        bubble.lifetime += time.delta_secs();

        // Follow robber
        let Ok((_robber, robber_gt)) = robbers.get(bubble.robber_entity) else {
            // Robber gone — despawn bubble
            commands.entity(entity).try_despawn();
            continue;
        };

        let pos = robber_gt.translation();
        let bob = (bubble.lifetime * 3.0).sin() * 2.0;
        transform.translation.x = pos.x;
        transform.translation.y = pos.y + 35.0 + bob;
    }
}

// ============ Stolen Cable Sprite ============

/// Update stolen cable sprite — follows the robber at a slight offset, despawns when robber is gone.
pub fn update_stolen_cable_sprite(
    mut commands: Commands,
    robbers: Query<(&crate::components::robber::Robber, &GlobalTransform)>,
    mut cables: Query<(Entity, &StolenCableSprite, &mut Transform)>,
) {
    for (entity, cable, mut transform) in cables.iter_mut() {
        let Ok((_robber, robber_gt)) = robbers.get(cable.robber_entity) else {
            commands.entity(entity).try_despawn();
            continue;
        };

        let pos = robber_gt.translation();
        transform.translation.x = pos.x + 14.0;
        transform.translation.y = pos.y - 8.0;
    }
}

// ============ Broken Charger Icons ============

/// Component for broken charger indicator
#[derive(Component)]
pub struct BrokenChargerIcon {
    pub charger_entity: Entity,
}

/// Spawn broken icon above faulted chargers
pub fn spawn_broken_charger_icons(
    mut commands: Commands,
    chargers: Query<(Entity, &Charger, &GlobalTransform), Changed<Charger>>,
    existing_icons: Query<(Entity, &BrokenChargerIcon)>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    let icon_image = image_assets.icon_fault.clone();
    let icon_size = sprite_metadata::vfx_world_size(VfxType::BrokenChargerIcon);
    let icon_scale = if let Some(image) = images.get(&icon_image) {
        icon_size.scale_for_image(image)
    } else {
        0.6 // Fallback
    };

    for (entity, charger, global_transform) in &chargers {
        // Check if charger has a fault
        let needs_icon = charger.current_fault.is_some();
        let existing_icon = existing_icons
            .iter()
            .find(|(_, icon)| icon.charger_entity == entity);

        match (needs_icon, existing_icon) {
            (true, None) => {
                // Spawn icon above charger (use GlobalTransform for world position)
                // Y offset = 70.0, Z = 18.0 to render above vehicle indicators
                let pos = global_transform.translation();
                commands.spawn((
                    Sprite::from_image(icon_image.clone()),
                    Transform::from_xyz(pos.x, pos.y + 70.0, 18.0)
                        .with_scale(Vec3::splat(icon_scale)),
                    BrokenChargerIcon {
                        charger_entity: entity,
                    },
                ));
            }
            (false, Some((icon_entity, _))) => {
                // Fault cleared, despawn icon
                commands.entity(icon_entity).try_despawn();
            }
            _ => {}
        }
    }
}

/// Update broken charger icon positions
pub fn update_broken_charger_icon_positions(
    mut commands: Commands,
    mut icons: Query<(Entity, &BrokenChargerIcon, &mut Transform)>,
    chargers: Query<&GlobalTransform, (With<Charger>, Without<BrokenChargerIcon>)>,
) {
    for (icon_entity, icon, mut icon_transform) in &mut icons {
        let Ok(charger_global) = chargers.get(icon.charger_entity) else {
            // Charger no longer exists (e.g. sold), despawn the orphaned icon
            commands.entity(icon_entity).try_despawn();
            continue;
        };
        let pos = charger_global.translation();
        icon_transform.translation.x = pos.x;
        icon_transform.translation.y = pos.y + 70.0; // Match spawn Y offset
    }
}

// ============ Frustration Indicators ============

/// Component for frustration indicator above drivers
#[derive(Component)]
pub struct FrustrationIndicator {
    pub driver_entity: Entity,
}

/// Spawn frustration indicators above frustrated drivers
pub fn spawn_frustration_indicators(
    mut commands: Commands,
    drivers: Query<(Entity, &Driver), Changed<Driver>>,
    vehicle_sprites: Query<(&VehicleSprite, &GlobalTransform)>,
    existing_indicators: Query<(Entity, &FrustrationIndicator)>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    let icon_image = image_assets.icon_warning.clone();
    let icon_size = sprite_metadata::vfx_world_size(VfxType::FrustrationIcon);
    let icon_scale = if let Some(image) = images.get(&icon_image) {
        icon_size.scale_for_image(image)
    } else {
        0.5 // Fallback
    };

    for (driver_entity, driver) in &drivers {
        let needs_indicator = driver.state == DriverState::Frustrated;
        let existing_indicator = existing_indicators
            .iter()
            .find(|(_, ind)| ind.driver_entity == driver_entity);

        match (needs_indicator, existing_indicator) {
            (true, None) => {
                // Find vehicle world position (use GlobalTransform for hierarchy-aware position)
                // Y offset = 80.0 to appear above SoC bar and chargers
                // Z = 20.0 to render above all vehicle/charger sprites
                if let Some((_, vehicle_global_transform)) = vehicle_sprites
                    .iter()
                    .find(|(vs, _)| vs.driver_entity == driver_entity)
                {
                    let pos = vehicle_global_transform.translation();
                    commands.spawn((
                        Sprite::from_image(icon_image.clone()),
                        Transform::from_xyz(pos.x, pos.y + 80.0, 20.0)
                            .with_scale(Vec3::splat(icon_scale)),
                        FrustrationIndicator { driver_entity },
                    ));
                }
            }
            (false, Some((indicator_entity, _))) => {
                // No longer frustrated, despawn indicator
                commands.entity(indicator_entity).try_despawn();
            }
            _ => {}
        }
    }
}

/// Update frustration indicator positions to follow vehicles
pub fn update_frustration_indicator_positions(
    mut indicators: Query<(&FrustrationIndicator, &mut Transform)>,
    vehicle_sprites: Query<(&VehicleSprite, &GlobalTransform), Without<FrustrationIndicator>>,
) {
    for (indicator, mut indicator_transform) in &mut indicators {
        if let Some((_, vehicle_global_transform)) = vehicle_sprites
            .iter()
            .find(|(vs, _)| vs.driver_entity == indicator.driver_entity)
        {
            let pos = vehicle_global_transform.translation();
            indicator_transform.translation.x = pos.x;
            indicator_transform.translation.y = pos.y + 80.0; // Match spawn Y offset
        }
    }
}

// ============ Technician Sprite Systems ============

/// Spawn technician sprite when technician entity is created
pub fn spawn_technician_sprite(
    mut commands: Commands,
    technicians: Query<
        (
            Entity,
            &crate::components::technician::Technician,
            &Transform,
            &BelongsToSite,
        ),
        Added<crate::components::technician::Technician>,
    >,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    let tech_image = image_assets.character_technician_idle.clone();
    let tech_size = sprite_metadata::technician_world_size();
    let tech_scale = if let Some(image) = images.get(&tech_image) {
        tech_size.scale_for_image(image)
    } else {
        1.5 // Fallback
    };

    for (tech_entity, _technician, _transform, belongs) in technicians.iter() {
        // Spawn sprite as child of technician entity - always use idle sprite
        commands.entity(tech_entity).with_children(|parent| {
            parent.spawn((
                Sprite::from_image(tech_image.clone()),
                // Z = 4.0 to render above vehicles (Z = 2.0) and driver mood icons (Z = 3.0)
                // Scale for better visibility
                Transform::from_xyz(0.0, 0.0, 4.0).with_scale(Vec3::splat(tech_scale)),
                TechnicianSprite {
                    technician_entity: tech_entity,
                },
                *belongs,
            ));
        });
    }
}

/// Update technician sprite based on phase changes
pub fn update_technician_sprite(
    technicians: Query<
        (Entity, &crate::components::technician::Technician),
        Changed<crate::components::technician::Technician>,
    >,
    mut sprites: Query<(&TechnicianSprite, &mut Sprite)>,
    image_assets: Res<ImageAssets>,
) {
    for (tech_entity, technician) in technicians.iter() {
        // Find sprite that belongs to this technician and update image based on phase
        for (tech_sprite, mut sprite) in sprites.iter_mut() {
            if tech_sprite.technician_entity == tech_entity {
                let new_image = match technician.phase {
                    crate::components::technician::TechnicianPhase::Working => {
                        image_assets.character_technician_working.clone()
                    }
                    _ => image_assets.character_technician_idle.clone(),
                };
                sprite.image = new_image;
            }
        }
    }
}

/// Animate technician while working (bobbing and slight rotation)
pub fn animate_technician_working(
    time: Res<Time>,
    game_clock: Res<crate::resources::GameClock>,
    mut technicians: Query<(Entity, &mut crate::components::technician::Technician)>,
    mut tech_sprites: Query<(&TechnicianSprite, &mut Transform)>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();

    for (tech_entity, mut technician) in technicians.iter_mut() {
        if technician.phase != crate::components::technician::TechnicianPhase::Working {
            continue;
        }

        // Update work timer
        technician.work_timer += dt * 4.0; // Speed up animation

        // Find and animate the technician sprite (bobbing and rotation)
        for (tech_sprite, mut transform) in tech_sprites.iter_mut() {
            if tech_sprite.technician_entity == tech_entity {
                // Bobbing motion
                let bob_offset = (technician.work_timer * 3.0).sin() * 3.0;
                transform.translation.y = bob_offset;
                // Slight rotation to simulate working motion
                transform.rotation =
                    Quat::from_rotation_z((technician.work_timer * 2.0).sin() * 0.1);
            }
        }
    }
}

// ============ Power Throttle Indicators ============

/// Component for power throttle indicator above chargers
#[derive(Component)]
pub struct PowerThrottleIcon {
    pub charger_entity: Entity,
}

/// Threshold below which we show throttle indicator (getting less than 80% of requested power)
const POWER_THROTTLE_THRESHOLD: f32 = 0.8;

/// Spawn power throttle icons above chargers that are power-limited
pub fn spawn_power_throttle_icons(
    mut commands: Commands,
    chargers: Query<(Entity, &Charger, &GlobalTransform), Changed<Charger>>,
    existing_icons: Query<(Entity, &PowerThrottleIcon)>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    let icon_image = image_assets.icon_warning.clone();
    let icon_size = sprite_metadata::vfx_world_size(VfxType::PowerThrottleIcon);
    let icon_scale = if let Some(image) = images.get(&icon_image) {
        icon_size.scale_for_image(image)
    } else {
        0.5 // Fallback
    };

    for (entity, charger, global_transform) in &chargers {
        // Check if charger is actively charging but throttled
        let is_charging = charger.is_charging;
        let is_throttled = charger.requested_power_kw > 0.0
            && (charger.allocated_power_kw / charger.requested_power_kw) < POWER_THROTTLE_THRESHOLD;

        let needs_icon = is_charging && is_throttled;
        let existing_icon = existing_icons
            .iter()
            .find(|(_, icon)| icon.charger_entity == entity);

        match (needs_icon, existing_icon) {
            (true, None) => {
                // Spawn warning icon to the right of charger (use GlobalTransform for world position)
                let pos = global_transform.translation();
                commands.spawn((
                    Sprite::from_image(icon_image.clone()),
                    Transform::from_xyz(pos.x + 25.0, pos.y + 50.0, 15.0)
                        .with_scale(Vec3::splat(icon_scale)),
                    PowerThrottleIcon {
                        charger_entity: entity,
                    },
                ));
            }
            (false, Some((icon_entity, _))) => {
                // No longer throttled, despawn icon
                commands.entity(icon_entity).try_despawn();
            }
            _ => {}
        }
    }
}

/// Update power throttle icon positions to follow chargers
pub fn update_power_throttle_icon_positions(
    mut icons: Query<(&PowerThrottleIcon, &mut Transform)>,
    chargers: Query<&GlobalTransform, (With<Charger>, Without<PowerThrottleIcon>)>,
) {
    for (icon, mut icon_transform) in &mut icons {
        if let Ok(charger_global) = chargers.get(icon.charger_entity) {
            let pos = charger_global.translation();
            icon_transform.translation.x = pos.x + 25.0;
            icon_transform.translation.y = pos.y + 50.0;
        }
    }
}

// ============ Security Camera Swivel Animation ============

/// Animate security camera heads with a slow swivel sweep.
/// Only the camera head rotates; the pole/base stays fixed.
/// The camera oscillates ±25° around its base heading toward the lot center.
pub fn animate_security_camera_swivel(
    time: Res<Time>,
    game_clock: Res<crate::resources::GameClock>,
    mut cameras: Query<(
        &mut crate::systems::scene::SecurityCameraHead,
        &mut Transform,
    )>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();
    // Swivel parameters
    let swivel_speed = 0.6; // radians/sec oscillation frequency
    let swivel_amplitude = 0.44; // ~25 degrees in radians

    for (mut head, mut transform) in &mut cameras {
        head.timer += dt;
        let swivel_offset = bevy::math::ops::sin(head.timer * swivel_speed) * swivel_amplitude;
        transform.rotation = Quat::from_rotation_z(head.base_angle + swivel_offset);
    }
}

// ============ Security Alert Bubble ============

/// Floating "SECURITY ALERT" text that appears near the security camera when it deters a robbery.
/// Fades out and floats upward, then self-despawns.
#[derive(Component)]
pub struct SecurityAlertBubble {
    pub lifetime: f32,
    pub max_lifetime: f32,
}

/// Update security alert bubbles (float up, fade out, despawn).
pub fn update_security_alert_bubble(
    mut commands: Commands,
    time: Res<Time>,
    mut bubbles: Query<(
        Entity,
        &mut SecurityAlertBubble,
        &mut Transform,
        &mut TextColor,
    )>,
) {
    for (entity, mut bubble, mut transform, mut color) in bubbles.iter_mut() {
        bubble.lifetime += time.delta_secs();

        transform.translation.y += 25.0 * time.delta_secs();

        let progress = bubble.lifetime / bubble.max_lifetime;
        let alpha = (1.0 - progress).clamp(0.0, 1.0);
        color.0 = Color::srgba(0.2, 1.0, 0.4, alpha);

        if bubble.lifetime >= bubble.max_lifetime {
            commands.entity(entity).try_despawn();
        }
    }
}

// ============ Security Camera LED Blink ============

/// Blink the green recording LED on security cameras at ~1 Hz.
/// The LED smoothly oscillates between dim and bright to show the system is active.
pub fn animate_security_camera_led(
    time: Res<Time>,
    game_clock: Res<crate::resources::GameClock>,
    mut leds: Query<(&mut crate::systems::scene::SecurityCameraLed, &mut Sprite)>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();
    let blink_speed = std::f32::consts::TAU; // 1 Hz full cycle

    for (mut led, mut sprite) in &mut leds {
        led.timer += dt;
        let t = bevy::math::ops::sin(led.timer * blink_speed);
        let alpha = 0.25 + (t * 0.5 + 0.5) * 0.75; // range 0.25 to 1.0
        sprite.color = Color::srgba(0.2, 1.0, 0.3, alpha);
    }
}

// ============ Anti-Theft Shield Indicator ============

/// Sync the small shield indicator sprite on chargers that have the anti-theft cable upgrade.
///
/// - When `charger.anti_theft_cable` becomes `true`, spawns an `AntiTheftShieldIndicator`
///   child entity with a shield sprite.
/// - When the upgrade is removed (sold), despawns the indicator.
///
/// The indicator is spawned as a child of the `Charger` entity so it inherits the charger's
/// transform and is automatically despawned when the charger entity is removed.
pub fn sync_anti_theft_shield_indicators(
    mut commands: Commands,
    chargers: Query<(Entity, &Charger), Changed<Charger>>,
    existing_shields: Query<(Entity, &ChildOf), With<AntiTheftShieldIndicator>>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
) {
    for (charger_entity, charger) in &chargers {
        // Find existing shield indicator that is a child of this charger
        let existing = existing_shields
            .iter()
            .find(|(_, child_of)| child_of.parent() == charger_entity);

        match (charger.anti_theft_cable, existing) {
            (true, None) => {
                // Spawn shield indicator as child of charger entity
                let shield_image = image_assets.icon_shield_indicator.clone();

                // Calculate scale: we want the shield ~16px in world space
                let target_size = 16.0_f32;
                let shield_scale = if let Some(image) = images.get(&shield_image) {
                    let img_width = image.width() as f32;
                    if img_width > 0.0 {
                        target_size / img_width
                    } else {
                        0.33
                    }
                } else {
                    0.33 // Fallback if image not loaded yet
                };

                // Offset: bottom-right corner of charger, local coords
                commands.entity(charger_entity).with_children(|parent| {
                    parent.spawn((
                        Sprite::from_image(shield_image),
                        Transform::from_xyz(12.0, -12.0, 0.2).with_scale(Vec3::splat(shield_scale)),
                        AntiTheftShieldIndicator,
                    ));
                });
            }
            (false, Some((shield_entity, _))) => {
                // Upgrade removed (sold), despawn the indicator
                commands.entity(shield_entity).try_despawn();
            }
            _ => {
                // Either already has indicator and still upgraded, or doesn't have either
            }
        }
    }
}

// ============ In-World Ad Screen ============

/// Marker component for the physical screen panel next to a DCFC100 charger.
/// Always present on DCFC100 chargers (built-in hardware). Spawned at charger
/// creation time in `sync_chargers_with_grid` as a child of the `Charger` entity.
#[derive(Component)]
pub struct AdScreenPanel {
    pub charger_entity: Entity,
}

/// Marker component for the ad content displayed on the screen panel while
/// charging with video ads enabled. Child of the `Charger` entity.
#[derive(Component)]
pub struct AdScreenOverlay;

/// Sprite-based GIF animator for world-space sprites (as opposed to `GifAnimator`
/// which targets `ImageNode` UI elements).
#[derive(Component)]
pub struct WorldGifAnimator {
    pub frames: Vec<Handle<Image>>,
    pub current_frame: usize,
    pub timer: Timer,
}

/// Frame duration for world-space ad GIF playback (seconds per frame).
const WORLD_AD_GIF_FRAME_DURATION: f32 = 0.08;

/// Spawn or despawn ad content on the screen panel of DCFC100 chargers.
///
/// The screen panel itself is always present (spawned in `sync_chargers_with_grid`).
/// This system only controls the ad content that plays on the screen while charging.
///
/// When a charger is actively charging and has video ads enabled, ad content is
/// spawned as a child of the charger overlaying the screen panel. When charging
/// stops, the ad content is removed (leaving the dark screen panel visible).
pub fn sync_ad_screen_overlays(
    mut commands: Commands,
    chargers: Query<(Entity, &Charger), Changed<Charger>>,
    existing_overlays: Query<(Entity, &ChildOf), With<AdScreenOverlay>>,
    screen_panels: Query<(&AdScreenPanel, &Transform), Without<AdScreenOverlay>>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    gif_frames: Res<crate::ui::radial_menu::GifAnimationFrames>,
) {
    for (charger_entity, charger) in &chargers {
        let wants_overlay = charger.video_ad_enabled && charger.state() == ChargerState::Charging;

        // Check if an overlay already exists as a child of this charger
        let existing = existing_overlays
            .iter()
            .find(|(_, child_of)| child_of.parent() == charger_entity);

        match (wants_overlay, existing) {
            (true, None) => {
                // Find the screen panel to get its position and dimensions
                let Some((_panel, panel_transform)) = screen_panels
                    .iter()
                    .find(|(p, _)| p.charger_entity == charger_entity)
                else {
                    // No screen panel on this charger (not a DCFC100 with ads)
                    continue;
                };

                // Get the screen panel image to determine its world-space size
                let screen_image = &image_assets.charger_dcfc100_screen;
                let (screen_w, screen_h) = if let Some(img) = images.get(screen_image) {
                    let scale = panel_transform.scale.x;
                    (img.width() as f32 * scale, img.height() as f32 * scale)
                } else {
                    // Fallback: use charger dimensions
                    let charger_size = crate::resources::sprite_metadata::charger_world_size(
                        charger.charger_type,
                        charger.rated_power_kw,
                    );
                    (charger_size.width, charger_size.width * 2.0)
                };

                // Apply some inset so the ad content sits inside the screen bezel
                let inset_x = screen_w * 0.15;
                let inset_y = screen_h * 0.08;
                let content_w = screen_w - inset_x * 2.0;
                let content_h = screen_h - inset_y * 2.0;

                // Determine ad image
                let ad_image = if !gif_frames.dancing_banana.is_empty() {
                    gif_frames.dancing_banana[0].clone()
                } else {
                    image_assets.ad_dancing_banana.clone()
                };

                // Spawn ad content at the same position as the screen panel,
                // slightly in front (higher Z)
                let panel_pos = panel_transform.translation;
                commands.entity(charger_entity).with_children(|parent| {
                    let mut overlay_cmd = parent.spawn((
                        Sprite {
                            image: ad_image,
                            custom_size: Some(Vec2::new(content_w, content_h)),
                            ..default()
                        },
                        Transform::from_xyz(panel_pos.x, panel_pos.y, panel_pos.z + 0.05),
                        AdScreenOverlay,
                    ));

                    // Attach animated GIF if frames are loaded
                    if gif_frames.dancing_banana.len() > 1 {
                        overlay_cmd.insert(WorldGifAnimator {
                            frames: gif_frames.dancing_banana.clone(),
                            current_frame: 0,
                            timer: Timer::from_seconds(
                                WORLD_AD_GIF_FRAME_DURATION,
                                TimerMode::Repeating,
                            ),
                        });
                    }
                });
            }
            (false, Some((overlay_entity, _))) => {
                // Charging stopped or ads disabled: remove ad content (screen panel stays)
                commands.entity(overlay_entity).try_despawn();
            }
            _ => {
                // Already showing ad or doesn't need one
            }
        }
    }
}

/// Advance world-space GIF animations by cycling through sprite frames.
pub fn update_world_gif_animations(
    mut query: Query<(&mut Sprite, &mut WorldGifAnimator)>,
    time: Res<Time>,
) {
    for (mut sprite, mut animator) in &mut query {
        if animator.frames.is_empty() {
            continue;
        }

        animator.timer.tick(time.delta());

        if animator.timer.just_finished() {
            animator.current_frame = (animator.current_frame + 1) % animator.frames.len();
            sprite.image = animator.frames[animator.current_frame].clone();
        }
    }
}
