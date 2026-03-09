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
    SiteSoldEvent, TechnicianDispatchEvent,
};
use crate::resources::{
    BASE_TRAVEL_TIME, GameClock, GameState, MultiSiteManager, QueuedDispatch, RepairRequestId,
    RepairRequestRegistry, RepairRequestSource, RepairRequestStatus, RepairResolution,
    SelectedChargerEntity, SiteGrid, SiteId, TECHNICIAN_HOURLY_RATE, TechStatus, TechnicianState,
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

fn is_viewed_site_active_job(tech_state: &TechnicianState, multi_site: &MultiSiteManager) -> bool {
    tech_state.active_site_id().is_some()
        && tech_state.active_site_id() == multi_site.viewed_site_id
}

fn despawn_technician_avatar(
    commands: &mut Commands,
    technicians: &Query<(Entity, &Technician, &BelongsToSite)>,
) {
    for (entity, _, _) in technicians.iter() {
        commands.entity(entity).try_despawn();
    }
}

fn spawn_walking_avatar(
    commands: &mut Commands,
    tech_state: &TechnicianState,
    multi_site: &MultiSiteManager,
    chargers: &Query<(&Charger, &Transform, &BelongsToSite)>,
    game_clock: &GameClock,
    technicians: &Query<(Entity, &Technician, &BelongsToSite)>,
) {
    if !matches!(
        tech_state.status(),
        TechStatus::WaitingAtSite | TechStatus::WalkingOnSite | TechStatus::Repairing
    ) || !is_viewed_site_active_job(tech_state, multi_site)
    {
        return;
    }

    if !technicians.is_empty() {
        return;
    }

    let Some(charger_entity) = tech_state.active_charger() else {
        return;
    };
    let Some(site_id) = tech_state.destination_site_id() else {
        return;
    };
    let Some(site) = multi_site.get_site(site_id) else {
        return;
    };
    let Some(site_root_entity) = site.root_entity else {
        return;
    };
    let Ok((charger, charger_transform, belongs)) = chargers.get(charger_entity) else {
        return;
    };

    let target_bay = if let Some(bay_pos) = charger.grid_position {
        resolve_technician_target_bay(&site.grid, bay_pos)
    } else {
        let charger_world = Vec2::new(
            charger_transform.translation.x - site.world_offset().x,
            charger_transform.translation.y - site.world_offset().y,
        );
        resolve_technician_target_bay(&site.grid, SiteGrid::world_to_grid(charger_world))
    };
    let entry_pos = site.grid.entry_pos;
    let start_world = SiteGrid::grid_to_world(entry_pos.0, entry_pos.1);

    commands.entity(site_root_entity).with_children(|parent| {
        parent.spawn((
            Technician {
                target_charger: charger_entity,
                phase: TechnicianPhase::WalkingToCharger,
                work_timer: 0.0,
                target_bay: Some(target_bay),
            },
            TechnicianMovement {
                phase: TechnicianPhase::WalkingToCharger,
                speed: 60.0,
            },
            AgentPos(UVec3::new(entry_pos.0 as u32, entry_pos.1 as u32, 0)),
            Pathfind::new_2d(target_bay.0 as u32, target_bay.1 as u32).mode(PathfindMode::AStar),
            Transform::from_xyz(start_world.x, start_world.y, 15.0),
            GlobalTransform::default(),
            *belongs,
            TechnicianEmotion::new(
                TechnicianEmotionReason::ArrivingAtSite,
                game_clock.total_real_time,
            ),
        ));
    });
}

fn technician_is_visibly_ready_to_repair(
    tech_state: &TechnicianState,
    multi_site: &MultiSiteManager,
    technicians: &mut Query<(
        Entity,
        &mut Technician,
        &mut TechnicianMovement,
        &mut TechnicianEmotion,
        &BelongsToSite,
        Option<&AgentPos>,
    )>,
) -> bool {
    let Some(site_id) = tech_state.destination_site_id() else {
        return false;
    };
    if multi_site.viewed_site_id != Some(site_id) {
        return false;
    }
    let Some(charger_entity) = tech_state.active_charger() else {
        return false;
    };

    technicians.iter_mut().any(
        |(_entity, technician, movement, _emotion, belongs, agent_pos)| {
            if belongs.site_id != site_id
                || technician.target_charger != charger_entity
                || technician.phase != TechnicianPhase::Working
                || movement.phase != TechnicianPhase::Working
            {
                return false;
            }

            match (technician.target_bay, agent_pos) {
                (Some((bay_x, bay_y)), Some(agent_pos)) => {
                    agent_pos.0.x == bay_x as u32 && agent_pos.0.y == bay_y as u32
                }
                _ => true,
            }
        },
    )
}

enum ActiveJobValidation {
    Valid {
        request_id: RepairRequestId,
        charger_entity: Entity,
        site_id: SiteId,
    },
    Invalid {
        request_id: Option<RepairRequestId>,
        resolution: Option<RepairResolution>,
        reason: &'static str,
    },
}

fn validate_active_job(
    tech_state: &TechnicianState,
    chargers: &Query<(&Charger, &BelongsToSite)>,
    repair_requests: &RepairRequestRegistry,
) -> ActiveJobValidation {
    let Some(request_id) = tech_state.active_request_id() else {
        return ActiveJobValidation::Invalid {
            request_id: None,
            resolution: None,
            reason: "active technician job has no repair request",
        };
    };
    let Some(charger_entity) = tech_state.active_charger() else {
        return ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: None,
            reason: "active technician job has no charger target",
        };
    };
    let Some(site_id) = tech_state.destination_site_id() else {
        return ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: None,
            reason: "active technician job has no destination site",
        };
    };

    let Some(request) = repair_requests.get(request_id) else {
        return ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: None,
            reason: "active technician job lost its repair request",
        };
    };

    if !request.status.is_open() {
        return ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: None,
            reason: "active technician job references a terminal request",
        };
    }

    if request.charger_entity != charger_entity || request.site_id != site_id {
        return ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: None,
            reason: "active technician job no longer matches its repair request",
        };
    }

    let Ok((charger, belongs)) = chargers.get(charger_entity) else {
        return ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: Some(RepairResolution::Cancelled),
            reason: "active technician charger no longer exists",
        };
    };

    if belongs.site_id != site_id {
        return ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: None,
            reason: "active technician charger moved to a different site",
        };
    }

    match charger.current_fault {
        None => ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: Some(RepairResolution::Resolved),
            reason: "active technician charger fault already cleared",
        },
        Some(fault_type) if !fault_type.requires_technician() => ActiveJobValidation::Invalid {
            request_id: Some(request_id),
            resolution: Some(RepairResolution::Cancelled),
            reason: "active technician charger fault no longer needs a technician",
        },
        Some(_) => ActiveJobValidation::Valid {
            request_id,
            charger_entity,
            site_id,
        },
    }
}

