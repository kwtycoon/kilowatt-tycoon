//! Fleet contract system -- commercial vehicle operators that send guaranteed
//! daily volume in exchange for a retainer and discounted per-kWh rate.
//!
//! See `spec/FLEET.md` for the full design.

use bevy::prelude::*;
use serde::Deserialize;

use crate::components::driver::VehicleType;
use crate::resources::multi_site::SiteArchetype;

/// A time window during which fleet vehicles arrive.
#[derive(Debug, Clone, Deserialize)]
pub struct FleetTimeWindow {
    pub start_hour: u32,
    pub end_hour: u32,
    pub vehicle_count: u32,
}

/// Serialisable color for JSON data files.
#[derive(Debug, Clone, Deserialize)]
pub struct SrgbColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl SrgbColor {
    pub fn to_bevy_color(&self) -> Color {
        Color::srgb(self.r, self.g, self.b)
    }
}

/// A fleet contract definition loaded from JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct FleetContractDef {
    pub id: String,
    pub company_name: String,
    pub company_color: SrgbColor,
    pub vehicle_types: Vec<VehicleType>,
    pub vehicles_per_day: u32,
    pub time_windows: Vec<FleetTimeWindow>,
    pub contracted_price_per_kwh: f32,
    pub daily_payment: f32,
    pub penalty_per_miss: f32,
    pub reputation_penalty_per_miss: i32,
    pub max_breaches_before_termination: u32,
    pub termination_fine: f32,
    pub site_archetype: SiteArchetype,
}

/// Runtime state for an accepted fleet contract.
#[derive(Debug, Clone)]
pub struct ActiveFleetContract {
    pub def: FleetContractDef,

    pub vehicles_spawned_today: u32,
    pub vehicles_charged_today: u32,
    pub vehicles_missed_today: u32,

    /// Per-window spawn tracking for the current day.
    pub window_spawned: Vec<u32>,

    /// Cumulative breaches across ALL days.
    pub breaches_total: u32,
    pub terminated: bool,
    pub day_accepted: u32,
}

impl ActiveFleetContract {
    pub fn new(def: FleetContractDef, day: u32) -> Self {
        let window_count = def.time_windows.len();
        Self {
            def,
            vehicles_spawned_today: 0,
            vehicles_charged_today: 0,
            vehicles_missed_today: 0,
            window_spawned: vec![0; window_count],
            breaches_total: 0,
            terminated: false,
            day_accepted: day,
        }
    }

    pub fn reset_daily(&mut self) {
        self.vehicles_spawned_today = 0;
        self.vehicles_charged_today = 0;
        self.vehicles_missed_today = 0;
        for count in &mut self.window_spawned {
            *count = 0;
        }
    }

    /// Returns `true` if this breach caused contract termination.
    pub fn record_breach(&mut self) -> bool {
        self.vehicles_missed_today += 1;
        self.breaches_total += 1;
        if self.breaches_total >= self.def.max_breaches_before_termination {
            self.terminated = true;
        }
        self.terminated
    }

    pub fn record_charged(&mut self) {
        self.vehicles_charged_today += 1;
    }

    pub fn breaches_remaining(&self) -> u32 {
        self.def
            .max_breaches_before_termination
            .saturating_sub(self.breaches_total)
    }

    pub fn retainer_earned(&self) -> f32 {
        if self.terminated {
            0.0
        } else {
            self.def.daily_payment
        }
    }

    pub fn penalties_today(&self) -> f32 {
        self.vehicles_missed_today as f32 * self.def.penalty_per_miss
    }

    pub fn company_color(&self) -> Color {
        self.def.company_color.to_bevy_color()
    }
}

/// Central resource managing all fleet contracts for the session.
///
/// Persists across `DayEnd` -> `Playing` transitions.
#[derive(Resource, Debug, Clone, Default)]
pub struct FleetContractManager {
    pub available: Vec<FleetContractDef>,
    pub active: Vec<ActiveFleetContract>,
    pub offer_shown_today: bool,
}

impl FleetContractManager {
    pub fn load_for_archetype(&mut self, defs: Vec<FleetContractDef>, archetype: SiteArchetype) {
        for def in defs.into_iter().filter(|d| d.site_archetype == archetype) {
            let already = self.active.iter().any(|a| a.def.id == def.id)
                || self.available.iter().any(|a| a.id == def.id);
            if !already {
                self.available.push(def);
            }
        }
    }

    pub fn accept_contract(&mut self, contract_id: &str, day: u32) -> bool {
        if let Some(idx) = self.available.iter().position(|d| d.id == contract_id) {
            let def = self.available.remove(idx);
            self.active.push(ActiveFleetContract::new(def, day));
            true
        } else {
            false
        }
    }

