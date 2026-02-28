//! Vehicle movement using bevy_northstar pathfinding.
//!
//! This module provides smooth movement for vehicles using bevy_northstar's
//! pathfinding system. Vehicles move from their current position to the
//! next position in their path (NextPos) with smooth interpolation.

use bevy::prelude::*;
use bevy_northstar::prelude::*;

use crate::components::BelongsToSite;
use crate::components::driver::{Driver, DriverState, MovementPhase, VehicleMovement};
use crate::resources::MultiSiteManager;
use crate::resources::game_clock::GameClock;
use crate::resources::site_grid::SiteGrid;

/// Cooldown timer for vehicles that failed to reroute.
/// This prevents rapid retry loops that can cause stuttering and stuck vehicles.
#[derive(Component)]
pub struct RerouteCooldown {
    /// Time remaining before retry (in real seconds)
    pub timer: f32,
    /// Total time spent stuck (accumulates across retries)
    pub total_stuck_time: f32,
}

impl Default for RerouteCooldown {
    fn default() -> Self {
        Self {
            timer: 0.5, // Wait 0.5 seconds before retrying
            total_stuck_time: 0.0,
        }
    }
}

/// Maximum time a vehicle can be stuck before being forced to leave (in real seconds)
const MAX_STUCK_TIME: f32 = 5.0;

/// Cooldown timer for pathfinding failures (no path found).
#[derive(Component)]
pub struct PathfindCooldown {
    pub timer: f32,
    pub total_stuck_time: f32,
}

impl Default for PathfindCooldown {
    fn default() -> Self {
        Self {
            timer: 0.5,
            total_stuck_time: 0.0,
        }
    }
}

/// System that moves vehicles smoothly towards their NextPos.
///
/// This system:
/// 1. Reads the NextPos component inserted by bevy_northstar
/// 2. Smoothly interpolates the vehicle's Transform towards the target
/// 3. Updates AgentPos when the vehicle reaches the target
/// 4. Removes NextPos to signal readiness for the next step
pub fn northstar_move_vehicles(
    mut commands: Commands,
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut query: Query<(
        Entity,
        &mut AgentPos,
        &NextPos,
        &mut Transform,
        &mut VehicleMovement,
    )>,
) {
    if game_clock.is_paused() {
        return;
    }

    let speed_multiplier = game_clock.speed.visual_multiplier();

    for (entity, mut agent_pos, next_pos, mut transform, mut movement) in query.iter_mut() {
        // Calculate world position from grid position
        let target_world = SiteGrid::grid_to_world(next_pos.0.x as i32, next_pos.0.y as i32);
        let current = transform.translation.truncate();

        // Use the vehicle's speed for smooth movement
        let speed = movement.speed * speed_multiplier;
        let step = speed * time.delta_secs();

        let distance = current.distance(target_world);

        if distance <= step || distance < 1.0 {
            // Arrived at next position
            transform.translation.x = target_world.x;
            transform.translation.y = target_world.y;

            // Update agent position to match
            agent_pos.0 = next_pos.0;

            // Remove NextPos to signal we're ready for the next step
            commands.entity(entity).try_remove::<NextPos>();

            // Update waypoints for debug overlay compatibility
            // (VehicleMovement waypoint fields are not used for actual pathfinding -
            // that's handled by bevy_northstar)
            movement.waypoints = vec![target_world];
            movement.current_waypoint = 0;
            movement.progress = 1.0;
        } else {
            // Move towards target
            let direction = (target_world - current).normalize();
            transform.translation.x += direction.x * step;
            transform.translation.y += direction.y * step;
        }

        // Update rotation to face movement direction
        if distance > 1.0 {
            let direction = (target_world - current).normalize();
            // Snap to cardinal direction (0, 90, 180, 270 degrees)
            let angle = snap_to_cardinal_angle(direction);
            movement.target_rotation = angle;
            transform.rotation = Quat::from_rotation_z(angle);
        }
    }
}

