//! Demand warning system - monitors demand charges and emits player-facing alerts.
//!
//! Key design constraint: **do not spam**. Players don't care about every incremental new peak;
//! they care when demand charges become a meaningful slice of profit and what they can do about it.
//!
//! This system emits a `DemandBurdenEvent` (single alert surface) using **real-time** cooldowns
//! so game speed doesn't make UI unreadable.

use bevy::prelude::*;

use crate::events::DemandBurdenEvent;
use crate::resources::{GameClock, MultiSiteManager};

/// Demand share threshold (demand_charge / current_day_revenue) to warn the player.
const DEMAND_SHARE_THRESHOLD: f32 = 0.15; // 15% of current day revenue

/// Minimum projected daily demand charge ($) before warning (avoids early-session noise).
const MIN_DEMAND_CHARGE: f32 = 500.0;

/// Minimum real time between alerts.
const ALERT_COOLDOWN_REAL: f32 = 60.0;

/// Minimum change in share before emitting again.
const MIN_SHARE_DELTA: f32 = 0.03; // 3 percentage points

/// System to detect and emit demand-burden warnings (active site only).
pub fn monitor_demand_warnings(
    multi_site: Res<MultiSiteManager>,
    game_state: Res<crate::resources::GameState>,
    game_clock: Res<GameClock>,
    time: Res<Time>,
    mut demand_burden: MessageWriter<DemandBurdenEvent>,
    mut last_alert_time_real: Local<f32>,
    mut last_alert_share: Local<f32>,
) {
    if game_clock.is_paused() {
        return;
    }

    let Some(site_state) = multi_site.active_site() else {
        return;
    };

    let now_real = time.elapsed_secs();
    // Allow the very first alert immediately (Local<f32> defaults to 0.0).
    if *last_alert_time_real > 0.0 && (now_real - *last_alert_time_real) < ALERT_COOLDOWN_REAL {
        return;
    }

    let demand_charge = site_state.utility_meter.demand_charge;
    if demand_charge < MIN_DEMAND_CHARGE {
        return;
    }

    let energy_cost = site_state.utility_meter.total_energy_cost;
    let revenue_today = game_state.daily_history.current_day.total_revenue();
    // Use revenue as denominator to avoid nonsensical % when margin is ~0 or negative.
    let revenue = revenue_today.max(1.0);
    let demand_share = demand_charge / revenue;

    if demand_share < DEMAND_SHARE_THRESHOLD {
        return;
    }
    if (demand_share - *last_alert_share).abs() < MIN_SHARE_DELTA {
        return;
    }

    demand_burden.write(DemandBurdenEvent {
        site_id: site_state.id,
        demand_charge,
        energy_cost,
        revenue_today,
        margin: (revenue_today - energy_cost).max(1.0), // kept for potential future use
        demand_share,
        grid_kva: site_state.grid_import.current_kva,
        peak_kw: site_state.utility_meter.peak_demand_kw,
        demand_rate: site_state.site_energy_config.demand_rate_per_kw,
    });

    *last_alert_share = demand_share;
    *last_alert_time_real = now_real;
}
