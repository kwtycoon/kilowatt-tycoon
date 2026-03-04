//! Charger systems

use bevy::prelude::*;
use rand::Rng;

use crate::components::charger::{Charger, FaultType};
use crate::components::driver::{Driver, DriverState};
use crate::data::LoadedChargers;
use crate::events::{ChargerFaultEvent, DriverLeftEvent};
use crate::resources::{
    EnvironmentState, GameClock, GameState, MultiSiteManager, OemTier, SiteConfig, TileContent,
    TutorialState, TutorialStep, WeatherType,
};

// ============ RF Environment Constants ============

const BASE_SIGNAL: f32 = 1.0;
const BOOSTER_GAIN_PER_UNIT: f32 = 0.20;
const BOOSTER_DIMINISHING_EXP: f32 = 0.7;
const STAFF_FAULT_REDUCTION: f32 = 0.85;
pub const STAFF_DETECTION_DELAY_SECS: f32 = 60.0;

fn rush_hour_noise(hour: u32) -> f32 {
    match hour {
        0..=5 => 0.00,
        6 => 0.05,
        7..=8 => 0.12,
        9..=11 => 0.05,
        12 => 0.08,
        13..=16 => 0.05,
        17..=18 => 0.15,
        19..=20 => 0.08,
        21..=23 => 0.03,
        _ => 0.0,
    }
}

fn weather_noise(weather: WeatherType) -> f32 {
    match weather {
        WeatherType::Sunny => 0.0,
        WeatherType::Overcast => 0.03,
        WeatherType::Rainy => 0.15,
        WeatherType::Heatwave => 0.10,
        WeatherType::Cold => 0.05,
    }
}

/// Computes per-site RF environment (noise floor, SNR, fault multipliers).
/// Runs in GameSystemSet::Environment before fault systems.
pub fn rf_environment_system(
    chargers: Query<(&Charger, &crate::components::BelongsToSite)>,
    mut multi_site: ResMut<MultiSiteManager>,
    environment: Res<EnvironmentState>,
    game_clock: Res<GameClock>,
) {
    if game_clock.is_paused() {
        return;
    }

    let hour = game_clock.hour();
    let current_weather = environment.current_weather;

    // Pre-count active sessions per site
    let mut sessions_per_site = std::collections::HashMap::new();
    for (charger, belongs) in &chargers {
        if charger.is_charging {
            *sessions_per_site.entry(belongs.site_id).or_insert(0u32) += 1;
        }
    }

    for (site_id, site) in multi_site.owned_sites.iter_mut() {
        let active_sessions = sessions_per_site.get(site_id).copied().unwrap_or(0);

        let charging_noise = 0.06 * active_sessions as f32;
        let vehicle_noise = 0.03 * site.driver_count as f32;
        let w_noise = weather_noise(current_weather);
        let r_noise = rush_hour_noise(hour);

        let amenity_counts = site.service_strategy.amenity_counts;
        let wifi_count = amenity_counts[0];
        let lounge_count = amenity_counts[1];
        let restaurant_count = amenity_counts[2];
        let amenity_noise =
            0.05 * wifi_count as f32 + 0.08 * lounge_count as f32 + 0.12 * restaurant_count as f32;

        let noise_floor = charging_noise + vehicle_noise + w_noise + r_noise + amenity_noise;

        let booster_count = site.grid.count_content(TileContent::BoosterPad);
        let booster_bonus = if booster_count > 0 {
            BOOSTER_GAIN_PER_UNIT
                * bevy::math::ops::powf(booster_count as f32, BOOSTER_DIMINISHING_EXP)
        } else {
            0.0
        };

        let snr = (BASE_SIGNAL + booster_bonus - noise_floor).max(0.0);

        let comm_fault_multiplier = (1.0 - snr).clamp(0.0, 2.0);
        let jam_multiplier = (1.5 - snr).clamp(0.5, 2.5);

        let staff_fault_multiplier =
            bevy::math::ops::powf(STAFF_FAULT_REDUCTION, restaurant_count as f32);
        let staff_detection_bonus = restaurant_count > 0;

        site.rf_environment = crate::resources::RfEnvironment {
            noise_floor,
            snr,
            comm_fault_multiplier,
            jam_multiplier,
            staff_fault_multiplier,
            staff_detection_bonus,
            booster_count,
        };
    }
}

