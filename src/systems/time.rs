//! Time management system

use bevy::prelude::*;

use crate::components::driver::{Driver, DriverState, MovementPhase, VehicleMovement};
use crate::resources::{GameClock, GameState, MultiSiteManager};
use crate::states::AppState;

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
        // Snap the clock to 23:59 so the HUD shows 11:59 PM during wind-down.
        // (tick() will no longer advance game_time while day_ending is true.)
        game_clock.game_time = 86340.0; // 23h 59m 0s
    }
}

/// Manages the end-of-day wind-down phase.
///
/// When `day_ending` is true this system:
/// 1. Ends all active charging sessions immediately, crediting partial revenue.
/// 2. Kicks non-charging drivers (queued, waiting, frustrated, arrived) so they depart.
/// 3. Monitors remaining drivers — once all have exited (or none remain), transitions to `DayEnd`.
pub fn day_ending_system(
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
) {
    if !game_clock.day_ending {
        return;
    }

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
                        .map(|site| site.service_strategy.energy_price_kwh)
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
                    game_state.sessions_completed += 1;
                    game_state.change_reputation(2);

                    // Update charger KPIs
                    charger.total_energy_delivered_kwh += driver.charge_received_kwh;
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

    // Clear charger queues so the queue assignment system doesn't re-assign kicked drivers
    for site in multi_site.owned_sites.values_mut() {
        site.charger_queue.clear();
    }

    if !any_remaining {
        info!(
            "Day {} wind-down complete — all drivers have left, transitioning to DayEnd",
            game_clock.day
        );
        next_state.set(AppState::DayEnd);
    }
}
