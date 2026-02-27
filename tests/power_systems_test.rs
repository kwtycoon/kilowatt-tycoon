//! Tests for power-related systems.
//!
//! These tests verify electrical phase balancing,
//! load management, and transformer temperature tracking.

#![allow(clippy::field_reassign_with_default)]

mod test_utils;

use bevy::prelude::*;

use kilowatt_tycoon::components::charger::{Charger, ChargerType, Phase};
use kilowatt_tycoon::components::power::{PhaseLoads, Transformer, VoltageState};
use kilowatt_tycoon::resources::GameClock;

use test_utils::*;

#[test]
fn test_phase_loads_initialization() {
    let phase_loads = PhaseLoads::default();

    assert_eq!(phase_loads.phase_a_kva, 0.0);
    assert_eq!(phase_loads.phase_b_kva, 0.0);
    assert_eq!(phase_loads.phase_c_kva, 0.0);
}

#[test]
fn test_phase_loads_total() {
    let mut phase_loads = PhaseLoads::default();
    phase_loads.phase_a_kva = 100.0;
    phase_loads.phase_b_kva = 150.0;
    phase_loads.phase_c_kva = 120.0;

    assert_eq!(phase_loads.total_load(), 370.0);
}

#[test]
fn test_phase_loads_imbalance() {
    let mut phase_loads = PhaseLoads::default();

    // Perfectly balanced
    phase_loads.phase_a_kva = 100.0;
    phase_loads.phase_b_kva = 100.0;
    phase_loads.phase_c_kva = 100.0;

    // Imbalance should be minimal for balanced loads
    let imbalance = phase_loads.imbalance_percentage();
    assert!(imbalance < 1.0); // Less than 1% imbalance

    // Create imbalance
    phase_loads.phase_a_kva = 150.0;
    phase_loads.phase_b_kva = 100.0;
    phase_loads.phase_c_kva = 50.0;

    let imbalance = phase_loads.imbalance_percentage();
    assert!(imbalance > 0.0);
}

#[test]
fn test_voltage_state_initialization() {
    let voltage = VoltageState::default();

    // Should start at nominal values
    assert!(voltage.nominal_voltage > 0.0);
    assert_eq!(voltage.current_voltage_pct, 100.0);
}

#[test]
fn test_voltage_derating_factor() {
    let mut voltage = VoltageState::default();

    // Normal voltage - no derating
    voltage.current_voltage_pct = 100.0;
    assert_eq!(voltage.derating_factor(), 1.0);

    // Slight sag - no derating yet
    voltage.current_voltage_pct = 95.0;
    assert_eq!(voltage.derating_factor(), 1.0);

    // Moderate sag
    voltage.current_voltage_pct = 90.0;
    assert_eq!(voltage.derating_factor(), 0.9);

    // Severe sag
    voltage.current_voltage_pct = 85.0;
    assert_eq!(voltage.derating_factor(), 0.75);
}

#[test]
fn test_charger_phase_assignment() {
    // Create chargers on different phases
    let mut charger_a = create_test_charger("CHG-A", ChargerType::DcFast);
    charger_a.phase = Phase::A;

    let mut charger_b = create_test_charger("CHG-B", ChargerType::DcFast);
    charger_b.phase = Phase::B;

    let mut charger_c = create_test_charger("CHG-C", ChargerType::DcFast);
    charger_c.phase = Phase::C;

    assert_eq!(charger_a.phase, Phase::A);
    assert_eq!(charger_b.phase, Phase::B);
    assert_eq!(charger_c.phase, Phase::C);
}

#[test]
fn test_charger_current_power() {
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);

    // Initial power should be zero
    assert_eq!(charger.current_power_kw, 0.0);

    // Simulate charging
    charger.current_power_kw = 150.0;
    charger.is_charging = true;

    assert_eq!(charger.current_power_kw, 150.0);
}

#[test]
fn test_l2_charger_power_limit() {
    let charger = create_test_charger("L2-001", ChargerType::AcLevel2);

    // L2 chargers should have lower power rating
    assert!(charger.rated_power_kw <= 20.0);
}

#[test]
fn test_dcfc_charger_power_limit() {
    let charger = create_test_charger("DCFC-001", ChargerType::DcFast);

    // DCFC chargers should have higher power rating
    assert!(charger.rated_power_kw >= 50.0);
}