/// Spawn charger entities from loaded data
pub fn spawn_chargers_system(
    mut commands: Commands,
    loaded_chargers: Option<Res<LoadedChargers>>,
    site_config: Res<SiteConfig>,
    multi_site: Res<crate::resources::MultiSiteManager>,
) {
    let Some(loaded) = loaded_chargers else {
        return;
    };

    info!(
        "Spawning {} chargers for site {}",
        loaded.0.len(),
        site_config.name
    );

    for charger_data in &loaded.0 {
        let scripted_fault = charger_data.scripted_fault.as_ref();

        let charger = Charger {
            id: charger_data.id.clone(),
            charger_type: charger_data.charger_type,
            rated_power_kw: charger_data.rated_power_kw,
            phase: charger_data.phase,
            health: charger_data.health,
            connector_jam_chance: charger_data.connector_jam_chance,
            scripted_fault_time: scripted_fault.map(|f| f.trigger_time),
            scripted_fault_type: scripted_fault.map(|f| f.fault_type),
            ..default()
        };

        // Spawn charger entity with sprite
        let pos = Vec3::new(charger_data.position.x, charger_data.position.y, 0.0);

        // Get active site ID for tagging
        let site_id = multi_site
            .viewed_site_id
            .unwrap_or(crate::resources::SiteId(0));

        commands.spawn((
            charger,
            Transform::from_translation(pos),
            Visibility::default(),
            crate::components::BelongsToSite::new(site_id),
        ));

        info!(
            "  Spawned charger {} at ({}, {}) for site {:?}",
            charger_data.id, charger_data.position.x, charger_data.position.y, site_id
        );
    }
}

/// Update charger cooldowns
pub fn charger_cooldown_system(
    mut chargers: Query<&mut Charger>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta = time.delta_secs() * game_clock.speed.multiplier();

    for mut charger in &mut chargers {
        charger.update_cooldowns(delta);
    }
}

/// Helper to inject a fault and immediately clear all session state.
/// This ensures no power flows through a faulted charger even within the same frame.
/// The fault is NOT immediately visible to the player - detection depends on OEM tier.
fn inject_fault(charger: &mut Charger, fault_type: FaultType, game_time: f32) {
    charger.current_fault = Some(fault_type);
    charger.fault_occurred_at = Some(game_time);
    charger.fault_detected_at = None;
    charger.fault_is_detected = false;
    charger.fault_discovered = false;
    charger.reboot_attempts = 0;
    // Degrade reliability on fault occurrence (severity-based)
    charger.degrade_reliability_fault(&fault_type);
    // Immediately clear ALL session state to prevent any power flow
    charger.is_charging = false;
    charger.current_power_kw = 0.0;
    charger.requested_power_kw = 0.0;
    charger.allocated_power_kw = 0.0;
    charger.session_start_game_time = None;
}

