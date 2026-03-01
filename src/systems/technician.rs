//! Technician dispatch and repair systems
//!
//! Technicians now use bevy_northstar for pathfinding, similar to vehicles.

use bevy::prelude::*;
use bevy_northstar::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::Charger;
use crate::components::emotion::{TechnicianEmotion, TechnicianEmotionReason};
use crate::components::technician::{Technician, TechnicianMovement, TechnicianPhase};
use crate::events::{
    ChargerFaultEvent, ChargerFaultResolvedEvent, RepairCompleteEvent, RepairFailedEvent,
    TechnicianDispatchEvent,
};
use crate::resources::{
    BASE_TRAVEL_TIME, GameClock, GameState, MultiSiteManager, QueuedDispatch,
    SelectedChargerEntity, SiteGrid, TECHNICIAN_HOURLY_RATE, TechStatus, TechnicianState,
    calculate_travel_time,
};
use rand::Rng;

/// Get a random technician failure message for family-friendly frustration
fn get_failure_message() -> String {
    let messages = [
        "Don't have the right parts for this one...",
        "Diagnostics aren't giving me what I need here.",
        "Need more detailed schematics to fix this properly.",
        "This requires specialized equipment I don't have on the truck.",
    ];
    let mut rng = rand::rng();
    let index = rng.random_range(0..messages.len());
    messages[index].to_string()
}

fn resolve_technician_target_bay(grid: &SiteGrid, grid_pos: (i32, i32)) -> (i32, i32) {
    grid.get_tile(grid_pos.0, grid_pos.1)
        .and_then(|tile| tile.linked_parking_bay)
        .unwrap_or(grid_pos)
}

/// Computed values returned by `start_job` for use by the caller.
struct JobResult {
    charger_id: String,
    repair_duration: f32,
    dispatch_cost: f32,
}

/// Shared job-setup helper used by both `start_next_queued_job` and same-site chaining.
///
/// Handles everything that is identical regardless of whether the technician
/// is travelling to the site or is already standing on it:
/// - Repair duration (with perk multiplier)
/// - Dispatch / parts cost (cable theft vs. regular)
/// - Cost deduction and dispatch recording on `game_state`
/// - Core `TechnicianState` field updates
///
/// The caller is responsible for setting status (`EnRoute` vs `WalkingOnSite`)
/// and any travel-related fields.
fn start_job(
    tech_state: &mut TechnicianState,
    game_state: &mut GameState,
    profile: &crate::resources::PlayerProfile,
    dispatch: &QueuedDispatch,
    fault_type: crate::components::charger::FaultType,
    cable_replacement_cost: f32,
    warranty_tier: crate::resources::WarrantyTier,
) -> JobResult {
    let base_repair = fault_type.repair_duration_secs();
    let downtime_mult = match profile.active_perk() {
        Some(crate::resources::CharacterPerk::EfficiencyFreak {
            downtime_multiplier,
        }) => downtime_multiplier,
        _ => 1.0,
    };
    let repair_duration = base_repair * downtime_mult;

    let is_cable_theft = fault_type == crate::components::charger::FaultType::CableTheft;
    let original_cost = if is_cable_theft {
        cable_replacement_cost
    } else {
        fault_type.repair_cost()
    };
    let dispatch_cost = original_cost * warranty_tier.parts_cost_multiplier(fault_type);
    let covered = original_cost - dispatch_cost;

    if is_cable_theft {
        game_state.add_cable_theft_cost(original_cost);
    } else {
        game_state.add_opex(original_cost);
    }
    if covered > 0.0 {
        game_state.add_warranty_recovery(covered);
    }
    game_state.record_dispatch();

    tech_state.target_charger = Some(dispatch.charger_entity);
    tech_state.repair_remaining = repair_duration;
    tech_state.job_time_elapsed = 0.0;

    JobResult {
        charger_id: dispatch.charger_id.clone(),
        repair_duration,
        dispatch_cost,
    }
}