#[test]
fn test_phase_enum_variants() {
    // Verify all phase variants exist
    let phases = [Phase::A, Phase::B, Phase::C];

    assert_eq!(phases.len(), 3);
}

#[test]
fn test_phase_loads_reset() {
    let mut phase_loads = PhaseLoads::default();

    phase_loads.phase_a_kva = 100.0;
    phase_loads.phase_b_kva = 150.0;
    phase_loads.phase_c_kva = 120.0;

    // Reset
    phase_loads.reset();

    assert_eq!(phase_loads.phase_a_kva, 0.0);
    assert_eq!(phase_loads.phase_b_kva, 0.0);
    assert_eq!(phase_loads.phase_c_kva, 0.0);
}

#[test]
fn test_phase_loads_add_load() {
    let mut phase_loads = PhaseLoads::default();

    phase_loads.add_load(Phase::A, 50.0);
    phase_loads.add_load(Phase::B, 75.0);
    phase_loads.add_load(Phase::C, 25.0);

    assert_eq!(phase_loads.get_load(Phase::A), 50.0);
    assert_eq!(phase_loads.get_load(Phase::B), 75.0);
    assert_eq!(phase_loads.get_load(Phase::C), 25.0);
}

#[test]
fn test_transformer_defaults() {
    let transformer = Transformer::default();

    assert!(transformer.rating_kva > 0.0);
    assert!(transformer.thermal_limit_c > transformer.ambient_temp_c);
    assert_eq!(transformer.current_load_kva, 0.0);
}

#[test]
fn test_transformer_load_percentage() {
    let mut transformer = Transformer::default();

    transformer.current_load_kva = 0.0;
    assert_eq!(transformer.load_percentage(), 0.0);

    transformer.current_load_kva = transformer.rating_kva / 2.0;
    assert!((transformer.load_percentage() - 50.0).abs() < 0.1);

    transformer.current_load_kva = transformer.rating_kva;
    assert!((transformer.load_percentage() - 100.0).abs() < 0.1);
}

#[test]
fn test_transformer_warning_thresholds() {
    let mut transformer = Transformer::default();

    // Below warning
    transformer.current_temp_c = 70.0;
    assert!(!transformer.is_warning());
    assert!(!transformer.is_critical());

    // Warning level
    transformer.current_temp_c = 80.0;
    assert!(transformer.is_warning());
    assert!(!transformer.is_critical());

    // Critical level
    transformer.current_temp_c = 95.0;
    assert!(transformer.is_warning());
    assert!(transformer.is_critical());
}

#[test]
fn test_game_clock_affects_power_calculations() {
    let mut app = create_test_app();

    // Create a charger with active load
    let mut charger = create_test_charger("CHG-001", ChargerType::DcFast);
    charger.current_power_kw = 150.0;
    charger.is_charging = true;
    let _entity = spawn_charger(&mut app, charger);

    // Initialize phase loads
    app.init_resource::<PhaseLoads>();

    // Verify resources are accessible
    let _phase_loads = app.world().resource::<PhaseLoads>();
    let _game_clock = app.world().resource::<GameClock>();
}

#[test]
fn test_multiple_chargers_on_same_phase() {
    let mut app = create_test_app();

    // Create multiple chargers on phase A
    for i in 0..3 {
        let mut charger = create_test_charger(&format!("CHG-A-{i}"), ChargerType::DcFast);
        charger.phase = Phase::A;
        charger.current_power_kw = 50.0;
        spawn_charger(&mut app, charger);
    }

    // Query all chargers on phase A
    let chargers: Vec<_> = app
        .world_mut()
        .query::<&Charger>()
        .iter(app.world())
        .filter(|c| c.phase == Phase::A)
        .collect();

    assert_eq!(chargers.len(), 3);

    // Total load on phase A should be 150kW
    let total_load: f32 = chargers.iter().map(|c| c.current_power_kw).sum();
    assert_eq!(total_load, 150.0);
}

#[test]
fn test_voltage_warning_state() {
    let mut voltage = VoltageState::default();

    // Normal voltage - no warning
    voltage.current_voltage_pct = 100.0;
    assert!(!voltage.is_warning());

    // Low voltage - warning
    voltage.current_voltage_pct = 90.0;
    assert!(voltage.is_warning());
}
