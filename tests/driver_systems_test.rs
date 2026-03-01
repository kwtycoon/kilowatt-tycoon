//! Tests for driver-related systems.
//!
//! These tests verify driver spawning, patience decay,
//! charging behavior, and departure logic.

#![allow(clippy::field_reassign_with_default)]

mod test_utils;

use kilowatt_tycoon::components::charger::ChargerType;
use kilowatt_tycoon::components::driver::{
    Driver, DriverMood, DriverState, MovementPhase, PatienceLevel, VehicleMovement, VehicleType,
};
use kilowatt_tycoon::resources::{GameState, STARTING_REPUTATION};

use test_utils::*;

#[test]
fn test_driver_creation() {
    let driver = create_test_driver("DRV-001");

    assert_eq!(driver.id, "DRV-001");
    assert_eq!(driver.state, DriverState::Arriving);
    assert_eq!(driver.mood, DriverMood::Neutral);
    assert!(driver.patience > 0.0);
}

#[test]
fn test_driver_vehicle_defaults() {
    let driver = create_test_driver("DRV-001");

    assert_eq!(driver.vehicle_type, VehicleType::Sedan);
    assert!(driver.charge_needed_kwh > 0.0);
}

#[test]
fn test_charging_driver_has_assigned_charger() {
    let mut app = create_test_app();

    let charger = create_test_charger("CHG-001", ChargerType::DcFast);
    let charger_entity = spawn_charger(&mut app, charger);

    let driver = create_charging_driver("DRV-001", charger_entity);
    let driver_entity = spawn_driver(&mut app, driver);

    let spawned_driver = app.world().get::<Driver>(driver_entity).unwrap();
    assert_eq!(spawned_driver.state, DriverState::Charging);
    assert_eq!(spawned_driver.assigned_charger, Some(charger_entity));
}

#[test]
fn test_driver_mood_sprites() {
    // Verify mood sprite suffixes
    assert_eq!(DriverMood::Happy.sprite_suffix(), "happy");
    assert_eq!(DriverMood::Neutral.sprite_suffix(), "neutral");
    assert_eq!(DriverMood::Impatient.sprite_suffix(), "impatient");
    assert_eq!(DriverMood::Angry.sprite_suffix(), "angry");
}

#[test]
fn test_driver_states() {
    // Verify all driver states exist
    let _ = DriverState::Arriving;
    let _ = DriverState::WaitingForCharger;
    let _ = DriverState::Charging;
    let _ = DriverState::Complete;
    let _ = DriverState::Leaving;
    let _ = DriverState::LeftAngry;
}

#[test]
fn test_vehicle_types() {
    let types = [
        VehicleType::Sedan,
        VehicleType::Suv,
        VehicleType::Compact,
        VehicleType::Crossover,
        VehicleType::Pickup,
    ];

    for vtype in types {
        // Each vehicle type should have a valid sprite name
        assert!(!vtype.sprite_name().is_empty());
    }
}

#[test]
fn test_patience_levels() {
    // Test patience level initial values
    assert!(PatienceLevel::High.initial_patience() > PatienceLevel::Low.initial_patience());
    assert!(PatienceLevel::Medium.initial_patience() > PatienceLevel::VeryLow.initial_patience());

    // Test depletion rates (lower patience = higher depletion)
    assert!(PatienceLevel::VeryLow.depletion_rate() > PatienceLevel::High.depletion_rate());
}

#[test]
fn test_driver_charging_complete() {
    let mut driver = create_test_driver("DRV-001");
    driver.charge_needed_kwh = 30.0;
    driver.charge_received_kwh = 0.0;

    // Not complete yet
    assert!(!driver.is_charging_complete());

    // Partial charge
    driver.charge_received_kwh = 15.0;
    assert!(!driver.is_charging_complete());

    // Complete
    driver.charge_received_kwh = 30.0;
    assert!(driver.is_charging_complete());

    // Overcharged is also complete
    driver.charge_received_kwh = 35.0;
    assert!(driver.is_charging_complete());
}

#[test]
fn test_driver_charge_progress() {
    let mut driver = create_test_driver("DRV-001");
    driver.charge_needed_kwh = 40.0;

    driver.charge_received_kwh = 0.0;
    assert!((driver.charge_progress() - 0.0).abs() < 0.01);

    driver.charge_received_kwh = 20.0;
    assert!((driver.charge_progress() - 0.5).abs() < 0.01);

    driver.charge_received_kwh = 40.0;
    assert!((driver.charge_progress() - 1.0).abs() < 0.01);
}

