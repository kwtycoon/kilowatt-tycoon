//! Power dispatch system - allocates available power to chargers using FCFS policy.
//!
//! This system runs before `charging_system` and sets `allocated_power_kw` on each
//! charger based on site constraints (contracted capacity, transformer rating),
//! solar generation, BESS state, ServiceStrategy, and EnvironmentState.

use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::{Charger, ChargerState};
use crate::resources::SiteId;
use crate::resources::{
    EnvironmentState, GameClock, GameState, MultiSiteManager, SiteConfig, SolarExportPolicy,
    TouPeriod,
};

/// Power dispatch system - allocates power to chargers within site constraints
/// Now respects ServiceStrategy.target_power_density, EnvironmentState, and per-site grid capacity
pub fn power_dispatch_system(
    mut chargers: Query<(Entity, &mut Charger, &BelongsToSite)>,
    mut multi_site: ResMut<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    _site_config: Res<SiteConfig>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    environment: Res<EnvironmentState>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };
    let site_id = &viewed_id;

    // Sync installed capacity from grid to state resources
    // (This ensures placed solar/battery tiles are reflected in the simulation)
    if (site_state.solar_state.installed_kw_peak - site_state.grid.total_solar_kw).abs() > 0.1 {
        site_state.solar_state.installed_kw_peak = site_state.grid.total_solar_kw;
    }
    if (site_state.bess_state.capacity_kwh - site_state.grid.total_battery_kwh).abs() > 0.1 {
        site_state.bess_state.capacity_kwh = site_state.grid.total_battery_kwh;
        site_state.bess_state.max_charge_kw = site_state.grid.total_battery_kw;
        site_state.bess_state.max_discharge_kw = site_state.grid.total_battery_kw;
        // Initialize SOC to 50% if capacity changed
        if site_state.bess_state.soc_kwh > site_state.bess_state.capacity_kwh {
            site_state.bess_state.soc_kwh = site_state.bess_state.capacity_kwh * 0.5;
        }
    }

    // Grid connection is the hard dispatch limit; transformer overload is
    // handled by the thermal model (not by capping dispatch).
    let site_limit_kva = site_state.dispatch_limit_kva();

    // Update solar generation based on time of day and weather
    let solar_factor = site_state
        .site_energy_config
        .solar_generation_factor(game_clock.game_time)
        * environment.current_weather.solar_multiplier();
    site_state.solar_state.update_generation(solar_factor);
    let solar_kw = site_state.solar_state.current_generation_kw;

    // Collect all charging sessions with their requests for this site
    // Each charger that is Charging and has a session_start_game_time is considered active
    // Apply ServiceStrategy target_power_density and EnvironmentState health multiplier
    // Track both kW (output to vehicle) and kVA (draw from grid)
    let mut active_sessions: Vec<(Entity, f32, f32, f32)> = Vec::new(); // (entity, session_start, requested_kw, requested_kva)
    let mut total_requested_kw = 0.0_f32;
    let mut total_requested_kva = 0.0_f32;

    // Calculate effective multipliers
    // Higher power density = faster charging but more heat/stress on equipment
    let power_density_mult = site_state.service_strategy.target_power_density;
    let health_mult = environment.current_weather.charger_health_multiplier();
    // Cold temperatures reduce charging speed (battery chemistry limitation)
    let cold_mult = site_state
        .archetype
        .cold_charging_multiplier(environment.temperature_f);
    let effective_mult = power_density_mult * health_mult * cold_mult;

    // Note: High power density increases transformer heat (handled in HUD display)
    // Heat generation factor: base_heat * (power_density ^ 1.3)
    // This creates non-linear heat buildup at high power densities

    for (entity, charger, belongs) in &chargers {
        // Only include chargers that:
        // 1. Belong to this site
        // 2. Have an active charging session
        // 3. Can actually deliver power (not disabled, not faulted)
        if belongs.site_id == *site_id && charger.is_charging && charger.can_deliver_power() {
            let start_time = charger.session_start_game_time.unwrap_or(0.0);
            // Apply strategy and environment multipliers to requested power
            let base_requested = charger.requested_power_kw;
            let requested_kw = base_requested * effective_mult;
            // Calculate apparent power (kVA) for infrastructure limits
            let requested_kva = charger.input_kva(requested_kw);
            if requested_kw > 0.0 {
                active_sessions.push((entity, start_time, requested_kw, requested_kva));
                total_requested_kw += requested_kw;
                total_requested_kva += requested_kva;
            }
        }
    }

    // Update cached charger utilization for this site
    {
        let mut enabled = 0u32;
        let mut occupied = 0u32;
        for (_, charger, belongs) in &chargers {
            if belongs.site_id == *site_id && !charger.is_disabled {
                enabled += 1;
                if charger.state() == ChargerState::Charging {
                    occupied += 1;
                }
            }
        }
        site_state.charger_utilization = if enabled > 0 {
            occupied as f32 / enabled as f32
        } else {
            0.0
        };
    }

    // Sort by session start time (FCFS - earlier sessions get priority)
    active_sessions.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Determine how much power is available for chargers
    // BESS controller: peak shave if load approaches limit, charge during off-peak
    // Note: BESS thresholds use kVA since they're about infrastructure protection
    let tou_period = site_state
        .site_energy_config
        .current_tou_period(game_clock.game_time);
    let peak_shave_threshold_kva = site_limit_kva * site_state.bess_state.peak_shave_threshold;
    let charge_threshold_kva = site_limit_kva * site_state.bess_state.charge_threshold;

    // First pass: calculate gross load (what chargers are requesting)
    let gross_charger_load_kw = total_requested_kw;
    let gross_charger_load_kva = total_requested_kva;

    // Achievement tracking: detect grid overload (demand exceeds capacity before throttling)
    if gross_charger_load_kva > site_limit_kva && site_limit_kva > 0.0 {
        game_state.grid_overload_triggered = true;
    }

    // Calculate BESS contribution (operates in kW for energy calculations)
    // Smart dispatch priorities:
    // 1. Prevent new demand peaks (highest priority - reduces demand charges)
    // 2. Peak shave when approaching threshold
    // 3. Store excess solar generation (skipped when MaxExport policy is active)
    // 4. Charge during off-peak low-load periods
    let mut bess_contribution_kw = 0.0_f32;
    let export_policy = site_state.service_strategy.solar_export_policy;

    if site_state.bess_state.capacity_kwh > 0.0 {
        use crate::resources::site_energy::BessMode;

        // Net load after solar (using kVA for infrastructure threshold comparison)
        let solar_kva_reduction = if gross_charger_load_kw > 0.0 {
            solar_kw * (gross_charger_load_kva / gross_charger_load_kw)
        } else {
            0.0
        };
        let load_after_solar_kw = (gross_charger_load_kw - solar_kw).max(0.0);
        let load_after_solar_kva = (gross_charger_load_kva - solar_kva_reduction).max(0.0);

        // Calculate available discharge capacity
        let available_discharge = site_state
            .bess_state
            .max_discharge_kw
            .min(site_state.bess_state.soc_kwh / (delta_game_seconds / 3600.0));

        // Calculate available charge capacity
        let available_charge = site_state.bess_state.max_charge_kw.min(
            site_state.bess_state.available_charge_kwh()
                / (delta_game_seconds / 3600.0)
                / site_state.bess_state.round_trip_efficiency,
        );

        // Current peak demand from utility meter
        let current_peak_kw = site_state.utility_meter.peak_demand_kw;

        match site_state.bess_state.mode {
            BessMode::TouArbitrage => {
                // TOU Arbitrage: charge during off-peak, discharge during on-peak.
                // Discharge is capped to actual load (after solar) so stored
                // energy offsets expensive on-peak imports rather than being
                // dumped to the grid at lower wholesale rates.
                if tou_period == TouPeriod::OnPeak
                    && site_state.bess_state.soc_kwh > 0.0
                    && available_discharge > 0.0
                    && load_after_solar_kw > 0.0
                {
                    bess_contribution_kw = available_discharge.min(load_after_solar_kw);
                } else if tou_period == TouPeriod::OffPeak
                    && site_state.bess_state.soc_percent() < 95.0
                    && available_charge > 0.0
                {
                    bess_contribution_kw = -available_charge;
                }
            }
            BessMode::Backup | BessMode::Manual => {
                // Backup/Manual: do nothing automatically
            }
            BessMode::PeakShaving => {
                // PRIORITY 1: Prevent new demand peaks (demand charge optimization)
                // If current load would set a new peak, discharge to prevent it
                if load_after_solar_kw > current_peak_kw
                    && site_state.bess_state.soc_kwh > 0.0
                    && available_discharge > 0.0
                {
                    // Discharge enough to keep load at or below current peak
                    let needed_discharge = load_after_solar_kw - current_peak_kw;
                    bess_contribution_kw = needed_discharge.min(available_discharge);
                }
                // PRIORITY 2: Peak shave when approaching site capacity threshold
                else if load_after_solar_kva > peak_shave_threshold_kva
                    && site_state.bess_state.soc_kwh > 0.0
                    && available_discharge > 0.0
                {
                    let needed_discharge_kva = load_after_solar_kva - peak_shave_threshold_kva;
                    bess_contribution_kw = needed_discharge_kva.min(available_discharge);
                }
                // PRIORITY 3: Store excess solar generation
                // Skipped when MaxExport is active -- that solar is exported to the grid instead
                else if export_policy != SolarExportPolicy::MaxExport
                    && solar_kw > gross_charger_load_kw
                    && site_state.bess_state.soc_percent() < 95.0
                    && available_charge > 0.0
                {
                    let excess_solar = solar_kw - gross_charger_load_kw;
                    bess_contribution_kw = -excess_solar.min(available_charge); // Negative = charging
                }
                // PRIORITY 4: Off-peak charging when load is low
                else if tou_period == TouPeriod::OffPeak
                    && load_after_solar_kva < charge_threshold_kva
                    && site_state.bess_state.soc_percent() < 95.0
                    && available_charge > 0.0
                {
                    let headroom = charge_threshold_kva - load_after_solar_kva;
                    bess_contribution_kw = -headroom.min(available_charge); // Negative = charging
                }
            } // end PeakShaving
        } // end match

        // Apply BESS action
        if bess_contribution_kw > 0.0 {
            // Discharging
            let discharged_kwh = site_state.bess_state.discharge(
                bess_contribution_kw * (delta_game_seconds / 3600.0),
                delta_game_seconds,
            );
            bess_contribution_kw = discharged_kwh / (delta_game_seconds / 3600.0);
        } else if bess_contribution_kw < 0.0 {
            // Charging
            let charge_requested = -bess_contribution_kw * (delta_game_seconds / 3600.0);
            let charged_kwh = site_state
                .bess_state
                .charge(charge_requested, delta_game_seconds);
            bess_contribution_kw = -(charged_kwh / (delta_game_seconds / 3600.0));
        } else {
            site_state.bess_state.current_power_kw = 0.0;
        }
    }

    // Calculate available apparent power (kVA) for chargers after solar + BESS.
    // Apply thermal throttle factor: when the transformer overheats, the available
    // kVA is reduced so all chargers are proportionally throttled via FCFS allocation.
    // During a hacker overload attack, the throttle is bypassed.
    let throttle = if site_state.hacker_overload_remaining_secs > 0.0 {
        1.0
    } else {
        site_state.thermal_throttle_factor
    };
    let available_kva_for_chargers = (site_limit_kva + solar_kw + bess_contribution_kw) * throttle;

    // Allocate power to chargers using FCFS, respecting kVA limit
    let mut remaining_kva = available_kva_for_chargers;

    for (entity, _start_time, requested_kw, requested_kva) in &active_sessions {
        if let Ok((_, mut charger, _)) = chargers.get_mut(*entity) {
            // Limit allocation based on remaining kVA capacity
            let kva_ratio = if *requested_kva > 0.0 {
                remaining_kva.max(0.0) / *requested_kva
            } else {
                1.0
            };
            let allocation_ratio = kva_ratio.min(1.0);
            let allocation_kw = *requested_kw * allocation_ratio;
            charger.allocated_power_kw = allocation_kw;

            // Deduct the kVA used
            let used_kva = charger.input_kva(allocation_kw);
            remaining_kva -= used_kva;
        }
    }

    // Zero out allocation for non-charging chargers at this site
    for (_, mut charger, belongs) in &mut chargers {
        if belongs.site_id == *site_id && !charger.is_charging {
            charger.allocated_power_kw = 0.0;
            charger.requested_power_kw = 0.0;
        }
    }

    // Calculate actual grid import for this site (both kW and kVA)
    let mut actual_charger_load_kw: f32 = 0.0;
    let mut actual_charger_load_kva: f32 = 0.0;
    for (_, charger, belongs) in chargers.iter() {
        if belongs.site_id == *site_id {
            actual_charger_load_kw += charger.allocated_power_kw;
            actual_charger_load_kva += charger.input_kva(charger.allocated_power_kw);
        }
    }

    // Update grid import resource for this site (both kW for billing and kVA for infrastructure)
    site_state.grid_import.gross_load_kw = actual_charger_load_kw;
    site_state.grid_import.gross_load_kva = actual_charger_load_kva;
    site_state.grid_import.solar_kw = solar_kw;
    site_state.grid_import.bess_kw = bess_contribution_kw;
    site_state.grid_import.calculate();

    // Suppress export when policy is Never (curtail excess solar)
    if export_policy == SolarExportPolicy::Never {
        site_state.grid_import.export_kw = 0.0;
    }

    // Track solar generation
    if delta_game_seconds > 0.0 {
        site_state.solar_state.total_generated_kwh += solar_kw * (delta_game_seconds / 3600.0);
    }
}

/// Fraction of enabled chargers at a site that are actively charging (0.0 - 1.0).
pub fn charger_utilization(
    chargers: &Query<
        (Entity, &Charger, &BelongsToSite),
        Without<crate::components::driver::Driver>,
    >,
    site_id: SiteId,
) -> f32 {
    let mut total = 0u32;
    let mut occupied = 0u32;
    for (_, charger, belongs) in chargers.iter() {
        if belongs.site_id == site_id && !charger.is_disabled {
            total += 1;
            if charger.state() == ChargerState::Charging {
                occupied += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        occupied as f32 / total as f32
    }
}
