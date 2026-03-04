//! Tests for demand and procedural driver generation systems.
//!
//! These tests verify time-of-day curves, reputation factors,
//! charge need generation, and demand calculations.

#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::uninlined_format_args)]

mod test_utils;

use kilowatt_tycoon::components::driver::{PatienceLevel, VehicleType};
use kilowatt_tycoon::resources::{
    DemandState, EnvironmentState, GameState, SiteUpgrades, WeatherType, charge_needed_for_vehicle,
    reputation_factor, time_of_day_multiplier,
};

use test_utils::*;

// ============ Time-of-Day Curve Tests ============

#[test]
fn test_time_of_day_multiplier_night() {
    // Night hours (0-5) should have low demand
    assert!((time_of_day_multiplier(2) - 0.2).abs() < 0.01);
    assert!((time_of_day_multiplier(4) - 0.2).abs() < 0.01);
}

#[test]
fn test_time_of_day_multiplier_morning_rush() {
    // Morning rush (7-9) should have high demand
    assert!(time_of_day_multiplier(8) >= 1.3);
    assert!(time_of_day_multiplier(7) >= 1.3);
}

#[test]
fn test_time_of_day_multiplier_evening_rush() {
    // Evening rush (17-19) should have peak demand
    assert!(time_of_day_multiplier(18) >= 1.5);
    assert!(time_of_day_multiplier(17) >= 1.5);
}

#[test]
fn test_time_of_day_multiplier_midday() {
    // Mid-day should be lower than rush hours
    let midday = time_of_day_multiplier(10);
    let evening_rush = time_of_day_multiplier(18);
    assert!(midday < evening_rush);
}

#[test]
fn test_time_of_day_covers_all_hours() {
    // Every hour should return a valid multiplier
    for hour in 0..24 {
        let mult = time_of_day_multiplier(hour);
        assert!(
            mult >= 0.1 && mult <= 2.0,
            "Hour {} returned {}",
            hour,
            mult
        );
    }
}

// ============ Reputation Factor Tests ============

#[test]
fn test_reputation_factor_minimum() {
    // Reputation 0 = 0.5x demand (punishing but not zero)
    assert!((reputation_factor(0) - 0.5).abs() < 0.01);
}

#[test]
fn test_reputation_factor_baseline() {
    // Reputation 50 = 1.0x demand (neutral)
    assert!((reputation_factor(50) - 1.0).abs() < 0.01);
}

#[test]
fn test_reputation_factor_maximum() {
    // Reputation 100 = 1.5x demand (excellent)
    assert!((reputation_factor(100) - 1.5).abs() < 0.01);
}

#[test]
fn test_reputation_factor_linear() {
    // Should be linear between 0 and 100
    let f25 = reputation_factor(25);
    let f75 = reputation_factor(75);
    assert!((f25 - 0.75).abs() < 0.01);
    assert!((f75 - 1.25).abs() < 0.01);
}

#[test]
fn test_reputation_factor_clamped() {
    // Values outside 0-100 should be clamped
    assert!((reputation_factor(-10) - 0.5).abs() < 0.01);
    assert!((reputation_factor(150) - 1.5).abs() < 0.01);
}

// ============ Charge Need Generation Tests ============

#[test]
fn test_charge_needed_compact_range() {
    let mut rng = rand::rng();
    for _ in 0..100 {
        let charge = charge_needed_for_vehicle(&mut rng, VehicleType::Compact);
        assert!(
            charge >= 20.0 && charge <= 50.0,
            "Compact charge {} out of range",
            charge
        );
    }
}

#[test]
fn test_charge_needed_sedan_range() {
    let mut rng = rand::rng();
    for _ in 0..100 {
        let charge = charge_needed_for_vehicle(&mut rng, VehicleType::Sedan);
        assert!(
            charge >= 30.0 && charge <= 80.0,
            "Sedan charge {} out of range",
            charge
        );
    }
}

#[test]
fn test_charge_needed_crossover_range() {
    let mut rng = rand::rng();
    for _ in 0..100 {
        let charge = charge_needed_for_vehicle(&mut rng, VehicleType::Crossover);
        assert!(
            charge >= 40.0 && charge <= 100.0,
            "Crossover charge {} out of range",
            charge
        );
    }
}

#[test]
fn test_charge_needed_suv_range() {
    let mut rng = rand::rng();
    for _ in 0..100 {
        let charge = charge_needed_for_vehicle(&mut rng, VehicleType::Suv);
        assert!(
            charge >= 60.0 && charge <= 150.0,
            "SUV charge {} out of range",
            charge
        );
    }
}

