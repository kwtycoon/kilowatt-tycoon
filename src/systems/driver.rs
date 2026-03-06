//! Driver spawn and behavior systems.
//!
//! # Driver Decision Rules
//!
//! These rules govern how simulated drivers choose chargers and decide to leave.
//!
//! ## Rule 1 — OCPI-Only Information (pre-plug-in)
//!
//! Before plugging in, a driver can only use data that would be visible on a
//! public charging app backed by the OCPI 2.3 feed (see [`crate::ocpi::types`]):
//!
//! | Available (OCPI)                | NOT available (internal)         |
//! |---------------------------------|----------------------------------|
//! | `EvseStatus` (Available, …)     | `charger.health`                 |
//! | `Connector.max_electric_power`  | `charger.reliability`            |
//! | `Connector.standard` (CCS/J1772)| `charger.get_derated_power()`    |
//! | `Connector.power_type` (AC/DC)  | Grid allocation / queue lengths  |
//!
//! The charger scoring function therefore uses `rated_power_kw` (the OCPI
//! advertised max) — not health, reliability, or derated power.
//!
//! ## Rule 2 — Direct Experience (post-plug-in)
//!
//! Once physically plugged in, a driver can observe their own charging rate on
//! the vehicle dashboard. This means they **can** detect 0 kW delivery and poor
//! power ratios — justifying the zero-energy departure logic.
//!
//! ## Rule 3 — Visual Observation (at the site)
//!
//! A driver physically at the site can see which bays are empty and which
//! charger screens show errors. This justifies the alternative-charger search
//! when a driver is frustrated or receiving zero power.

use bevy::prelude::*;
use bevy_northstar::prelude::*;

use crate::components::VehicleFootprint;
use crate::components::charger::{Charger, ChargerState, ChargerType};
use crate::components::driver::{
    ChargingSession, Driver, DriverMood, DriverState, MovementPhase, VehicleMovement, VehicleType,
};
use crate::events::{
    ChargerFaultEvent, ChargingCompleteEvent, DriverArrivedEvent, DriverLeftEvent,
};
use crate::resources::{
    BuildState, EnvironmentState, GameClock, GameState, ReputationSource, SiteGrid, TechStatus,
    TechnicianState, TutorialState, TutorialStep, generate_evcc_mac,
    generate_procedural_driver_for_site,
};
use crate::systems::charger::check_connector_jam;
use crate::systems::sprite::spawn_floating_money;

/// How long (in game-seconds) a driver tolerates receiving 0 kW before seeking
/// an alternative charger or leaving the site. Two game-minutes.
pub const ZERO_POWER_TOLERANCE_GAME_SECONDS: f32 = 120.0;

/// Check if technician is currently working on this charger
fn is_technician_active_on_charger(tech_state: &TechnicianState, charger_entity: Entity) -> bool {
    matches!(
        tech_state.status,
        TechStatus::EnRoute | TechStatus::Repairing
    ) && tech_state.target_charger == Some(charger_entity)
}

/// Score a charger using only OCPI-visible attributes (Rule 1).
///
/// Returns `rated_power_kw` — the advertised maximum power that a driver
/// would see on their charging app. Internal fields like `health`,
/// `reliability`, and `get_derated_power()` are intentionally excluded.
fn score_charger_ocpi(charger: &Charger) -> f32 {
    charger.rated_power_kw
}

/// Charger candidate snapshot for [`find_best_alternative_charger`].
struct ChargerCandidate {
    entity: Entity,
    site_id: crate::resources::SiteId,
    charger_type: ChargerType,
    can_accept: bool,
    rated_power_kw: f32,
    grid_position: Option<(i32, i32)>,
}

/// Find the best available alternative charger at a site, excluding an
/// optional entity (typically the driver's current charger).
///
/// Uses [`score_charger_ocpi`] to rank candidates by OCPI-visible power
/// rating (Rule 1). When a driver is physically at the site, they can see
/// which chargers appear operational and which bays are empty (Rule 3).
///
/// Only considers chargers whose linked parking bay is NOT already occupied
/// by another driver, preventing the "ghost bay" problem where a charger is
/// used cross-bay while its physical bay appears free.
///
/// Returns `(charger_entity, linked_bay_position)` so the caller can move
/// the driver to the alternative bay.
fn find_best_alternative_charger(
    candidates: &[ChargerCandidate],
    site_id: crate::resources::SiteId,
    compatible_types: &[ChargerType],
    exclude_entity: Option<Entity>,
    tech_state: &TechnicianState,
    occupied_bays: &[(i32, i32)],
    grid: &SiteGrid,
) -> Option<(Entity, (i32, i32))> {
    let mut best: Option<(Entity, (i32, i32), f32)> = None;

    for cand in candidates {
        if cand.site_id != site_id {
            continue;
        }
        if Some(cand.entity) == exclude_entity {
            continue;
        }
        if !compatible_types.contains(&cand.charger_type) {
            continue;
        }
        if !cand.can_accept {
            continue;
        }
        if is_technician_active_on_charger(tech_state, cand.entity) {
            continue;
        }

        let Some(charger_pos) = cand.grid_position else {
            continue;
        };
        let Some(bay_pos) = grid
            .get_tile(charger_pos.0, charger_pos.1)
            .and_then(|t| t.linked_parking_bay)
        else {
            continue;
        };

        if occupied_bays.contains(&bay_pos) {
            continue;
        }

        if best.is_none_or(|(_, _, s)| cand.rated_power_kw > s) {
            best = Some((cand.entity, bay_pos, cand.rated_power_kw));
        }
    }

    best.map(|(e, bay, _)| (e, bay))
}

/// Collect charger candidate data for [`find_best_alternative_charger`].
///
/// Snapshots the charger query into a simple vec so the helper is decoupled
/// from query mutability.
fn collect_charger_candidates(
    chargers: &Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
) -> Vec<ChargerCandidate> {
    chargers
        .iter()
        .map(|(e, c, b)| ChargerCandidate {
            entity: e,
            site_id: b.site_id,
            charger_type: c.charger_type,
            can_accept: c.can_accept_driver(),
            rated_power_kw: score_charger_ocpi(c),
            grid_position: c.grid_position,
        })
        .collect()
}