fn resolve_skipped_dispatch(
    dispatch: &QueuedDispatch,
    repair_requests: &mut RepairRequestRegistry,
    game_clock: &GameClock,
    resolution: RepairResolution,
) {
    let _ = repair_requests.resolve(dispatch.request_id, game_clock.total_game_time, resolution);
}

fn is_recoverable_request_status(
    status: RepairRequestStatus,
    last_dispatch_at: Option<f32>,
) -> bool {
    matches!(status, RepairRequestStatus::NeedsRetry)
        || matches!(status, RepairRequestStatus::OpenDiscovered) && last_dispatch_at.is_some()
}

fn abort_invalid_active_job(
    tech_state: &mut TechnicianState,
    chargers: &Query<(&Charger, &BelongsToSite)>,
    multi_site: &MultiSiteManager,
    game_state: &mut GameState,
    profile: &crate::resources::PlayerProfile,
    repair_requests: &mut RepairRequestRegistry,
    game_clock: &GameClock,
    reason: &'static str,
    request_id: Option<RepairRequestId>,
    resolution: Option<RepairResolution>,
) {
    warn!("Aborting technician job: {reason}");
    if let (Some(request_id), Some(resolution)) = (request_id, resolution) {
        let _ = repair_requests.resolve(request_id, game_clock.total_game_time, resolution);
    }
    tech_state.set_idle();
    start_next_queued_job(
        tech_state,
        chargers,
        multi_site,
        game_state,
        profile,
        repair_requests,
        game_clock,
    );
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
    game_state: &mut GameState,
    profile: &crate::resources::PlayerProfile,
    dispatch: &QueuedDispatch,
    fault_type: crate::components::charger::FaultType,
    cable_replacement_cost: f32,
    warranty_tier: crate::resources::WarrantyTier,
    bill_dispatch_costs: bool,
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

    if bill_dispatch_costs {
        if is_cable_theft {
            game_state.add_cable_theft_cost(original_cost);
        } else {
            game_state.add_repair_parts(original_cost);
        }
        if covered > 0.0 {
            game_state.add_warranty_recovery(covered);
        }
        game_state.record_dispatch();
    }

    JobResult {
        charger_id: dispatch.charger_id.clone(),
        repair_duration,
        dispatch_cost,
    }
}