/// Check for scripted fault triggers
pub fn scripted_fault_system(
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    game_clock: Res<GameClock>,
    mut game_state: ResMut<GameState>,
    _multi_site: Res<crate::resources::MultiSiteManager>,
    tutorial_state: Option<Res<TutorialState>>,
) {
    // Suppress non-tutorial faults during the FixCharger tutorial step
    if tutorial_state
        .as_ref()
        .is_some_and(|ts| ts.current_step == Some(TutorialStep::FixCharger))
    {
        return;
    }

    for (_entity, mut charger, _belongs_to_site) in &mut chargers {
        // Check scripted fault
        if let (Some(trigger_time), Some(fault_type)) =
            (charger.scripted_fault_time, charger.scripted_fault_type)
            && game_clock.game_time >= trigger_time
            && charger.current_fault.is_none()
        {
            // Trigger the fault and clear all session state immediately
            inject_fault(&mut charger, fault_type, game_clock.total_game_time);

            // Clear the trigger so it doesn't fire again
            charger.scripted_fault_time = None;

            info!(
                "Scripted fault triggered on {}: {:?} (detection pending)",
                charger.id, fault_type
            );

            // NOTE: ChargerFaultEvent is now emitted by fault_detection_system
            // after the detection delay, NOT immediately here.

            // First fault tutorial
            if !game_state.first_fault_seen {
                game_state.first_fault_seen = true;
            }
        }
    }
}

/// Safety watchdog system - enforces state consistency every frame.
/// If a charger cannot deliver power (disabled or faulted), this system
/// force-clears all session-related fields to prevent any other system
/// from accidentally thinking a charging session is active.
pub fn charger_state_system(mut chargers: Query<&mut Charger>, game_clock: Res<GameClock>) {
    if game_clock.is_paused() {
        return;
    }

    for mut charger in &mut chargers {
        // SAFETY WATCHDOG: If hardware cannot deliver power, force-clear ALL session state.
        // This is the authoritative enforcer - even if other systems incorrectly set
        // is_charging = true, this will catch and correct it every frame.
        if !charger.can_deliver_power() {
            charger.is_charging = false;
            charger.current_power_kw = 0.0;
            charger.requested_power_kw = 0.0;
            charger.allocated_power_kw = 0.0;
            continue;
        }

        // Clear power if not charging (but hardware is functional)
        if !charger.is_charging {
            charger.current_power_kw = 0.0;
            charger.requested_power_kw = 0.0;
            charger.allocated_power_kw = 0.0;
        }
    }
}

/// Check for connector jam when session ends (uses tier-adjusted chance).
/// Suppressed during the FixCharger tutorial step to avoid blocking tutorial progression.
pub fn check_connector_jam(
    charger: &mut Charger,
    game_time: f32,
    tutorial_active: bool,
    rf_jam_multiplier: f32,
) -> bool {
    if tutorial_active {
        return false;
    }

    let effective_chance = charger.effective_jam_chance() * rf_jam_multiplier;
    if effective_chance > 0.0 {
        let mut rng = rand::rng();
        if rng.random::<f32>() < effective_chance {
            inject_fault(charger, FaultType::CableDamage, game_time);
            return true;
        }
    }
    false
}

