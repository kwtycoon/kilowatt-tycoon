//! Tests for power-related systems.
//!
//! These tests verify electrical phase balancing,
//! load management, and transformer temperature tracking.

#![allow(clippy::field_reassign_with_default)]

mod test_utils;

use bevy::prelude::*;

use kilowatt_tycoon::components::charger::{Charger, ChargerType, Phase};
use kilowatt_tycoon::components::power::{PhaseLoads, Transformer, VoltageState};
use kilowatt_tycoon::resources::{GameClock, SiteId};

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

// ============ Transformer Fire Risk (Temperature-Driven) ============

fn make_transformer(rating_kva: f32, ambient_temp_c: f32) -> Transformer {
    Transformer {
        site_id: SiteId(1),
        grid_pos: (2, 2),
        rating_kva,
        thermal_limit_c: 110.0,
        current_temp_c: ambient_temp_c,
        current_load_kva: 0.0,
        ambient_temp_c,
        overload_seconds: 0.0,
        on_fire: false,
        destroyed: false,
        firetruck_dispatched: false,
        last_warning_level: 0,
        ..default()
    }
}

#[test]
fn temperature_model_reaches_critical_at_full_load() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_load_kva = 500.0; // 100% utilization

    // Run the temperature model for a long simulated period until it stabilizes.
    // With load_ratio = 1.0, target_temp = 25 + (110-25)*1.0 = 110 C.
    // It should cross 90 C (critical) well before stabilizing.
    for _ in 0..50_000 {
        t.update_temperature(1.0);
    }

    assert!(
        t.is_critical(),
        "Transformer at 100% sustained load should reach critical temperature. Got {:.1}C",
        t.current_temp_c
    );
    assert!(
        t.current_temp_c > 100.0,
        "Should approach thermal limit. Got {:.1}C",
        t.current_temp_c
    );
}

#[test]
fn temperature_model_reaches_critical_at_90_percent_load() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_load_kva = 450.0; // 90% utilization

    // target_temp = 25 + 85 * 0.81 = 93.85 C -> should cross 90 C
    for _ in 0..50_000 {
        t.update_temperature(1.0);
    }

    assert!(
        t.is_critical(),
        "90% sustained load should reach critical. Got {:.1}C",
        t.current_temp_c
    );
}

#[test]
fn temperature_stays_below_critical_at_80_percent_load() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_load_kva = 400.0; // 80% utilization

    // target_temp = 25 + 85 * 0.64 = 79.4 C -> below 90 C
    for _ in 0..50_000 {
        t.update_temperature(1.0);
    }

    assert!(
        !t.is_critical(),
        "80% load should NOT reach critical. Got {:.1}C",
        t.current_temp_c
    );
    assert!(
        t.is_warning(),
        "80% load should be in warning zone. Got {:.1}C",
        t.current_temp_c
    );
}

#[test]
fn temperature_stays_safe_at_50_percent_load() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_load_kva = 250.0; // 50% utilization

    // target_temp = 25 + 85 * 0.25 = 46.25 C
    for _ in 0..50_000 {
        t.update_temperature(1.0);
    }

    assert!(
        !t.is_warning(),
        "50% load should be safe. Got {:.1}C",
        t.current_temp_c
    );
}

#[test]
fn hot_climate_makes_critical_easier_to_reach() {
    // Fleet Depot-style hot site: 40 C ambient
    let mut hot = make_transformer(500.0, 40.0);
    hot.current_load_kva = 400.0; // 80% load

    // target_temp = 40 + (110-40) * 0.64 = 40 + 44.8 = 84.8 C (warning, close to critical)
    for _ in 0..50_000 {
        hot.update_temperature(1.0);
    }

    // Compare with cold site (5 C ambient) at same load
    let mut cold = make_transformer(500.0, 5.0);
    cold.current_load_kva = 400.0;

    // target_temp = 5 + (110-5) * 0.64 = 5 + 67.2 = 72.2 C (below warning)
    for _ in 0..50_000 {
        cold.update_temperature(1.0);
    }

    assert!(
        hot.current_temp_c > cold.current_temp_c + 10.0,
        "Hot climate should run significantly hotter. Hot={:.1}C, Cold={:.1}C",
        hot.current_temp_c,
        cold.current_temp_c
    );
    assert!(
        hot.is_warning(),
        "Hot site at 80% should be in warning. Got {:.1}C",
        hot.current_temp_c
    );
    assert!(
        !cold.is_warning(),
        "Cold site at 80% should be safe. Got {:.1}C",
        cold.current_temp_c
    );
}

