//! Test utilities for ChargeOps Simulator tests.
//!
//! This module provides helper functions and fixtures for testing
//! Bevy ECS systems in isolation.

#![allow(dead_code)] // Test utilities are available for future tests

use bevy::prelude::*;

use kilowatt_tycoon::components::charger::{Charger, ChargerState, ChargerType, FaultType, Phase};
use kilowatt_tycoon::components::driver::{
    Driver, DriverMood, DriverState, PatienceLevel, VehicleType,
};
use kilowatt_tycoon::components::power::{PhaseLoads, VoltageState};
use kilowatt_tycoon::components::site::BelongsToSite;
use kilowatt_tycoon::events::*;
use kilowatt_tycoon::resources::{
    GameClock, GameSpeed, GameState, MultiSiteManager, PlayerProfile, SiteId,
};

// Re-export specific events for convenience
pub use kilowatt_tycoon::events::{
    RepairCompleteEvent, RepairFailedEvent, TechnicianDispatchEvent,
};

/// Creates a minimal Bevy App for testing with required resources.
pub fn create_test_app() -> App {
    let mut app = App::new();

    // Add minimal plugins
    app.add_plugins(MinimalPlugins);

    // Initialize resources
    app.init_resource::<GameClock>();
    app.init_resource::<GameState>();
    app.init_resource::<PhaseLoads>();
    app.init_resource::<VoltageState>();
    app.init_resource::<MultiSiteManager>();
    app.init_resource::<PlayerProfile>();

    // Register messages (events in Bevy 0.17)
    app.add_message::<ChargerFaultEvent>();
    app.add_message::<ChargerFaultResolvedEvent>();
    app.add_message::<DriverArrivedEvent>();
    app.add_message::<DriverLeftEvent>();
    app.add_message::<ChargingCompleteEvent>();
    app.add_message::<TicketCreatedEvent>();
    app.add_message::<TicketResolvedEvent>();
    app.add_message::<TicketEscalatedEvent>();
    app.add_message::<RemoteActionRequestEvent>();
    app.add_message::<RemoteActionResultEvent>();
    app.add_message::<CashChangedEvent>();
    app.add_message::<ReputationChangedEvent>();
    app.add_message::<GameEndedEvent>();
    app.add_message::<TransformerWarningEvent>();
    app.add_message::<SpeedChangedEvent>();
    app.add_message::<TechnicianDispatchEvent>();
    app.add_message::<RepairCompleteEvent>();
    app.add_message::<RepairFailedEvent>();

    app
}

/// Creates a test charger with default values.
pub fn create_test_charger(id: &str, charger_type: ChargerType) -> Charger {
    Charger {
        id: id.to_string(),
        name: id.to_string(),
        charger_type,
        max_power_kw: match charger_type {
            ChargerType::DcFast => 150.0,
            ChargerType::AcLevel2 => 7.0,
        },
        rated_power_kw: match charger_type {
            ChargerType::DcFast => 150.0,
            ChargerType::AcLevel2 => 7.0,
        },
        phase: Phase::A,
        health: 1.0,
        current_power_kw: 0.0,
        is_disabled: false,
        current_fault: None,
        cooldowns: Default::default(),
        scripted_fault_time: None,
        scripted_fault_type: None,
        connector_jam_chance: 0.0,
        connector_type: "CCS".to_string(),
        grid_position: None,
        ..Default::default()
    }
}

/// Creates a test charger with a specific fault.
pub fn create_faulted_charger(id: &str, fault: FaultType) -> Charger {
    let mut charger = create_test_charger(id, ChargerType::DcFast);
    // Setting current_fault causes state() to compute the appropriate fault state
    charger.current_fault = Some(fault);
    charger
}

/// Creates a test driver with default values.
pub fn create_test_driver(id: &str) -> Driver {
    Driver {
        id: id.to_string(),
        evcc_id: format!("{:012x}", id.len() as u64),
        vehicle_name: format!("Vehicle-{id}"),
        vehicle_type: VehicleType::Sedan,
        patience_level: PatienceLevel::Medium,
        patience: 75.0,
        charge_needed_kwh: 30.0,
        charge_received_kwh: 0.0,
        state: DriverState::Arriving,
        target_charger_id: None,
        assigned_charger: None,
        assigned_bay: None,
        mood: DriverMood::Neutral,
    }
}

/// Creates a test driver that is currently charging.
pub fn create_charging_driver(id: &str, charger_entity: Entity) -> Driver {
    let mut driver = create_test_driver(id);
    driver.state = DriverState::Charging;
    driver.assigned_charger = Some(charger_entity);
    driver
}

/// Advances the game clock by a specified amount.
pub fn advance_game_time(app: &mut App, delta: f32) {
    let mut game_clock = app.world_mut().resource_mut::<GameClock>();
    game_clock.tick(delta);
}

/// Sets the game clock speed.
pub fn set_game_speed(app: &mut App, speed: GameSpeed) {
    let mut game_clock = app.world_mut().resource_mut::<GameClock>();
    game_clock.set_speed(speed);
}

/// Gets the current game state.
pub fn get_game_state(app: &App) -> GameState {
    app.world().resource::<GameState>().clone()
}

/// Spawns a charger entity and returns its Entity ID.
pub fn spawn_charger(app: &mut App, charger: Charger) -> Entity {
    app.world_mut()
        .spawn((
            charger,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(SiteId(0)),
        ))
        .id()
}

/// Spawns a driver entity and returns its Entity ID.
pub fn spawn_driver(app: &mut App, driver: Driver) -> Entity {
    app.world_mut()
        .spawn((driver, Transform::default(), Visibility::default()))
        .id()
}

/// Checks if a charger has a specific state.
pub fn charger_has_state(app: &App, entity: Entity, expected_state: ChargerState) -> bool {
    app.world()
        .get::<Charger>(entity)
        .map(|c| c.state() == expected_state)
        .unwrap_or(false)
}

/// Checks if a charger has a specific fault.
pub fn charger_has_fault(app: &App, entity: Entity, expected_fault: Option<FaultType>) -> bool {
    app.world()
        .get::<Charger>(entity)
        .map(|c| c.current_fault == expected_fault)
        .unwrap_or(false)
}

/// Gets the driver state for an entity.
pub fn get_driver_state(app: &App, entity: Entity) -> Option<DriverState> {
    app.world().get::<Driver>(entity).map(|d| d.state)
}