/// Spawn drivers according to schedule (only when station is open)
///
/// Uses bevy_northstar for pathfinding - spawns vehicles with AgentPos and Pathfind
/// components, letting the pathfinding plugin handle routing.
pub fn driver_spawn_system(
    mut commands: Commands,
    build_state: Res<BuildState>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    existing_drivers: Query<(&Driver, &crate::components::BelongsToSite, &AgentPos)>,
    chargers: Query<(Entity, &Charger, &crate::components::BelongsToSite), Without<Driver>>,
    mut arrived_events: MessageWriter<DriverArrivedEvent>,
    game_state: Res<GameState>,
    environment: Res<EnvironmentState>,
    blocking_map: Res<BlockingMap>,
    profile: Res<crate::resources::PlayerProfile>,
    transformers: Query<&crate::components::power::Transformer>,
    charger_index: Res<crate::hooks::ChargerIndex>,
) {
    // Only spawn drivers when station is open
    if !build_state.is_open {
        return;
    }

    if game_clock.is_paused() {
        return;
    }

    // Don't spawn new drivers during end-of-day wind-down
    if game_clock.day_ending {
        return;
    }

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };
    let site_id = &viewed_id;

    // Site offline during transformer fire or after destruction -- no new arrivals
    if transformers
        .iter()
        .any(|t| t.site_id == *site_id && (t.on_fire || t.destroyed))
    {
        return;
    }

    let charger_bays = site_state.grid.get_charger_bays();
    if charger_bays.is_empty() {
        return;
    }
    let charger_bay_positions: Vec<(i32, i32)> =
        charger_bays.iter().map(|&(x, y, _)| (x, y)).collect();

    // Count current drivers at this site - cap to site's max_vehicles limit
    let current_driver_count = existing_drivers
        .iter()
        .filter(|(_, b, _)| b.site_id == *site_id)
        .count();

    if current_driver_count >= site_state.max_vehicles {
        return;
    }

    // Check if entry is occupied using BlockingMap
    // This checks both the entry tile and adjacent tiles that vehicles path through
    let entry_pos = site_state.grid.entry_pos;
    let entry_uvec = UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0);

    let entry_blocked = blocking_map.0.contains_key(&entry_uvec)
        || [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
            let nx = entry_pos.0 + dx;
            let ny = entry_pos.1 + dy;
            site_state.grid.get_content(nx, ny).is_public_road()
                && blocking_map
                    .0
                    .contains_key(&UVec3::new(nx as u32, ny as u32, 0))
        });

    if entry_blocked {
        return;
    }

    // Compute occupied bays (bays with drivers already assigned)
    let mut occupied_bays: Vec<(i32, i32)> = existing_drivers
        .iter()
        .filter(|(_, b, _)| b.site_id == *site_id)
        .filter(|(d, _, _)| {
            !matches!(
                d.state,
                DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
            )
        })
        .filter_map(|(d, _, _)| d.assigned_bay)
        .collect();

    // Compute occupied waiting tiles (tiles with drivers waiting for a bay)
    let mut occupied_waiting: Vec<(i32, i32)> = existing_drivers
        .iter()
        .filter(|(_, b, _)| b.site_id == *site_id)
        .filter(|(d, _, _)| {
            !matches!(
                d.state,
                DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
            )
        })
        .filter_map(|(d, _, _)| d.waiting_tile)
        .collect();

    // IMPORTANT: limit spawning to at most 1 vehicle per site per frame.
    //
    // Bevy queries won't "see" entities spawned earlier in this same system run,
    // so without this guard multiple vehicles can be spawned onto the entry tile
    // in a single frame (scheduled while-loop + procedural), causing immediate
    // gridlock and reroute failures.
    let mut spawned_vehicle_this_frame = false;

    // Process scheduled drivers
    while site_state.driver_schedule.next_driver_index < site_state.driver_schedule.drivers.len() {
        let driver_data =
            &site_state.driver_schedule.drivers[site_state.driver_schedule.next_driver_index];

        if game_clock.game_time >= driver_data.arrival_time {
            // Filter bays to those compatible with this vehicle type
            let compatible_bays: Vec<_> = charger_bays
                .iter()
                .filter(|(_, _, pad_type)| {
                    driver_data
                        .vehicle
                        .is_compatible_with(pad_type.to_charger_type())
                })
                .collect();

            // No compatible chargers - skip this driver
            if compatible_bays.is_empty() {
                info!(
                    "No compatible chargers for {} ({:?}) - skipping",
                    driver_data.id, driver_data.vehicle
                );
                site_state.driver_schedule.next_driver_index += 1;
                continue;
            }

            // Check for available (unoccupied) bays
            let available_compatible: Vec<_> = compatible_bays
                .iter()
                .filter(|(x, y, _)| !occupied_bays.contains(&(*x, *y)))
                .collect();

            // Get site root entity
            let Some(root_entity) = site_state.root_entity else {
                warn!("Site {:?} has no root entity, cannot spawn driver", site_id);
                continue;
            };

            let entry_pos = site_state.grid.entry_pos;
            let exit_pos = site_state.grid.exit_pos;
            let world_pos = SiteGrid::grid_to_world(entry_pos.0, entry_pos.1);
            let pos = Vec3::new(world_pos.x, world_pos.y, 1.0);

            // No free bays -- try to find a waiting tile near the charger area
            if available_compatible.is_empty() {
                let wait_tile = site_state.grid.find_waiting_tile(
                    &occupied_waiting,
                    &occupied_bays,
                    &charger_bay_positions,
                );

                if let Some((wx, wy)) = wait_tile {
                    let speed = 100.0 + (driver_data.id.len() as f32 * 5.0);
                    let movement = VehicleMovement {
                        speed,
                        phase: MovementPhase::Arriving,
                        waypoints: vec![world_pos],
                        ..default()
                    };

                    let footprint = VehicleFootprint {
                        length_tiles: driver_data.vehicle.footprint_length_tiles(),
                    };

                    let evcc_id = driver_data
                        .evcc_id
                        .clone()
                        .unwrap_or_else(|| generate_evcc_mac(&mut rand::rng()));

                    let is_roaming = rand::random::<f32>() < 0.25;
                    let driver = Driver {
                        id: driver_data.id.clone(),
                        evcc_id,
                        vehicle_name: driver_data.vehicle_name.clone(),
                        vehicle_type: driver_data.vehicle,
                        patience_level: driver_data.patience,
                        patience: driver_data.patience.initial_patience(),
                        charge_needed_kwh: driver_data.charge_needed_kwh,
                        target_charger_id: driver_data.target_charger.clone(),
                        assigned_charger: None,
                        assigned_bay: None,
                        waiting_tile: Some((wx, wy)),
                        state: DriverState::Arriving,
                        is_roaming,
                        ..default()
                    };

                    let driver_id = driver.id.clone();
                    let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
                    let pathfind = Pathfind::new_2d(wx as u32, wy as u32);

                    let mut entity = Entity::PLACEHOLDER;
                    commands.entity(root_entity).with_children(|parent| {
                        entity = parent
                            .spawn((
                                driver,
                                movement,
                                footprint,
                                agent_pos,
                                pathfind,
                                Blocking,
                                Transform::from_translation(pos),
                                GlobalTransform::default(),
                                Visibility::default(),
                                crate::components::BelongsToSite::new(*site_id),
                            ))
                            .id();
                    });

                    info!(
                        "Driver {} heading to waiting tile ({}, {}) - all bays occupied",
                        driver_id, wx, wy
                    );

                    arrived_events.write(DriverArrivedEvent {
                        driver_entity: entity,
                        driver_id,
                        target_charger_id: None,
                    });

                    occupied_waiting.push((wx, wy));
                    site_state.driver_schedule.next_driver_index += 1;
                    spawned_vehicle_this_frame = true;
                    break;
                }

                // Fallback: no waiting tile available, spawn as drive-through
                let is_angry = rand::random::<f32>() < 0.2;
                let (state, mood) = if is_angry {
                    (DriverState::LeftAngry, DriverMood::Angry)
                } else {
                    (DriverState::Leaving, DriverMood::Neutral)
                };

                let speed = 100.0 + (driver_data.id.len() as f32 * 5.0);
                let movement = VehicleMovement {
                    speed,
                    phase: MovementPhase::DepartingHappy,
                    waypoints: vec![world_pos],
                    ..default()
                };

                let footprint = VehicleFootprint {
                    length_tiles: driver_data.vehicle.footprint_length_tiles(),
                };

                let evcc_id = driver_data
                    .evcc_id
                    .clone()
                    .unwrap_or_else(|| generate_evcc_mac(&mut rand::rng()));

                let driver = Driver {
                    id: driver_data.id.clone(),
                    evcc_id,
                    vehicle_name: driver_data.vehicle_name.clone(),
                    vehicle_type: driver_data.vehicle,
                    patience_level: driver_data.patience,
                    patience: driver_data.patience.initial_patience(),
                    charge_needed_kwh: driver_data.charge_needed_kwh,
                    target_charger_id: driver_data.target_charger.clone(),
                    assigned_charger: None,
                    assigned_bay: None,
                    state,
                    mood,
                    ..default()
                };

                let driver_id = driver.id.clone();
                let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
                let pathfind = Pathfind::new_2d(exit_pos.0 as u32, exit_pos.1 as u32);

                commands.entity(root_entity).with_children(|parent| {
                    parent.spawn((
                        driver,
                        movement,
                        footprint,
                        agent_pos,
                        pathfind,
                        Transform::from_translation(pos),
                        GlobalTransform::default(),
                        Visibility::default(),
                        crate::components::BelongsToSite::new(*site_id),
                    ));
                });

                info!(
                    "Driver {} couldn't find a bay, driving through ({})",
                    driver_id,
                    if is_angry { "angry" } else { "neutral" }
                );

                site_state.driver_schedule.next_driver_index += 1;
                spawned_vehicle_this_frame = true;
                break;
            }

            // Pick the bay with the highest OCPI-advertised charger power (Rule 1).
            // Small random jitter breaks ties when chargers have equal power.
            let selected_bay = {
                let mut best: Option<(
                    &(i32, i32, crate::resources::site_grid::ChargerPadType),
                    f32,
                )> = None;
                for bay in &available_compatible {
                    let score = site_state
                        .grid
                        .get_tile(bay.0, bay.1)
                        .and_then(|t| t.linked_charger_pad)
                        .and_then(|(px, py)| charger_index.get_by_position(px, py))
                        .and_then(|e| chargers.get(e).ok())
                        .map(|(_, c, _)| score_charger_ocpi(c))
                        .unwrap_or(0.0)
                        + rand::random::<f32>() * 0.01;
                    if best.is_none_or(|(_, s)| score > s) {
                        best = Some((bay, score));
                    }
                }
                best.map(|(b, _)| b)
            };

            let Some(&(bay_x, bay_y, _charger_type)) = selected_bay else {
                break;
            };

            let speed = 100.0 + (driver_data.id.len() as f32 * 5.0);
            let movement = VehicleMovement {
                speed,
                phase: MovementPhase::Arriving,
                waypoints: vec![world_pos],
                ..default()
            };

            let footprint = VehicleFootprint {
                length_tiles: driver_data.vehicle.footprint_length_tiles(),
            };

            let evcc_id = driver_data
                .evcc_id
                .clone()
                .unwrap_or_else(|| generate_evcc_mac(&mut rand::rng()));

            let is_roaming = rand::random::<f32>() < 0.25;
            let driver = Driver {
                id: driver_data.id.clone(),
                evcc_id,
                vehicle_name: driver_data.vehicle_name.clone(),
                vehicle_type: driver_data.vehicle,
                patience_level: driver_data.patience,
                patience: driver_data.patience.initial_patience(),
                charge_needed_kwh: driver_data.charge_needed_kwh,
                target_charger_id: driver_data.target_charger.clone(),
                assigned_charger: None,
                assigned_bay: Some((bay_x, bay_y)),
                state: DriverState::Arriving,
                is_roaming,
                ..default()
            };

            let driver_id = driver.id.clone();

            // bevy_northstar components for pathfinding
            let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
            let pathfind = Pathfind::new_2d(bay_x as u32, bay_y as u32);

            let mut entity = Entity::PLACEHOLDER;
            commands.entity(root_entity).with_children(|parent| {
                entity = parent
                    .spawn((
                        driver,
                        movement,
                        footprint,
                        agent_pos,
                        pathfind,
                        Blocking, // Enable collision avoidance from spawn
                        Transform::from_translation(pos),
                        GlobalTransform::default(),
                        Visibility::default(),
                        crate::components::BelongsToSite::new(*site_id),
                    ))
                    .id();
            });

            info!(
                "Driver {} arrived at site {:?}{}, pathfinding to bay ({}, {})",
                driver_id,
                site_id,
                if is_roaming { " [roaming]" } else { "" },
                bay_x,
                bay_y
            );

            arrived_events.write(DriverArrivedEvent {
                driver_entity: entity,
                driver_id,
                target_charger_id: None,
            });

            occupied_bays.push((bay_x, bay_y));
            site_state.driver_schedule.next_driver_index += 1;
            spawned_vehicle_this_frame = true;
            break; // Only spawn one vehicle per site per frame
        } else {
            break;
        }
    }

    // === PROCEDURAL DRIVER GENERATION ===
    if site_state.driver_schedule.next_driver_index >= site_state.driver_schedule.drivers.len()
        && site_state.demand_state.enabled
    {
        let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();
        site_state.demand_state.tick(delta_game_seconds);

        // If we already spawned a scheduled driver this frame, don't also spawn
        // a procedural driver onto the same entry tile.
        if !spawned_vehicle_this_frame && site_state.demand_state.should_spawn() {
            let hour = game_clock.hour();
            let effective_price = site_state.service_strategy.pricing.effective_price(
                game_clock.game_time,
                &site_state.site_energy_config,
                site_state.charger_utilization,
            );
            let base_demand = site_state
                .demand_state
                .calculate_effective_demand_for_archetype(
                    game_state.reputation,
                    environment.current_weather.demand_multiplier(),
                    environment.news_demand_multiplier,
                    site_state.site_upgrades.demand_multiplier(),
                    hour,
                    crate::resources::demand::price_elasticity_factor(effective_price),
                    site_state.archetype,
                );

            // Apply average charger reliability as a demand multiplier.
            // Drivers prefer sites with reliable chargers - word gets around.
            // Calculate average session_attraction across all site chargers.
            let mut total_attraction = 0.0f32;
            let mut charger_count = 0u32;
            for (_, charger, charger_belongs) in &chargers {
                if charger_belongs.site_id == *site_id && !charger.is_disabled {
                    total_attraction += charger.session_attraction();
                    charger_count += 1;
                }
            }
            let reliability_multiplier = if charger_count > 0 {
                total_attraction / charger_count as f32
            } else {
                1.0
            };

            // Apply character perk multiplier if CustomerMagnet is active
            let customer_perk_multiplier = match profile.active_perk() {
                Some(crate::resources::CharacterPerk::CustomerMagnet { demand_multiplier }) => {
                    demand_multiplier
                }
                _ => 1.0,
            };
            let capacity_multiplier =
                crate::resources::demand::capacity_demand_multiplier(charger_count);
            let scenario_multiplier = site_state.scenario_effects.demand_multiplier();
            let effective_demand = base_demand
                * reliability_multiplier
                * customer_perk_multiplier
                * capacity_multiplier
                * scenario_multiplier;

            let next_interval = site_state
                .demand_state
                .calculate_spawn_interval(effective_demand);
            site_state.demand_state.reset_timer(next_interval);

            let mut rng = rand::rng();
            let id_counter = site_state.demand_state.next_id();
            let driver_data =
                generate_procedural_driver_for_site(&mut rng, id_counter, site_state.archetype);

            let compatible_bays: Vec<_> = charger_bays
                .iter()
                .filter(|(_, _, pad_type)| {
                    driver_data
                        .vehicle
                        .is_compatible_with(pad_type.to_charger_type())
                })
                .collect();

            if compatible_bays.is_empty() {
                return;
            }

            let available_compatible: Vec<_> = compatible_bays
                .iter()
                .filter(|(x, y, _)| !occupied_bays.contains(&(*x, *y)))
                .collect();

            let Some(root_entity) = site_state.root_entity else {
                return;
            };

            let entry_pos = site_state.grid.entry_pos;
            let exit_pos = site_state.grid.exit_pos;
            let world_pos = SiteGrid::grid_to_world(entry_pos.0, entry_pos.1);
            let pos = Vec3::new(world_pos.x, world_pos.y, 1.0);

            // No free bays -- try to find a waiting tile near the charger area
            if available_compatible.is_empty() {
                let wait_tile = site_state.grid.find_waiting_tile(
                    &occupied_waiting,
                    &occupied_bays,
                    &charger_bay_positions,
                );

                if let Some((wx, wy)) = wait_tile {
                    let speed = 100.0 + (driver_data.id.len() as f32 * 5.0);
                    let movement = VehicleMovement {
                        speed,
                        phase: MovementPhase::Arriving,
                        waypoints: vec![world_pos],
                        ..default()
                    };

                    let footprint = VehicleFootprint {
                        length_tiles: driver_data.vehicle.footprint_length_tiles(),
                    };

                    let evcc_id = driver_data
                        .evcc_id
                        .clone()
                        .unwrap_or_else(|| generate_evcc_mac(&mut rand::rng()));

                    let is_roaming = rand::random::<f32>() < 0.25;
                    let driver = Driver {
                        id: driver_data.id.clone(),
                        evcc_id,
                        vehicle_name: driver_data.vehicle_name.clone(),
                        vehicle_type: driver_data.vehicle,
                        patience_level: driver_data.patience,
                        patience: driver_data.patience.initial_patience(),
                        charge_needed_kwh: driver_data.charge_needed_kwh,
                        target_charger_id: driver_data.target_charger.clone(),
                        assigned_charger: None,
                        assigned_bay: None,
                        waiting_tile: Some((wx, wy)),
                        state: DriverState::Arriving,
                        is_roaming,
                        ..default()
                    };

                    let driver_id = driver.id.clone();
                    let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
                    let pathfind = Pathfind::new_2d(wx as u32, wy as u32);

                    let mut entity = Entity::PLACEHOLDER;
                    commands.entity(root_entity).with_children(|parent| {
                        entity = parent
                            .spawn((
                                driver,
                                movement,
                                footprint,
                                agent_pos,
                                pathfind,
                                Blocking,
                                Transform::from_translation(pos),
                                GlobalTransform::default(),
                                Visibility::default(),
                                crate::components::BelongsToSite::new(*site_id),
                            ))
                            .id();
                    });

                    info!(
                        "Procedural driver {} heading to waiting tile ({}, {}) - all bays occupied",
                        driver_id, wx, wy
                    );

                    arrived_events.write(DriverArrivedEvent {
                        driver_entity: entity,
                        driver_id,
                        target_charger_id: None,
                    });

                    occupied_waiting.push((wx, wy));
                    return;
                }

                // Fallback: no waiting tile available, spawn as drive-through
                let is_angry = rand::random::<f32>() < 0.2;
                let (state, mood) = if is_angry {
                    (DriverState::LeftAngry, DriverMood::Angry)
                } else {
                    (DriverState::Leaving, DriverMood::Neutral)
                };

                let speed = 100.0 + (driver_data.id.len() as f32 * 5.0);
                let movement = VehicleMovement {
                    speed,
                    phase: MovementPhase::DepartingHappy,
                    waypoints: vec![world_pos],
                    ..default()
                };

                let footprint = VehicleFootprint {
                    length_tiles: driver_data.vehicle.footprint_length_tiles(),
                };

                let evcc_id = driver_data
                    .evcc_id
                    .clone()
                    .unwrap_or_else(|| generate_evcc_mac(&mut rand::rng()));

                let driver = Driver {
                    id: driver_data.id.clone(),
                    evcc_id,
                    vehicle_name: driver_data.vehicle_name.clone(),
                    vehicle_type: driver_data.vehicle,
                    patience_level: driver_data.patience,
                    patience: driver_data.patience.initial_patience(),
                    charge_needed_kwh: driver_data.charge_needed_kwh,
                    target_charger_id: driver_data.target_charger.clone(),
                    assigned_charger: None,
                    assigned_bay: None,
                    state,
                    mood,
                    ..default()
                };

                let driver_id = driver.id.clone();
                let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
                let pathfind = Pathfind::new_2d(exit_pos.0 as u32, exit_pos.1 as u32);

                commands.entity(root_entity).with_children(|parent| {
                    parent.spawn((
                        driver,
                        movement,
                        footprint,
                        agent_pos,
                        pathfind,
                        Transform::from_translation(pos),
                        GlobalTransform::default(),
                        Visibility::default(),
                        crate::components::BelongsToSite::new(*site_id),
                    ));
                });

                info!(
                    "Procedural driver {} couldn't find a bay, driving through ({})",
                    driver_id,
                    if is_angry { "angry" } else { "neutral" }
                );

                return;
            }

            // Pick the bay with the highest OCPI-advertised charger power (Rule 1).
            // Small random jitter breaks ties when chargers have equal power.
            let selected_bay = {
                let mut best: Option<(
                    &(i32, i32, crate::resources::site_grid::ChargerPadType),
                    f32,
                )> = None;
                for bay in &available_compatible {
                    let score = site_state
                        .grid
                        .get_tile(bay.0, bay.1)
                        .and_then(|t| t.linked_charger_pad)
                        .and_then(|(px, py)| charger_index.get_by_position(px, py))
                        .and_then(|e| chargers.get(e).ok())
                        .map(|(_, c, _)| score_charger_ocpi(c))
                        .unwrap_or(0.0)
                        + rand::random::<f32>() * 0.01;
                    if best.is_none_or(|(_, s)| score > s) {
                        best = Some((bay, score));
                    }
                }
                best.map(|(b, _)| b)
            };

            let Some(&(bay_x, bay_y, _charger_type)) = selected_bay else {
                return;
            };

            let speed = 100.0 + (driver_data.id.len() as f32 * 5.0);
            let movement = VehicleMovement {
                speed,
                phase: MovementPhase::Arriving,
                waypoints: vec![world_pos],
                ..default()
            };

            let footprint = VehicleFootprint {
                length_tiles: driver_data.vehicle.footprint_length_tiles(),
            };

            let evcc_id = driver_data
                .evcc_id
                .clone()
                .unwrap_or_else(|| generate_evcc_mac(&mut rand::rng()));

            let is_roaming = rand::random::<f32>() < 0.25;
            let driver = Driver {
                id: driver_data.id.clone(),
                evcc_id,
                vehicle_name: driver_data.vehicle_name.clone(),
                vehicle_type: driver_data.vehicle,
                patience_level: driver_data.patience,
                patience: driver_data.patience.initial_patience(),
                charge_needed_kwh: driver_data.charge_needed_kwh,
                target_charger_id: driver_data.target_charger.clone(),
                assigned_charger: None,
                assigned_bay: Some((bay_x, bay_y)),
                state: DriverState::Arriving,
                is_roaming,
                ..default()
            };

            let driver_id = driver.id.clone();
            let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
            let pathfind = Pathfind::new_2d(bay_x as u32, bay_y as u32);

            let mut entity = Entity::PLACEHOLDER;
            commands.entity(root_entity).with_children(|parent| {
                entity = parent
                    .spawn((
                        driver,
                        movement,
                        footprint,
                        agent_pos,
                        pathfind,
                        Blocking, // Enable collision avoidance from spawn
                        Transform::from_translation(pos),
                        GlobalTransform::default(),
                        Visibility::default(),
                        crate::components::BelongsToSite::new(*site_id),
                    ))
                    .id();
            });

            info!(
                "Procedural driver {} arrived (demand: {:.2}x){}, pathfinding to bay ({}, {})",
                driver_id,
                effective_demand,
                if is_roaming { " [roaming]" } else { "" },
                bay_x,
                bay_y
            );

            arrived_events.write(DriverArrivedEvent {
                driver_entity: entity,
                driver_id,
                target_charger_id: None,
            });

            occupied_bays.push((bay_x, bay_y));
        }
    }
}