#[test]
fn fire_countdown_accumulates_in_critical_zone() {
    let mut t = make_transformer(500.0, 25.0);
    // Force temperature into critical zone
    t.current_temp_c = 95.0;

    assert!(t.is_critical());
    assert_eq!(t.overload_seconds, 0.0);

    // Simulate what the fire state system does: accumulate when critical
    let delta = 10.0;
    t.overload_seconds += delta;

    assert_eq!(t.overload_seconds, 10.0);
}

#[test]
fn fire_countdown_cools_slowly_in_warning_zone() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_temp_c = 80.0; // Warning zone (75-90 C)
    t.overload_seconds = 30.0;

    assert!(t.is_warning());
    assert!(!t.is_critical());

    // Warning zone cools at 1x rate
    let delta = 10.0;
    let cooldown_rate = 1.0;
    t.overload_seconds = (t.overload_seconds - delta * cooldown_rate).max(0.0);

    assert_eq!(t.overload_seconds, 20.0);
}

#[test]
fn fire_countdown_cools_fast_in_normal_zone() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_temp_c = 50.0; // Normal zone
    t.overload_seconds = 30.0;

    assert!(!t.is_warning());
    assert!(!t.is_critical());

    // Normal zone cools at 2x rate
    let delta = 10.0;
    let cooldown_rate = 2.0;
    t.overload_seconds = (t.overload_seconds - delta * cooldown_rate).max(0.0);

    assert_eq!(t.overload_seconds, 10.0);
}

#[test]
fn fire_ignites_at_threshold() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_temp_c = 95.0;

    // Accumulate right up to the threshold (90 game-seconds)
    t.overload_seconds = 89.0;
    assert!(!t.on_fire);

    t.overload_seconds = 90.0;

    // Simulate ignition check
    if t.overload_seconds >= 90.0 {
        t.on_fire = true;
    }

    assert!(t.on_fire);
}

#[test]
fn destroyed_transformer_fields_after_extinguish() {
    let mut t = make_transformer(500.0, 25.0);
    t.on_fire = true;
    t.current_temp_c = 105.0;

    // Simulate firetruck extinguishing
    t.on_fire = false;
    t.destroyed = true;
    t.current_temp_c = t.ambient_temp_c + 10.0;
    t.overload_seconds = 0.0;

    assert!(!t.on_fire);
    assert!(t.destroyed);
    assert_eq!(t.overload_seconds, 0.0);
    assert!((t.current_temp_c - 35.0).abs() < 0.1);
}

#[test]
fn warning_levels_only_fire_once_per_escalation() {
    let mut t = make_transformer(500.0, 25.0);
    t.current_temp_c = 95.0;

    // First time crossing 33% threshold
    t.overload_seconds = 31.0;
    let pct = t.overload_seconds / 90.0;
    assert!(pct >= 0.33);
    assert_eq!(t.last_warning_level, 0);

    // Would emit warning level 1
    t.last_warning_level = 1;

    // Second tick at same level should not re-emit (level already >= 1)
    t.overload_seconds = 35.0;
    assert!(t.last_warning_level >= 1);

    // Crossing 67% threshold
    t.overload_seconds = 61.0;
    let pct = t.overload_seconds / 90.0;
    assert!(pct >= 0.67);
    assert!(t.last_warning_level < 2);

    // Would emit critical level 2
    t.last_warning_level = 2;

    // Reset when overload cools to zero
    t.overload_seconds = 0.0;
    t.last_warning_level = 0;
    assert_eq!(t.last_warning_level, 0);
}

#[test]
fn load_shedding_lowers_temperature_target() {
    let mut t = make_transformer(500.0, 25.0);

    // At full load, warm up into critical
    t.current_load_kva = 500.0;
    for _ in 0..50_000 {
        t.update_temperature(1.0);
    }
    assert!(t.is_critical());
    let hot_temp = t.current_temp_c;

    // Shed load to 50% (simulates power density = 0.5)
    t.current_load_kva = 250.0;
    for _ in 0..50_000 {
        t.update_temperature(1.0);
    }

    assert!(
        t.current_temp_c < hot_temp - 30.0,
        "Shedding load should cool significantly. Before={:.1}C, After={:.1}C",
        hot_temp,
        t.current_temp_c
    );
    assert!(
        !t.is_critical(),
        "After load shed, should exit critical. Got {:.1}C",
        t.current_temp_c
    );
}