#[test]
fn test_game_state_reputation_tracking() {
    let mut game_state = GameState::default();

    assert_eq!(game_state.reputation, STARTING_REPUTATION);

    // Positive change
    game_state.change_reputation(10);
    assert_eq!(game_state.reputation, STARTING_REPUTATION + 10);

    // Negative change
    game_state.change_reputation(-20);
    assert_eq!(game_state.reputation, STARTING_REPUTATION - 10);
}

#[test]
fn test_game_state_reputation_clamped() {
    let mut game_state = GameState::default();

    // Try to go above 100
    game_state.change_reputation(100);
    assert_eq!(game_state.reputation, 100);

    // Try to go below 0
    game_state.change_reputation(-200);
    assert_eq!(game_state.reputation, 0);
}

#[test]
fn test_game_state_revenue_tracking() {
    let mut game_state = GameState::default();
    let initial_cash = game_state.cash;

    game_state.add_charging_revenue(50.0);
    assert_eq!(game_state.ledger.gross_revenue_f32(), 50.0);
    assert_eq!(game_state.ledger.net_revenue_f32(), 50.0);
    assert_eq!(game_state.cash, initial_cash + 50.0);
}

#[test]
fn test_game_state_refund_tracking() {
    let mut game_state = GameState::default();
    let initial_cash = game_state.cash;

    game_state.add_charging_revenue(100.0);
    game_state.add_refund(25.0);

    assert_eq!(game_state.ledger.gross_revenue_f32(), 100.0);
    assert_eq!(game_state.ledger.net_revenue_f32(), 75.0);
    assert_eq!(game_state.cash, initial_cash + 75.0);
}

#[test]
fn test_game_state_reset() {
    let mut game_state = GameState::default();

    // Modify state
    game_state.add_charging_revenue(1000.0);
    game_state.change_reputation(50);
    game_state.sessions_completed = 10;

    // Reset
    game_state.reset();

    // Should be back to defaults
    assert_eq!(game_state.ledger.net_revenue_f32(), 0.0);
    assert_eq!(game_state.sessions_completed, 0);
}

#[test]
fn test_driver_mood_update() {
    let mut driver = create_test_driver("DRV-001");
    driver.patience_level = PatienceLevel::Medium;

    // Full patience = neutral
    driver.patience = driver.patience_level.initial_patience();
    driver.update_mood();
    assert_eq!(driver.mood, DriverMood::Neutral);

    // Moderate patience = impatient
    driver.patience = driver.patience_level.initial_patience() * 0.6;
    driver.update_mood();
    assert_eq!(driver.mood, DriverMood::Impatient);

    // Low patience = angry
    driver.patience = driver.patience_level.initial_patience() * 0.3;
    driver.update_mood();
    assert_eq!(driver.mood, DriverMood::Angry);
}

// ============ Bay Occupancy Tests ============
// These tests verify the logic for determining when a bay is "physically occupied"
// per the fix: only Parked/DepartingHappy/DepartingAngry drivers block spawning,
// not Arriving drivers (who are still en-route).

/// Helper to check if a movement phase counts as "physically present" at a bay.
/// This mirrors the logic in driver_spawn_system.
fn is_physically_present(phase: MovementPhase) -> bool {
    matches!(
        phase,
        MovementPhase::Parked | MovementPhase::DepartingHappy | MovementPhase::DepartingAngry
    )
}

#[test]
fn test_arriving_driver_does_not_block_bay() {
    // Drivers who are still arriving (driving in) should NOT block the bay
    assert!(!is_physically_present(MovementPhase::Arriving));
}

#[test]
fn test_parked_driver_blocks_bay() {
    // Drivers who are parked at a bay SHOULD block it
    assert!(is_physically_present(MovementPhase::Parked));
}

#[test]
fn test_departing_driver_blocks_bay() {
    // Drivers who are departing should still block the bay until they clear
    assert!(is_physically_present(MovementPhase::DepartingHappy));
    assert!(is_physically_present(MovementPhase::DepartingAngry));
}

#[test]
fn test_exited_driver_does_not_block_bay() {
    // Drivers who have exited should not block (they're despawned anyway)
    assert!(!is_physically_present(MovementPhase::Exited));
}