/// Snap a direction vector to the nearest cardinal angle for sprite rotation.
///
/// Vehicle sprites are drawn facing North (up) in the SVG files.
/// Returns the rotation angle to make the sprite face the movement direction.
fn snap_to_cardinal_angle(direction: Vec2) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    // Sprite faces North (up) by default, so:
    // - North (up):    0° rotation
    // - East (right): -90° rotation (90° clockwise)
    // - South (down): 180° rotation
    // - West (left):   90° rotation (90° counter-clockwise)
    if direction.x.abs() > direction.y.abs() {
        // Horizontal movement
        if direction.x > 0.0 {
            -FRAC_PI_2 // East
        } else {
            FRAC_PI_2 // West
        }
    } else {
        // Vertical movement
        if direction.y > 0.0 {
            0.0 // North
        } else {
            PI // South
        }
    }
}

/// System that detects when vehicles have reached their goal.
///
/// When a vehicle reaches its destination (no more path), update its movement phase.
pub fn northstar_arrival_detection(
    mut query: Query<
        (&AgentPos, &Driver, &mut VehicleMovement, &BelongsToSite),
        (Without<NextPos>, Without<Pathfind>, Without<Path>),
    >,
    multi_site: Res<MultiSiteManager>,
) {
    for (agent_pos, driver, mut movement, belongs) in query.iter_mut() {
        match movement.phase {
            MovementPhase::Arriving => {
                // Check if we've reached our assigned bay
                if let Some((bay_x, bay_y)) = driver.assigned_bay {
                    let at_bay = agent_pos.0.x == bay_x as u32 && agent_pos.0.y == bay_y as u32;
                    if at_bay {
                        movement.phase = MovementPhase::Parked;
                        // Blocking component is already present from spawn
                        info!("Vehicle {} arrived at bay and parked", driver.id);
                    }
                }
            }
            MovementPhase::DepartingHappy | MovementPhase::DepartingAngry => {
                // Check if we've reached the exit position for this site
                let exit_pos = multi_site
                    .get_site(belongs.site_id)
                    .map(|site| site.grid.exit_pos)
                    .unwrap_or((15, 11));

                let at_exit =
                    agent_pos.0.x == exit_pos.0 as u32 && agent_pos.0.y == exit_pos.1 as u32;

                if at_exit {
                    movement.phase = MovementPhase::Exited;
                    info!("Vehicle {} exited the lot", driver.id);
                }
            }
            _ => {}
        }
    }
}

/// System that triggers pathfinding when vehicles need to depart.
///
/// When a driver's state changes to Leaving or LeftAngry, insert a Pathfind
/// component to route them to the exit.
pub fn northstar_trigger_departure(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &Driver,
            &AgentPos,
            &mut VehicleMovement,
            &BelongsToSite,
        ),
        (Without<Pathfind>, Without<Path>),
    >,
    multi_site: Res<MultiSiteManager>,
) {
    for (entity, driver, _agent_pos, mut movement, belongs) in query.iter_mut() {
        // Check if driver should be departing from a parked state
        let should_depart = matches!(
            driver.state,
            DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
        ) && movement.phase == MovementPhase::Parked;

        // Drive-through cars (and any stuck departing car) are spawned/transitioned into
        // DepartingHappy/DepartingAngry with a Pathfind, but if that Pathfind is stripped
        // by the reroute-failure timeout they end up with no path and no way to trigger
        // `should_depart` (which requires Parked phase). Detect that case and re-insert a
        // Pathfind so they can eventually reach the exit instead of blocking the entry forever.
        let needs_repath = matches!(
            movement.phase,
            MovementPhase::DepartingHappy | MovementPhase::DepartingAngry
        );

        if !should_depart && !needs_repath {
            continue;
        }

        if should_depart {
            // Update movement phase and speed
            movement.phase = if driver.state == DriverState::LeftAngry {
                movement.speed = 280.0; // Angry drivers are faster
                MovementPhase::DepartingAngry
            } else {
                movement.speed = 180.0; // Normal departure speed
                MovementPhase::DepartingHappy
            };
        }
        // For needs_repath: phase/speed are already set correctly; just re-add Pathfind below.

        // Remove Blocking so departing vehicles don't deadlock on single-lane exits.
        // They still pathfind around static obstacles; this just relaxes local avoidance.
        commands.entity(entity).try_remove::<Blocking>();

        // Get the exit position from the site's grid
        let exit_pos = multi_site
            .get_site(belongs.site_id)
            .map(|site| site.grid.exit_pos)
            .unwrap_or((15, 11)); // Fallback to default if site not found

        let exit_uvec = UVec3::new(exit_pos.0 as u32, exit_pos.1 as u32, 0);

        // Insert Pathfind to route to exit
        commands
            .entity(entity)
            .try_insert(Pathfind::new(exit_uvec).mode(PathfindMode::AStar));

        if should_depart {
            info!(
                "Vehicle {} departing to exit at ({}, {})",
                driver.id, exit_uvec.x, exit_uvec.y
            );
        } else {
            info!(
                "Vehicle {} re-pathing to exit after reroute timeout ({}, {})",
                driver.id, exit_uvec.x, exit_uvec.y
            );
        }
    }
}

