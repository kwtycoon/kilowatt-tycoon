//! Tests for the energy simulation system.
//!
//! These tests verify:
//! - FCFS power allocation
//! - TOU period calculation

#![allow(clippy::field_reassign_with_default)]
//! - Demand peak tracking
//! - Solar generation
//! - BESS peak shaving
//! - Charger tiers and reliability

use kilowatt_tycoon::components::charger::{Charger, ChargerTier};
use kilowatt_tycoon::resources::{
    BessState, GridImport, SiteEnergyConfig, SolarState, TouPeriod, UtilityMeter,
};

// ============ SiteEnergyConfig Tests ============

#[test]
fn test_site_energy_config_defaults() {
    let config = SiteEnergyConfig::default();

    assert!(config.day_length_game_seconds > 0.0);
    assert!(config.demand_window_seconds > 0.0);
    assert!(config.off_peak_rate > 0.0);
    assert!(config.on_peak_rate > config.off_peak_rate);
    assert!(config.demand_rate_per_kw > 0.0);
}

#[test]
fn test_tou_period_calculation() {
    let config = SiteEnergyConfig::default();

    // Off-peak at start of day (midnight)
    let period = config.current_tou_period(0.0);
    assert_eq!(period, TouPeriod::OffPeak);

    // On-peak at midday (half of day length)
    let midday = config.day_length_game_seconds * 0.5;
    let period = config.current_tou_period(midday);
    assert_eq!(period, TouPeriod::OnPeak);

    // Off-peak late at night (near end of day)
    let late_night = config.day_length_game_seconds * 0.95;
    let period = config.current_tou_period(late_night);
    assert_eq!(period, TouPeriod::OffPeak);
}

#[test]
fn test_current_rate() {
    let config = SiteEnergyConfig::default();

    // Off-peak rate at midnight
    let rate = config.current_rate(0.0);
    assert_eq!(rate, config.off_peak_rate);

    // On-peak rate at midday
    let rate = config.current_rate(config.day_length_game_seconds * 0.5);
    assert_eq!(rate, config.on_peak_rate);
}

#[test]
fn test_solar_generation_factor() {
    let config = SiteEnergyConfig::default();

    // No generation at night (early morning)
    let night = config.day_length_game_seconds * 0.1;
    assert_eq!(config.solar_generation_factor(night), 0.0);

    // Peak generation at noon
    let noon = config.day_length_game_seconds * 0.5;
    let factor = config.solar_generation_factor(noon);
    assert!(factor > 0.9);

    // No generation at night (late evening)
    let evening = config.day_length_game_seconds * 0.9;
    assert_eq!(config.solar_generation_factor(evening), 0.0);
}

// ============ UtilityMeter Tests ============

#[test]
fn test_utility_meter_defaults() {
    let meter = UtilityMeter::default();

    assert_eq!(meter.current_avg_kw, 0.0);
    assert_eq!(meter.peak_demand_kw, 0.0);
    assert_eq!(meter.off_peak_kwh, 0.0);
    assert_eq!(meter.on_peak_kwh, 0.0);
}

#[test]
fn test_utility_meter_demand_tracking() {
    let mut meter = UtilityMeter::default();
    let window = 900.0; // 15 minutes

    // Add samples
    meter.add_sample(0.0, 100.0, window);
    meter.add_sample(100.0, 150.0, window);
    meter.add_sample(200.0, 200.0, window);

    // Average should be calculated
    assert!(meter.current_avg_kw > 0.0);

    // Peak should track highest average
    assert!(meter.peak_demand_kw > 0.0);
}

#[test]
fn test_utility_meter_rolling_window() {
    let mut meter = UtilityMeter::default();
    let window = 100.0; // Short window for testing

    // Add sample at t=0
    meter.add_sample(0.0, 100.0, window);
    assert!(meter.current_avg_kw > 0.0);

    // Add sample outside window at t=200
    meter.add_sample(200.0, 50.0, window);

    // Old sample should be dropped, only new sample counts
    assert!((meter.current_avg_kw - 50.0).abs() < 0.1);
}