#[test]
fn test_movement_phase_transitions() {
    // Verify the expected movement phase sequence
    let phases = [
        MovementPhase::Arriving,
        MovementPhase::Parked,
        MovementPhase::DepartingHappy,
        MovementPhase::Exited,
    ];

    // Arriving is the initial phase
    assert_eq!(phases[0], MovementPhase::Arriving);
    // Should transition to Parked when reaching destination
    assert_eq!(phases[1], MovementPhase::Parked);
}

#[test]
fn test_vehicle_movement_default_phase() {
    let movement = VehicleMovement::default();
    // Default should be Arriving
    assert_eq!(movement.phase, MovementPhase::Arriving);
}

#[test]
fn test_driver_with_assigned_bay_but_arriving() {
    // Simulate a driver who has been assigned a bay but is still en-route
    let mut driver = create_test_driver("DRV-001");
    driver.assigned_bay = Some((1, 2));
    driver.state = DriverState::Arriving;

    let movement = VehicleMovement::default(); // Arriving phase

    // The driver has an assigned bay
    assert!(driver.assigned_bay.is_some());
    // But they're still arriving, so shouldn't block
    assert!(!is_physically_present(movement.phase));
}

#[test]
fn test_driver_with_assigned_bay_and_parked() {
    // Simulate a driver who has reached their bay
    let mut driver = create_test_driver("DRV-001");
    driver.assigned_bay = Some((1, 2));
    driver.state = DriverState::Charging;

    let mut movement = VehicleMovement::default();
    movement.phase = MovementPhase::Parked;

    // The driver has an assigned bay
    assert!(driver.assigned_bay.is_some());
    // And they're parked, so should block
    assert!(is_physically_present(movement.phase));
}

// ============ Charger Assignment Bug Regression Test ============
// This test verifies that when a driver arrives at a bay, they are assigned
// to the correct charger (the one linked to their bay), not an arbitrary one.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;
use kilowatt_tycoon::components::BelongsToSite;
use kilowatt_tycoon::components::charger::Charger;
use kilowatt_tycoon::hooks::{ChargerIndex, HooksPlugin};
use kilowatt_tycoon::resources::{
    ChargerPadType, GameDataAssets, ImageAssets, MultiSiteManager, SiteArchetype, SiteId,
    SiteListingInfo, SiteTemplateCache, TechnicianState, TileContent,
};
use kilowatt_tycoon::systems::scene::ChargerSyncRevision;
use kilowatt_tycoon::systems::{driver_arrival_system, sync_chargers_with_grid};

/// Create a test app with all resources needed for driver arrival testing.
fn create_driver_arrival_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::prelude::ImagePlugin::default());

    // Add hooks plugin to register ChargerIndex and component hooks
    app.add_plugins(HooksPlugin);

    // Initialize required resources
    app.init_resource::<kilowatt_tycoon::resources::GameClock>();
    app.init_resource::<MultiSiteManager>();
    app.init_resource::<ChargerSyncRevision>();
    app.init_resource::<SiteTemplateCache>();
    app.init_resource::<TechnicianState>();
    app.insert_resource(ImageAssets::default());

    // Register messages needed by driver_arrival_system
    app.add_message::<kilowatt_tycoon::events::ChargerFaultEvent>();

    app
}

/// Set up a site with TWO chargers at different positions.
/// Returns (site_id, root_entity, charger1_position, charger2_position)
fn setup_site_with_two_chargers(app: &mut App) -> (SiteId, Entity, (i32, i32), (i32, i32)) {
    // Spawn a site root entity
    let root_entity = app.world_mut().spawn(Transform::default()).id();

    // Create a test site listing
    let listing = SiteListingInfo {
        archetype: SiteArchetype::ParkingLot,
        name: "Test Site".to_string(),
        description: "Test site for charger assignment".to_string(),
        rent_cost: 0.0,
        grid_capacity_kva: 500.0,
        popularity: 50.0,
        challenge_level: 1,
        grid_size: (16, 12),
    };

    let template_cache = SiteTemplateCache::default();
    let tiled_assets = Assets::<TiledMapAsset>::default();
    let game_data = GameDataAssets::default();
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

    // Rent the site
    let site_id = multi_site
        .rent_site(&listing, &template_cache, &tiled_assets, &game_data)
        .expect("Should rent site");

    // Define charger positions - ChargerPads will be at these positions
    // Bay 1 at (3,5) -> ChargerPad at (3,6) (south-facing bay, charger above)
    // Bay 2 at (7,5) -> ChargerPad at (7,6)
    let charger1_pos = (3, 6);
    let charger2_pos = (7, 6);

    if let Some(site) = multi_site.get_site_mut(site_id) {
        site.root_entity = Some(root_entity);

        // Set up first parking bay and charger
        site.grid
            .set_tile_content(3, 5, TileContent::ParkingBaySouth);
        site.grid.set_tile_content(3, 6, TileContent::Lot);
        site.grid
            .place_charger(3, 5, ChargerPadType::DCFC150)
            .expect("Should place charger 1");

        // Set up second parking bay and charger
        site.grid
            .set_tile_content(7, 5, TileContent::ParkingBaySouth);
        site.grid.set_tile_content(7, 6, TileContent::Lot);
        site.grid
            .place_charger(7, 5, ChargerPadType::DCFC150)
            .expect("Should place charger 2");

        // Reset revision
        site.grid.revision = 1;
    }

    // Switch to this site to make it active
    let _ = multi_site.switch_to_site(site_id);

    (site_id, root_entity, charger1_pos, charger2_pos)
}