/// Stochastic fault system - randomly injects faults based on charger tier MTBF
///
/// Chargers can develop faults both while charging (high stress) and while idle (wear and tear).
/// Idle fault probability is 10x lower than charging fault probability.
///
/// Also applies per-tick maintenance effects:
/// - `failure_rate_multiplier` from the site's maintenance investment
/// - Wear (operating_hours) reduction from maintenance
/// - Passive reliability recovery from maintenance
pub fn stochastic_fault_system(
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
    multi_site: Res<crate::resources::MultiSiteManager>,
    tutorial_state: Option<Res<TutorialState>>,
) {
    if game_clock.is_paused() {
        return;
    }

    // Suppress non-tutorial faults during the FixCharger tutorial step
    if tutorial_state
        .as_ref()
        .is_some_and(|ts| ts.current_step == Some(TutorialStep::FixCharger))
    {
        return;
    }

    let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();
    let delta_hours = delta_game_seconds / 3600.0;

    let mut rng = rand::rng();

    for (_entity, mut charger, belongs_to_site) in &mut chargers {
        let strategy = multi_site
            .get_site(belongs_to_site.site_id)
            .map(|s| &s.service_strategy);

        // Maintenance effects: wear reduction and passive reliability recovery
        if let Some(strat) = strategy {
            let maintenance_rate = (strat.maintenance_investment / 50.0).clamp(0.0, 1.0);
            let wear_recovery = (strat.maintenance_investment / 30.0).min(1.0);
            charger.operating_hours =
                (charger.operating_hours - wear_recovery * delta_hours).max(0.0);
            charger.recover_reliability_maintenance(maintenance_rate, delta_hours);
        }

        // Only check operating chargers without existing faults for fault injection
        if charger.current_fault.is_some() || charger.is_disabled {
            continue;
        }

        // Track operating hours (both charging and idle count towards wear)
        charger.operating_hours += delta_hours;
        charger.hours_since_last_fault += delta_hours;

        // RF-driven fault injection: comm faults scale with RF noise, hw faults use MTBF
        let failure_mult = strategy.map(|s| s.failure_rate_multiplier()).unwrap_or(1.0);
        let rf = multi_site
            .get_site(belongs_to_site.site_id)
            .map(|s| &s.rf_environment);

        let base_fault_prob = charger.fault_probability(delta_hours);
        let staff_mult = rf.map(|r| r.staff_fault_multiplier).unwrap_or(1.0);
        let comm_mult = rf.map(|r| r.comm_fault_multiplier).unwrap_or(0.5);

        // Communication faults: driven by RF environment
        let comm_prob = 0.4 * base_fault_prob * comm_mult * staff_mult * failure_mult;
        if comm_prob > 0.0 && rng.random::<f32>() < comm_prob {
            inject_fault(
                &mut charger,
                FaultType::CommunicationError,
                game_clock.total_game_time,
            );
            charger.hours_since_last_fault = 0.0;
            info!(
                "RF comm fault on {} (SNR {:.2}, noise {:.2}): CommunicationError",
                charger.id,
                rf.map(|r| r.snr).unwrap_or(1.0),
                rf.map(|r| r.noise_floor).unwrap_or(0.0),
            );
            if !game_state.first_fault_seen {
                game_state.first_fault_seen = true;
            }
            continue;
        }

        // Hardware faults: MTBF-based, unaffected by RF
        let hw_prob = 0.6 * base_fault_prob * staff_mult * failure_mult;
        if hw_prob > 0.0 && rng.random::<f32>() < hw_prob {
            let fault_type = match rng.random_range(0..100) {
                0..=34 => FaultType::PaymentError,   // 35%
                35..=64 => FaultType::FirmwareFault, // 30%
                65..=84 => FaultType::GroundFault,   // 20%
                _ => FaultType::CableDamage,         // 15%
            };

            inject_fault(&mut charger, fault_type, game_clock.total_game_time);
            charger.hours_since_last_fault = 0.0;

            info!(
                "Hardware fault on {} (tier {:?}, {:.0} operating hours): {:?} (detection pending)",
                charger.id, charger.tier, charger.operating_hours, fault_type
            );

            if !game_state.first_fault_seen {
                game_state.first_fault_seen = true;
            }
        }
    }
}