    pub fn reset_daily(&mut self) {
        self.offer_shown_today = false;
        for contract in &mut self.active {
            contract.reset_daily();
        }
    }

    pub fn has_offers(&self) -> bool {
        !self.available.is_empty()
    }

    pub fn has_active_contracts(&self) -> bool {
        self.active.iter().any(|c| !c.terminated)
    }

    pub fn total_retainer_today(&self) -> f32 {
        self.active.iter().map(|c| c.retainer_earned()).sum()
    }

    pub fn total_penalties_today(&self) -> f32 {
        self.active.iter().map(|c| c.penalties_today()).sum()
    }
}

/// Marker component for fleet vehicles.
#[derive(Component, Debug, Clone)]
pub struct FleetVehicle {
    pub contract_id: String,
    pub company_color: Color,
}

/// Marker for the floating fleet badge sprite.
#[derive(Component, Debug)]
pub struct FleetBadge;

/// Toggles the bright debug "FLEET" labels above all fleet vehicles (F5).
#[derive(Resource, Debug, Default)]
pub struct FleetDebugMode {
    pub active: bool,
}

/// Marker for the debug text label spawned above fleet vehicles.
#[derive(Component, Debug)]
pub struct FleetDebugLabel;

/// Event fired when a fleet contract is terminated.
#[derive(Event, Message, Debug, Clone)]
pub struct FleetContractTerminatedEvent {
    pub contract_id: String,
    pub company_name: String,
    pub breaches_total: u32,
    pub termination_fine: f32,
}

/// Marker for the fleet contract offer banner UI.
#[derive(Component, Debug)]
pub struct FleetOfferBanner;

/// Marker for the accept button on the fleet offer banner.
#[derive(Component, Debug)]
pub struct FleetOfferAcceptButton {
    pub contract_id: String,
}

/// Marker for the decline button on the fleet offer banner.
#[derive(Component, Debug)]
pub struct FleetOfferDeclineButton;

/// Load built-in fleet contract definitions from embedded JSON.
pub fn builtin_fleet_contracts() -> Vec<FleetContractDef> {
    let metro: FleetContractDef = serde_json::from_str(include_str!(
        "../../assets/data/fleets/metro_transit.fleet.json"
    ))
    .expect("metro_transit.fleet.json should be valid");
    let grab: FleetContractDef = serde_json::from_str(include_str!(
        "../../assets/data/fleets/grabfood_saigon.fleet.json"
    ))
    .expect("grabfood_saigon.fleet.json should be valid");
    vec![metro, grab]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_def() -> FleetContractDef {
        FleetContractDef {
            id: "test_fleet".into(),
            company_name: "Test Fleet Co".into(),
            company_color: SrgbColor {
                r: 0.2,
                g: 0.4,
                b: 0.8,
            },
            vehicle_types: vec![VehicleType::Bus],
            vehicles_per_day: 4,
            time_windows: vec![FleetTimeWindow {
                start_hour: 5,
                end_hour: 7,
                vehicle_count: 4,
            }],
            contracted_price_per_kwh: 0.25,
            daily_payment: 8000.0,
            penalty_per_miss: 500.0,
            reputation_penalty_per_miss: 5,
            max_breaches_before_termination: 10,
            termination_fine: 1000.0,
            site_archetype: SiteArchetype::FleetDepot,
        }
    }

    #[test]
    fn test_accept_contract() {
        let mut mgr = FleetContractManager::default();
        mgr.available.push(sample_def());
        assert!(mgr.has_offers());
        assert!(!mgr.has_active_contracts());
        assert!(mgr.accept_contract("test_fleet", 1));
        assert!(!mgr.has_offers());
        assert!(mgr.has_active_contracts());
    }

    #[test]
    fn test_breach_tracking_across_days() {
        let mut contract = ActiveFleetContract::new(sample_def(), 1);
        assert_eq!(contract.breaches_remaining(), 10);

        for _ in 0..9 {
            assert!(!contract.record_breach());
        }
        assert_eq!(contract.breaches_remaining(), 1);

        contract.reset_daily();
        assert_eq!(contract.breaches_total, 9);
        assert_eq!(contract.vehicles_missed_today, 0);

        assert!(contract.record_breach());
        assert!(contract.terminated);
    }

    #[test]
    fn test_retainer_zero_when_terminated() {
        let mut contract = ActiveFleetContract::new(sample_def(), 1);
        assert_eq!(contract.retainer_earned(), 8000.0);
        contract.terminated = true;
        assert_eq!(contract.retainer_earned(), 0.0);
    }

    #[test]
    fn test_builtin_contracts_parse() {
        let defs = builtin_fleet_contracts();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].id, "metro_transit");
        assert_eq!(defs[1].id, "grabfood_saigon");
    }
}