#[test]
fn test_charge_needed_pickup_range() {
    let mut rng = rand::rng();
    for _ in 0..100 {
        let charge = charge_needed_for_vehicle(&mut rng, VehicleType::Pickup);
        assert!(
            charge >= 80.0 && charge <= 250.0,
            "Pickup charge {} out of range",
            charge
        );
    }
}

#[test]
fn test_charge_needed_increases_with_vehicle_size() {
    let mut rng = rand::rng();
    let compact_avg: f32 = (0..100)
        .map(|_| charge_needed_for_vehicle(&mut rng, VehicleType::Compact))
        .sum::<f32>()
        / 100.0;
    let pickup_avg: f32 = (0..100)
        .map(|_| charge_needed_for_vehicle(&mut rng, VehicleType::Pickup))
        .sum::<f32>()
        / 100.0;
    assert!(
        pickup_avg > compact_avg * 2.0,
        "Pickups should need significantly more charge: pickup={}, compact={}",
        pickup_avg,
        compact_avg
    );
}

#[test]
fn test_charge_needed_sedan_between_compact_and_suv() {
    let mut rng = rand::rng();
    let compact_avg: f32 = (0..100)
        .map(|_| charge_needed_for_vehicle(&mut rng, VehicleType::Compact))
        .sum::<f32>()
        / 100.0;
    let sedan_avg: f32 = (0..100)
        .map(|_| charge_needed_for_vehicle(&mut rng, VehicleType::Sedan))
        .sum::<f32>()
        / 100.0;
    let suv_avg: f32 = (0..100)
        .map(|_| charge_needed_for_vehicle(&mut rng, VehicleType::Suv))
        .sum::<f32>()
        / 100.0;

    assert!(sedan_avg > compact_avg);
    assert!(sedan_avg < suv_avg);
}

// ============ Demand State Tests ============

#[test]
fn test_demand_state_default() {
    let demand = DemandState::default();
    assert!(demand.base_customers_per_hour > 0.0);
    assert_eq!(demand.procedural_counter, 0);
    assert!(demand.enabled);
}

#[test]
fn test_effective_demand_calculation() {
    let demand = DemandState::default();

    // At baseline conditions (rep=50, sunny weather, no news, no marketing, noon)
    let effective = demand.calculate_effective_demand(50, 1.0, 1.0, 1.0, 12, 1.0);

    // Should be close to base rate at noon with baseline conditions
    // Noon has 1.0x multiplier, rep 50 = 1.0x, all others 1.0x
    assert!((effective - demand.base_customers_per_hour).abs() < 0.01);
}

#[test]
fn test_effective_demand_with_high_reputation() {
    let demand = DemandState::default();

    // High reputation should increase demand
    let effective_high_rep = demand.calculate_effective_demand(100, 1.0, 1.0, 1.0, 12, 1.0);
    let effective_low_rep = demand.calculate_effective_demand(0, 1.0, 1.0, 1.0, 12, 1.0);

    assert!(effective_high_rep > effective_low_rep);
    assert!(effective_high_rep > demand.base_customers_per_hour);
}

#[test]
fn test_effective_demand_with_weather() {
    let demand = DemandState::default();

    // Rainy weather should reduce demand
    let effective_rainy = demand.calculate_effective_demand(50, 0.8, 1.0, 1.0, 12, 1.0);
    let effective_sunny = demand.calculate_effective_demand(50, 1.0, 1.0, 1.0, 12, 1.0);

    assert!(effective_rainy < effective_sunny);
}

#[test]
fn test_effective_demand_evening_rush() {
    let demand = DemandState::default();

    // Evening rush should have higher demand than night
    let effective_evening = demand.calculate_effective_demand(50, 1.0, 1.0, 1.0, 18, 1.0);
    let effective_night = demand.calculate_effective_demand(50, 1.0, 1.0, 1.0, 3, 1.0);

    assert!(effective_evening > effective_night);
}

#[test]
fn test_spawn_interval_inversely_proportional_to_demand() {
    let demand = DemandState::default();

    let low_demand_interval = demand.calculate_spawn_interval(0.5);
    let high_demand_interval = demand.calculate_spawn_interval(2.0);

    assert!(
        low_demand_interval > high_demand_interval,
        "Higher demand should mean shorter intervals: low={}, high={}",
        low_demand_interval,
        high_demand_interval
    );
}

#[test]
fn test_spawn_interval_handles_zero_demand() {
    let demand = DemandState::default();

    let interval = demand.calculate_spawn_interval(0.0);
    assert!(
        interval > 0.0,
        "Should return a positive interval even with zero demand"
    );
}

#[test]
fn test_demand_state_tick() {
    let mut demand = DemandState::default();
    let initial_time = demand.time_until_next_spawn;

    demand.tick(100.0);

    assert!(demand.time_until_next_spawn < initial_time);
}