/// Guaranteed Day 1 technician fault - ensures players experience the technician dispatch mechanic.
///
/// On Day 1 only, after sufficient gameplay (15+ real seconds and 6-10 completed sessions),
/// this system injects a `GroundFault` (requires technician dispatch) on a random
/// charging charger. This guarantees players learn about technician dispatch.
pub fn guaranteed_day1_technician_fault_system(
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    game_clock: Res<GameClock>,
    mut game_state: ResMut<GameState>,
    _multi_site: Res<crate::resources::MultiSiteManager>,
    tutorial_state: Option<Res<TutorialState>>,
) {
    // Only run on Day 1
    if game_clock.day != 1 {
        return;
    }

    // Suppress non-tutorial faults during the FixCharger tutorial step
    if tutorial_state
        .as_ref()
        .is_some_and(|ts| ts.current_step == Some(TutorialStep::FixCharger))
    {
        return;
    }

    // Skip if already injected this fault
    if game_state.first_technician_fault_injected {
        return;
    }

    // Initialize target session count if not yet determined (random 6-10)
    let mut rng = rand::rng();
    let target_sessions = *game_state
        .day1_fault_target_session
        .get_or_insert_with(|| rng.random_range(6..=10));

    // Wait until at least 15 seconds of real wall clock time has passed
    const MIN_REAL_TIME: f32 = 15.0;
    if game_clock.total_real_time < MIN_REAL_TIME {
        return;
    }

    // Wait until the random target number of sessions have been completed
    if game_state.sessions_completed < target_sessions {
        return;
    }

    // Collect all charging chargers without existing faults
    let charging_chargers: Vec<_> = chargers
        .iter_mut()
        .filter(|(_, charger, _)| charger.is_charging && charger.current_fault.is_none())
        .collect();

    // Wait until at least one charger is actively charging
    if charging_chargers.is_empty() {
        return;
    }

    // Pick a random charging charger
    let index = rng.random_range(0..charging_chargers.len());

    // Need to re-query to get mutable access to just the selected charger
    let selected_entity = {
        let charging_entities: Vec<_> = chargers
            .iter()
            .filter(|(_, charger, _)| charger.is_charging && charger.current_fault.is_none())
            .map(|(e, _, _)| e)
            .collect();

        if charging_entities.is_empty() {
            return;
        }
        charging_entities[index % charging_entities.len()]
    };

    // Inject ground fault on the selected charger
    if let Ok((_entity, mut charger, _belongs_to_site)) = chargers.get_mut(selected_entity) {
        // Use GroundFault - requires technician (15 min repair)
        inject_fault(
            &mut charger,
            FaultType::GroundFault,
            game_clock.total_game_time,
        );

        info!(
            "Day 1 guaranteed technician fault triggered on {} after {} sessions: GroundFault (detection pending)",
            charger.id, game_state.sessions_completed
        );

        // NOTE: ChargerFaultEvent is now emitted by fault_detection_system
        // after the detection delay, NOT immediately here.

        // Mark as injected so it doesn't trigger again
        game_state.first_technician_fault_injected = true;

        // Also mark first fault seen for tutorial purposes
        if !game_state.first_fault_seen {
            game_state.first_fault_seen = true;
        }
    }
}