pub fn reconcile_repair_requests_system(
    chargers: Query<(Entity, &Charger, &BelongsToSite)>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
    game_clock: Res<GameClock>,
) {
    let mut live_chargers = std::collections::HashSet::new();

    for (charger_entity, charger, belongs) in &chargers {
        live_chargers.insert(charger_entity);

        let Some(fault_type) = charger.current_fault else {
            let _ = repair_requests.resolve_for_charger(
                charger_entity,
                game_clock.total_game_time,
                RepairResolution::Resolved,
            );
            continue;
        };

        if !fault_type.requires_technician() {
            let _ = repair_requests.resolve_for_charger(
                charger_entity,
                game_clock.total_game_time,
                RepairResolution::Cancelled,
            );
            continue;
        }

        let occurred_at = charger
            .fault_occurred_at
            .unwrap_or(game_clock.total_game_time);
        repair_requests.create_or_update_for_fault(
            charger_entity,
            charger.id.clone(),
            belongs.site_id,
            fault_type,
            occurred_at,
            RepairRequestSource::Reconciliation,
        );

        if charger.fault_discovered || charger.fault_is_detected {
            let source = if charger.fault_is_detected {
                RepairRequestSource::OemDetection
            } else {
                RepairRequestSource::DriverDiscovery
            };
            let discovered_at = charger
                .fault_detected_at
                .unwrap_or(game_clock.total_game_time);
            let _ = repair_requests.mark_discovered(charger_entity, discovered_at, source);
        }
    }

    let stale: Vec<RepairRequestId> = repair_requests
        .iter()
        .filter(|request| {
            request.status.is_open() && !live_chargers.contains(&request.charger_entity)
        })
        .map(|request| request.id)
        .collect();

    for request_id in stale {
        let _ = repair_requests.resolve(
            request_id,
            game_clock.total_game_time,
            RepairResolution::Cancelled,
        );
    }
}