#[test]
fn test_demand_state_should_spawn() {
    let mut demand = DemandState::default();
    demand.time_until_next_spawn = 10.0;

    assert!(!demand.should_spawn());

    demand.time_until_next_spawn = -1.0;
    assert!(demand.should_spawn());
}

#[test]
fn test_demand_state_reset_timer() {
    let mut demand = DemandState::default();
    demand.time_until_next_spawn = -10.0;

    demand.reset_timer(500.0);

    assert!((demand.time_until_next_spawn - 500.0).abs() < 0.01);
}

#[test]
fn test_demand_state_next_id() {
    let mut demand = DemandState::default();
    assert_eq!(demand.procedural_counter, 0);

    let id1 = demand.next_id();
    assert_eq!(id1, 1);
    assert_eq!(demand.procedural_counter, 1);

    let id2 = demand.next_id();
    assert_eq!(id2, 2);
    assert_eq!(demand.procedural_counter, 2);
}

// ============ Weather Integration Tests ============

#[test]
fn test_weather_affects_demand() {
    // Rainy weather should reduce demand
    assert!(WeatherType::Rainy.demand_multiplier() < 1.0);

    // Sunny weather should be baseline
    assert!((WeatherType::Sunny.demand_multiplier() - 1.0).abs() < 0.01);

    // Heatwave should reduce demand slightly
    assert!(WeatherType::Heatwave.demand_multiplier() < 1.0);
}

#[test]
fn test_weather_multipliers_are_positive() {
    let weather_types = [
        WeatherType::Sunny,
        WeatherType::Overcast,
        WeatherType::Rainy,
        WeatherType::Heatwave,
        WeatherType::Cold,
    ];

    for weather in weather_types {
        assert!(
            weather.demand_multiplier() > 0.0,
            "{:?} should have positive demand multiplier",
            weather
        );
    }
}

// ============ Patience Level Tests ============

#[test]
fn test_patience_levels_ordered() {
    // Higher patience levels should have higher initial values
    assert!(PatienceLevel::High.initial_patience() > PatienceLevel::Medium.initial_patience());
    assert!(PatienceLevel::Medium.initial_patience() > PatienceLevel::Low.initial_patience());
    assert!(PatienceLevel::Low.initial_patience() > PatienceLevel::VeryLow.initial_patience());
}

#[test]
fn test_patience_depletion_rates_ordered() {
    // Lower patience should deplete faster
    assert!(PatienceLevel::VeryLow.depletion_rate() > PatienceLevel::Low.depletion_rate());
    assert!(PatienceLevel::Low.depletion_rate() > PatienceLevel::Medium.depletion_rate());
    assert!(PatienceLevel::Medium.depletion_rate() > PatienceLevel::High.depletion_rate());
}

// ============ Integration Tests with Bevy App ============

#[test]
fn test_procedural_spawning_with_bevy_app() {
    use kilowatt_tycoon::resources::DriverSchedule;

    let mut app = create_test_app();
    app.init_resource::<DemandState>();
    app.init_resource::<DriverSchedule>();

    // Exhaust the schedule
    {
        let mut schedule = app.world_mut().resource_mut::<DriverSchedule>();
        schedule.next_driver_index = schedule.drivers.len();
    }

    // Set demand state to spawn immediately
    {
        let mut demand = app.world_mut().resource_mut::<DemandState>();
        demand.time_until_next_spawn = -1.0; // Ready to spawn
    }

    // Note: We can't easily test the full spawn system without all dependencies
    // (grid, chargers, etc.), but we can verify the demand state works correctly
    let demand = app.world().resource::<DemandState>();
    assert!(demand.should_spawn());
}

#[test]
fn test_demand_respects_reputation() {
    let mut app = create_test_app();
    app.init_resource::<DemandState>();

    // Set low reputation
    {
        let mut game_state = app.world_mut().resource_mut::<GameState>();
        game_state.reputation = 10;
    }

    let demand = app.world().resource::<DemandState>();
    let interval_low_rep = demand.calculate_spawn_interval(reputation_factor(10));
    let interval_high_rep = demand.calculate_spawn_interval(reputation_factor(90));

    assert!(
        interval_low_rep > interval_high_rep,
        "Low reputation should result in longer spawn intervals"
    );
}

#[test]
fn test_demand_state_integration_with_environment() {
    let mut app = create_test_app();
    app.init_resource::<DemandState>();
    app.init_resource::<EnvironmentState>();
    app.init_resource::<SiteUpgrades>();

    let demand = app.world().resource::<DemandState>();
    let environment = app.world().resource::<EnvironmentState>();
    let upgrades = app.world().resource::<SiteUpgrades>();

    // Calculate effective demand with real resources
    let effective = demand.calculate_effective_demand(
        50,
        environment.current_weather.demand_multiplier(),
        environment.news_demand_multiplier,
        upgrades.demand_multiplier(),
        12,
        1.0,
    );

    assert!(effective > 0.0);
}