/// System to transition drivers from Arriving to Charging when they reach their bay
pub fn driver_arrival_system(
    mut commands: Commands,
    mut drivers: Query<(
        Entity,
        &mut Driver,
        &mut VehicleMovement,
        &crate::components::BelongsToSite,
    )>,
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    game_clock: Res<GameClock>,
    tech_state: Res<TechnicianState>,
    charger_index: Res<crate::hooks::ChargerIndex>,
    mut fault_events: MessageWriter<ChargerFaultEvent>,
) {
    // During end-of-day wind-down, don't start new charging sessions.
    // Arriving drivers will be kicked by day_ending_system when they park.
    if game_clock.day_ending {
        return;
    }

    let occupied_bays: Vec<(i32, i32)> = drivers
        .iter()
        .filter(|(_, d, _, _)| {
            !matches!(
                d.state,
                DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
            )
        })
        .filter_map(|(_, d, _, _)| d.assigned_bay)
        .collect();

    for (driver_entity, mut driver, mut movement, belongs) in &mut drivers {
        // Check if driver just arrived (movement complete and still in Arriving state)
        if driver.state == DriverState::Arriving && movement.phase == MovementPhase::Parked {
            // Get the site's charger queue
            let Some(site_state) = multi_site.get_site_mut(belongs.site_id) else {
                continue;
            };

            // Waiting-tile arrival: no bay assigned yet, join the queue
            if driver.waiting_tile.is_some() && driver.assigned_bay.is_none() {
                driver.state = DriverState::Queued;
                for ct in driver.vehicle_type.compatible_charger_types() {
                    match ct {
                        ChargerType::DcFast => {
                            site_state.charger_queue.join_dcfc_queue(driver_entity);
                        }
                        ChargerType::AcLevel2 => {
                            site_state.charger_queue.join_l2_queue(driver_entity);
                        }
                    }
                }
                info!("Driver {} parked at waiting tile, joined queue", driver.id);
                continue;
            }

            // Resolve assigned charger (prefer existing assignment, otherwise derive from bay->pad link).
            // Uses ChargerIndex for reliable lookup - tile.charger_entity was never populated.
            let bay_charger = driver.assigned_bay.and_then(|(bay_x, bay_y)| {
                let bay_tile = site_state.grid.get_tile(bay_x, bay_y)?;
                if let Some((pad_x, pad_y)) = bay_tile.linked_charger_pad {
                    // Use ChargerIndex for reliable entity lookup by grid position
                    charger_index.get_by_position(pad_x, pad_y)
                } else {
                    None
                }
            });

            let resolved_charger = bay_charger.or(driver.assigned_charger);

            // Transition to charging if we have an assigned charger
            if let Some(charger_entity) = resolved_charger {
                driver.assigned_charger = Some(charger_entity);
                if let Ok((_, mut charger, _)) = chargers.get_mut(charger_entity) {
                    // Check if technician is working on this charger
                    if is_technician_active_on_charger(&tech_state, charger_entity) {
                        // Technician is working - driver becomes frustrated
                        // (mood will be set by emotion system based on state)
                        driver.state = DriverState::Frustrated;
                        driver.patience -= 30.0;
                        info!(
                            "Driver {} found technician at charger {}",
                            driver.id, charger.id
                        );
                    } else if charger.can_accept_driver() {
                        // Charger is available and working
                        driver.state = DriverState::Charging;
                        charger.is_charging = true;
                        // Record session start time for FCFS power allocation
                        charger.session_start_game_time = Some(game_clock.game_time);
                        // Set requested power so dispatch can allocate on this frame
                        charger.requested_power_kw = charger.get_derated_power();
                        info!("Driver {} started charging", driver.id);
                    } else if let Some(fault_type) = charger.current_fault {
                        // Charger is BROKEN - driver is frustrated!
                        // (mood will be set by emotion system based on state)
                        driver.state = DriverState::Frustrated;
                        driver.patience -= 30.0; // Large patience hit
                        info!(
                            "Driver {} arrived at broken charger {} ({:?})",
                            driver.id, charger.id, fault_type
                        );

                        // FAULT DISCOVERY: Driver discovers the fault!
                        if !charger.fault_discovered {
                            charger.fault_discovered = true;
                            fault_events.write(ChargerFaultEvent {
                                charger_entity,
                                charger_id: charger.id.clone(),
                                fault_type,
                                auto_remediated: false,
                            });
                            info!(
                                "  Driver discovered fault on {}: {:?}",
                                charger.id, fault_type
                            );
                        }
                    } else {
                        // Charger is busy — find an alternative with an unoccupied bay (Rule 3)
                        let compatible_types = driver.vehicle_type.compatible_charger_types();
                        let candidates = collect_charger_candidates(&chargers);
                        let alt = find_best_alternative_charger(
                            &candidates,
                            belongs.site_id,
                            compatible_types,
                            Some(charger_entity),
                            &tech_state,
                            &occupied_bays,
                            &site_state.grid,
                        );

                        let found_alternative = if let Some((alt_entity, alt_bay)) = alt
                            && let Ok((_, alt_charger, _)) = chargers.get(alt_entity)
                            && alt_charger.can_accept_driver()
                        {
                            driver.assigned_charger = Some(alt_entity);
                            driver.assigned_bay = Some(alt_bay);
                            driver.state = DriverState::Arriving;
                            driver.just_switched_charger = true;
                            movement.phase = MovementPhase::Arriving;
                            commands
                                .entity(driver_entity)
                                .try_insert(Pathfind::new_2d(alt_bay.0 as u32, alt_bay.1 as u32));
                            info!(
                                "Driver {} moving to alternative bay ({},{}) for charger {} (original was busy)",
                                driver.id, alt_bay.0, alt_bay.1, alt_charger.id
                            );
                            true
                        } else {
                            false
                        };

                        if !found_alternative {
                            driver.state = DriverState::Queued;
                            for charger_type in compatible_types {
                                match charger_type {
                                    ChargerType::DcFast => {
                                        site_state.charger_queue.join_dcfc_queue(driver_entity);
                                    }
                                    ChargerType::AcLevel2 => {
                                        site_state.charger_queue.join_l2_queue(driver_entity);
                                    }
                                }
                            }
                            info!("Driver {} joined queue (no free chargers)", driver.id);
                        }
                    }
                }
            } else {
                // No charger assigned, join queues for all compatible types
                driver.state = DriverState::Queued;
                for charger_type in driver.vehicle_type.compatible_charger_types() {
                    match charger_type {
                        ChargerType::DcFast => {
                            site_state.charger_queue.join_dcfc_queue(driver_entity);
                        }
                        ChargerType::AcLevel2 => {
                            site_state.charger_queue.join_l2_queue(driver_entity);
                        }
                    }
                }
                info!("Driver {} joined queue (no charger assigned)", driver.id);
            }
        }
    }
}

