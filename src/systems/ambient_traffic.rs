//! Ambient road traffic system - cars driving by on the public road
//!
//! Uses PNG assets for all vehicle rendering (generated from SVG source files).
//! Ambient vehicles use bevy_northstar for pathfinding but NOT collision avoidance
//! (no Blocking component). They pathfind around static obstacles and parked drivers.

use bevy::prelude::*;
use bevy_northstar::prelude::*;
use rand::Rng;
use rand::prelude::IteratorRandom;

use crate::components::VehicleFootprint;
use crate::components::driver::{DriverMood, MovementPhase, VehicleMovement, VehicleType};
use crate::resources::{EnvironmentState, GameClock, ImageAssets, MultiSiteManager, SiteGrid};
use crate::systems::sprite::{DriverCharacterSprite, VehicleSprite, get_mood_image};

/// Marker for ambient (non-customer) traffic
#[derive(Component)]
pub struct AmbientVehicle {
    pub vehicle_type: VehicleType,
    pub direction: TrafficDirection,
    pub mood: DriverMood,
    /// Whether this vehicle is "interested" in the station (slows down to look)
    pub is_interested: bool,
    /// Original speed before slowing down
    pub original_speed: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum TrafficDirection {
    LeftToRight,
    RightToLeft,
}

/// Timer for spawning ambient traffic
#[derive(Resource)]
pub struct AmbientTrafficTimer {
    pub timer: Timer,
}

impl Default for AmbientTrafficTimer {
    fn default() -> Self {
        Self {
            // Reduced from 2.0s to 1.2s for more visual activity
            timer: Timer::from_seconds(1.2, TimerMode::Repeating),
        }
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

/// Spawn a drive-by vehicle (ambient traffic that drives past the station)
///
/// This helper is used both for random ambient traffic spawning and for
/// scheduled drivers when the station is full.
///
/// Uses bevy_northstar for pathfinding and collision avoidance.
pub fn spawn_drive_by_vehicle(
    commands: &mut Commands,
    root_entity: Entity,
    grid: &SiteGrid,
    image_assets: &ImageAssets,
    images: &Assets<Image>,
    vehicle_type: VehicleType,
    direction: TrafficDirection,
    speed: f32,
    mood: DriverMood,
    site_id: crate::resources::SiteId,
) -> Option<Entity> {
    // Determine start and end grid positions based on direction
    let road_y = grid.entry_pos.1;
    let (start_grid_x, end_grid_x) = match direction {
        TrafficDirection::LeftToRight => (grid.entry_pos.0, grid.exit_pos.0),
        TrafficDirection::RightToLeft => (grid.exit_pos.0, grid.entry_pos.0),
    };

    // Spawn at grid edge (not off-grid) for proper pathfinding
    let start_world = SiteGrid::grid_to_world(start_grid_x, road_y);

    // Simple waypoints - just the start position, pathfinding handles the rest
    let waypoints = vec![start_world];

    let movement = VehicleMovement {
        phase: MovementPhase::Arriving, // Reuse for ambient movement
        waypoints,
        current_waypoint: 0,
        progress: 0.0,
        speed,
        target_rotation: 0.0,
    };

    let footprint_len = vehicle_type.footprint_length_tiles();
    let footprint = VehicleFootprint {
        length_tiles: footprint_len,
    };

    // Get images before entering spawn closure
    let vehicle_image = get_vehicle_image(image_assets, vehicle_type);
    let mood_image = get_mood_image(image_assets, mood);

    // Calculate scale using intended world size and actual PNG dimensions
    let intended_size = crate::resources::sprite_metadata::vehicle_world_size(vehicle_type);
    let ambient_scale = if let Some(image) = images.get(&vehicle_image) {
        intended_size.scale_for_image(image)
    } else {
        0.2 // Fallback if image not loaded yet
    };

    // bevy_northstar components for pathfinding
    let agent_pos = AgentPos(UVec3::new(start_grid_x as u32, road_y as u32, 0));
    let pathfind = Pathfind::new_2d(end_grid_x as u32, road_y as u32).mode(PathfindMode::AStar);

    // Spawn ambient vehicle entity as child of site root, with visual sprites as nested children
    let mut ambient_entity = Entity::PLACEHOLDER;
    commands.entity(root_entity).with_children(|parent| {
        let mut entity_commands = parent.spawn((
            AmbientVehicle {
                vehicle_type,
                direction,
                mood,
                is_interested: false,
                original_speed: speed,
            },
            movement,
            footprint,
            agent_pos,
            pathfind,
            // NOTE: No Blocking component - ambient vehicles don't participate in collision avoidance.
            // This prevents them from getting stuck with RerouteFailed (which is only handled for drivers).
            // They still pathfind around static obstacles and parked drivers.
            Transform::from_xyz(start_world.x, start_world.y, 2.5),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            crate::components::BelongsToSite::new(site_id),
        ));

        // Capture entity ID for use in child sprites
        ambient_entity = entity_commands.id();

        // Spawn visual sprites as children within the same spawn block
        entity_commands.with_children(|vehicle_parent| {
            // Spawn vehicle sprite (scale adjusted for PNG)
            // Z = 2.0 to render above parking lines/decals (Z ~1.3)
            // No rotation on child - parent handles rotation in update_ambient_sprite_positions
            vehicle_parent.spawn((
                Sprite::from_image(vehicle_image.clone()),
                Transform::from_xyz(0.0, 0.0, 2.0).with_scale(Vec3::splat(ambient_scale)),
                VehicleSprite {
                    driver_entity: ambient_entity,
                },
                Visibility::Inherited,
                crate::components::BelongsToSite::new(site_id),
            ));

            // Spawn mood icon centered on vehicle
            // Z = 3.0 to render just above vehicle sprite (Z = 2.0)
            vehicle_parent.spawn((
                Sprite::from_image(mood_image.clone()),
                Transform::from_xyz(0.0, 0.0, 3.0).with_scale(Vec3::splat(0.3)),
                DriverCharacterSprite {
                    driver_entity: ambient_entity,
                },
                Visibility::Inherited,
                crate::components::BelongsToSite::new(site_id),
            ));
        });
    });

    info!(
        "Spawned drive-by {:?} ({:?}) at ({:.0}, {:.0}) heading {:?}",
        vehicle_type, mood, start_world.x, start_world.y, direction
    );

    Some(ambient_entity)
}

/// Spawn ambient vehicles on the road at intervals
pub fn spawn_ambient_traffic(
    mut commands: Commands,
    mut timer: ResMut<AmbientTrafficTimer>,
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut multi_site: ResMut<MultiSiteManager>,
    image_assets: Res<ImageAssets>,
    images: Res<Assets<Image>>,
    environment: Res<EnvironmentState>,
    blocking_map: Res<BlockingMap>,
    existing_ambient: Query<&AgentPos, With<AmbientVehicle>>,
) {
    if game_clock.is_paused() {
        return;
    }

    // Don't spawn new ambient traffic during end-of-day wind-down
    if game_clock.day_ending {
        return;
    }

    // Get active site grid or return early
    let viewed_site_id = multi_site.viewed_site_id;
    let Some(site_id) = viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.get_site_mut(site_id) else {
        return;
    };

    // Scale timer speed based on demand multipliers
    // Higher demand = more traffic on the road
    let demand_multiplier =
        environment.total_demand_multiplier() * site_state.site_upgrades.demand_multiplier();
    let scaled_delta = time.delta().mul_f32(demand_multiplier);
    timer.timer.tick(scaled_delta);

    if !timer.timer.just_finished() {
        return;
    }

    let mut rng = rand::rng();

    // Random vehicle type with weighted distribution
    // Common: Sedan (25%), Crossover (20%), SUV (18%), Compact (12%), Pickup (10%)
    // Two-wheelers: Scooter (5%), Motorcycle (5%)
    // Commercial (rare): Bus (2%), Semi (2%), Tractor (1%)
    let roll = rng.random_range(0..100);
    let vehicle_type = match roll {
        0..12 => VehicleType::Compact,
        12..37 => VehicleType::Sedan,
        37..55 => VehicleType::Crossover,
        55..73 => VehicleType::Suv,
        73..83 => VehicleType::Pickup,
        83..88 => VehicleType::Scooter,
        88..93 => VehicleType::Motorcycle,
        93..95 => VehicleType::Bus,
        95..97 => VehicleType::Semi,
        _ => VehicleType::Tractor,
    };

    // All ambient traffic goes left-to-right to avoid deadlocks on single-lane road
    let direction = TrafficDirection::LeftToRight;

    // Random speed variation
    let speed = 140.0 + rng.random_range(-20.0..30.0);

    // Random mood for ambient traffic (mostly neutral, occasionally others)
    let mood = match rng.random_range(0..10) {
        0 => DriverMood::Impatient,
        1 => DriverMood::Happy,
        _ => DriverMood::Neutral,
    };

    let Some(root_entity) = site_state.root_entity else {
        warn!(
            "Site {:?} has no root entity, cannot spawn ambient traffic",
            site_state.id
        );
        return;
    };

    // Check if spawn tile is blocked - don't spawn if something is there
    let spawn_pos = match direction {
        TrafficDirection::LeftToRight => site_state.grid.entry_pos,
        TrafficDirection::RightToLeft => site_state.grid.exit_pos,
    };
    let spawn_uvec = UVec3::new(spawn_pos.0 as u32, spawn_pos.1 as u32, 0);
    if blocking_map.0.contains_key(&spawn_uvec) {
        // Spawn tile is blocked by a driver/parked vehicle, skip this spawn
        return;
    }

    // Also check if an ambient vehicle is already at the spawn position
    // (ambient vehicles don't have Blocking, so they're not in blocking_map)
    let ambient_at_spawn = existing_ambient
        .iter()
        .any(|pos| pos.0.x == spawn_uvec.x && pos.0.y == spawn_uvec.y);
    if ambient_at_spawn {
        return;
    }

    let entity = spawn_drive_by_vehicle(
        &mut commands,
        root_entity,
        &site_state.grid,
        &image_assets,
        &images,
        vehicle_type,
        direction,
        speed,
        mood,
        site_state.id,
    );

    let Some(entity) = entity else {
        return;
    };

    // 20% of left-to-right traffic is "interested" in the station
    if matches!(direction, TrafficDirection::LeftToRight)
        && rng.random::<f32>() < 0.2
        && let Ok(mut entity_mut) = commands.get_entity(entity)
    {
        entity_mut.insert(InterestedVehicle);
    }
}

/// Marker for vehicles that are interested in the station
#[derive(Component)]
pub struct InterestedVehicle;

/// System to make interested vehicles slow down near the station entrance
pub fn update_interested_vehicles(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut VehicleMovement,
        &mut AmbientVehicle,
        &InterestedVehicle,
    )>,
    multi_site: Res<MultiSiteManager>,
) {
    // Get active site grid or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let grid = &site.grid;

    let entry_world = SiteGrid::grid_to_world(grid.entry_pos.0, grid.entry_pos.1);
    let interest_zone_start = entry_world.x - 100.0;
    let interest_zone_end = entry_world.x + 50.0;

    for (entity, mut movement, mut ambient, _) in &mut query {
        if let Some(current_pos) = movement.current_position() {
            let in_interest_zone =
                current_pos.x >= interest_zone_start && current_pos.x <= interest_zone_end;

            if in_interest_zone && !ambient.is_interested {
                // Entering interest zone - slow down
                ambient.is_interested = true;
                movement.speed *= 0.5; // Slow to half speed
            } else if !in_interest_zone
                && ambient.is_interested
                && current_pos.x > interest_zone_end
            {
                // Leaving interest zone - speed back up (station was full or not appealing)
                ambient.is_interested = false;
                movement.speed = ambient.original_speed;
                // Change mood to impatient (disappointed they couldn't stop)
                ambient.mood = DriverMood::Impatient;
                // Remove the interested marker
                commands.entity(entity).remove::<InterestedVehicle>();
            }
        }
    }
}

/// Update ambient vehicle movement
pub fn update_ambient_traffic(
    mut commands: Commands,
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut query: Query<(Entity, &mut VehicleMovement, &AmbientVehicle)>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();

    for (entity, mut movement, _ambient) in &mut query {
        if movement.waypoints.len() < 2 {
            commands.entity(entity).try_despawn();
            continue;
        }

        let from = movement.waypoints[0];
        let to = movement.waypoints[1];
        let distance = (to - from).length();

        if distance < 0.1 {
            commands.entity(entity).try_despawn();
            continue;
        }

        // Move along path - use visual speed (half of simulation speed for readability)
        let speed = movement.speed * game_clock.speed.visual_multiplier();
        let progress_delta = (speed * dt) / distance;
        movement.progress += progress_delta;

        // Despawn if reached end
        if movement.progress >= 1.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Update ambient vehicle sprite rotation based on direction.
///
/// NOTE: Position is now handled by northstar_move_vehicles via bevy_northstar pathfinding.
/// This system only sets rotation based on travel direction.
pub fn update_ambient_sprite_positions(
    mut ambient_vehicles: Query<(&AmbientVehicle, &mut Transform)>,
) {
    for (ambient, mut ambient_transform) in &mut ambient_vehicles {
        // Set rotation based on direction (sprites inherit this)
        // Position is handled by northstar_move_vehicles
        let rotation = match ambient.direction {
            TrafficDirection::LeftToRight => -std::f32::consts::FRAC_PI_2,
            TrafficDirection::RightToLeft => std::f32::consts::FRAC_PI_2,
        };
        ambient_transform.rotation = Quat::from_rotation_z(rotation);
    }
}

// NOTE: cleanup_ambient_sprites was removed - cleanup is now handled by
// cleanup_driver_sprites in sprite.rs which checks for both Driver and AmbientVehicle parents.

/// System to convert some ambient traffic into customers after station opens
pub fn ambient_to_customer_system(
    mut commands: Commands,
    build_state: Res<crate::resources::BuildState>,
    multi_site: Res<MultiSiteManager>,
    mut query: Query<(Entity, &AmbientVehicle, &VehicleMovement, &Transform)>,
    existing_drivers: Query<&crate::components::driver::Driver>,
    game_clock: Res<GameClock>,
) {
    // Only divert traffic when station is open
    if !build_state.is_open {
        return;
    }

    // Don't convert ambient traffic into customers during end-of-day wind-down
    if game_clock.day_ending {
        return;
    }

    // Get active site grid or return early
    let Some(site) = multi_site.active_site() else {
        return;
    };
    let grid = &site.grid;

    let mut rng = rand::rng();

    // Get available bays
    let charger_bays = grid.get_charger_bays();
    if charger_bays.is_empty() {
        return;
    }

    let occupied_bays: Vec<(i32, i32)> = existing_drivers
        .iter()
        .filter_map(|d| d.assigned_bay)
        .collect();

    let available_bays: Vec<_> = charger_bays
        .iter()
        .filter(|(x, y, _)| !occupied_bays.contains(&(*x, *y)))
        .collect();

    if available_bays.is_empty() {
        return; // No space
    }

    for (entity, ambient, _movement, transform) in &mut query {
        // Only check cars driving left-to-right
        if !matches!(ambient.direction, TrafficDirection::LeftToRight) {
            continue;
        }

        // Only check if car is in the "decision zone" (roughly 10-30% across the site road).
        // NOTE: We do NOT use `movement.progress` here because movement is multi-waypoint (tile-based).
        let entry_world = SiteGrid::grid_to_world(grid.entry_pos.0, grid.entry_pos.1);
        let exit_world = SiteGrid::grid_to_world(grid.exit_pos.0, grid.exit_pos.1);
        let road_len = (exit_world.x - entry_world.x).abs().max(1.0);
        let t = ((transform.translation.x - entry_world.x) / road_len).clamp(0.0, 1.0);
        if !(0.1..=0.3).contains(&t) {
            continue;
        }

        // 20% chance to divert
        if rng.random::<f32>() > 0.002 {
            // Low per-frame chance = ~20% in decision zone
            continue;
        }

        // Pick a random available bay
        let Some(&(bay_x, bay_y, _charger_type)) = available_bays.iter().choose(&mut rng) else {
            continue;
        };

        // Create a driver from this ambient vehicle
        use crate::components::driver::{Driver, DriverState, MovementPhase, PatienceLevel};
        use bevy_northstar::prelude::*;

        let driver_id = format!("ambient_{}", entity.index());

        // Get current grid position
        let current_pos = transform.translation;
        let current_grid = SiteGrid::world_to_grid(Vec2::new(current_pos.x, current_pos.y));

        let driver = Driver {
            id: driver_id.clone(),
            vehicle_name: format!("{:?}", ambient.vehicle_type),
            vehicle_type: ambient.vehicle_type,
            patience_level: PatienceLevel::Medium,
            patience: PatienceLevel::Medium.initial_patience(),
            charge_needed_kwh: rng.random_range(15.0..40.0),
            charge_received_kwh: 0.0,
            target_charger_id: None,
            assigned_charger: None,
            assigned_bay: Some((*bay_x, *bay_y)),
            state: DriverState::Arriving,
            mood: DriverMood::Neutral,
        };

        // Update movement phase
        let new_movement = VehicleMovement {
            phase: MovementPhase::Arriving,
            speed: 110.0,
            ..default()
        };

        // bevy_northstar components for pathfinding
        let agent_pos = AgentPos(UVec3::new(current_grid.0 as u32, current_grid.1 as u32, 0));
        let pathfind = Pathfind::new_2d(*bay_x as u32, *bay_y as u32);

        // Remove ambient component, add driver and pathfinding components
        commands.entity(entity).remove::<AmbientVehicle>();
        commands
            .entity(entity)
            .insert((driver, new_movement, agent_pos, pathfind));

        info!(
            "Ambient vehicle {} diverted to bay ({}, {})",
            driver_id, bay_x, bay_y
        );

        // Only convert one vehicle per frame
        break;
    }
}