#[test]
fn test_demand_timer_advances() {
    let mut app = create_test_app();
    app.init_resource::<DemandState>();

    let initial_time = {
        let demand = app.world().resource::<DemandState>();
        demand.time_until_next_spawn
    };

    // Manually tick the demand state
    {
        let mut demand = app.world_mut().resource_mut::<DemandState>();
        demand.tick(100.0);
    }

    let new_time = {
        let demand = app.world().resource::<DemandState>();
        demand.time_until_next_spawn
    };

    assert!(new_time < initial_time, "Timer should advance");
}

#[test]
fn test_procedural_counter_increments() {
    let mut app = create_test_app();
    app.init_resource::<DemandState>();

    let id1 = {
        let mut demand = app.world_mut().resource_mut::<DemandState>();
        demand.next_id()
    };

    let id2 = {
        let mut demand = app.world_mut().resource_mut::<DemandState>();
        demand.next_id()
    };

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

#[test]
fn test_demand_calculation_with_all_multipliers() {
    let mut app = create_test_app();
    app.init_resource::<DemandState>();
    app.init_resource::<EnvironmentState>();
    app.init_resource::<SiteUpgrades>();

    // Set up a scenario with multiple multipliers
    {
        let mut game_state = app.world_mut().resource_mut::<GameState>();
        game_state.reputation = 75; // 1.25x
    }

    {
        let mut environment = app.world_mut().resource_mut::<EnvironmentState>();
        environment.current_weather = WeatherType::Sunny; // 1.0x
        environment.news_demand_multiplier = 1.2; // 1.2x (positive news)
    }

    {
        let mut upgrades = app.world_mut().resource_mut::<SiteUpgrades>();
        upgrades.has_marketing = true; // 1.1x
    }

    let demand = app.world().resource::<DemandState>();
    let game_state = app.world().resource::<GameState>();
    let environment = app.world().resource::<EnvironmentState>();
    let upgrades = app.world().resource::<SiteUpgrades>();

    // Evening rush hour (18:00) = 1.5x
    let effective = demand.calculate_effective_demand(
        game_state.reputation,
        environment.current_weather.demand_multiplier(),
        environment.news_demand_multiplier,
        upgrades.demand_multiplier(),
        18,
        1.0,
    );

    // Expected: 3.0 * 1.25 * 1.0 * 1.2 * 1.1 * 1.5 = ~7.425
    assert!(
        effective > 7.0 && effective < 8.0,
        "Expected ~7.425, got {}",
        effective
    );
}

// ============ Capacity Demand Multiplier Tests ============

#[test]
fn test_capacity_demand_multiplier_single_charger() {
    let mult = kilowatt_tycoon::resources::capacity_demand_multiplier(1);
    assert!(
        (mult - 1.0).abs() < 0.001,
        "1 charger should be 1.0x, got {}",
        mult
    );
}

#[test]
fn test_capacity_demand_multiplier_four_chargers() {
    let mult = kilowatt_tycoon::resources::capacity_demand_multiplier(4);
    assert!(
        (mult - 2.0).abs() < 0.001,
        "4 chargers should be 2.0x, got {}",
        mult
    );
}

#[test]
fn test_capacity_demand_multiplier_36_chargers() {
    let mult = kilowatt_tycoon::resources::capacity_demand_multiplier(36);
    assert!(
        (mult - 6.0).abs() < 0.001,
        "36 chargers should be 6.0x, got {}",
        mult
    );
}

#[test]
fn test_capacity_demand_multiplier_zero_chargers_floors_to_one() {
    let mult = kilowatt_tycoon::resources::capacity_demand_multiplier(0);
    assert!(
        (mult - 1.0).abs() < 0.001,
        "0 chargers should floor to 1.0x, got {}",
        mult
    );
}

#[test]
fn test_capacity_demand_scales_sublinearly() {
    let m4 = kilowatt_tycoon::resources::capacity_demand_multiplier(4);
    let m16 = kilowatt_tycoon::resources::capacity_demand_multiplier(16);
    let m36 = kilowatt_tycoon::resources::capacity_demand_multiplier(36);
    assert!(
        m16 < m4 * 4.0,
        "16 chargers should be less than 4x the 4-charger value"
    );
    assert!(
        m36 < m16 * 4.0,
        "36 chargers should be less than 4x the 16-charger value"
    );
    assert!(m4 < m16, "More chargers should mean higher demand");
    assert!(m16 < m36, "More chargers should mean higher demand");
}