/// O&M Optimize auto-dispatch system - automatically dispatches technician when a fault occurs.
///
/// When O&M Optimize tier is active, this system listens for ChargerFaultEvent and
/// automatically dispatches a technician if the fault requires one.
pub fn om_auto_dispatch_system(
    mut fault_events: MessageReader<ChargerFaultEvent>,
    mut dispatch_events: MessageWriter<TechnicianDispatchEvent>,
    chargers: Query<&BelongsToSite>,
    multi_site: Res<MultiSiteManager>,
) {
    for event in fault_events.read() {
        // Check if charger still exists and get its site
        let Ok(belongs) = chargers.get(event.charger_entity) else {
            continue;
        };

        // Auto-dispatch requires O&M Optimize tier
        let has_auto_dispatch = multi_site
            .get_site(belongs.site_id)
            .map(|site| site.site_upgrades.oem_tier.has_auto_dispatch())
            .unwrap_or(false);

        if !has_auto_dispatch {
            continue;
        }

        // Only auto-dispatch for faults that require technicians
        if !event.fault_type.requires_technician() {
            continue;
        }

        // Automatically dispatch technician
        info!(
            "O&M Optimize auto-dispatching technician for {} (fault: {:?})",
            event.charger_id, event.fault_type
        );

        dispatch_events.write(TechnicianDispatchEvent {
            charger_entity: event.charger_entity,
            charger_id: event.charger_id.clone(),
        });
    }
}

/// Handle technician dispatch requests
///
/// This system queues all incoming dispatch requests, then starts the next job
/// if the technician is idle. This ensures dispatch requests are never dropped
/// even when the technician is busy.
pub fn dispatch_technician_system(
    mut dispatch_events: MessageReader<TechnicianDispatchEvent>,
    mut tech_state: ResMut<TechnicianState>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    multi_site: Res<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    profile: Res<crate::resources::PlayerProfile>,
) {
    // Step 1: Queue all incoming dispatch events (validates charger still needs repair)
    for event in dispatch_events.read() {
        // Validate charger exists and still needs a technician
        let Ok((charger, belongs)) = chargers.get(event.charger_entity) else {
            warn!(
                "Technician dispatch requested for non-existent charger {}",
                event.charger_id
            );
            continue;
        };

        // Check if charger has a fault that requires technician
        let Some(fault_type) = charger.current_fault else {
            warn!(
                "Technician dispatch requested for charger {} with no fault",
                event.charger_id
            );
            continue;
        };

        if !fault_type.requires_technician() {
            warn!(
                "Technician dispatch requested for charger {} with fault {:?} that doesn't require technician",
                event.charger_id, fault_type
            );
            continue;
        }

        // Queue the dispatch (skips duplicates)
        if tech_state.queue_dispatch(
            event.charger_entity,
            event.charger_id.clone(),
            belongs.site_id,
        ) {
            info!(
                "Queued technician dispatch for charger {}",
                event.charger_id
            );
        } else {
            info!(
                "Charger {} already queued or being serviced",
                event.charger_id
            );
        }
    }

    // Step 2: If technician is idle, start the next job from the queue
    start_next_queued_job(
        &mut tech_state,
        &chargers,
        &multi_site,
        &mut game_state,
        &profile,
    );
}