/// System that cleans up exited vehicles.
pub fn northstar_cleanup_exited(
    mut commands: Commands,
    query: Query<(Entity, &VehicleMovement, &Driver)>,
) {
    for (entity, movement, driver) in query.iter() {
        if movement.phase == MovementPhase::Exited {
            info!("Despawning exited vehicle {}", driver.id);
            commands.entity(entity).try_despawn();
        }
    }
}

/// System that handles pathfinding failures.
///
/// If a vehicle can't find a path, log a warning and potentially retry.
/// Works for both Driver and AmbientVehicle entities.
pub fn northstar_handle_pathfinding_failed(
    mut commands: Commands,
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut query: Query<(
        Entity,
        Option<&mut Driver>,
        Option<&mut VehicleMovement>,
        &PathfindingFailed,
        Option<&mut PathfindCooldown>,
        Option<&Pathfind>,
    )>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();

    for (entity, driver, movement, _pathfind_failed, cooldown, pathfind) in query.iter_mut() {
        if let Some(mut cooldown) = cooldown {
            cooldown.timer -= dt;
            cooldown.total_stuck_time += dt;

            if cooldown.total_stuck_time >= MAX_STUCK_TIME {
                if let Some(mut driver) = driver {
                    warn!(
                        "Vehicle {} stuck on pathfinding for {:.1}s - forcing exit",
                        driver.id, cooldown.total_stuck_time
                    );
                    driver.state = DriverState::LeftAngry;
                    driver.patience = 0.0;
                }

                if let Some(mut movement) = movement {
                    movement.phase = MovementPhase::Exited;
                }

                commands
                    .entity(entity)
                    .try_remove::<Pathfind>()
                    .try_remove::<Path>()
                    .try_remove::<PathfindingFailed>()
                    .try_remove::<PathfindCooldown>();
                continue;
            }

            if cooldown.timer <= 0.0 {
                if let Some(pathfind) = pathfind {
                    commands.entity(entity).try_insert(pathfind.clone());
                }
                cooldown.timer = 0.5;
            }
        } else {
            if let Some(driver) = driver.as_ref() {
                warn!(
                    "Pathfinding failed for driver {} - starting cooldown",
                    driver.id
                );
            } else {
                trace!(
                    "Pathfinding failed for ambient {:?} - starting cooldown",
                    entity
                );
            }
            commands
                .entity(entity)
                .try_insert(PathfindCooldown::default());
        }
    }
}