/// Kick out any drivers at a charger that just developed a fault.
///
/// Handles drivers in both Charging and WaitingForCharger states.
/// WaitingForCharger can occur when the charging system detects the fault first
/// and pauses the session before this kick system runs.
///
/// Drivers are immediately sent away angry with no payment - the charger breaking
/// during their session is a critical failure that ruins the customer experience.
///
/// Note: This system does NOT need to clear charger.is_charging because inject_fault()
/// already clears all session state when a fault is injected (see line 89).
pub fn kick_drivers_from_faulted_chargers(
    mut drivers: Query<(Entity, &mut Driver)>,
    chargers: Query<(Entity, &Charger), Changed<Charger>>,
    mut game_state: ResMut<GameState>,
    mut left_events: MessageWriter<DriverLeftEvent>,
) {
    for (charger_entity, charger) in chargers.iter() {
        // Only process chargers that have a fault
        if charger.current_fault.is_none() {
            continue;
        }

        // Find any driver assigned to this charger who is charging or waiting
        for (driver_entity, mut driver) in drivers.iter_mut() {
            if driver.assigned_charger == Some(charger_entity)
                && matches!(
                    driver.state,
                    DriverState::Charging | DriverState::WaitingForCharger
                )
            {
                let previous_state = driver.state;

                // Driver leaves immediately - angry, no payment
                driver.state = DriverState::LeftAngry;

                // Reputation penalty - charger broke during their session
                game_state.change_reputation(-4);
                game_state.sessions_failed += 1;
                game_state.daily_history.current_day.sessions_failed_today += 1;

                info!(
                    "Driver {} kicked out - charger {} broke during session (was {:?})",
                    driver.id, charger.id, previous_state
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
}

// ============ Fault Detection System ============

/// Fault detection system - handles the delay between when a fault occurs and when
/// the player is notified, based on OEM tier.
///
/// Without O&M: faults are not detected until a driver tries to use the charger.
/// With O&M: detection is near-instant (~10s).
/// Both O&M tiers auto-remediate software faults (no player action needed).
pub fn fault_detection_system(
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    game_clock: Res<GameClock>,
    multi_site: Res<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    mut fault_events: MessageWriter<ChargerFaultEvent>,
) {
    if game_clock.is_paused() {
        return;
    }

    for (entity, mut charger, belongs) in &mut chargers {
        // Skip chargers without undetected faults
        if charger.current_fault.is_none() || charger.fault_is_detected {
            continue;
        }

        let Some(occurred_at) = charger.fault_occurred_at else {
            // Fault exists but no timestamp - legacy, mark as detected immediately
            charger.fault_is_detected = true;
            charger.fault_detected_at = Some(game_clock.total_game_time);
            continue;
        };

        // Get the OEM tier for this charger's site
        let oem_tier = multi_site
            .get_site(belongs.site_id)
            .map(|site| site.site_upgrades.oem_tier)
            .unwrap_or(OemTier::None);

        // Use total_game_time for elapsed calculation (monotonic, survives day boundaries)
        let elapsed = game_clock.total_game_time - occurred_at;

        let staff_bonus = multi_site
            .get_site(belongs.site_id)
            .map(|s| s.rf_environment.staff_detection_bonus)
            .unwrap_or(false);

        let should_detect = match oem_tier.detection_delay_secs() {
            Some(delay) => elapsed >= delay,
            None => {
                if staff_bonus {
                    elapsed >= STAFF_DETECTION_DELAY_SECS
                } else {
                    false
                }
            }
        };

        if should_detect {
            charger.fault_is_detected = true;
            charger.fault_discovered = true; // Sync legacy flag so drivers don't re-discover
            charger.fault_detected_at = Some(game_clock.total_game_time);

            let fault_type = charger.current_fault.unwrap();

            info!(
                "Fault detected on {} ({:?}) after {:.0}s (O&M: {:?})",
                charger.id, fault_type, elapsed, oem_tier
            );

            let will_auto_remediate =
                oem_tier.has_auto_remediation() && !fault_type.requires_technician();

            fault_events.write(ChargerFaultEvent {
                charger_entity: entity,
                charger_id: charger.id.clone(),
                fault_type,
                auto_remediated: will_auto_remediate,
            });

            // Achievement tracking: reset fleet session streak on fault at a fleet site
            if let Some(site) = multi_site.get_site(belongs.site_id)
                && site.archetype.is_fleet()
            {
                game_state.fleet_sessions_without_fault = 0;
            }

            // Auto-remediation: directly fix software faults (no cooldown, no player action)
            if will_auto_remediate {
                info!(
                    "Auto-remediation: instantly clearing {:?} on {}",
                    fault_type, charger.id
                );

                let resolved_at = game_clock.total_game_time;

                // Directly clear the fault (bypassing cooldowns and success rolls)
                charger.current_fault = None;
                charger.reboot_attempts = 0;
                let downtime = resolved_at - occurred_at;
                charger.recover_reliability_fast_fix(
                    downtime,
                    oem_tier.reliability_recovery_multiplier(),
                );
                charger.fault_occurred_at = None;
                charger.fault_detected_at = None;
                charger.fault_is_detected = false;
            }
        }
    }
}

// ============ O&M Upgrade → Existing Fault Handler ============

/// When an O&M package is purchased, immediately process pre-existing faults at
/// the site so the player gets the benefit they just paid for:
///
/// - **Detect tier**: detect any still-undetected faults, auto-remediate software
///   faults (including ones previously discovered only by a driver).
/// - **Optimize tier**: auto-dispatch technicians for hardware faults that were
///   already detected but never dispatched.
pub fn handle_oem_upgrade_existing_faults(
    mut oem_events: MessageReader<crate::events::OemUpgradeEvent>,
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    game_clock: Res<GameClock>,
    mut fault_events: MessageWriter<ChargerFaultEvent>,
    mut dispatch_events: MessageWriter<crate::events::TechnicianDispatchEvent>,
) {
    for event in oem_events.read() {
        for (entity, mut charger, belongs) in &mut chargers {
            if belongs.site_id != event.site_id {
                continue;
            }
            let Some(fault_type) = charger.current_fault else {
                continue;
            };

            // Detect any faults that haven't been detected yet (e.g. legacy faults
            // without timestamps that fault_detection_system marks detected but
            // doesn't emit events for, or faults the system hasn't reached yet).
            if !charger.fault_is_detected && event.new_tier.detection_delay_secs().is_some() {
                charger.fault_is_detected = true;
                charger.fault_discovered = true;
                charger.fault_detected_at = Some(game_clock.total_game_time);

                let will_auto_remediate =
                    event.new_tier.has_auto_remediation() && !fault_type.requires_technician();

                fault_events.write(ChargerFaultEvent {
                    charger_entity: entity,
                    charger_id: charger.id.clone(),
                    fault_type,
                    auto_remediated: will_auto_remediate,
                });

                info!(
                    "O&M upgrade: detecting pre-existing {:?} on {}",
                    fault_type, charger.id
                );
            }

            // Auto-remediate detected software faults
            if charger.fault_is_detected
                && event.new_tier.has_auto_remediation()
                && !fault_type.requires_technician()
            {
                let occurred_at = charger
                    .fault_occurred_at
                    .unwrap_or(game_clock.total_game_time);
                let downtime = game_clock.total_game_time - occurred_at;
                charger.current_fault = None;
                charger.reboot_attempts = 0;
                charger.recover_reliability_fast_fix(
                    downtime,
                    event.new_tier.reliability_recovery_multiplier(),
                );
                charger.fault_occurred_at = None;
                charger.fault_detected_at = None;
                charger.fault_is_detected = false;

                info!(
                    "O&M upgrade: auto-remediated {:?} on {}",
                    fault_type, charger.id
                );
                continue;
            }

            // Auto-dispatch technician for detected hardware faults
            if charger.fault_is_detected
                && event.new_tier.has_auto_dispatch()
                && fault_type.requires_technician()
            {
                dispatch_events.write(crate::events::TechnicianDispatchEvent {
                    charger_entity: entity,
                    charger_id: charger.id.clone(),
                });

                info!(
                    "O&M upgrade: auto-dispatching technician for {:?} on {}",
                    fault_type, charger.id
                );
            }
        }
    }
}

// ============ Reliability Degradation System ============

/// Degrades charger reliability while faulted (ongoing downtime penalty).
/// Also tracks analytics for downtime.
pub fn reliability_degradation_system(
    mut chargers: Query<(Entity, &mut Charger, &crate::components::BelongsToSite)>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();
    let delta_hours = delta_game_seconds / 3600.0;

    for (_entity, mut charger, _belongs) in &mut chargers {
        // Only degrade while charger has an active fault
        if charger.current_fault.is_some() {
            charger.degrade_reliability_downtime(delta_hours);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RfEnvironment;

    #[test]
    fn rf_environment_default_is_clean() {
        let rf = RfEnvironment::default();
        assert!((rf.noise_floor - 0.0).abs() < f32::EPSILON);
        assert!((rf.snr - 1.0).abs() < f32::EPSILON);
        assert!((rf.comm_fault_multiplier - 0.0).abs() < f32::EPSILON);
        assert!((rf.jam_multiplier - 1.0).abs() < f32::EPSILON);
        assert!((rf.staff_fault_multiplier - 1.0).abs() < f32::EPSILON);
        assert!(!rf.staff_detection_bonus);
        assert_eq!(rf.booster_count, 0);
    }

    #[test]
    fn rush_hour_noise_peaks_during_commute() {
        assert!((rush_hour_noise(3) - 0.0).abs() < f32::EPSILON);
        assert!((rush_hour_noise(7) - 0.12).abs() < f32::EPSILON);
        assert!((rush_hour_noise(8) - 0.12).abs() < f32::EPSILON);
        assert!((rush_hour_noise(12) - 0.08).abs() < f32::EPSILON);
        assert!((rush_hour_noise(17) - 0.15).abs() < f32::EPSILON);
        assert!((rush_hour_noise(18) - 0.15).abs() < f32::EPSILON);
        assert!((rush_hour_noise(22) - 0.03).abs() < f32::EPSILON);
    }

    #[test]
    fn weather_noise_values() {
        assert!((weather_noise(WeatherType::Sunny) - 0.0).abs() < f32::EPSILON);
        assert!((weather_noise(WeatherType::Rainy) - 0.15).abs() < f32::EPSILON);
        assert!((weather_noise(WeatherType::Heatwave) - 0.10).abs() < f32::EPSILON);
        assert!((weather_noise(WeatherType::Cold) - 0.05).abs() < f32::EPSILON);
        assert!((weather_noise(WeatherType::Overcast) - 0.03).abs() < f32::EPSILON);
    }

    #[test]
    fn snr_computation_empty_site() {
        let noise_floor = 0.0;
        let booster_bonus = 0.0;
        let snr = (BASE_SIGNAL + booster_bonus - noise_floor).max(0.0);
        assert!((snr - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn snr_computation_noisy_site() {
        let noise_floor = 0.6;
        let booster_bonus = 0.0;
        let snr = (BASE_SIGNAL + booster_bonus - noise_floor).max(0.0);
        assert!((snr - 0.4).abs() < 0.01);
    }

    #[test]
    fn snr_improves_with_boosters() {
        let noise_floor = 0.6;
        let booster_count = 2u32;
        let booster_bonus = BOOSTER_GAIN_PER_UNIT
            * bevy::math::ops::powf(booster_count as f32, BOOSTER_DIMINISHING_EXP);
        let snr_boosted = (BASE_SIGNAL + booster_bonus - noise_floor).max(0.0);
        let snr_unboosted = (BASE_SIGNAL - noise_floor).max(0.0);
        assert!(snr_boosted > snr_unboosted);
    }

    #[test]
    fn comm_fault_multiplier_derivation() {
        let snr_clean = 1.0_f32;
        let snr_noisy = 0.2_f32;
        let mult_clean = (1.0 - snr_clean).clamp(0.0, 2.0);
        let mult_noisy = (1.0 - snr_noisy).clamp(0.0, 2.0);
        assert!((mult_clean - 0.0).abs() < f32::EPSILON);
        assert!((mult_noisy - 0.8).abs() < 0.01);
    }

    #[test]
    fn jam_multiplier_derivation() {
        let snr = 0.5_f32;
        let jam = (1.5 - snr).clamp(0.5, 2.5);
        assert!((jam - 1.0).abs() < f32::EPSILON);

        let snr_clean = 1.0_f32;
        let jam_clean = (1.5 - snr_clean).clamp(0.5, 2.5);
        assert!((jam_clean - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn staff_multiplier_scales_with_restaurants() {
        let no_restaurant = bevy::math::ops::powf(STAFF_FAULT_REDUCTION, 0.0);
        assert!((no_restaurant - 1.0).abs() < f32::EPSILON);

        let one_restaurant = bevy::math::ops::powf(STAFF_FAULT_REDUCTION, 1.0);
        assert!((one_restaurant - 0.85).abs() < 0.01);

        let two_restaurants = bevy::math::ops::powf(STAFF_FAULT_REDUCTION, 2.0);
        assert!((two_restaurants - 0.7225).abs() < 0.01);
    }
}