/// Helper function to start the next queued job if technician is idle.
/// Called from dispatch_technician_system and cleanup_exited_technicians.
pub fn start_next_queued_job(
    tech_state: &mut TechnicianState,
    chargers: &Query<(&Charger, &BelongsToSite)>,
    multi_site: &MultiSiteManager,
    game_state: &mut GameState,
    profile: &crate::resources::PlayerProfile,
) {
    if !tech_state.is_available() {
        return;
    }

    // Pop the next dispatch from the queue
    let Some(next_dispatch) = tech_state.pop_next_dispatch() else {
        return;
    };

    // Validate charger still exists and needs repair
    let Ok((charger, belongs)) = chargers.get(next_dispatch.charger_entity) else {
        warn!(
            "Queued charger {} no longer exists, skipping",
            next_dispatch.charger_id
        );
        // Try the next one in queue recursively
        start_next_queued_job(tech_state, chargers, multi_site, game_state, profile);
        return;
    };

    // Check if charger still has a fault requiring technician
    let Some(fault_type) = charger.current_fault else {
        info!(
            "Queued charger {} no longer has a fault, skipping",
            next_dispatch.charger_id
        );
        // Try the next one in queue recursively
        start_next_queued_job(tech_state, chargers, multi_site, game_state, profile);
        return;
    };

    if !fault_type.requires_technician() {
        info!(
            "Queued charger {} fault {:?} no longer requires technician, skipping",
            next_dispatch.charger_id, fault_type
        );
        // Try the next one in queue recursively
        start_next_queued_job(tech_state, chargers, multi_site, game_state, profile);
        return;
    }

    let destination_site_id = belongs.site_id;

    // Calculate travel time based on current location
    let travel_time = if let Some(current_site) = tech_state.current_site_id {
        let current_archetype = multi_site
            .get_site(current_site)
            .map(|s| s.archetype)
            .unwrap_or(crate::resources::SiteArchetype::ParkingLot);
        let dest_archetype = multi_site
            .get_site(destination_site_id)
            .map(|s| s.archetype)
            .unwrap_or(crate::resources::SiteArchetype::ParkingLot);

        if current_site == destination_site_id {
            0.0
        } else {
            calculate_travel_time(current_archetype, dest_archetype)
        }
    } else {
        BASE_TRAVEL_TIME
    };

    let warranty_tier = multi_site
        .get_site(destination_site_id)
        .map(|s| s.service_strategy.warranty_tier)
        .unwrap_or_default();

    let job = start_job(
        tech_state,
        game_state,
        profile,
        &next_dispatch,
        fault_type,
        charger.cable_replacement_cost(),
        warranty_tier,
    );

    tech_state.status = TechStatus::EnRoute;
    tech_state.destination_site_id = Some(destination_site_id);
    tech_state.travel_remaining = travel_time;
    tech_state.travel_total = travel_time;

    if travel_time > 0.0 {
        info!(
            "Technician dispatched to {} at site {:?} - Travel: {:.0}m, Cost: ${:.2}, Repair time: {:.0}s",
            job.charger_id,
            destination_site_id,
            travel_time / 60.0,
            job.dispatch_cost,
            job.repair_duration
        );
    } else {
        info!(
            "Technician already at site {:?}, responding to {} - Cost: ${:.2}, Repair time: {:.0}s",
            destination_site_id, job.charger_id, job.dispatch_cost, job.repair_duration
        );
    }
}