/// System that handles reroute failures with cooldown.
///
/// When a vehicle fails to reroute, we add a cooldown timer to prevent
/// rapid retry loops. For drivers, if stuck too long they become frustrated
/// and leave. For ambient vehicles, they just retry until they can move.
pub fn northstar_handle_reroute_failed(
    mut commands: Commands,
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut query: Query<(
        Entity,
        Option<&mut Driver>,
        &RerouteFailed,
        Option<&mut RerouteCooldown>,
    )>,
) {
    if game_clock.is_paused() {
        return;
    }

    let dt = time.delta_secs();

    for (entity, driver, _reroute_failed, cooldown) in query.iter_mut() {
        let is_driver = driver.is_some();
        let driver_id = driver.as_ref().map(|d| d.id.clone());

        if let Some(mut cooldown) = cooldown {
            // Tick the cooldown timer
            cooldown.timer -= dt;
            cooldown.total_stuck_time += dt;

            // Check if stuck too long - force driver to leave (ambient vehicles just keep waiting)
            if is_driver
                && cooldown.total_stuck_time >= MAX_STUCK_TIME
                && let Some(mut driver) = driver
            {
                warn!(
                    "Vehicle {} stuck for {:.1}s - forcing departure",
                    driver.id, cooldown.total_stuck_time
                );

                // Make driver leave (mood set by emotion system based on state)
                driver.state = DriverState::LeftAngry;
                driver.patience = 0.0;

                // Remove pathfinding components so departure system can take over
                commands
                    .entity(entity)
                    .try_remove::<RerouteFailed>()
                    .try_remove::<RerouteCooldown>()
                    .try_remove::<Path>()
                    .try_remove::<Pathfind>();

                continue;
            }

            // Check if cooldown expired - retry pathfinding
            if cooldown.timer <= 0.0 {
                if let Some(id) = &driver_id {
                    info!(
                        "Vehicle {} cooldown expired, retrying pathfind (stuck {:.1}s)",
                        id, cooldown.total_stuck_time
                    );
                } else {
                    trace!(
                        "Ambient {:?} cooldown expired, retrying pathfind (stuck {:.1}s)",
                        entity, cooldown.total_stuck_time
                    );
                }

                // Keep total_stuck_time but reset timer for next potential failure
                let total_stuck = cooldown.total_stuck_time;

                // Remove RerouteFailed to allow bevy_northstar to retry
                // Keep RerouteCooldown with updated total_stuck_time in case it fails again
                commands.entity(entity).try_remove::<RerouteFailed>();

                // Update cooldown for potential next failure
                commands.entity(entity).try_insert(RerouteCooldown {
                    timer: 0.5, // Reset timer for next potential failure
                    total_stuck_time: total_stuck,
                });
            }
        } else {
            // No cooldown yet - add one
            if let Some(id) = &driver_id {
                warn!("Reroute failed for vehicle {} - adding cooldown", id);
            } else {
                trace!("Reroute failed for ambient {:?} - adding cooldown", entity);
            }
            commands
                .entity(entity)
                .try_insert(RerouteCooldown::default());
        }
    }
}

/// System that clears reroute cooldown when pathfinding succeeds.
///
/// If a vehicle has a cooldown but no longer has RerouteFailed, it means
/// pathfinding succeeded and we should clear the cooldown.
pub fn northstar_clear_cooldown_on_success(
    mut commands: Commands,
    query: Query<Entity, (With<RerouteCooldown>, Without<RerouteFailed>, With<Path>)>,
) {
    for entity in query.iter() {
        commands.entity(entity).try_remove::<RerouteCooldown>();
    }
}

/// System that cleans up ambient vehicles that have reached their destination.
///
/// Ambient vehicles pathfind from entry to exit (or exit to entry). When they
/// reach their goal and have no more path, despawn them.
pub fn northstar_cleanup_ambient(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            &AgentPos,
            &crate::systems::ambient_traffic::AmbientVehicle,
            &BelongsToSite,
        ),
        (Without<NextPos>, Without<Pathfind>, Without<Path>),
    >,
    multi_site: Res<MultiSiteManager>,
) {
    use crate::systems::ambient_traffic::TrafficDirection;

    for (entity, agent_pos, ambient, belongs) in query.iter() {
        // Get the expected exit position based on direction
        let Some(site) = multi_site.get_site(belongs.site_id) else {
            continue;
        };

        let goal_pos = match ambient.direction {
            TrafficDirection::LeftToRight => site.grid.exit_pos,
            TrafficDirection::RightToLeft => site.grid.entry_pos,
        };

        let at_goal = agent_pos.0.x == goal_pos.0 as u32 && agent_pos.0.y == goal_pos.1 as u32;

        if at_goal {
            // Reached destination, despawn
            commands.entity(entity).try_despawn();
        }
    }
}