#[test]
fn test_utility_meter_peak_demand() {
    let mut meter = UtilityMeter::default();
    let window = 900.0;

    // Add increasing samples
    for i in 0..10 {
        meter.add_sample(i as f32 * 100.0, (i + 1) as f32 * 50.0, window);
    }

    // Peak should track the highest average
    assert!(meter.peak_demand_kw > 0.0);
}

#[test]
fn test_utility_meter_energy_tracking() {
    let mut meter = UtilityMeter::default();

    meter.add_energy(10.0, TouPeriod::OffPeak, 0.12);
    meter.add_energy(5.0, TouPeriod::OnPeak, 0.28);

    assert_eq!(meter.off_peak_kwh, 10.0);
    assert_eq!(meter.on_peak_kwh, 5.0);
    assert_eq!(meter.total_imported_kwh(), 15.0);

    // Cost should be calculated
    let expected_cost = 10.0 * 0.12 + 5.0 * 0.28;
    assert!((meter.total_energy_cost - expected_cost).abs() < 0.01);
}

#[test]
fn test_utility_meter_demand_charge() {
    let mut meter = UtilityMeter::default();
    meter.peak_demand_kw = 100.0;

    let demand_rate = 15.0;
    meter.update_demand_charge(demand_rate, 1.0);

    // Demand charge is projected daily portion of the monthly bill.
    // 100 kW * $15/kW / 30 days = $50/day
    assert!((meter.demand_charge - 50.0).abs() < 0.001);
}

#[test]
fn test_utility_meter_total_cost() {
    let mut meter = UtilityMeter::default();

    meter.add_energy(100.0, TouPeriod::OnPeak, 0.20);
    meter.peak_demand_kw = 50.0;
    meter.update_demand_charge(10.0, 1.0);

    let total = meter.total_cost();
    // Energy: 100 kWh * $0.20 = $20
    // Demand: 50 kW * $10/kW / 30 days = $16.666...
    assert!((total - (20.0 + 50.0 * 10.0 / 30.0)).abs() < 0.001);
}

#[test]
fn test_utility_meter_reset() {
    let mut meter = UtilityMeter::default();

    meter.add_energy(100.0, TouPeriod::OnPeak, 0.20);
    meter.peak_demand_kw = 50.0;

    meter.reset();

    assert_eq!(meter.off_peak_kwh, 0.0);
    assert_eq!(meter.on_peak_kwh, 0.0);
    assert_eq!(meter.peak_demand_kw, 0.0);
}

// ============ SolarState Tests ============

#[test]
fn test_solar_state_defaults() {
    let solar = SolarState::default();

    assert_eq!(solar.installed_kw_peak, 0.0);
    assert_eq!(solar.current_generation_kw, 0.0);
}

#[test]
fn test_solar_add_capacity() {
    let mut solar = SolarState::default();

    solar.add_capacity(25.0);
    assert_eq!(solar.installed_kw_peak, 25.0);

    solar.add_capacity(25.0);
    assert_eq!(solar.installed_kw_peak, 50.0);
}

#[test]
fn test_solar_remove_capacity() {
    let mut solar = SolarState::default();
    solar.installed_kw_peak = 100.0;

    solar.remove_capacity(30.0);
    assert_eq!(solar.installed_kw_peak, 70.0);

    // Should clamp to zero
    solar.remove_capacity(100.0);
    assert_eq!(solar.installed_kw_peak, 0.0);
}

#[test]
fn test_solar_generation() {
    let mut solar = SolarState::default();
    solar.installed_kw_peak = 100.0;

    // Full generation
    solar.update_generation(1.0);
    assert_eq!(solar.current_generation_kw, 100.0);

    // Partial generation
    solar.update_generation(0.5);
    assert_eq!(solar.current_generation_kw, 50.0);

    // No generation
    solar.update_generation(0.0);
    assert_eq!(solar.current_generation_kw, 0.0);
}

// ============ BessState Tests ============

