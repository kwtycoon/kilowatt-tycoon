//! Utility billing system - tracks energy costs (TOU) and demand charges.
//!
//! This system runs after power dispatch and updates the utility meter with:
//! - Energy consumption by TOU period
//! - Rolling 15-minute demand average
//! - Peak demand tracking
//! - Cost application to game state
//! - Grid event multipliers on import/export rates (level 2+ sites)

use bevy::prelude::*;

use crate::components::BelongsToSite;
use crate::components::charger::Charger;
use crate::resources::{
    CharacterPerk, EnvironmentState, GameClock, ImageAssets, MultiSiteManager, PlayerProfile,
};

/// Minimum challenge level required for grid events.
/// Level 1 (starter) keeps fixed TOU rates with no events.
const GRID_EVENT_MIN_CHALLENGE_LEVEL: u8 = 2;

/// Tick grid event state for the viewed site.
///
/// Runs before `utility_billing_system` so event multipliers are fresh
/// when import/export costs are calculated. Sites with `challenge_level < 2`
/// are skipped -- they keep plain TOU rates.
pub fn grid_event_system(
    mut commands: Commands,
    mut multi_site: ResMut<MultiSiteManager>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    image_assets: Res<ImageAssets>,
    environment: Res<EnvironmentState>,
    toast_container: Single<Entity, With<crate::ui::toast::ToastContainer>>,
) {
    if game_clock.is_paused() {
        return;
    }

    let delta_game_seconds = time.delta_secs() * game_clock.speed.multiplier();
    if delta_game_seconds <= 0.0 {
        return;
    }

    let game_time = game_clock.game_time;
    let weather = environment.current_weather;

    let mut rng = rand::rng();

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };
    if site_state.challenge_level >= GRID_EVENT_MIN_CHALLENGE_LEVEL {
        let had_event = site_state.grid_events.active_event.is_some();
        let prev_event_type = site_state.grid_events.active_event;
        let prev_event_revenue = site_state.grid_events.current_event_revenue;

        site_state
            .grid_events
            .tick(site_state.challenge_level, game_time, weather, &mut rng);

        if !had_event && let Some(event) = site_state.grid_events.active_event {
            let has_pm = site_state.site_upgrades.has_power_management();
            crate::ui::toast::spawn_grid_event_toast(
                &mut commands,
                *toast_container,
                event,
                weather,
                game_clock.game_time,
                time.elapsed_secs(),
                image_assets.icon_bolt.clone(),
                has_pm,
            );
        }

        if had_event
            && site_state.grid_events.active_event.is_none()
            && let Some(prev) = prev_event_type
        {
            let event_name = prev.name();
            let msg = if prev_event_revenue > 0.01 {
                format!("{event_name} ended - you earned ${prev_event_revenue:.2} from the spike!")
            } else {
                format!("{event_name} ended.")
            };
            crate::ui::toast::spawn_grid_event_end_toast(
                &mut commands,
                *toast_container,
                &msg,
                game_clock.game_time,
                time.elapsed_secs(),
                image_assets.icon_bolt.clone(),
            );
        }
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

    let Some(viewed_id) = multi_site.viewed_site_id else {
        return;
    };
    let Some(site_state) = multi_site.owned_sites.get_mut(&viewed_id) else {
        return;
    };
    let _site_id = &viewed_id;

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
        let tou_period = site_state
            .site_energy_config
            .current_tou_period(game_clock.game_time);
        let base_rate = site_state
            .site_energy_config
            .current_rate(game_clock.game_time);
        let import_mult = site_state.grid_events.current_import_multiplier();
        let rate = base_rate * import_mult;

        site_state
            .utility_meter
            .add_energy(energy_kwh, tou_period, rate);

        // Track import surcharge cost during grid events
        if import_mult > 1.0 {
            let surcharge = energy_kwh * base_rate * (import_mult - 1.0);
            site_state.grid_events.event_import_surcharge_today += surcharge;
        }
    }

    // Solar export: accumulate on meter (flushed to ledger at day-end)
    let export_kw = site_state.grid_import.export_kw;
    if export_kw > 0.0 {
        let export_kwh = export_kw * (delta_game_seconds / 3600.0);
        let base_export_rate = site_state
            .site_energy_config
            .current_export_rate(game_clock.game_time);
        let export_mult = site_state.grid_events.current_export_multiplier();
        let export_rate = base_export_rate * export_mult;

        site_state.utility_meter.add_export(export_kwh, export_rate);

        // Track revenue earned during grid events for day-end breakdown
        if site_state.grid_events.active_event.is_some() {
            let event_tick_revenue = export_kwh * export_rate;
            site_state.grid_events.event_revenue_today += event_tick_revenue;
            site_state.grid_events.current_event_revenue += event_tick_revenue;
        }
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