/// O&M Optimize auto-dispatch system - automatically dispatches technician when a fault occurs.
///
/// When O&M Optimize tier is active, this system listens for ChargerFaultEvent and
/// automatically dispatches a technician if the fault requires one.
pub fn om_auto_dispatch_system(
    mut fault_events: MessageReader<ChargerFaultEvent>,
    mut dispatch_events: MessageWriter<TechnicianDispatchEvent>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    multi_site: Res<MultiSiteManager>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
) {
    for event in fault_events.read() {
        // Check if charger still exists and get its site
        let Ok((charger, belongs)) = chargers.get(event.charger_entity) else {
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

        let request_id = repair_requests
            .active_request_id_for_charger(event.charger_entity)
            .unwrap_or_else(|| {
                repair_requests.create_or_update_for_fault(
                    event.charger_entity,
                    event.charger_id.clone(),
                    belongs.site_id,
                    event.fault_type,
                    charger.fault_occurred_at.unwrap_or_default(),
                    RepairRequestSource::AutoDispatch,
                )
            });
        let _ = repair_requests.mark_discovered(
            event.charger_entity,
            charger.fault_detected_at.unwrap_or_default(),
            RepairRequestSource::OemDetection,
        );

        // Automatically dispatch technician
        info!(
            "O&M Optimize auto-dispatching technician for {} (fault: {:?})",
            event.charger_id, event.fault_type
        );

        dispatch_events.write(TechnicianDispatchEvent {
            charger_entity: event.charger_entity,
            charger_id: event.charger_id.clone(),
            request_id,
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
    mut repair_requests: ResMut<RepairRequestRegistry>,
    game_clock: Res<GameClock>,
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

        let _ = repair_requests.mark_discovered(
            event.charger_entity,
            charger
                .fault_detected_at
                .unwrap_or(game_clock.total_game_time),
            RepairRequestSource::ManualDispatch,
        );
        let request_queued = repair_requests.queue(
            event.request_id,
            game_clock.total_game_time,
            RepairRequestSource::ManualDispatch,
        );

        if !request_queued {
            warn!(
                "Technician dispatch ignored for {} because repair request {:?} is not queueable",
                event.charger_id, event.request_id
            );
            continue;
        }

        // Queue the dispatch (skips duplicates)
        if tech_state.queue_dispatch(
            event.request_id,
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
        &mut repair_requests,
        &game_clock,
    );
}

/// Rebuild transient dispatch queue entries from durable repair requests.
///
/// This keeps retryable work and day-end interrupted work recoverable even when
/// the on-screen technician entity or transient queue state has been cleared.
pub fn recover_dispatchable_requests_system(
    mut tech_state: ResMut<TechnicianState>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
    game_clock: Res<GameClock>,
) {
    let candidate_ids: Vec<RepairRequestId> = repair_requests
        .iter()
        .filter(|request| is_recoverable_request_status(request.status, request.last_dispatch_at))
        .map(|request| request.id)
        .collect();

    for request_id in candidate_ids {
        let Some(request) = repair_requests.get(request_id).cloned() else {
            continue;
        };

        let Ok((charger, belongs)) = chargers.get(request.charger_entity) else {
            continue;
        };

        let Some(fault_type) = charger.current_fault else {
            continue;
        };

        if !fault_type.requires_technician() || belongs.site_id != request.site_id {
            continue;
        }

        let source = if request.status == RepairRequestStatus::NeedsRetry {
            RepairRequestSource::Retry
        } else {
            request.source
        };

        if !repair_requests.queue(request_id, game_clock.total_game_time, source) {
            continue;
        }

        let _ = tech_state.queue_dispatch(
            request_id,
            request.charger_entity,
            request.charger_id.clone(),
            request.site_id,
        );
    }
}

/// Helper function to start the next queued job if technician is idle.
/// Called from dispatch_technician_system and cleanup_exited_technicians.
pub fn start_next_queued_job(
    tech_state: &mut TechnicianState,
    chargers: &Query<(&Charger, &BelongsToSite)>,
    multi_site: &MultiSiteManager,
    game_state: &mut GameState,
    profile: &crate::resources::PlayerProfile,
    repair_requests: &mut RepairRequestRegistry,
    game_clock: &GameClock,
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
        resolve_skipped_dispatch(
            &next_dispatch,
            repair_requests,
            game_clock,
            RepairResolution::Cancelled,
        );
        // Try the next one in queue recursively
        start_next_queued_job(
            tech_state,
            chargers,
            multi_site,
            game_state,
            profile,
            repair_requests,
            game_clock,
        );
        return;
    };

    // Check if charger still has a fault requiring technician
    let Some(fault_type) = charger.current_fault else {
        info!(
            "Queued charger {} no longer has a fault, skipping",
            next_dispatch.charger_id
        );
        resolve_skipped_dispatch(
            &next_dispatch,
            repair_requests,
            game_clock,
            RepairResolution::Resolved,
        );
        // Try the next one in queue recursively
        start_next_queued_job(
            tech_state,
            chargers,
            multi_site,
            game_state,
            profile,
            repair_requests,
            game_clock,
        );
        return;
    };

    if !fault_type.requires_technician() {
        info!(
            "Queued charger {} fault {:?} no longer requires technician, skipping",
            next_dispatch.charger_id, fault_type
        );
        resolve_skipped_dispatch(
            &next_dispatch,
            repair_requests,
            game_clock,
            RepairResolution::Cancelled,
        );
        // Try the next one in queue recursively
        start_next_queued_job(
            tech_state,
            chargers,
            multi_site,
            game_state,
            profile,
            repair_requests,
            game_clock,
        );
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
    let bill_dispatch_costs = !repair_requests.dispatch_costs_recorded(next_dispatch.request_id);

    if !repair_requests.set_status(next_dispatch.request_id, RepairRequestStatus::EnRoute) {
        warn!(
            "Queued charger {} no longer has an open repair request, skipping",
            next_dispatch.charger_id
        );
        start_next_queued_job(
            tech_state,
            chargers,
            multi_site,
            game_state,
            profile,
            repair_requests,
            game_clock,
        );
        return;
    }

    let job = start_job(
        game_state,
        profile,
        &next_dispatch,
        fault_type,
        charger.cable_replacement_cost(),
        warranty_tier,
        bill_dispatch_costs,
    );
    if bill_dispatch_costs {
        let _ = repair_requests.mark_dispatch_costs_recorded(next_dispatch.request_id);
    }
    tech_state.begin_en_route(
        next_dispatch.request_id,
        next_dispatch.charger_entity,
        destination_site_id,
        travel_time,
        job.repair_duration,
    );

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

fn recover_from_travel_abort(
    tech_state: &mut TechnicianState,
    chargers: &Query<(&Charger, &BelongsToSite)>,
    multi_site: &MultiSiteManager,
    game_state: &mut GameState,
    profile: &crate::resources::PlayerProfile,
    repair_requests: &mut RepairRequestRegistry,
    game_clock: &GameClock,
) {
    if let Some(request_id) = tech_state.active_request_id() {
        let _ = repair_requests.set_status(request_id, RepairRequestStatus::NeedsRetry);
    }
    tech_state.set_idle();

    start_next_queued_job(
        tech_state,
        chargers,
        multi_site,
        game_state,
        profile,
        repair_requests,
        game_clock,
    );
}

/// Update technician travel timer and spawn entity when arriving at site
pub fn technician_travel_system(
    mut commands: Commands,
    mut tech_state: ResMut<TechnicianState>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    multi_site: Res<MultiSiteManager>,
    chargers: Query<(&Charger, &Transform, &BelongsToSite)>,
    dispatch_chargers: Query<(&Charger, &BelongsToSite)>,
    mut game_state: ResMut<GameState>,
    profile: Res<crate::resources::PlayerProfile>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
) {
    if game_clock.is_paused() || tech_state.status() != TechStatus::EnRoute {
        return;
    }

    match validate_active_job(&tech_state, &dispatch_chargers, &repair_requests) {
        ActiveJobValidation::Valid { .. } => {}
        ActiveJobValidation::Invalid {
            request_id,
            resolution,
            reason,
        } => {
            abort_invalid_active_job(
                &mut tech_state,
                &dispatch_chargers,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
                reason,
                request_id,
                resolution,
            );
            return;
        }
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();
    tech_state.job_time_elapsed += delta;
    let travel_remaining = tech_state.tick_travel(delta).unwrap_or(0.0);

    if travel_remaining <= 0.0 {
        // Arrived at site, spawn technician entity and start walking to charger
        let Some(charger_entity) = tech_state.active_charger() else {
            warn!("Technician arrived but no target charger set");
            recover_from_travel_abort(
                &mut tech_state,
                &dispatch_chargers,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
            );
            return;
        };

        let Ok((charger, charger_transform, belongs)) = chargers.get(charger_entity) else {
            warn!("Technician arrived but target charger not found");
            recover_from_travel_abort(
                &mut tech_state,
                &dispatch_chargers,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
            );
            return;
        };

        // Get the charger's grid position
        let charger_bay = charger.grid_position;

        // Get the site to determine entry point and pathfinding
        let Some(site_id) = tech_state.destination_site_id() else {
            warn!("Technician arrived but no destination site set");
            recover_from_travel_abort(
                &mut tech_state,
                &dispatch_chargers,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
            );
            return;
        };

        let Some(site) = multi_site.get_site(site_id) else {
            warn!("Technician arrived but site not found");
            recover_from_travel_abort(
                &mut tech_state,
                &dispatch_chargers,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
            );
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

        if !is_viewed_site_active_job(&tech_state, &multi_site) {
            tech_state.complete_job_at(site_id);
            let _ = tech_state.begin_waiting_at_site();
            if let Some(request_id) = tech_state.active_request_id() {
                let _ = repair_requests.set_status(request_id, RepairRequestStatus::WaitingAtSite);
            }
            info!(
                "Technician arrived at offscreen site {:?}, waiting to visibly continue repair on {}",
                site_id, charger.id
            );
            return;
        }

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
            tech_state.begin_walking_on_site();
            if let Some(request_id) = tech_state.active_request_id() {
                let _ = repair_requests.set_status(request_id, RepairRequestStatus::WalkingOnSite);
            }

            info!(
                "Technician arrived at site {:?}, pathfinding to charger {} at ({}, {})",
                site_id, charger.id, target_bay.0, target_bay.1
            );
        } else {
            warn!("Site root entity not found for technician spawn");
            recover_from_travel_abort(
                &mut tech_state,
                &dispatch_chargers,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
            );
        }
    }
}

pub fn sync_viewed_technician_avatar_system(
    mut commands: Commands,
    mut tech_state: ResMut<TechnicianState>,
    game_clock: Res<GameClock>,
    multi_site: Res<MultiSiteManager>,
    chargers: Query<(&Charger, &Transform, &BelongsToSite)>,
    technicians: Query<(Entity, &Technician, &BelongsToSite)>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
) {
    if tech_state.status() == TechStatus::Idle || tech_state.status() == TechStatus::EnRoute {
        if !technicians.is_empty() {
            despawn_technician_avatar(&mut commands, &technicians);
        }
        return;
    }

    if !is_viewed_site_active_job(&tech_state, &multi_site) {
        if !technicians.is_empty() {
            despawn_technician_avatar(&mut commands, &technicians);
        }
        return;
    }

    if tech_state.status() == TechStatus::WaitingAtSite {
        let _ = tech_state.begin_walking_on_site();
        if let Some(request_id) = tech_state.active_request_id() {
            let _ = repair_requests.set_status(request_id, RepairRequestStatus::WalkingOnSite);
        }
    }

    spawn_walking_avatar(
        &mut commands,
        &tech_state,
        &multi_site,
        &chargers,
        &game_clock,
        &technicians,
    );
}

/// Move technicians smoothly toward their NextPos (bevy_northstar pathfinding).
///
/// This is the technician equivalent of `northstar_move_vehicles`.
pub fn technician_movement_system(
    mut commands: Commands,
    time: Res<Time>,
    game_clock: Res<GameClock>,
    build_state: Res<crate::resources::BuildState>,
    mut query: Query<(
        Entity,
        &mut AgentPos,
        &NextPos,
        &mut Transform,
        &TechnicianMovement,
    )>,
) {
    if game_clock.is_paused() || !build_state.is_open {
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
    build_state: Res<crate::resources::BuildState>,
    multi_site: Res<MultiSiteManager>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
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
    if !build_state.is_open {
        return;
    }

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
                        tech_state.begin_repairing();
                        if let Some(request_id) = tech_state.active_request_id() {
                            let _ = repair_requests
                                .set_status(request_id, RepairRequestStatus::Repairing);
                        }
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
    mut charger_queries: ParamSet<(
        Query<(&mut Charger, &GlobalTransform, &BelongsToSite)>,
        Query<(&Charger, &GlobalTransform, &BelongsToSite)>,
        Query<(&Charger, &BelongsToSite)>,
    )>,
    mut technicians: Query<(
        Entity,
        &mut Technician,
        &mut TechnicianMovement,
        &mut TechnicianEmotion,
        &BelongsToSite,
        Option<&AgentPos>,
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
    mut repair_requests: ResMut<RepairRequestRegistry>,
    image_assets: Res<crate::resources::ImageAssets>,
    images: Res<Assets<Image>>,
) {
    if game_clock.is_paused() || tech_state.status() != TechStatus::Repairing {
        return;
    }

    let charger_query = charger_queries.p2();
    match validate_active_job(&tech_state, &charger_query, &repair_requests) {
        ActiveJobValidation::Valid {
            request_id,
            charger_entity,
            site_id,
        } => {
            let _ = (request_id, charger_entity, site_id);
        }
        ActiveJobValidation::Invalid {
            request_id,
            resolution,
            reason,
        } => {
            abort_invalid_active_job(
                &mut tech_state,
                &charger_query,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
                reason,
                request_id,
                resolution,
            );
            return;
        }
    }

    if !technician_is_visibly_ready_to_repair(&tech_state, &multi_site, &mut technicians) {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();
    tech_state.job_time_elapsed += delta;
    let repair_remaining = tech_state.tick_repair(delta).unwrap_or(0.0);

    if repair_remaining <= 0.0 {
        // Repair attempt complete
        let Some(charger_entity) = tech_state.active_charger() else {
            warn!("Technician finished repair but no target charger set");
            tech_state.set_idle();
            return;
        };
        let active_request_id = tech_state.active_request_id();

        let mut charger_query = charger_queries.p0();
        let Ok((mut charger, global_transform, belongs)) = charger_query.get_mut(charger_entity)
        else {
            warn!(
                "Technician finished repair but target charger {:?} not found",
                charger_entity
            );
            if let Some(request_id) = active_request_id {
                let _ = repair_requests.resolve(
                    request_id,
                    game_clock.total_game_time,
                    RepairResolution::Cancelled,
                );
            }
            tech_state.set_idle();
            return;
        };
        let completed_site_id = tech_state.destination_site_id().unwrap_or(belongs.site_id);

        // Calculate labor cost (travel + repair time at hourly rate)
        let labor_cost = tech_state.calculate_job_cost();

        // Apply warranty labour coverage (Premium covers 80%)
        let warranty_tier = multi_site
            .get_site(belongs.site_id)
            .map(|s| s.service_strategy.warranty_tier)
            .unwrap_or_default();
        let labor_multiplier = warranty_tier.labor_cost_multiplier();
        let effective_labor = labor_cost * labor_multiplier;
        let labor_covered = labor_cost - effective_labor;

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

        charger.total_repair_opex += labor_cost;
        if was_cable_theft {
            game_state.add_cable_theft_cost(effective_labor);
        } else {
            game_state.add_repair_labor(effective_labor);
        }
        if labor_covered > 0.0 {
            game_state.add_warranty_recovery(labor_covered);
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
            if let Some(request_id) = active_request_id {
                let _ =
                    repair_requests.mark_retry_needed(request_id, Some(game_clock.total_game_time));
                tech_state.clear_active_request();
            }

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
                if let Some(request_id) = active_request_id {
                    dispatch_events.write(TechnicianDispatchEvent {
                        request_id,
                        charger_entity,
                        charger_id,
                    });
                }
            }
        } else {
            // Repair succeeded — clear fault

            // Recover reliability based on how fast the fix was
            let oem_recovery = multi_site
                .get_site(belongs.site_id)
                .map(|s| s.site_upgrades.oem_tier.reliability_recovery_multiplier())
                .unwrap_or(1.0);
            charger.recover_reliability_fast_fix(downtime, oem_recovery);

            game_state.record_reputation(crate::resources::ReputationSource::Repair);

            // Clear the fault and timestamps (state() will compute as Available)
            charger.current_fault = None;
            charger.fault_discovered = false;
            charger.reboot_attempts = 0;
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
            if let Some(request_id) = active_request_id {
                let _ = repair_requests.resolve(
                    request_id,
                    game_clock.total_game_time,
                    RepairResolution::Resolved,
                );
            }
        }

        // Record current site location — job just completed here regardless of outcome.
        tech_state.complete_job_at(completed_site_id);

        // Check whether another job is already queued at the same site.  If so,
        // redirect the technician directly to the next charger instead of routing
        // them to the exit and re-entering from scratch.
        let same_site_idx = tech_state.find_same_site_dispatch_index(completed_site_id);

        // Validate that the found dispatch still needs a technician before committing.
        let next_same_site = same_site_idx.and_then(|idx| {
            let queued = &tech_state.dispatch_queue[idx];
            let charger_query = charger_queries.p1();
            let Ok((next_charger, next_transform, belongs)) =
                charger_query.get(queued.charger_entity)
            else {
                return None;
            };
            let fault = next_charger.current_fault?;
            if !fault.requires_technician() {
                return None;
            }
            let target_bay = multi_site.get_site(belongs.site_id).map(|site| {
                if let Some(pos) = next_charger.grid_position {
                    resolve_technician_target_bay(&site.grid, pos)
                } else {
                    let charger_world = Vec2::new(
                        next_transform.translation().x - site.world_offset().x,
                        next_transform.translation().y - site.world_offset().y,
                    );
                    resolve_technician_target_bay(
                        &site.grid,
                        SiteGrid::world_to_grid(charger_world),
                    )
                }
            });
            Some((
                idx,
                fault,
                target_bay,
                next_charger.cable_replacement_cost(),
            ))
        });

        let mut chained_to_next_job = false;

        if let Some((idx, next_fault_type, next_target_bay, cable_cost)) = next_same_site {
            // --- Same-site job chaining ---
            if let Some(bay) = next_target_bay {
                let Some(next_dispatch) = tech_state.dispatch_queue.remove(idx) else {
                    return;
                };

                let chain_warranty_tier = tech_state
                    .current_site_id
                    .and_then(|sid| multi_site.get_site(sid))
                    .map(|s| s.service_strategy.warranty_tier)
                    .unwrap_or_default();
                let viewed_same_site = multi_site.viewed_site_id == Some(completed_site_id);
                let next_status = if viewed_same_site {
                    RepairRequestStatus::WalkingOnSite
                } else {
                    RepairRequestStatus::WaitingAtSite
                };

                if !repair_requests.set_status(next_dispatch.request_id, next_status) {
                    warn!(
                        "Skipping same-site technician chain for {} because request {:?} is no longer open",
                        next_dispatch.charger_id, next_dispatch.request_id
                    );
                } else {
                    let job = start_job(
                        &mut game_state,
                        &profile,
                        &next_dispatch,
                        next_fault_type,
                        cable_cost,
                        chain_warranty_tier,
                        !repair_requests.dispatch_costs_recorded(next_dispatch.request_id),
                    );
                    if !repair_requests.dispatch_costs_recorded(next_dispatch.request_id) {
                        let _ =
                            repair_requests.mark_dispatch_costs_recorded(next_dispatch.request_id);
                    }
                    tech_state.set_same_site_job(
                        next_dispatch.request_id,
                        next_dispatch.charger_entity,
                        next_dispatch.site_id,
                        job.repair_duration,
                    );
                    if !viewed_same_site {
                        tech_state.set_waiting_at_site_job(
                            next_dispatch.request_id,
                            next_dispatch.charger_entity,
                            next_dispatch.site_id,
                            job.repair_duration,
                        );
                    }
                    chained_to_next_job = true;

                    info!(
                        "Technician chaining to next job at same site: {} (repair: {:.0}s, cost: ${:.2})",
                        job.charger_id, job.repair_duration, job.dispatch_cost
                    );

                    // Redirect the on-site technician entity to the new charger.
                    if is_viewed_site_active_job(&tech_state, &multi_site) {
                        for (
                            tech_entity,
                            mut technician,
                            mut movement,
                            mut emotion,
                            _tech_belongs,
                            _agent_pos,
                        ) in technicians.iter_mut()
                        {
                            if technician.target_charger == charger_entity {
                                technician.target_charger = next_dispatch.charger_entity;
                                technician.target_bay = Some(bay);
                                technician.phase = TechnicianPhase::WalkingToCharger;
                                movement.phase = TechnicianPhase::WalkingToCharger;
                                emotion.set_reason(
                                    TechnicianEmotionReason::NextJob,
                                    game_clock.total_real_time,
                                );
                                commands.entity(tech_entity).try_insert(
                                    Pathfind::new_2d(bay.0 as u32, bay.1 as u32)
                                        .mode(PathfindMode::AStar),
                                );
                                info!(
                                    "Technician pathfinding to next charger at ({}, {})",
                                    bay.0, bay.1
                                );
                                break;
                            }
                        }
                    }
                }
            } else if let Some(queued) = tech_state.dispatch_queue.get(idx) {
                warn!(
                    "Next charger {} could not be routed on-site, leaving it queued",
                    queued.charger_id
                );
            }
        }

        if !chained_to_next_job {
            // --- Normal exit routing ---
            tech_state.begin_leaving_site(completed_site_id);
            if is_viewed_site_active_job(&tech_state, &multi_site) {
                let mut routed_to_exit = false;

                for (tech_entity, mut technician, mut movement, mut emotion, belongs, _agent_pos) in
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
                        routed_to_exit = true;
                        break;
                    }
                }

                if !routed_to_exit {
                    warn!(
                        "Technician completed job at viewed site {:?} but had no avatar to route to exit; leaving queued work pending",
                        completed_site_id
                    );
                }
            }
        }
    }
}

pub fn cleanup_technicians_on_day_end(
    mut commands: Commands,
    technicians: Query<Entity, With<Technician>>,
    mut tech_state: ResMut<TechnicianState>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
) {
    for entity in &technicians {
        commands.entity(entity).try_despawn();
    }

    repair_requests.reset_for_new_day();
    tech_state.reset_for_new_day();
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
    mut repair_requests: ResMut<RepairRequestRegistry>,
    game_clock: Res<GameClock>,
) {
    for (entity, _technician, movement) in technicians.iter() {
        if movement.phase == TechnicianPhase::Exited {
            // Despawn technician entity (despawn automatically handles children in Bevy 0.15+)
            commands.entity(entity).try_despawn();

            // Only reset technician state if we're in LeavingSite (prevents multiple resets)
            if tech_state.status() == TechStatus::LeavingSite {
                tech_state.set_idle();

                info!("Technician entity cleaned up, now idle");

                // Start next queued job if any
                start_next_queued_job(
                    &mut tech_state,
                    &chargers,
                    &multi_site,
                    &mut game_state,
                    &profile,
                    &mut repair_requests,
                    &game_clock,
                );
            }
        }
    }
}

pub fn recover_orphaned_leaving_technician_system(
    mut tech_state: ResMut<TechnicianState>,
    technicians: Query<Entity, With<Technician>>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    multi_site: Res<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    profile: Res<crate::resources::PlayerProfile>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
    game_clock: Res<GameClock>,
) {
    if tech_state.status() != TechStatus::LeavingSite || !technicians.is_empty() {
        return;
    }

    tech_state.set_idle();
    start_next_queued_job(
        &mut tech_state,
        &chargers,
        &multi_site,
        &mut game_state,
        &profile,
        &mut repair_requests,
        &game_clock,
    );
}

pub fn cleanup_sold_site_technician_state(
    mut commands: Commands,
    mut sold_events: MessageReader<SiteSoldEvent>,
    mut tech_state: ResMut<TechnicianState>,
    chargers: Query<(&Charger, &BelongsToSite)>,
    technicians: Query<(Entity, &Technician, &BelongsToSite)>,
    multi_site: Res<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    profile: Res<crate::resources::PlayerProfile>,
    mut repair_requests: ResMut<RepairRequestRegistry>,
    game_clock: Res<GameClock>,
) {
    for sold in sold_events.read() {
        let stale_request_ids: Vec<RepairRequestId> = repair_requests
            .iter()
            .filter(|request| request.status.is_open() && request.site_id == sold.site_id)
            .map(|request| request.id)
            .collect();
        for request_id in stale_request_ids {
            let _ = repair_requests.resolve(
                request_id,
                game_clock.total_game_time,
                RepairResolution::Cancelled,
            );
        }

        tech_state
            .dispatch_queue
            .retain(|queued| queued.site_id != sold.site_id);
        tech_state.clear_current_site_if_matches(sold.site_id);

        for (entity, _technician, belongs) in technicians.iter() {
            if belongs.site_id == sold.site_id {
                commands.entity(entity).try_despawn();
            }
        }

        let abort_active_job = tech_state.destination_site_id() == Some(sold.site_id)
            || tech_state.leaving_site_id() == Some(sold.site_id);

        if abort_active_job {
            tech_state.set_idle();
            start_next_queued_job(
                &mut tech_state,
                &chargers,
                &multi_site,
                &mut game_state,
                &profile,
                &mut repair_requests,
                &game_clock,
            );
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
    mut repair_requests: ResMut<RepairRequestRegistry>,
    game_clock: Res<GameClock>,
) {
    // 1. Clear UI selection if the charger entity no longer exists
    if let Some(entity) = selected.0
        && chargers.get(entity).is_err()
    {
        selected.0 = None;
        info!("Cleared stale charger selection (charger sold or removed)");
    }

    // 2. Remove stale entries from the technician dispatch queue
    let stale_request_ids: Vec<RepairRequestId> = tech_state
        .dispatch_queue
        .iter()
        .filter(|queued| chargers.get(queued.charger_entity).is_err())
        .map(|queued| queued.request_id)
        .collect();
    tech_state
        .dispatch_queue
        .retain(|queued| chargers.get(queued.charger_entity).is_ok());
    let purged = stale_request_ids.len();
    for request_id in stale_request_ids {
        let _ = repair_requests.resolve(
            request_id,
            game_clock.total_game_time,
            RepairResolution::Cancelled,
        );
    }
    if purged > 0 {
        info!(
            "Purged {} stale entries from technician dispatch queue",
            purged
        );
    }

    // 3. Abort the active technician job if its target charger was removed
    if let Some(target) = tech_state.active_charger()
        && chargers.get(target).is_err()
    {
        info!(
            "Target charger {:?} was removed, aborting technician job (status: {:?})",
            target,
            tech_state.status()
        );

        // Despawn any on-site technician entities targeting this charger
        for (tech_entity, technician) in &technicians {
            if technician.target_charger == target {
                commands.entity(tech_entity).try_despawn();
            }
        }

        // Reset technician state to idle
        if let Some(request_id) = tech_state.active_request_id() {
            let _ = repair_requests.resolve(
                request_id,
                game_clock.total_game_time,
                RepairResolution::Cancelled,
            );
        }
        tech_state.set_idle();

        // Try to start the next queued job (if any)
        start_next_queued_job(
            &mut tech_state,
            &chargers,
            &multi_site,
            &mut game_state,
            &profile,
            &mut repair_requests,
            &game_clock,
        );
    }
}