#[test]
fn test_bess_state_defaults() {
    let bess = BessState::default();

    assert_eq!(bess.capacity_kwh, 0.0);
    assert_eq!(bess.soc_kwh, 0.0);
    assert!(bess.round_trip_efficiency > 0.0);
}

#[test]
fn test_bess_soc_percentage() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.soc_kwh = 50.0;

    assert_eq!(bess.soc_percent(), 50.0);

    bess.soc_kwh = 100.0;
    assert_eq!(bess.soc_percent(), 100.0);

    bess.soc_kwh = 0.0;
    assert_eq!(bess.soc_percent(), 0.0);
}

#[test]
fn test_bess_available_discharge() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.soc_kwh = 75.0;

    assert_eq!(bess.available_discharge_kwh(), 75.0);
}

#[test]
fn test_bess_available_charge() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.soc_kwh = 75.0;

    assert_eq!(bess.available_charge_kwh(), 25.0);
}

#[test]
fn test_bess_discharge() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.max_discharge_kw = 50.0;
    bess.soc_kwh = 80.0;

    // Discharge 10 kWh over 1 hour
    let discharged = bess.discharge(10.0, 3600.0);

    assert!((discharged - 10.0).abs() < 0.1);
    assert!((bess.soc_kwh - 70.0).abs() < 0.1);
    assert!(bess.total_discharged_kwh > 0.0);
}

#[test]
fn test_bess_discharge_limited_by_soc() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.max_discharge_kw = 50.0;
    bess.soc_kwh = 5.0;

    // Try to discharge more than available
    let discharged = bess.discharge(10.0, 3600.0);

    // Should only discharge what's available
    assert!((discharged - 5.0).abs() < 0.1);
    assert!(bess.soc_kwh < 0.1);
}

#[test]
fn test_bess_charge() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.max_charge_kw = 50.0;
    bess.soc_kwh = 50.0;
    bess.round_trip_efficiency = 0.90;

    // Charge 10 kWh over 1 hour
    let charged = bess.charge(10.0, 3600.0);

    assert!(charged > 0.0);
    // SOC should increase by charged amount * efficiency
    assert!(bess.soc_kwh > 50.0);
    assert!(bess.total_charged_kwh > 0.0);
}

#[test]
fn test_bess_charge_limited_by_capacity() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.max_charge_kw = 50.0;
    bess.soc_kwh = 98.0;
    bess.round_trip_efficiency = 0.90;

    // Try to charge more than headroom
    let _charged = bess.charge(10.0, 3600.0);

    // Should only charge what fits
    assert!(bess.soc_kwh <= 100.0);
}

#[test]
fn test_bess_add_capacity() {
    let mut bess = BessState::default();

    bess.add_capacity(100.0, 50.0);

    assert_eq!(bess.capacity_kwh, 100.0);
    assert_eq!(bess.max_charge_kw, 50.0);
    assert_eq!(bess.max_discharge_kw, 50.0);
    // Should start at 50% SOC
    assert_eq!(bess.soc_kwh, 50.0);
}

#[test]
fn test_bess_remove_capacity() {
    let mut bess = BessState::default();
    bess.add_capacity(100.0, 50.0);

    bess.remove_capacity(50.0, 25.0);

    assert_eq!(bess.capacity_kwh, 50.0);
    assert_eq!(bess.max_charge_kw, 25.0);
    // SOC should be clamped
    assert!(bess.soc_kwh <= 50.0);
}

#[test]
fn test_bess_reset() {
    let mut bess = BessState::default();
    bess.capacity_kwh = 100.0;
    bess.soc_kwh = 10.0;
    bess.total_discharged_kwh = 50.0;
    bess.total_charged_kwh = 30.0;

    bess.reset();

    assert_eq!(bess.soc_kwh, 50.0); // Back to 50%
    assert_eq!(bess.total_discharged_kwh, 0.0);
    assert_eq!(bess.total_charged_kwh, 0.0);
}

// ============ GridImport Tests ============

