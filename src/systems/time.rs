//! Time management system

use bevy::prelude::*;

use crate::components::driver::{Driver, DriverState, MovementPhase, VehicleMovement};
use crate::components::power::Transformer;
use crate::resources::{GameClock, GameState, MultiSiteManager};
use crate::states::AppState;
use crate::systems::power::EmergencyFiretruck;

/// Tick down demand boost timers for all sites.
pub fn tick_demand_boosts(
    time: Res<Time>,
    game_clock: Res<GameClock>,
    mut multi_site: ResMut<MultiSiteManager>,
) {
    let delta_game_secs = time.delta_secs() * game_clock.speed.multiplier();
    if delta_game_secs <= 0.0 {
        return;
    }
    for site_state in multi_site.owned_sites.values_mut() {
        site_state.site_upgrades.tick_demand_boost(delta_game_secs);
    }
}

/// Advance game time based on speed multiplier
pub fn time_system(time: Res<Time>, mut game_clock: ResMut<GameClock>, game_state: Res<GameState>) {
    // Don't advance time if game has ended
    if game_state.result.is_ended() {
        return;
    }

    game_clock.tick(time.delta_secs());

    // When the day timer completes, begin the wind-down phase.
    // Charging sessions are ended immediately by `day_ending_system` and vehicles
    // drive off the map. The actual DayEnd transition happens once all drivers have exited.
    if game_clock.is_day_complete() && !game_clock.day_ending {
        info!(
            "Day {} timer complete at game_time {:.0} — winding down",
            game_clock.day, game_clock.game_time
        );
        game_clock.day_ending = true;
        game_clock.day_ending_since = game_clock.total_real_time;
        // Snap the clock to 23:59 so the HUD shows 11:59 PM during wind-down.
        // (tick() will no longer advance game_time while day_ending is true.)
        game_clock.game_time = 86340.0; // 23h 59m 0s
    }
}

/// Maximum real-time seconds the wind-down phase can last before force-transitioning
/// to DayEnd. Sized to accommodate transformer fire sequences (firetruck travel +
/// 10 s spray + return) on top of normal driver departure.
const MAX_WIND_DOWN_SECS: f32 = 60.0;

