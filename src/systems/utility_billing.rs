//! Utility billing system - tracks energy costs (TOU) and demand charges.
//!
//! This system runs after power dispatch and updates the utility meter with:
//! - Energy consumption by TOU period
//! - Rolling 15-minute demand average
//! - Peak demand tracking
//! - Cost application to game state

use bevy::prelude::*;

use crate::resources::{CharacterPerk, GameClock, GameState, MultiSiteManager, PlayerProfile};

/// Utility billing system - tracks consumption and applies costs
pub fn utility_billing_system(
    mut multi_site: ResMut<MultiSiteManager>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
    profile: Res<PlayerProfile>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();
    if delta_game_seconds <= 0.0 {
        return;
    }

    // Process each site independently
    for (_site_id, site_state) in multi_site.owned_sites.iter_mut() {
        let grid_kw = site_state.grid_import.current_kw;

        // Update current grid import on meter
        site_state.utility_meter.current_grid_import_kw = grid_kw;

        // Add demand sample for rolling average
        site_state.utility_meter.add_sample(
            game_clock.game_time,
            grid_kw,
            site_state.site_energy_config.demand_window_seconds,
        );

        // Calculate energy imported this tick
        let energy_kwh = grid_kw * (delta_game_seconds / 3600.0);

        if energy_kwh > 0.0 {
            // Determine TOU period and rate
            let tou_period = site_state
                .site_energy_config
                .current_tou_period(game_clock.game_time);
            let rate = site_state
                .site_energy_config
                .current_rate(game_clock.game_time);

            // Add to meter
            site_state
                .utility_meter
                .add_energy(energy_kwh, tou_period, rate);

            // Apply energy cost (continuously)
            let energy_cost = energy_kwh * rate;
            game_state.add_energy_cost(energy_cost);
        }

        // Solar export revenue: credit for power sold back to the grid
        let export_kw = site_state.grid_import.export_kw;
        if export_kw > 0.0 {
            let export_kwh = export_kw * (delta_game_seconds / 3600.0);
            let export_rate = site_state
                .site_energy_config
                .current_export_rate(game_clock.game_time);

            site_state.utility_meter.add_export(export_kwh, export_rate);

            let export_revenue = export_kwh * export_rate;
            game_state.add_solar_export_revenue(export_revenue);
        }

        // Update demand charge based on current peak
        // Apply character perk multiplier if UtilityInsider is active
        let demand_perk_multiplier = match profile.active_perk() {
            Some(CharacterPerk::UtilityInsider {
                demand_charge_multiplier,
            }) => demand_charge_multiplier,
            _ => 1.0,
        };
        site_state.utility_meter.update_demand_charge(
            site_state.site_energy_config.demand_rate_per_kw,
            demand_perk_multiplier,
        );

        // Apply demand charge delta to game state (since peak only goes up)
        let demand_delta =
            site_state.utility_meter.demand_charge - site_state.utility_meter.demand_charge_applied;
        if demand_delta > 0.0 {
            game_state.add_demand_charge(demand_delta);
            site_state.utility_meter.demand_charge_applied += demand_delta;
        }

        // Charge maintenance and amenity OPEX (per-tick, per-site)
        let hourly_opex = site_state.service_strategy.hourly_maintenance_cost()
            + site_state.service_strategy.amenity_cost_per_hour();
        let opex_this_tick = hourly_opex * (delta_game_seconds / 3600.0);
        if opex_this_tick > 0.0 {
            game_state.add_opex(opex_this_tick);
        }
    }
}
