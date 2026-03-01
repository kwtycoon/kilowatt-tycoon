//! Utility billing system - tracks energy costs (TOU) and demand charges.
//!
//! This system runs after power dispatch and updates the utility meter with:
//! - Energy consumption by TOU period
//! - Rolling 15-minute demand average
//! - Peak demand tracking
//! - Cost application to game state
//! - Wholesale spot market pricing for solar export (level 2+ sites)

use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::Charger;
use crate::resources::{
    CharacterPerk, EnvironmentState, GameClock, MultiSiteManager, PlayerProfile,
};

/// Minimum challenge level required for spot market pricing.
/// Level 1 (starter) keeps fixed TOU export rates.
const SPOT_MARKET_MIN_CHALLENGE_LEVEL: u8 = 2;

/// Tick each site's wholesale spot market price.
///
/// Runs before `utility_billing_system` so the spot price is fresh when
/// export revenue is calculated.  Sites with `challenge_level < 2` are
/// skipped -- they keep the fixed TOU export rates.
pub fn spot_market_system(
    mut multi_site: ResMut<MultiSiteManager>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    environment: Res<EnvironmentState>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();
    if delta_game_seconds <= 0.0 {
        return;
    }

    let day_length = 86400.0_f32;
    let day_fraction = (game_clock.game_time % day_length) / day_length;
    let weather_multiplier = environment.current_weather.spot_price_multiplier();
    let game_time = game_clock.game_time;

    let mut rng = rand::rng();

    for (_site_id, site_state) in multi_site.owned_sites.iter_mut() {
        if site_state.challenge_level < SPOT_MARKET_MIN_CHALLENGE_LEVEL {
            continue;
        }

        site_state.spot_market.tick(
            day_fraction,
            weather_multiplier,
            delta_game_seconds,
            game_time,
            &mut rng,
        );
    }
}

/// Utility billing system - tracks consumption and accumulates costs on the
/// site's utility meter / pending fields. Dollar amounts are flushed to the
/// ledger at day-end (not per-tick) to avoid f32-vs-Decimal rounding drift.
pub fn utility_billing_system(
    mut multi_site: ResMut<MultiSiteManager>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    profile: Res<PlayerProfile>,
    chargers: Query<(&Charger, &BelongsToSite)>,
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

            // Accumulate on meter (dollar total flushed to ledger at day-end)
            site_state
                .utility_meter
                .add_energy(energy_kwh, tou_period, rate);
        }

        // Solar export: accumulate on meter (flushed to ledger at day-end)
        let export_kw = site_state.grid_import.export_kw;
        if export_kw > 0.0 {
            let export_kwh = export_kw * (delta_game_seconds / 3600.0);
            let export_rate = if site_state.challenge_level >= SPOT_MARKET_MIN_CHALLENGE_LEVEL {
                site_state.spot_market.current_price_per_kwh
            } else {
                site_state
                    .site_energy_config
                    .current_export_rate(game_clock.game_time)
            };

            site_state.utility_meter.add_export(export_kwh, export_rate);
        }

        // Update demand charge based on current peak
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

        // Accumulate opex on site (flushed to ledger at day-end)
        let hourly_opex = site_state.service_strategy.hourly_maintenance_cost()
            + site_state.service_strategy.amenity_cost_per_hour();
        let opex_this_tick = hourly_opex * (delta_game_seconds / 3600.0);
        if opex_this_tick > 0.0 {
            site_state.pending_opex += opex_this_tick;
        }

        // Accumulate warranty on site (flushed to ledger at day-end)
        let warranty_hourly = site_state
            .service_strategy
            .hourly_warranty_cost_for_chargers(
                chargers
                    .iter()
                    .filter(|(_, b)| b.site_id == *_site_id)
                    .map(|(c, _)| (c.charger_type, c.rated_power_kw)),
            );
        let warranty_this_tick = warranty_hourly * (delta_game_seconds / 3600.0);
        if warranty_this_tick > 0.0 {
            site_state.pending_warranty += warranty_this_tick;
        }
    }
}