/// Update technician travel timer and spawn entity when arriving at site
pub fn technician_travel_system(
    mut commands: Commands,
    mut tech_state: ResMut<TechnicianState>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    multi_site: Res<MultiSiteManager>,
    chargers: Query<(&Charger, &Transform, &BelongsToSite)>,
) {
    if game_clock.is_paused() || tech_state.status != TechStatus::EnRoute {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();
    tech_state.job_time_elapsed += delta;
    tech_state.travel_remaining -= delta;

    if tech_state.travel_remaining <= 0.0 {
        // Arrived at site, spawn technician entity and start walking to charger
        tech_state.travel_remaining = 0.0;

        let Some(charger_entity) = tech_state.target_charger else {
            warn!("Technician arrived but no target charger set");
            tech_state.status = TechStatus::Idle;
            return;
        };

        let Ok((charger, charger_transform, belongs)) = chargers.get(charger_entity) else {
            warn!("Technician arrived but target charger not found");
            tech_state.status = TechStatus::Idle;
            tech_state.target_charger = None;
            return;
        };

        // Get the charger's grid position
        let charger_bay = charger.grid_position;

        // Get the site to determine entry point and pathfinding
        let Some(site_id) = tech_state.destination_site_id else {
            warn!("Technician arrived but no destination site set");
            tech_state.status = TechStatus::Idle;
            return;
        };

        let Some(site) = multi_site.get_site(site_id) else {
            warn!("Technician arrived but site not found");
            tech_state.status = TechStatus::Idle;
            return;
        };

        // Determine target position for pathfinding
        let target_bay = if let Some(bay_pos) = charger_bay {
            resolve_technician_target_bay(&site.grid, bay_pos)
        } else {
            // Fallback: convert charger world position to grid
            let charger_world = Vec2::new(
                charger_transform.translation.x - site.world_offset().x,
                charger_transform.translation.y - site.world_offset().y,
            );
            resolve_technician_target_bay(&site.grid, SiteGrid::world_to_grid(charger_world))
        };

        // Get starting position from entry
        let entry_pos = site.grid.entry_pos;
        let start_world = SiteGrid::grid_to_world(entry_pos.0, entry_pos.1);

        // Create movement component (bevy_northstar handles pathfinding)
        let movement = TechnicianMovement {
            phase: TechnicianPhase::WalkingToCharger,
            speed: 60.0, // Slow walking speed
        };

        // bevy_northstar components for pathfinding
        let agent_pos = AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0));
        let pathfind =
            Pathfind::new_2d(target_bay.0 as u32, target_bay.1 as u32).mode(PathfindMode::AStar);

        // Get site root entity for parenting
        if let Some(site_root_entity) = site.root_entity {
            // Spawn technician entity as child of site root with northstar components
            commands.entity(site_root_entity).with_children(|parent| {
                parent.spawn((
                    Technician {
                        target_charger: charger_entity,
                        phase: TechnicianPhase::WalkingToCharger,
                        work_timer: 0.0,
                        target_bay: Some(target_bay),
                    },
                    movement,
                    agent_pos,
                    pathfind,
                    Transform::from_xyz(start_world.x, start_world.y, 15.0),
                    GlobalTransform::default(),
                    *belongs,
                    TechnicianEmotion::new(
                        TechnicianEmotionReason::ArrivingAtSite,
                        game_clock.total_real_time,
                    ),
                ));
            });

            // Transition to WalkingOnSite to prevent spawning duplicate entities
            tech_state.status = TechStatus::WalkingOnSite;

            info!(
                "Technician arrived at site {:?}, pathfinding to charger {} at ({}, {})",
                site_id, charger.id, target_bay.0, target_bay.1
            );
        } else {
            warn!("Site root entity not found for technician spawn");
            tech_state.status = TechStatus::Idle;
        }
    }
}

/// Move technicians smoothly toward their NextPos (bevy_northstar pathfinding).
///
/// This is the technician equivalent of `northstar_move_vehicles`.
pub fn technician_movement_system(
    mut commands: Commands,
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut query: Query<(
        Entity,
        &mut AgentPos,
        &NextPos,
        &mut Transform,
        &TechnicianMovement,
    )>,
) {
    if game_clock.is_paused() {
        return;
    }

    let speed_multiplier = game_clock.speed.visual_multiplier();

    for (entity, mut agent_pos, next_pos, mut transform, movement) in query.iter_mut() {
        // Calculate world position from grid position
        let target_world = SiteGrid::grid_to_world(next_pos.0.x as i32, next_pos.0.y as i32);
        let current = transform.translation.truncate();

        // Use the technician's speed for smooth movement
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
        } else {
            // Move towards target
            let direction = (target_world - current).normalize();
            transform.translation.x += direction.x * step;
            transform.translation.y += direction.y * step;
        }
    }
}