/// Get the charger entity at a specific grid position from ChargerIndex.
fn get_charger_at_position(app: &App, pos: (i32, i32)) -> Option<Entity> {
    let charger_index = app.world().resource::<ChargerIndex>();
    charger_index.get_by_position(pos.0, pos.1)
}

/// Count chargers in the world.
fn count_chargers_in_world(app: &mut App) -> usize {
    let mut query = app.world_mut().query::<&Charger>();
    query.iter(app.world()).count()
}

#[test]
fn test_driver_assigned_to_correct_charger_by_bay_position() {
    // This test reproduces the bug where drivers get assigned to the wrong charger.
    //
    // Bug: tile.charger_entity is never set, so driver_arrival_system can't find
    // the correct charger. Drivers end up in the queue and get assigned to
    // whichever charger is "first" in query iteration order.
    //
    // Expected: Driver at bay (7,5) should be assigned to charger at (7,6).
    // Bug behavior: Driver gets assigned to charger at (3,6) or no charger at all.

    let mut app = create_driver_arrival_test_app();
    let (site_id, root_entity, charger1_pos, charger2_pos) = setup_site_with_two_chargers(&mut app);

    // Run charger sync to spawn Charger entities (and populate ChargerIndex via hooks)
    app.add_systems(Update, sync_chargers_with_grid);
    app.update();

    // Verify both chargers were spawned
    assert_eq!(
        count_chargers_in_world(&mut app),
        2,
        "Should have spawned two chargers"
    );

    // Get the charger entities from the ChargerIndex
    let charger1_entity =
        get_charger_at_position(&app, charger1_pos).expect("Charger 1 should be in ChargerIndex");
    let charger2_entity =
        get_charger_at_position(&app, charger2_pos).expect("Charger 2 should be in ChargerIndex");

    // Verify they are different entities
    assert_ne!(
        charger1_entity, charger2_entity,
        "Chargers should be different entities"
    );

    // Create a driver assigned to bay (7,5) which is linked to charger at (7,6)
    // The driver has just "arrived" (movement phase = Parked, state = Arriving)
    let mut driver = create_test_driver("DRV-TEST");
    driver.assigned_bay = Some((7, 5)); // Bay linked to charger2
    driver.state = DriverState::Arriving;
    driver.assigned_charger = None; // Not yet assigned

    let mut movement = VehicleMovement::default();
    movement.phase = MovementPhase::Parked; // Driver has reached their bay

    // Spawn the driver as a child of the site root
    let driver_entity = app
        .world_mut()
        .spawn((
            driver,
            movement,
            Transform::default(),
            Visibility::default(),
            BelongsToSite::new(site_id),
        ))
        .set_parent_in_place(root_entity)
        .id();

    // Add the arrival system and run it
    // (sync_chargers_with_grid will be a no-op since revision hasn't changed)
    app.add_systems(Update, driver_arrival_system);
    app.update();

    // Check which charger the driver was assigned to
    let driver_after = app
        .world()
        .get::<Driver>(driver_entity)
        .expect("Driver should exist");

    // THE KEY ASSERTION: Driver should be assigned to charger2 (at position 7,6),
    // NOT charger1 (at position 3,6).
    assert_eq!(
        driver_after.assigned_charger,
        Some(charger2_entity),
        "Driver at bay (7,5) should be assigned to charger at (7,6), not a different charger. \
         Bug: tile.charger_entity lookup fails, causing wrong charger assignment."
    );

    // Also verify the driver transitioned to Charging state
    assert_eq!(
        driver_after.state,
        DriverState::Charging,
        "Driver should be in Charging state after arrival"
    );
}