/// Manages the end-of-day wind-down phase.
///
/// When `day_ending` is true this system:
/// 1. Ends all active charging sessions immediately, crediting partial revenue.
/// 2. Kicks non-charging drivers (queued, waiting, frustrated, arrived) so they depart.
/// 3. Monitors remaining drivers — once all have exited (or none remain), transitions to `DayEnd`.
/// 4. Force-transitions after [`MAX_WIND_DOWN_SECS`] real seconds even if drivers remain.
pub fn day_ending_system(
    mut commands: Commands,
    game_clock: Res<GameClock>,
    mut next_state: ResMut<NextState<AppState>>,
    mut drivers: Query<(
        &mut Driver,
        &VehicleMovement,
        &crate::components::BelongsToSite,
    )>,
    mut chargers: Query<&mut crate::components::charger::Charger>,
    mut multi_site: ResMut<MultiSiteManager>,
    mut game_state: ResMut<GameState>,
    mut transformers: Query<&mut Transformer>,
    firetrucks: Query<Entity, With<EmergencyFiretruck>>,
) {
    if !game_clock.day_ending {
        return;
    }

    let wind_down_elapsed = game_clock.total_real_time - game_clock.day_ending_since;
    let force_end = wind_down_elapsed >= MAX_WIND_DOWN_SECS;

    let mut any_remaining = false;

    for (mut driver, movement, belongs) in &mut drivers {
        match driver.state {
            // End active charging sessions — credit partial revenue and depart
            DriverState::Charging => {
                if let Some(charger_entity) = driver.assigned_charger
                    && let Ok(mut charger) = chargers.get_mut(charger_entity)
                {
                    let price_per_kwh = multi_site
                        .get_site(belongs.site_id)
                        .map(|site| {
                            site.service_strategy.pricing.effective_price(
                                game_clock.game_time,
                                &site.site_energy_config,
                                site.charger_utilization,
                            )
                        })
                        .unwrap_or(0.0);

                    let oem_recovery = multi_site
                        .get_site(belongs.site_id)
                        .map(|s| s.site_upgrades.oem_tier.reliability_recovery_multiplier())
                        .unwrap_or(1.0);

                    let revenue = driver.charge_received_kwh * price_per_kwh;

                    // Credit partial revenue
                    if revenue > 0.0 {
                        game_state.add_charging_revenue(revenue);
                    }
                    // Flush accumulated ad revenue for this session
                    if charger.pending_ad_revenue > 0.0 {
                        game_state.add_ad_revenue(charger.pending_ad_revenue);
                        charger.pending_ad_revenue = 0.0;
                    }
                    game_state.sessions_completed += 1;
                    game_state.change_reputation(2);

                    // Update charger KPIs
                    charger.total_energy_delivered_kwh += driver.charge_received_kwh;
                    charger.energy_delivered_kwh_today += driver.charge_received_kwh;
                    charger.session_count += 1;
                    charger.total_revenue += revenue;
                    charger.recover_reliability_session(oem_recovery);

                    // Clear charger charging state
                    charger.is_charging = false;
                    charger.current_power_kw = 0.0;
                    charger.requested_power_kw = 0.0;
                    charger.allocated_power_kw = 0.0;
                    charger.session_start_game_time = None;

                    // Achievement tracking: cumulative energy delivered
                    game_state.total_energy_delivered_kwh += driver.charge_received_kwh;

                    // Track per-site stats
                    if let Some(site_state) = multi_site.get_site_mut(belongs.site_id) {
                        site_state.total_revenue += revenue;
                        site_state.total_sessions += 1;
                        site_state.sessions_today += 1;
                        site_state.energy_delivered_kwh_today += driver.charge_received_kwh;
                    }

                    info!(
                        "Day ending: {} partial session — {:.1} kWh, ${:.2}",
                        driver.id, driver.charge_received_kwh, revenue
                    );
                }
                driver.state = DriverState::Leaving;
                any_remaining = true;
            }

            // Queued / waiting / frustrated drivers leave immediately
            DriverState::Queued | DriverState::WaitingForCharger | DriverState::Frustrated => {
                driver.state = DriverState::Leaving;
                any_remaining = true;
            }

            // Arriving drivers that have parked leave immediately; in-transit ones
            // will be caught on the next frame once they park.
            DriverState::Arriving => {
                if movement.phase == MovementPhase::Parked {
                    driver.state = DriverState::Leaving;
                }
                any_remaining = true;
            }

            // Completed drivers are transitioning to departure — let them continue
            DriverState::Complete => {
                any_remaining = true;
            }

            // Already departing — just wait for them to exit
            DriverState::Leaving | DriverState::LeftAngry => {
                if movement.phase != MovementPhase::Exited {
                    any_remaining = true;
                }
            }
        }
    }

    if let Some(site) = multi_site.active_site_mut() {
        site.charger_queue.clear();
    }

    // Block day transition while the fire sequence is still playing out
    let fire_active = transformers.iter().any(|t| t.on_fire);
    let firetrucks_active = !firetrucks.is_empty();
    if fire_active || firetrucks_active {
        any_remaining = true;
    }

    if !any_remaining {
        info!(
            "Day {} wind-down complete — all drivers have left, transitioning to DayEnd",
            game_clock.day
        );
        next_state.set(AppState::DayEnd);
    } else if force_end {
        // Force-resolve any active fires so the day doesn't stall indefinitely
        for mut transformer in &mut transformers {
            if transformer.on_fire {
                transformer.on_fire = false;
                transformer.destroyed = true;
                transformer.firetruck_dispatched = false;
                transformer.current_temp_c = transformer.ambient_temp_c + 10.0;
                transformer.overload_seconds = 0.0;
                warn!(
                    "Force-resolved fire on transformer at {:?} during day end",
                    transformer.grid_pos
                );
            }
        }
        for entity in &firetrucks {
            commands.entity(entity).try_despawn();
        }

        warn!(
            "Day {} wind-down force-ended after {:.0}s — transitioning to DayEnd",
            game_clock.day, wind_down_elapsed
        );
        next_state.set(AppState::DayEnd);
    }
}