/// Detect when technicians have reached their destination.
///
/// When a technician reaches their target (no more path), transition their phase.
pub fn technician_arrival_detection(
    mut commands: Commands,
    mut tech_state: ResMut<TechnicianState>,
    game_clock: Res<GameClock>,
    multi_site: Res<MultiSiteManager>,
    mut query: Query<
        (
            Entity,
            &AgentPos,
            &mut Technician,
            &mut TechnicianMovement,
            &mut TechnicianEmotion,
            &BelongsToSite,
        ),
        (Without<NextPos>, Without<Pathfind>, Without<Path>),
    >,
) {
    for (entity, agent_pos, mut technician, mut movement, mut emotion, belongs) in query.iter_mut()
    {
        match movement.phase {
            TechnicianPhase::WalkingToCharger => {
                // Check if we've reached the target bay
                if let Some((bay_x, bay_y)) = technician.target_bay {
                    let at_bay = agent_pos.0.x == bay_x as u32 && agent_pos.0.y == bay_y as u32;
                    if at_bay {
                        movement.phase = TechnicianPhase::Working;
                        technician.phase = TechnicianPhase::Working;
                        tech_state.status = TechStatus::Repairing;
                        emotion.set_reason(
                            TechnicianEmotionReason::StartingRepair,
                            game_clock.total_real_time,
                        );
                        info!("Technician arrived at charger, starting repair");
                    }
                }
            }
            TechnicianPhase::WalkingToExit => {
                // Check if we've reached the exit position for this site
                let exit_pos = multi_site
                    .get_site(belongs.site_id)
                    .map(|site| site.grid.exit_pos)
                    .unwrap_or((15, 11));

                let at_exit =
                    agent_pos.0.x == exit_pos.0 as u32 && agent_pos.0.y == exit_pos.1 as u32;

                if at_exit {
                    movement.phase = TechnicianPhase::Exited;
                    technician.phase = TechnicianPhase::Exited;
                    // Remove pathfinding components
                    commands.entity(entity).try_remove::<AgentPos>();
                    info!("Technician reached exit");
                }
            }
            _ => {}
        }
    }
}