/// System to assign queued drivers to available chargers.
///
/// Handles two cases:
/// - Driver at a bay (existing): start charging immediately.
/// - Driver at a waiting tile (new): resolve bay from charger, pathfind to bay.
pub fn queue_assignment_system(
    mut commands: Commands,
    mut drivers: Query<(
        Entity,
        &mut Driver,
        &mut VehicleMovement,
        &crate::components::BelongsToSite,
    )>,
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    game_clock: Res<GameClock>,
    tech_state: Res<TechnicianState>,
) {
    if game_clock.day_ending {
        return;
    }

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };
    let site_id = &viewed_id;

    // Pre-collect occupied bays from all active drivers at this site.
    let mut occupied_bays: Vec<(i32, i32)> = drivers
        .iter()
        .filter(|(_, _, _, b)| b.site_id == *site_id)
        .filter(|(_, d, _, _)| {
            !matches!(
                d.state,
                DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
            )
        })
        .filter_map(|(_, d, _, _)| d.assigned_bay)
        .collect();

    for (charger_entity, mut charger, charger_belongs) in &mut chargers {
        if charger_belongs.site_id != *site_id {
            continue;
        }

        if charger.state() != ChargerState::Available || charger.is_disabled {
            continue;
        }

        if is_technician_active_on_charger(&tech_state, charger_entity) {
            continue;
        }

        let charger_type = charger.charger_type;
        let queue_driver = match charger_type {
            ChargerType::DcFast => site_state.charger_queue.peek_dcfc().and_then(|e| {
                if let Ok((_, driver, _, _)) = drivers.get(e) {
                    if driver.vehicle_type.is_compatible_with(charger_type) {
                        Some(e)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }),
            ChargerType::AcLevel2 => site_state.charger_queue.peek_l2().and_then(|e| {
                if let Ok((_, driver, _, _)) = drivers.get(e) {
                    if driver.vehicle_type.is_compatible_with(charger_type) {
                        Some(e)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }),
        };

        let Some(driver_entity) = queue_driver else {
            continue;
        };
        let Ok((_, mut driver, mut movement, _)) = drivers.get_mut(driver_entity) else {
            continue;
        };
        if driver.state != DriverState::Queued {
            continue;
        }

        if driver.waiting_tile.is_some() {
            // Driver is at a waiting tile -- find the bay linked to this charger
            let bay_pos = charger
                .grid_position
                .and_then(|(px, py)| site_state.grid.get_tile(px, py)?.linked_parking_bay);

            let target_bay = bay_pos.filter(|b| !occupied_bays.contains(b));

            if let Some((bx, by)) = target_bay {
                driver.assigned_bay = Some((bx, by));
                driver.waiting_tile = None;
                driver.assigned_charger = Some(charger_entity);
                driver.state = DriverState::Arriving;
                movement.phase = MovementPhase::Arriving;

                commands
                    .entity(driver_entity)
                    .try_insert(Pathfind::new_2d(bx as u32, by as u32));

                site_state.charger_queue.leave_all_queues(driver_entity);
                occupied_bays.push((bx, by));

                info!(
                    "Queued driver {} moving from waiting tile to bay ({}, {}) for charger {}",
                    driver.id, bx, by, charger.id
                );
            }
            // If bay is occupied, skip -- driver stays queued, will retry next frame
        } else {
            // Driver is already at a bay -- only charge if this is the bay's linked charger
            let charger_bay = charger
                .grid_position
                .and_then(|(px, py)| site_state.grid.get_tile(px, py)?.linked_parking_bay);

            if charger_bay != driver.assigned_bay {
                continue;
            }

            driver.assigned_charger = Some(charger_entity);
            driver.state = DriverState::Charging;
            charger.is_charging = true;
            charger.session_start_game_time = Some(game_clock.game_time);
            charger.requested_power_kw = charger.get_derated_power();

            site_state.charger_queue.leave_all_queues(driver_entity);

            info!(
                "Assigned queued driver {} to charger {}",
                driver.id, charger.id
            );
        }
    }
}

/// Handle charging sessions - deliver energy and complete
pub fn charging_system(
    mut commands: Commands,
    mut drivers: Query<(Entity, &mut Driver, &crate::components::BelongsToSite)>,
    mut chargers: Query<(&mut Charger, &GlobalTransform)>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    mut complete_events: MessageWriter<ChargingCompleteEvent>,
    mut left_events: MessageWriter<DriverLeftEvent>,
    mut fault_events: MessageWriter<ChargerFaultEvent>,
    image_assets: Res<crate::resources::ImageAssets>,
    images: Res<Assets<Image>>,
    tech_state: Res<TechnicianState>,
    tutorial_state: Option<Res<TutorialState>>,
) {
    if game_clock.is_paused() {
        return;
    }

    // Stop delivering energy during end-of-day wind-down.
    // Sessions are terminated with partial revenue by day_ending_system.
    if game_clock.day_ending {
        return;
    }

    // Suppress connector jams during the FixCharger tutorial step
    let tutorial_fix_active = tutorial_state
        .as_ref()
        .is_some_and(|ts| ts.current_step == Some(TutorialStep::FixCharger));

    let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();

    for (driver_entity, mut driver, belongs) in &mut drivers {
        if driver.state != DriverState::Charging {
            continue;
        }

        let Some(charger_entity) = driver.assigned_charger else {
            continue;
        };

        let Ok((mut charger, global_transform)) = chargers.get_mut(charger_entity) else {
            continue;
        };

        // Check if technician just arrived at this charger
        if is_technician_active_on_charger(&tech_state, charger_entity) {
            // Mood will be set by emotion system based on state
            driver.state = DriverState::Frustrated;
            driver.patience -= 20.0;
            info!(
                "Driver {} interrupted - technician arrived at charger {}",
                driver.id, charger.id
            );
            continue;
        }

        // ROBUST CHECK: Use centralized operational guard instead of state matching.
        // If the charger hardware cannot deliver power (disabled or faulted),
        // the session must pause immediately.
        if !charger.can_deliver_power() {
            // Charger faulted or disabled during session - session pauses
            driver.state = DriverState::WaitingForCharger;
            // The watchdog system will clear charger.is_charging,
            // but we also ensure no energy flows this frame
            continue;
        }

        // Charger is functional - session can proceed.
        // Note: We do NOT unconditionally set is_charging = true here.
        // The is_charging flag should only be set when a session truly starts
        // (in driver_arrival_system or queue_assignment_system).

        // Update requested power (what the session wants based on charger rating/health)
        // This was initially set when charging started; update it in case health changed
        charger.requested_power_kw = charger.get_derated_power();

        // Use allocated power from dispatch system (respects site constraints)
        // Dispatch runs before this system, so allocated_power_kw is always valid.
        // If allocation is 0, the grid is at capacity and this charger must wait.
        let power_kw = charger.allocated_power_kw;
        let energy_kwh = power_kw * (delta_game_seconds / 3600.0);

        charger.current_power_kw = power_kw;
        driver.charge_received_kwh += energy_kwh;

        // Track zero-power duration (Rule 2: driver sees 0 kW on dashboard).
        if power_kw > 0.0 {
            driver.zero_power_seconds = 0.0;
        } else {
            driver.zero_power_seconds += delta_game_seconds;

            if driver.zero_power_seconds >= ZERO_POWER_TOLERANCE_GAME_SECONDS {
                // Driver has been getting nothing for too long — leave angry.
                // The frustrated_driver_system handles alternative-charger search
                // each frame for WaitingForCharger/Frustrated drivers, so here we
                // just transition to LeftAngry since the charger IS operational
                // but the grid simply cannot allocate power to it.
                charger.is_charging = false;
                charger.current_power_kw = 0.0;
                charger.requested_power_kw = 0.0;
                charger.allocated_power_kw = 0.0;
                charger.session_start_game_time = None;

                driver.state = DriverState::LeftAngry;
                driver.waiting_tile = None;

                game_state.record_reputation(ReputationSource::AngryDriver(-3));
                game_state.sessions_failed += 1;
                game_state.daily_history.current_day.sessions_failed_today += 1;

                info!(
                    "Driver {} left angry - received 0 kW for {:.0}s",
                    driver.id, driver.zero_power_seconds
                );

                left_events.write(DriverLeftEvent {
                    driver_entity,
                    driver_id: driver.id.clone(),
                    angry: true,
                    revenue: 0.0,
                });
                continue;
            }
        }

        // Accumulate video ad revenue if enabled (only while actively charging with power > 0).
        // Revenue is flushed to the ledger at session completion, not per-frame.
        if charger.video_ad_enabled && power_kw > 0.0 {
            let ad_rate_per_second = multi_site
                .get_site(belongs.site_id)
                .map(|s| s.service_strategy.ad_space_price_per_hour / 3600.0)
                .unwrap_or(0.0);
            let ad_revenue_this_frame = ad_rate_per_second * delta_game_seconds;
            charger.total_ad_revenue += ad_revenue_this_frame;
            charger.pending_ad_revenue += ad_revenue_this_frame;

            if let Some(site_state) = multi_site.get_site_mut(belongs.site_id) {
                site_state.total_revenue += ad_revenue_this_frame;
            }
        }

        // Check if charging complete
        if driver.is_charging_complete() {
            // Get site state for pricing and upgrades
            let Some(site_state) = multi_site.get_site(belongs.site_id) else {
                continue;
            };

            // Check for connector jam BEFORE completing the session
            let rf_jam = site_state.rf_environment.jam_multiplier;
            let jammed = check_connector_jam(
                &mut charger,
                game_clock.total_game_time,
                tutorial_fix_active,
                rf_jam,
            );

            if jammed {
                info!(
                    "Connector jammed on {} during {} session - customer unable to disconnect!",
                    charger.id, driver.id
                );

                // End charging session immediately (no successful completion)
                charger.is_charging = false;
                charger.current_power_kw = 0.0;
                charger.requested_power_kw = 0.0;
                charger.allocated_power_kw = 0.0;
                charger.session_start_game_time = None;

                // Driver leaves frustrated - couldn't disconnect, no payment
                driver.state = DriverState::LeftAngry;

                game_state.record_reputation(ReputationSource::AngryDriver(-5));
                game_state.sessions_failed += 1;
                game_state.daily_history.current_day.sessions_failed_today += 1;

                info!(
                    "Driver {} left angry - connector jammed, no payment received",
                    driver.id
                );

                // Emit driver left event (angry, no revenue)
                left_events.write(DriverLeftEvent {
                    driver_entity,
                    driver_id: driver.id.clone(),
                    angry: true,
                    revenue: 0.0,
                });

                // Check if O&M Software is active - if so, emit fault event immediately
                let has_om_software = site_state.site_upgrades.has_om_software();
                if has_om_software {
                    // O&M upgrade: immediate notification
                    charger.fault_discovered = true;
                    let fault_type = charger.current_fault.unwrap(); // Safe because jam just set it
                    fault_events.write(ChargerFaultEvent {
                        charger_entity,
                        charger_id: charger.id.clone(),
                        fault_type,
                        auto_remediated: false,
                    });
                    info!(
                        "  O&M Software detected connector jam immediately on {}",
                        charger.id
                    );
                } else {
                    info!(
                        "  Connector jam on {} will be discovered when next driver arrives",
                        charger.id
                    );
                }

                continue; // Skip the successful completion logic below
            }

            // No jam - proceed with successful session completion
            let price_per_kwh = site_state.service_strategy.pricing.effective_price(
                game_clock.game_time,
                &site_state.site_energy_config,
                site_state.charger_utilization,
            );
            let revenue = driver.charge_received_kwh * price_per_kwh;

            // Check for connector jam (post-session)
            let rf_jam_post = site_state.rf_environment.jam_multiplier;
            let jammed = check_connector_jam(
                &mut charger,
                game_clock.total_game_time,
                tutorial_fix_active,
                rf_jam_post,
            );

            if jammed {
                info!(
                    "Connector jammed on {} after {} session",
                    charger.id, driver.id
                );
            }

            // Complete the session (mood will be set by emotion system)
            driver.state = DriverState::Complete;

            // End charging session (state computed from is_charging and current_fault)
            charger.is_charging = false;
            charger.current_power_kw = 0.0;
            charger.requested_power_kw = 0.0;
            charger.allocated_power_kw = 0.0;
            charger.session_start_game_time = None;

            // Recover reliability on successful session (OEM tier boosts recovery rate)
            let oem_recovery = multi_site
                .get_site(belongs.site_id)
                .map(|s| s.site_upgrades.oem_tier.reliability_recovery_multiplier())
                .unwrap_or(1.0);
            charger.recover_reliability_session(oem_recovery);

            // Update charger KPIs
            charger.total_energy_delivered_kwh += driver.charge_received_kwh;
            charger.energy_delivered_kwh_today += driver.charge_received_kwh;
            charger.session_count += 1;
            charger.total_revenue += revenue;

            // Add charging revenue to game state (global)
            game_state.add_charging_revenue(revenue);
            game_state.sessions_completed += 1;

            // Flush accumulated ad revenue for this session
            if charger.pending_ad_revenue > 0.0 {
                game_state.add_ad_revenue(charger.pending_ad_revenue);
                charger.pending_ad_revenue = 0.0;
            }

            game_state.record_reputation(ReputationSource::ChargingSession);

            // Achievement tracking: cumulative energy delivered
            game_state.total_energy_delivered_kwh += driver.charge_received_kwh;

            // Track per-site cumulative revenue and sessions
            if let Some(site_state) = multi_site.get_site_mut(belongs.site_id) {
                site_state.total_revenue += revenue;
                site_state.total_sessions += 1;
                site_state.sessions_today += 1;
                // Track energy delivered today for carbon credits
                site_state.energy_delivered_kwh_today += driver.charge_received_kwh;

                // Achievement tracking: fleet sessions without fault
                let is_commercial = matches!(
                    driver.vehicle_type,
                    VehicleType::Bus | VehicleType::Semi | VehicleType::Tractor
                );
                let is_fleet_site = site_state.archetype.is_fleet();
                if is_commercial && is_fleet_site {
                    game_state.fleet_sessions_without_fault += 1;
                }
            }

            // Spawn floating money VFX at charger's world position
            spawn_floating_money(
                &mut commands,
                &image_assets,
                &images,
                global_transform.translation(),
                revenue,
            );

            if !game_state.first_session_completed {
                game_state.first_session_completed = true;
            }

            info!(
                "Session complete: {} charged {:.1} kWh, revenue ${:.2}",
                driver.id, driver.charge_received_kwh, revenue
            );

            complete_events.write(ChargingCompleteEvent {
                driver_entity,
                charger_entity,
                energy_delivered: driver.charge_received_kwh,
                revenue,
            });

            // Driver leaves
            left_events.write(DriverLeftEvent {
                driver_entity,
                driver_id: driver.id.clone(),
                angry: false,
                revenue,
            });

            // Despawn driver after a short delay (handled by cleanup)
            driver.state = DriverState::Leaving;
        }
    }
}

/// Update driver patience while waiting or queued
/// Now includes satisfaction evaluation based on ServiceStrategy, EnvironmentState, and SiteUpgrades
pub fn patience_system(
    mut drivers: Query<(
        Entity,
        &mut Driver,
        &mut VehicleMovement,
        Option<&ChargingSession>,
        &crate::components::BelongsToSite,
    )>,
    mut chargers: Query<&mut Charger>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
    mut multi_site: ResMut<crate::resources::MultiSiteManager>,
    environment: Res<EnvironmentState>,
    mut left_events: MessageWriter<DriverLeftEvent>,
) {
    if game_clock.is_paused() {
        return;
    }

    // Don't deplete patience during end-of-day wind-down
    if game_clock.day_ending {
        return;
    }

    let delta_game_minutes = time.delta_secs() * game_clock.speed.multiplier() / 60.0;

    for (entity, mut driver, mut movement, session, belongs) in &mut drivers {
        // Get site-specific resources
        let Some(site_state) = multi_site.get_site_mut(belongs.site_id) else {
            continue;
        };

        // Calculate satisfaction multipliers for patience depletion
        let mut patience_multiplier = 1.0;

        // Apply strategy amenity multiplier
        patience_multiplier *= site_state.service_strategy.patience_multiplier();

        // Apply weather multiplier
        patience_multiplier *= environment.current_weather.patience_multiplier();

        // Marketing upgrade makes drivers more patient (lower depletion rate)
        // Demand multiplier > 1.0 means better reputation, drivers tolerate more
        patience_multiplier *= 1.0 / site_state.site_upgrades.demand_multiplier();

        // If charging, check power delivery satisfaction
        if let Some(session) = session {
            // Use get() for immutable access when just checking power ratio
            if let Ok(charger) = chargers.get(session.charger_entity) {
                // Check if allocated power is significantly less than requested
                let power_ratio = if charger.requested_power_kw > 0.0 {
                    charger.allocated_power_kw / charger.requested_power_kw
                } else {
                    1.0
                };

                // If getting less than 50% of requested power, patience depletes faster
                if power_ratio < 0.5 {
                    patience_multiplier *= 2.0; // Double depletion rate
                } else if power_ratio < 0.75 {
                    patience_multiplier *= 1.5; // 1.5x depletion rate
                }
            }
        }

        // Only deplete patience while waiting, queued, or charging (but throttled)
        // NOTE: Mood is updated by the emotion system (sync_mood_with_emotion), not here.
        // Calling update_mood() here would conflict with emotion-based mood updates.
        match driver.state {
            DriverState::WaitingForCharger | DriverState::Queued => {
                // Full depletion while waiting
                let depletion = driver.patience_level.depletion_rate()
                    * delta_game_minutes
                    * patience_multiplier;
                driver.patience = (driver.patience - depletion).max(0.0);
            }
            DriverState::Charging => {
                // Slower depletion while charging (only if power delivery is poor)
                if patience_multiplier > 1.2 {
                    let depletion = driver.patience_level.depletion_rate()
                        * delta_game_minutes
                        * (patience_multiplier - 1.0) // Only apply the excess
                        * 0.3; // Much slower depletion while actively charging
                    driver.patience = (driver.patience - depletion).max(0.0);
                }
            }
            _ => {}
        }

        // Check if patience depleted - only transition if in a state where patience matters
        // This prevents re-emitting events every frame after driver is already LeftAngry
        if driver.patience <= 0.0
            && matches!(
                driver.state,
                DriverState::WaitingForCharger | DriverState::Queued | DriverState::Charging
            )
        {
            driver.state = DriverState::LeftAngry;
            movement.phase = MovementPhase::DepartingAngry;
            movement.speed = 280.0;
            driver.waiting_tile = None;

            // Clear charger state if driver was assigned to one
            if let Some(charger_entity) = driver.assigned_charger
                && let Ok(mut charger) = chargers.get_mut(charger_entity)
            {
                charger.is_charging = false;
                charger.current_power_kw = 0.0;
                charger.requested_power_kw = 0.0;
                charger.allocated_power_kw = 0.0;
                charger.session_start_game_time = None;
                info!(
                    "Cleared charging state on charger {} (driver {} left angry)",
                    charger.id, driver.id
                );
            }

            // Remove from any queue they might be in
            site_state.charger_queue.leave_all_queues(entity);

            let rep_penalty =
                if environment.current_weather == crate::resources::WeatherType::Heatwave {
                    -5
                } else {
                    -3
                };
            game_state.record_reputation(ReputationSource::AngryDriver(rep_penalty));
            game_state.sessions_failed += 1;
            game_state.daily_history.current_day.sessions_failed_today += 1;

            info!("Driver {} left angry (patience depleted)", driver.id);

            left_events.write(DriverLeftEvent {
                driver_entity: entity,
                driver_id: driver.id.clone(),
                angry: true,
                revenue: 0.0,
            });

            // Don't despawn immediately - let departure animation play
            // Movement system will handle cleanup when animation completes
        }
    }

    // Don't cleanup leaving drivers here - movement system handles it
}

/// Handle frustrated drivers and drivers whose charger faulted mid-session.
///
/// Covers both `Frustrated` (arrived at broken charger) and `WaitingForCharger`
/// (charger faulted during an active session). In both cases the driver will:
///
/// 1. Check if their own charger was repaired → resume charging.
/// 2. Look for the best alternative charger at the site (Rule 3: visual observation).
/// 3. If no alternatives, drain patience and eventually leave angry.
pub fn frustrated_driver_system(
    mut commands: Commands,
    mut drivers: Query<(
        Entity,
        &mut Driver,
        &mut VehicleMovement,
        &crate::components::BelongsToSite,
    )>,
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
    mut left_events: MessageWriter<DriverLeftEvent>,
    tech_state: Res<TechnicianState>,
) {
    if game_clock.is_paused() {
        return;
    }

    // During end-of-day wind-down, frustrated drivers are kicked by day_ending_system
    if game_clock.day_ending {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();

    // Snapshot charger state for the alternative-charger search (avoids borrow conflicts).
    let candidates = collect_charger_candidates(&chargers);

    let occupied_bays: Vec<(i32, i32)> = drivers
        .iter()
        .filter(|(_, d, _, _)| {
            !matches!(
                d.state,
                DriverState::Leaving | DriverState::LeftAngry | DriverState::Complete
            )
        })
        .filter_map(|(_, d, _, _)| d.assigned_bay)
        .collect();

    for (driver_entity, mut driver, mut movement, belongs) in &mut drivers {
        let is_frustrated = driver.state == DriverState::Frustrated;
        let is_waiting_for_charger = driver.state == DriverState::WaitingForCharger;

        if !is_frustrated && !is_waiting_for_charger {
            continue;
        }

        // Drain patience (faster when frustrated, normal rate when waiting)
        if is_frustrated {
            driver.patience -= delta * 2.0;
        }
        // WaitingForCharger patience drain is handled by patience_system

        // 1) Check if their own charger was repaired
        if let Some(charger_entity) = driver.assigned_charger
            && let Ok((_, mut charger, _)) = chargers.get_mut(charger_entity)
            && charger.current_fault.is_none()
            && !charger.is_disabled
        {
            if charger.can_accept_driver() {
                driver.state = DriverState::Charging;
                driver.zero_power_seconds = 0.0;
                if is_frustrated {
                    driver.patience += 10.0;
                }
                charger.is_charging = true;
                charger.session_start_game_time = Some(game_clock.game_time);
                charger.requested_power_kw = charger.get_derated_power();
                info!(
                    "Driver {} resumed charging - charger {} repaired!",
                    driver.id, charger.id
                );
            }
            continue;
        }

        // 2) Look for an alternative charger whose bay is unoccupied (Rule 3)
        let compatible_types = driver.vehicle_type.compatible_charger_types();
        let grid = multi_site
            .owned_sites
            .get(&belongs.site_id)
            .map(|s| &s.grid);
        let found_alt = grid.and_then(|g| {
            find_best_alternative_charger(
                &candidates,
                belongs.site_id,
                compatible_types,
                driver.assigned_charger,
                &tech_state,
                &occupied_bays,
                g,
            )
        });
        let alt_accepted = found_alt.and_then(|(alt_entity, alt_bay)| {
            let (_, alt_charger, _) = chargers.get(alt_entity).ok()?;
            alt_charger
                .can_accept_driver()
                .then(|| (alt_entity, alt_bay, alt_charger.id.clone()))
        });
        if let Some((alt_entity, alt_bay, alt_id)) = alt_accepted {
            // Clear charging state on the old charger
            if let Some(old_entity) = driver.assigned_charger
                && let Ok((_, mut old_charger, _)) = chargers.get_mut(old_entity)
            {
                old_charger.is_charging = false;
                old_charger.current_power_kw = 0.0;
                old_charger.requested_power_kw = 0.0;
                old_charger.allocated_power_kw = 0.0;
                old_charger.session_start_game_time = None;
            }
            driver.assigned_charger = Some(alt_entity);
            driver.assigned_bay = Some(alt_bay);
            driver.state = DriverState::Arriving;
            driver.zero_power_seconds = 0.0;
            driver.just_switched_charger = true;
            if is_frustrated {
                driver.patience += 5.0;
            }
            movement.phase = MovementPhase::Arriving;
            commands
                .entity(driver_entity)
                .try_insert(Pathfind::new_2d(alt_bay.0 as u32, alt_bay.1 as u32));
            info!(
                "Driver {} moving to alternative bay ({},{}) for charger {} (original broken/faulted)",
                driver.id, alt_bay.0, alt_bay.1, alt_id
            );
            continue;
        }

        // 3) No alternatives — if patience depleted, leave angry
        if driver.patience <= 0.0 {
            driver.state = DriverState::LeftAngry;
            movement.phase = MovementPhase::DepartingAngry;
            movement.speed = 280.0;
            driver.waiting_tile = None;

            if let Some(charger_entity) = driver.assigned_charger
                && let Ok((_, mut charger, _)) = chargers.get_mut(charger_entity)
            {
                charger.is_charging = false;
                charger.current_power_kw = 0.0;
                charger.requested_power_kw = 0.0;
                charger.allocated_power_kw = 0.0;
                charger.session_start_game_time = None;
                info!(
                    "Cleared charging state on charger {} (frustrated driver {} left)",
                    charger.id, driver.id
                );
            }

            game_state.record_reputation(ReputationSource::AngryDriver(-5));
            game_state.sessions_failed += 1;
            game_state.daily_history.current_day.sessions_failed_today += 1;

            info!(
                "Driver {} left angry (state was {:?})",
                driver.id,
                if is_frustrated {
                    "Frustrated"
                } else {
                    "WaitingForCharger"
                }
            );

            left_events.write(DriverLeftEvent {
                driver_entity,
                driver_id: driver.id.clone(),
                angry: true,
                revenue: 0.0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::site_grid::{SiteGrid, TileContent};

    fn make_grid_with_bay_and_charger(
        grid: &mut SiteGrid,
        bay: (i32, i32),
        charger_pad: (i32, i32),
    ) {
        grid.set_tile_content(bay.0, bay.1, TileContent::ParkingBaySouth);
        if let Some(tile) = grid.get_tile_mut(bay.0, bay.1) {
            tile.linked_charger_pad = Some(charger_pad);
        }
        grid.set_tile_content(charger_pad.0, charger_pad.1, TileContent::ChargerPad);
        if let Some(tile) = grid.get_tile_mut(charger_pad.0, charger_pad.1) {
            tile.linked_parking_bay = Some(bay);
        }
    }

    fn candidate(
        entity_index: u32,
        site_id: crate::resources::SiteId,
        charger_type: ChargerType,
        can_accept: bool,
        power: f32,
        grid_pos: (i32, i32),
    ) -> ChargerCandidate {
        ChargerCandidate {
            entity: Entity::from_bits(entity_index as u64),
            site_id,
            charger_type,
            can_accept,
            rated_power_kw: power,
            grid_position: Some(grid_pos),
        }
    }

    #[test]
    fn alternative_charger_skips_occupied_bay() {
        let site = crate::resources::SiteId(1);
        let mut grid = SiteGrid::default();
        make_grid_with_bay_and_charger(&mut grid, (5, 10), (5, 11));
        make_grid_with_bay_and_charger(&mut grid, (8, 10), (8, 11));

        let candidates = vec![
            candidate(10, site, ChargerType::DcFast, true, 150.0, (5, 11)),
            candidate(20, site, ChargerType::DcFast, true, 150.0, (8, 11)),
        ];
        let tech = TechnicianState::default();

        let occupied = vec![(5, 10)];
        let result = find_best_alternative_charger(
            &candidates,
            site,
            &[ChargerType::DcFast],
            None,
            &tech,
            &occupied,
            &grid,
        );
        assert_eq!(result.map(|(e, _)| e), Some(Entity::from_bits(20)));

        let occupied_both = vec![(5, 10), (8, 10)];
        let result = find_best_alternative_charger(
            &candidates,
            site,
            &[ChargerType::DcFast],
            None,
            &tech,
            &occupied_both,
            &grid,
        );
        assert!(result.is_none());
    }

    #[test]
    fn alternative_charger_returns_bay_position() {
        let site = crate::resources::SiteId(1);
        let mut grid = SiteGrid::default();
        make_grid_with_bay_and_charger(&mut grid, (5, 10), (5, 11));

        let candidates = vec![candidate(
            10,
            site,
            ChargerType::DcFast,
            true,
            150.0,
            (5, 11),
        )];
        let tech = TechnicianState::default();

        let result = find_best_alternative_charger(
            &candidates,
            site,
            &[ChargerType::DcFast],
            None,
            &tech,
            &[],
            &grid,
        );
        assert_eq!(result, Some((Entity::from_bits(10), (5, 10))));
    }

    #[test]
    fn alternative_charger_prefers_higher_power() {
        let site = crate::resources::SiteId(1);
        let mut grid = SiteGrid::default();
        make_grid_with_bay_and_charger(&mut grid, (5, 10), (5, 11));
        make_grid_with_bay_and_charger(&mut grid, (8, 10), (8, 11));

        let candidates = vec![
            candidate(10, site, ChargerType::DcFast, true, 50.0, (5, 11)),
            candidate(20, site, ChargerType::DcFast, true, 150.0, (8, 11)),
        ];
        let tech = TechnicianState::default();

        let result = find_best_alternative_charger(
            &candidates,
            site,
            &[ChargerType::DcFast],
            None,
            &tech,
            &[],
            &grid,
        );
        assert_eq!(result.map(|(e, _)| e), Some(Entity::from_bits(20)));
    }

    #[test]
    fn alternative_charger_skips_cannot_accept() {
        let site = crate::resources::SiteId(1);
        let mut grid = SiteGrid::default();
        make_grid_with_bay_and_charger(&mut grid, (5, 10), (5, 11));

        let candidates = vec![candidate(
            10,
            site,
            ChargerType::DcFast,
            false,
            150.0,
            (5, 11),
        )];
        let tech = TechnicianState::default();

        let result = find_best_alternative_charger(
            &candidates,
            site,
            &[ChargerType::DcFast],
            None,
            &tech,
            &[],
            &grid,
        );
        assert!(result.is_none());
    }
}