#[test]
fn test_grid_import_defaults() {
    let grid = GridImport::default();

    assert_eq!(grid.current_kw, 0.0);
    assert_eq!(grid.gross_load_kw, 0.0);
}

#[test]
fn test_grid_import_calculation() {
    let mut grid = GridImport::default();

    grid.gross_load_kw = 200.0;
    grid.solar_kw = 50.0;
    grid.bess_kw = 25.0; // Discharging

    grid.calculate();

    // Import = gross - solar - bess discharge
    assert_eq!(grid.current_kw, 125.0);
}

#[test]
fn test_grid_import_with_bess_charging() {
    let mut grid = GridImport::default();

    grid.gross_load_kw = 100.0;
    grid.solar_kw = 0.0;
    grid.bess_kw = -25.0; // Charging (negative)

    grid.calculate();

    // Import = gross - solar - bess (negative bess adds to import)
    assert_eq!(grid.current_kw, 125.0);
}

#[test]
fn test_grid_import_clamps_to_zero() {
    let mut grid = GridImport::default();

    grid.gross_load_kw = 50.0;
    grid.solar_kw = 100.0; // More solar than load
    grid.bess_kw = 0.0;

    grid.calculate();

    // Should clamp to zero (no export modeled)
    assert_eq!(grid.current_kw, 0.0);
}

// ============ ChargerTier Tests ============

#[test]
fn test_charger_tier_mtbf() {
    // Value has lower MTBF (fails more often)
    assert!(ChargerTier::Value.mtbf_hours() < ChargerTier::Standard.mtbf_hours());
    assert!(ChargerTier::Standard.mtbf_hours() < ChargerTier::Premium.mtbf_hours());
}

#[test]
fn test_charger_tier_efficiency() {
    // Value has lower efficiency
    assert!(ChargerTier::Value.efficiency() < ChargerTier::Standard.efficiency());
    assert!(ChargerTier::Standard.efficiency() < ChargerTier::Premium.efficiency());
}

#[test]
fn test_charger_tier_jam_multiplier() {
    // Value has higher jam multiplier
    assert!(ChargerTier::Value.jam_multiplier() > ChargerTier::Standard.jam_multiplier());
    assert!(ChargerTier::Standard.jam_multiplier() > ChargerTier::Premium.jam_multiplier());
}

#[test]
fn test_charger_tier_action_success_bonus() {
    // Value has negative bonus
    assert!(ChargerTier::Value.action_success_bonus() < 0.0);
    assert_eq!(ChargerTier::Standard.action_success_bonus(), 0.0);
    assert!(ChargerTier::Premium.action_success_bonus() > 0.0);
}

#[test]
fn test_charger_derated_power_with_tier() {
    let mut charger = Charger::default();
    charger.rated_power_kw = 100.0;
    charger.health = 1.0;

    // Value tier has lower efficiency
    charger.tier = ChargerTier::Value;
    let value_power = charger.get_derated_power();

    charger.tier = ChargerTier::Premium;
    let premium_power = charger.get_derated_power();

    assert!(value_power < premium_power);
}

#[test]
fn test_charger_effective_jam_chance() {
    let mut charger = Charger::default();
    charger.connector_jam_chance = 0.10;

    charger.tier = ChargerTier::Value;
    let value_chance = charger.effective_jam_chance();

    charger.tier = ChargerTier::Premium;
    let premium_chance = charger.effective_jam_chance();

    assert!(value_chance > premium_chance);
}

#[test]
fn test_charger_fault_probability() {
    let mut charger = Charger::default();

    charger.tier = ChargerTier::Value;
    let value_prob = charger.fault_probability(1.0);

    charger.tier = ChargerTier::Premium;
    let premium_prob = charger.fault_probability(1.0);

    // Value tier should have higher fault probability
    assert!(value_prob > premium_prob);
}

#[test]
fn test_charger_fault_probability_zero_delta() {
    let charger = Charger::default();

    // Zero delta should give zero probability
    assert_eq!(charger.fault_probability(0.0), 0.0);
}