/// Update technician repair timer and complete repairs
pub fn technician_repair_system(
    mut commands: Commands,
    mut tech_state: ResMut<TechnicianState>,
    mut chargers: Query<(&mut Charger, &GlobalTransform, &BelongsToSite)>,
    mut technicians: Query<(
        Entity,
        &mut Technician,
        &mut TechnicianMovement,
        &mut TechnicianEmotion,
        &BelongsToSite,
    )>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    multi_site: Res<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    profile: Res<crate::resources::PlayerProfile>,
    mut repair_events: MessageWriter<RepairCompleteEvent>,
    mut repair_failed_events: MessageWriter<RepairFailedEvent>,
    mut resolved_events: MessageWriter<ChargerFaultResolvedEvent>,
    mut dispatch_events: MessageWriter<TechnicianDispatchEvent>,
    image_assets: Res<crate::resources::ImageAssets>,
    images: Res<Assets<Image>>,
) {
    if game_clock.is_paused() || tech_state.status != TechStatus::Repairing {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();
    tech_state.job_time_elapsed += delta;
    tech_state.repair_remaining -= delta;

    if tech_state.repair_remaining <= 0.0 {
        // Repair attempt complete
        let Some(charger_entity) = tech_state.target_charger else {
            warn!("Technician finished repair but no target charger set");
            tech_state.status = TechStatus::Idle;
            return;
        };

        let Ok((mut charger, global_transform, belongs)) = chargers.get_mut(charger_entity) else {
            warn!(
                "Technician finished repair but target charger {:?} not found",
                charger_entity
            );
            tech_state.status = TechStatus::Idle;
            tech_state.target_charger = None;
            return;
        };

        // Calculate labor cost (travel + repair time at hourly rate)
        let labor_cost = tech_state.calculate_job_cost();

        // Capture fault info before potentially clearing
        let occurred_at = charger
            .fault_occurred_at
            .unwrap_or(game_clock.total_game_time);
        let resolved_at = game_clock.total_game_time;
        let downtime = resolved_at - occurred_at;
        let was_cable_theft = matches!(
            charger.current_fault,
            Some(crate::components::charger::FaultType::CableTheft)
        );
        let charger_id = charger.id.clone();

        // Labor cost is always paid (technician worked regardless of outcome)
        charger.total_repair_opex += labor_cost;
        if was_cable_theft {
            game_state.add_cable_theft_cost(labor_cost);
        } else {
            game_state.add_opex(labor_cost);
        }

        let failure_chance = multi_site
            .get_site(belongs.site_id)
            .map(|s| s.service_strategy.repair_failure_chance())
            .unwrap_or(0.20);

        let mut rng = rand::rng();
        let repair_failed = rng.random::<f32>() < failure_chance;

        if repair_failed {
            // Repair failed - charger stays broken (fault remains on charger)
            let failure_reason = get_failure_message();

            info!(
                "Repair FAILED on {} - Tech says: \"{}\" - Labor cost: ${:.2} ({:.1} hours @ ${}/hour)",
                charger_id,
                failure_reason,
                labor_cost,
                tech_state.job_time_elapsed / 3600.0,
                TECHNICIAN_HOURLY_RATE
            );

            // Send failure event
            repair_failed_events.write(RepairFailedEvent {
                charger_entity,
                charger_id: charger_id.clone(),
                repair_cost: labor_cost,
                failure_reason: failure_reason.clone(),
            });

            // Clear target_charger NOW (before writing dispatch event) so the re-dispatch
            // isn't rejected as a duplicate. The cleanup system will handle the rest.
            tech_state.target_charger = None;

            // If O&M Optimize tier is active, automatically re-dispatch
            let has_auto_dispatch = multi_site
                .get_site(belongs.site_id)
                .map(|site| site.site_upgrades.oem_tier.has_auto_dispatch())
                .unwrap_or(false);

            if has_auto_dispatch {
                info!(
                    "O&M Optimize auto-dispatching technician for retry on {}",
                    charger_id
                );
                dispatch_events.write(TechnicianDispatchEvent {
                    charger_entity,
                    charger_id,
                });
            }
        } else {
            // Repair succeeded — clear fault

            // Recover reliability based on how fast the fix was
            let oem_recovery = multi_site
                .get_site(belongs.site_id)
                .map(|s| s.site_upgrades.oem_tier.reliability_recovery_multiplier())
                .unwrap_or(1.0);
            charger.recover_reliability_fast_fix(downtime, oem_recovery);

            // Successful fault resolution earns a small reputation bonus
            game_state.change_reputation(1);

            // Clear the fault and timestamps (state() will compute as Available)
            charger.current_fault = None;
            charger.fault_discovered = false;
            charger.fault_occurred_at = None;
            charger.fault_detected_at = None;
            charger.fault_is_detected = true; // Reset for next fault

            // Reset operating hours (wear) after technician repair
            charger.operating_hours = 0.0;

            // Spawn burst of floating wrenches at charger's world position
            crate::systems::sprite::spawn_wrench_burst(
                &mut commands,
                &image_assets,
                &images,
                global_transform.translation(),
            );

            info!(
                "Repair SUCCESS on {} - Labor cost: ${:.2} ({:.1} hours @ ${}/hour), downtime: {:.0}s, reliability: {:.2}",
                charger_id,
                labor_cost,
                tech_state.job_time_elapsed / 3600.0,
                TECHNICIAN_HOURLY_RATE,
                downtime,
                charger.reliability
            );

            // Send success events
            repair_events.write(RepairCompleteEvent {
                charger_entity,
                charger_id: charger_id.clone(),
                repair_cost: labor_cost,
            });

            resolved_events.write(ChargerFaultResolvedEvent {
                charger_entity,
                charger_id,
            });
        }

        // Record current site location — job just completed here regardless of outcome.
        tech_state.current_site_id = tech_state.destination_site_id;
        tech_state.repair_remaining = 0.0;

        // Check whether another job is already queued at the same site.  If so,
        // redirect the technician directly to the next charger instead of routing
        // them to the exit and re-entering from scratch.
        let same_site_idx = tech_state
            .destination_site_id
            .and_then(|site_id| tech_state.find_same_site_dispatch_index(site_id));

        // Validate that the found dispatch still needs a technician before committing.
        let next_same_site = same_site_idx.and_then(|idx| {
            let queued = &tech_state.dispatch_queue[idx];
            let Ok((next_charger, _, _)) = chargers.get(queued.charger_entity) else {
                return None;
            };
            let fault = next_charger.current_fault?;
            if !fault.requires_technician() {
                return None;
            }
            Some((idx, fault))
        });

        if let Some((idx, next_fault_type)) = next_same_site {
            // --- Same-site job chaining ---
            let next_dispatch = tech_state.dispatch_queue.remove(idx);

            // Resolve bay position and cable replacement cost in one lookup.
            let (next_target_bay, cable_cost) =
                if let Ok((charger, _, belongs)) = chargers.get(next_dispatch.charger_entity) {
                    if let (Some(site), Some(pos)) =
                        (multi_site.get_site(belongs.site_id), charger.grid_position)
                    {
                        (
                            Some(resolve_technician_target_bay(&site.grid, pos)),
                            charger.cable_replacement_cost(),
                        )
                    } else {
                        (None, next_fault_type.repair_cost())
                    }
                } else {
                    (None, next_fault_type.repair_cost())
                };

            let chain_warranty_tier = tech_state
                .destination_site_id
                .and_then(|sid| multi_site.get_site(sid))
                .map(|s| s.service_strategy.warranty_tier)
                .unwrap_or_default();

            let job = start_job(
                &mut tech_state,
                &mut game_state,
                &profile,
                &next_dispatch,
                next_fault_type,
                cable_cost,
                chain_warranty_tier,
            );
            tech_state.status = TechStatus::WalkingOnSite;

            info!(
                "Technician chaining to next job at same site: {} (repair: {:.0}s, cost: ${:.2})",
                job.charger_id, job.repair_duration, job.dispatch_cost
            );

            // Redirect the on-site technician entity to the new charger.
            for (tech_entity, mut technician, mut movement, mut emotion, tech_belongs) in
                technicians.iter_mut()
            {
                if technician.target_charger == charger_entity {
                    if let Some(bay) = next_target_bay {
                        technician.target_charger = next_dispatch.charger_entity;
                        technician.target_bay = Some(bay);
                        technician.phase = TechnicianPhase::WalkingToCharger;
                        movement.phase = TechnicianPhase::WalkingToCharger;
                        emotion.set_reason(
                            TechnicianEmotionReason::NextJob,
                            game_clock.total_real_time,
                        );
                        commands.entity(tech_entity).try_insert(
                            Pathfind::new_2d(bay.0 as u32, bay.1 as u32).mode(PathfindMode::AStar),
                        );
                        info!(
                            "Technician pathfinding to next charger at ({}, {})",
                            bay.0, bay.1
                        );
                    } else {
                        warn!(
                            "Next charger {} has no grid position, falling back to exit",
                            next_dispatch.charger_id
                        );
                        // Fall back to exit if we can't determine the bay.
                        let exit_pos = multi_site
                            .get_site(tech_belongs.site_id)
                            .map(|site| site.grid.exit_pos)
                            .unwrap_or((15, 11));
                        movement.phase = TechnicianPhase::WalkingToExit;
                        technician.phase = TechnicianPhase::WalkingToExit;
                        tech_state.status = TechStatus::LeavingSite;
                        commands.entity(tech_entity).try_insert(
                            Pathfind::new_2d(exit_pos.0 as u32, exit_pos.1 as u32)
                                .mode(PathfindMode::AStar),
                        );
                        emotion.set_reason(
                            TechnicianEmotionReason::RepairComplete,
                            game_clock.total_real_time,
                        );
                    }
                    break;
                }
            }
        } else {
            // --- Normal exit routing ---
            tech_state.status = TechStatus::LeavingSite;

            for (tech_entity, mut technician, mut movement, mut emotion, belongs) in
                technicians.iter_mut()
            {
                if technician.target_charger == charger_entity {
                    let exit_pos = multi_site
                        .get_site(belongs.site_id)
                        .map(|site| site.grid.exit_pos)
                        .unwrap_or((15, 11));

                    movement.phase = TechnicianPhase::WalkingToExit;
                    technician.phase = TechnicianPhase::WalkingToExit;

                    commands.entity(tech_entity).try_insert(
                        Pathfind::new_2d(exit_pos.0 as u32, exit_pos.1 as u32)
                            .mode(PathfindMode::AStar),
                    );

                    let emotion_reason = if repair_failed {
                        TechnicianEmotionReason::RepairFailed
                    } else {
                        TechnicianEmotionReason::RepairComplete
                    };
                    emotion.set_reason(emotion_reason, game_clock.total_real_time);

                    info!(
                        "Technician pathfinding to exit at ({}, {})",
                        exit_pos.0, exit_pos.1
                    );
                    break;
                }
            }
        }
    }
}

/// Cleanup technician entities that have exited and reset state.
/// Also starts the next queued job if any are waiting.
pub fn cleanup_exited_technicians(
    mut commands: Commands,
    mut tech_state: ResMut<TechnicianState>,
    technicians: Query<(Entity, &Technician, &TechnicianMovement)>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    multi_site: Res<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    profile: Res<crate::resources::PlayerProfile>,
) {
    for (entity, _technician, movement) in technicians.iter() {
        if movement.phase == TechnicianPhase::Exited {
            // Despawn technician entity (despawn automatically handles children in Bevy 0.15+)
            commands.entity(entity).try_despawn();

            // Only reset technician state if we're in LeavingSite (prevents multiple resets)
            if tech_state.status == TechStatus::LeavingSite {
                tech_state.destination_site_id = None;
                tech_state.status = TechStatus::Idle;
                tech_state.target_charger = None;
                tech_state.travel_remaining = 0.0;
                tech_state.travel_total = 0.0;
                tech_state.job_time_elapsed = 0.0;

                info!("Technician entity cleaned up, now idle");

                // Start next queued job if any
                start_next_queued_job(
                    &mut tech_state,
                    &chargers,
                    &multi_site,
                    &mut game_state,
                    &profile,
                );
            }
        }
    }
}

// ─────────────────────────────────────────────────────
//  Stale-reference cleanup when a charger is sold
// ─────────────────────────────────────────────────────

/// Clean up stale references when a charger entity is despawned (e.g. sold).
///
/// Runs every frame and checks whether referenced charger entities still exist.
/// Clears:
/// - `SelectedChargerEntity` UI selection
/// - `TechnicianState` active job and dispatch queue entries
/// - Active `Technician` entities whose target charger is gone
pub fn cleanup_sold_charger_references(
    mut commands: Commands,
    mut selected: ResMut<SelectedChargerEntity>,
    mut tech_state: ResMut<TechnicianState>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    technicians: Query<(Entity, &Technician)>,
    multi_site: Res<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    profile: Res<crate::resources::PlayerProfile>,
) {
    // 1. Clear UI selection if the charger entity no longer exists
    if let Some(entity) = selected.0
        && chargers.get(entity).is_err()
    {
        selected.0 = None;
        info!("Cleared stale charger selection (charger sold or removed)");
    }

    // 2. Remove stale entries from the technician dispatch queue
    let before_len = tech_state.dispatch_queue.len();
    tech_state
        .dispatch_queue
        .retain(|queued| chargers.get(queued.charger_entity).is_ok());
    let purged = before_len - tech_state.dispatch_queue.len();
    if purged > 0 {
        info!(
            "Purged {} stale entries from technician dispatch queue",
            purged
        );
    }

    // 3. Abort the active technician job if its target charger was removed
    if let Some(target) = tech_state.target_charger
        && chargers.get(target).is_err()
    {
        info!(
            "Target charger {:?} was removed, aborting technician job (status: {:?})",
            target, tech_state.status
        );

        // Despawn any on-site technician entities targeting this charger
        for (tech_entity, technician) in &technicians {
            if technician.target_charger == target {
                commands.entity(tech_entity).try_despawn();
            }
        }

        // Reset technician state to idle
        tech_state.status = TechStatus::Idle;
        tech_state.target_charger = None;
        tech_state.destination_site_id = None;
        tech_state.travel_remaining = 0.0;
        tech_state.travel_total = 0.0;
        tech_state.repair_remaining = 0.0;
        tech_state.job_time_elapsed = 0.0;

        // Try to start the next queued job (if any)
        start_next_queued_job(
            &mut tech_state,
            &chargers,
            &multi_site,
            &mut game_state,
            &profile,
        );
    }
}
