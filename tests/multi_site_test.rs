//! Multi-site integration tests
//!
//! Tests for concurrent operation of multiple charging sites

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;
use kilowatt_tycoon::components::BelongsToSite;
use kilowatt_tycoon::events::{SiteSoldEvent, SiteSwitchEvent, TechnicianDispatchEvent};
use kilowatt_tycoon::resources::{
    GameClock, GameDataAssets, MultiSiteManager, RepairRequestId, SiteArchetype, SiteId,
    SiteListingInfo, SiteTemplateCache, TechnicianMode, TechnicianState, calculate_travel_time,
};

/// Helper to create test site listings (mocks the JSON-loaded templates)
/// These values should match what the tests expect
fn create_test_listings() -> Vec<SiteListingInfo> {
    vec![
        SiteListingInfo {
            archetype: SiteArchetype::ParkingLot,
            name: "First Street Station".to_string(),
            description: "A simple parking lot".to_string(),
            rent_cost: 0.0,
            grid_capacity_kva: 1500.0,
            popularity: 50.0,
            challenge_level: 1,
            grid_size: (16, 12),
        },
        SiteListingInfo {
            archetype: SiteArchetype::GasStation,
            name: "QuickCharge Express".to_string(),
            description: "A gas station conversion".to_string(),
            rent_cost: 500.0,
            grid_capacity_kva: 500.0,
            popularity: 60.0,
            challenge_level: 2,
            grid_size: (16, 12),
        },
        SiteListingInfo {
            archetype: SiteArchetype::FleetDepot,
            name: "Fleet Depot".to_string(),
            description: "Commercial fleet depot".to_string(),
            rent_cost: 2200.0,
            grid_capacity_kva: 3000.0,
            popularity: 75.0,
            challenge_level: 5,
            grid_size: (30, 20),
        },
    ]
}

/// Helper to create a test app with multi-site manager
fn create_multi_site_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<MultiSiteManager>();
    app.init_resource::<TechnicianState>();
    app.init_resource::<GameClock>();
    app.init_resource::<SiteTemplateCache>();
    app.add_message::<SiteSwitchEvent>();
    app.add_message::<SiteSoldEvent>();
    app.add_message::<TechnicianDispatchEvent>();

    // Populate available sites with test data
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        multi_site.available_sites = create_test_listings();
    }

    app
}

/// Helper to rent a site from the default listings
fn rent_test_site(app: &mut App, listing_index: usize) -> SiteId {
    let template_cache = SiteTemplateCache::default();
    let tiled_assets = Assets::<TiledMapAsset>::default();
    let game_data = GameDataAssets::default();
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    let listing = multi_site.available_sites[listing_index].clone();
    multi_site
        .rent_site(&listing, &template_cache, &tiled_assets, &game_data)
        .expect("Failed to rent site")
}

#[test]
fn test_concurrent_site_operation() {
    let mut app = create_multi_site_test_app();

    // Rent 2 sites
    let site_a = rent_test_site(&mut app, 0); // First Street Station
    let site_b = rent_test_site(&mut app, 1); // QuickCharge Express

    // Verify both sites exist and are independent
    let multi_site = app.world().resource::<MultiSiteManager>();

    assert_eq!(multi_site.owned_sites.len(), 2, "Should have 2 sites");
    assert!(
        multi_site.owned_sites.contains_key(&site_a),
        "Site A should exist"
    );
    assert!(
        multi_site.owned_sites.contains_key(&site_b),
        "Site B should exist"
    );

    // Verify sites have independent state
    let state_a = multi_site.get_site(site_a).unwrap();
    let state_b = multi_site.get_site(site_b).unwrap();

    assert_eq!(state_a.total_revenue, 0.0, "Site A revenue starts at 0");
    assert_eq!(state_b.total_revenue, 0.0, "Site B revenue starts at 0");

    // Simulate revenue at site A
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    multi_site.get_site_mut(site_a).unwrap().total_revenue = 100.0;

    // Verify site B is unaffected
    let multi_site = app.world().resource::<MultiSiteManager>();
    assert_eq!(multi_site.get_site(site_a).unwrap().total_revenue, 100.0);
    assert_eq!(multi_site.get_site(site_b).unwrap().total_revenue, 0.0);
}

#[test]
fn test_site_power_independence() {
    let mut app = create_multi_site_test_app();

    // Rent sites with different utility capacities
    let site_a = rent_test_site(&mut app, 0); // 1500 kVA grid connection (First Street Station)
    let site_b = rent_test_site(&mut app, 1); // 500 kVA grid connection (Gas Station)

    // Without a transformer placed, L2-only sites use grid capacity directly
    // (L2 chargers use split-phase 240V, no transformer needed)
    {
        let multi_site = app.world().resource::<MultiSiteManager>();
        let state_a = multi_site.get_site(site_a).unwrap();
        let state_b = multi_site.get_site(site_b).unwrap();

        assert_eq!(state_a.grid_capacity_kva, 1500.0, "Site A grid capacity");
        assert_eq!(state_b.grid_capacity_kva, 500.0, "Site B grid capacity");

        // No transformer placed = use grid capacity (L2 chargers don't need transformer)
        assert_eq!(
            state_a.effective_capacity_kva(),
            1500.0,
            "Site A effective capacity should equal grid capacity without transformer"
        );
        assert_eq!(
            state_b.effective_capacity_kva(),
            500.0,
            "Site B effective capacity should equal grid capacity without transformer"
        );
    }

    // Place transformers and verify effective capacity updates
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

        // Place a 75 kVA transformer on site A (use corner position on grass)
        let site_a_state = multi_site.get_site_mut(site_a).unwrap();
        // Clear a 2x2 area for the transformer first
        // Some templates lock tiles to prevent selling/mutation; unlock for test setup.
        site_a_state.grid.unlock_tile(1, 1);
        site_a_state.grid.unlock_tile(2, 1);
        site_a_state.grid.unlock_tile(1, 2);
        site_a_state.grid.unlock_tile(2, 2);
        site_a_state
            .grid
            .set_tile_content(1, 1, kilowatt_tycoon::resources::TileContent::Grass);
        site_a_state
            .grid
            .set_tile_content(2, 1, kilowatt_tycoon::resources::TileContent::Grass);
        site_a_state
            .grid
            .set_tile_content(1, 2, kilowatt_tycoon::resources::TileContent::Grass);
        site_a_state
            .grid
            .set_tile_content(2, 2, kilowatt_tycoon::resources::TileContent::Grass);
        site_a_state.grid.place_transformer(1, 1, 75.0).unwrap();
        assert_eq!(
            site_a_state.effective_capacity_kva(),
            75.0,
            "Site A effective capacity after placing 75 kVA transformer"
        );

        // Place a 150 kVA transformer on site B
        let site_b_state = multi_site.get_site_mut(site_b).unwrap();
        // Clear a 2x2 area for the transformer first
        // Some templates lock tiles to prevent selling/mutation; unlock for test setup.
        site_b_state.grid.unlock_tile(1, 1);
        site_b_state.grid.unlock_tile(2, 1);
        site_b_state.grid.unlock_tile(1, 2);
        site_b_state.grid.unlock_tile(2, 2);
        site_b_state
            .grid
            .set_tile_content(1, 1, kilowatt_tycoon::resources::TileContent::Grass);
        site_b_state
            .grid
            .set_tile_content(2, 1, kilowatt_tycoon::resources::TileContent::Grass);
        site_b_state
            .grid
            .set_tile_content(1, 2, kilowatt_tycoon::resources::TileContent::Grass);
        site_b_state
            .grid
            .set_tile_content(2, 2, kilowatt_tycoon::resources::TileContent::Grass);
        site_b_state.grid.place_transformer(1, 1, 150.0).unwrap();
        assert_eq!(
            site_b_state.effective_capacity_kva(),
            150.0,
            "Site B effective capacity after placing 150 kVA transformer"
        );
    }

    // Verify phase loads are independent
    let multi_site = app.world().resource::<MultiSiteManager>();
    let state_a = multi_site.get_site(site_a).unwrap();
    let state_b = multi_site.get_site(site_b).unwrap();
    assert!(
        state_a.phase_loads.total_load() == 0.0,
        "Site A starts with no load"
    );
    assert!(
        state_b.phase_loads.total_load() == 0.0,
        "Site B starts with no load"
    );
}

#[test]
fn test_queue_isolation() {
    let mut app = create_multi_site_test_app();

    // Rent 2 sites
    let site_a = rent_test_site(&mut app, 0);
    let site_b = rent_test_site(&mut app, 1);

    // Create real entities for queue testing
    let entity_a1 = app.world_mut().spawn_empty().id();
    let entity_a2 = app.world_mut().spawn_empty().id();
    let entity_b1 = app.world_mut().spawn_empty().id();

    // Add drivers to queue at site A
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    multi_site
        .get_site_mut(site_a)
        .unwrap()
        .charger_queue
        .dcfc_queue
        .push_back(entity_a1);
    multi_site
        .get_site_mut(site_a)
        .unwrap()
        .charger_queue
        .dcfc_queue
        .push_back(entity_a2);

    // Add driver to queue at site B
    multi_site
        .get_site_mut(site_b)
        .unwrap()
        .charger_queue
        .dcfc_queue
        .push_back(entity_b1);

    // Verify queues are isolated
    let multi_site = app.world().resource::<MultiSiteManager>();

    let queue_a = &multi_site.get_site(site_a).unwrap().charger_queue;
    let queue_b = &multi_site.get_site(site_b).unwrap().charger_queue;

    assert_eq!(
        queue_a.dcfc_queue.len(),
        2,
        "Site A should have 2 drivers in queue"
    );
    assert_eq!(
        queue_b.dcfc_queue.len(),
        1,
        "Site B should have 1 driver in queue"
    );

    // Verify the right drivers are in each queue
    assert!(
        queue_a.dcfc_queue.contains(&entity_a1),
        "Site A queue should contain driver A1"
    );
    assert!(
        queue_a.dcfc_queue.contains(&entity_a2),
        "Site A queue should contain driver A2"
    );
    assert!(
        queue_b.dcfc_queue.contains(&entity_b1),
        "Site B queue should contain driver B1"
    );

    // Verify no cross-contamination
    assert!(
        !queue_a.dcfc_queue.contains(&entity_b1),
        "Site A queue should not contain driver B1"
    );
    assert!(
        !queue_b.dcfc_queue.contains(&entity_a1),
        "Site B queue should not contain driver A1"
    );
}

#[test]
fn test_site_selling() {
    let mut app = create_multi_site_test_app();

    // Rent 2 sites
    let site_a = rent_test_site(&mut app, 0);
    let site_b = rent_test_site(&mut app, 1);

    // Add some revenue to site A
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    multi_site.get_site_mut(site_a).unwrap().total_revenue = 1000.0;

    // Attempt to sell site A
    let sell_result = multi_site.sell_site(site_a);

    assert!(sell_result.is_ok(), "Should be able to sell site A");
    let refund_amount = sell_result.unwrap();
    assert!(
        refund_amount > 0.0,
        "Should receive refund for selling site"
    );

    // Verify site A is removed
    assert!(
        !multi_site.owned_sites.contains_key(&site_a),
        "Site A should be removed after sale"
    );
    assert!(
        multi_site.owned_sites.contains_key(&site_b),
        "Site B should still exist"
    );
    assert_eq!(
        multi_site.owned_sites.len(),
        1,
        "Should have 1 site remaining"
    );

    // Verify viewed site switched if necessary
    if multi_site.viewed_site_id == Some(site_a) {
        panic!("Viewed site should have been switched after selling");
    }
}

#[test]
fn test_site_selling_last_site_fails() {
    let mut app = create_multi_site_test_app();

    // Rent only 1 site
    let site_a = rent_test_site(&mut app, 0);

    // Attempt to sell the only site
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    let sell_result = multi_site.sell_site(site_a);

    assert!(
        sell_result.is_err(),
        "Should not be able to sell the last site"
    );
    assert_eq!(multi_site.owned_sites.len(), 1, "Site should still exist");
}

#[test]
fn test_pricing_independence() {
    let mut app = create_multi_site_test_app();

    // Rent 2 sites
    let site_a = rent_test_site(&mut app, 0);
    let site_b = rent_test_site(&mut app, 1);

    let multi_site = app.world().resource::<MultiSiteManager>();

    // Verify sites have independent per-site pricing via ServiceStrategy
    let state_a = multi_site.get_site(site_a).unwrap();
    let state_b = multi_site.get_site(site_b).unwrap();

    assert_eq!(
        state_a.service_strategy.pricing.flat.price_kwh, 0.45,
        "Site A default pricing"
    );
    assert_eq!(
        state_b.service_strategy.pricing.flat.price_kwh, 0.45,
        "Site B default pricing"
    );

    // Change pricing at site A
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    multi_site
        .get_site_mut(site_a)
        .unwrap()
        .service_strategy
        .pricing
        .flat
        .price_kwh = 0.50;

    // Verify site B is unaffected
    let multi_site = app.world().resource::<MultiSiteManager>();
    assert_eq!(
        multi_site
            .get_site(site_a)
            .unwrap()
            .service_strategy
            .pricing
            .flat
            .price_kwh,
        0.50
    );
    assert_eq!(
        multi_site
            .get_site(site_b)
            .unwrap()
            .service_strategy
            .pricing
            .flat
            .price_kwh,
        0.45
    );
}

#[test]
fn test_site_aggregation() {
    let mut app = create_multi_site_test_app();

    // Rent 3 sites
    let site_a = rent_test_site(&mut app, 0);
    let site_b = rent_test_site(&mut app, 1);
    let site_c = rent_test_site(&mut app, 2);

    // Set different revenues for each site
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    multi_site.get_site_mut(site_a).unwrap().total_revenue = 100.0;
    multi_site.get_site_mut(site_b).unwrap().total_revenue = 200.0;
    multi_site.get_site_mut(site_c).unwrap().total_revenue = 300.0;

    multi_site.get_site_mut(site_a).unwrap().total_sessions = 5;
    multi_site.get_site_mut(site_b).unwrap().total_sessions = 10;
    multi_site.get_site_mut(site_c).unwrap().total_sessions = 15;

    // Test aggregation methods
    let multi_site = app.world().resource::<MultiSiteManager>();

    assert_eq!(
        multi_site.total_revenue_all_sites(),
        600.0,
        "Total revenue should sum all sites"
    );
    assert_eq!(
        multi_site.total_sessions_all_sites(),
        30,
        "Total sessions should sum all sites"
    );
}

#[test]
fn test_entity_tagging_with_belongs_to_site() {
    let mut app = create_multi_site_test_app();

    // Rent 2 sites
    let site_a = rent_test_site(&mut app, 0);
    let site_b = rent_test_site(&mut app, 1);

    // Spawn entities tagged with different sites
    let _entity_a1 = app.world_mut().spawn(BelongsToSite::new(site_a)).id();
    let _entity_a2 = app.world_mut().spawn(BelongsToSite::new(site_a)).id();
    let _entity_b1 = app.world_mut().spawn(BelongsToSite::new(site_b)).id();

    // Query entities by site
    let world = app.world_mut();
    let mut belongs_query = world.query::<&BelongsToSite>();

    let site_a_entities: Vec<_> = belongs_query
        .iter(world)
        .filter(|b| b.site_id == site_a)
        .collect();
    let site_b_count = belongs_query
        .iter(world)
        .filter(|b| b.site_id == site_b)
        .count();

    assert_eq!(site_a_entities.len(), 2, "Site A should have 2 entities");
    assert_eq!(site_b_count, 1, "Site B should have 1 entity");
}

#[test]
fn test_site_world_offsets() {
    let mut app = create_multi_site_test_app();

    // Rent 3 sites
    let site_a = rent_test_site(&mut app, 0); // ID 1
    let site_b = rent_test_site(&mut app, 1); // ID 2
    let site_c = rent_test_site(&mut app, 2); // ID 3

    let multi_site = app.world().resource::<MultiSiteManager>();

    let offset_a = multi_site.get_site(site_a).unwrap().world_offset();
    let offset_b = multi_site.get_site(site_b).unwrap().world_offset();
    let offset_c = multi_site.get_site(site_c).unwrap().world_offset();

    // Sites should be spaced 2000px apart
    const SITE_SPACING: f32 = 2000.0;

    assert_eq!(offset_a.x, SITE_SPACING, "Site A at x=2000 (ID 1)");
    assert_eq!(offset_b.x, 2.0 * SITE_SPACING, "Site B at x=4000 (ID 2)");
    assert_eq!(offset_c.x, 3.0 * SITE_SPACING, "Site C at x=6000 (ID 3)");

    // All sites at y=0 for MVP
    assert_eq!(offset_a.y, 0.0);
    assert_eq!(offset_b.y, 0.0);
    assert_eq!(offset_c.y, 0.0);
}

// ============ Technician Travel Tests ============

#[test]
fn test_technician_travel_time_calculation() {
    let mut app = create_multi_site_test_app();

    // Rent 2 different archetype sites
    let site_a = rent_test_site(&mut app, 0); // ParkingLot
    let site_b = rent_test_site(&mut app, 1); // GasStation

    let multi_site = app.world().resource::<MultiSiteManager>();
    let archetype_a = multi_site.get_site(site_a).unwrap().archetype;
    let archetype_b = multi_site.get_site(site_b).unwrap().archetype;

    // Calculate travel time (ParkingLot → GasStation = 10 min base travel)
    let travel_time = calculate_travel_time(archetype_a, archetype_b);

    assert_eq!(
        travel_time,
        10.0 * 60.0,
        "ParkingLot to GasStation should be 10 minutes (nearby cluster)"
    );

    // Test technician state with manual dispatch simulation
    {
        let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
        tech_state.current_site_id = Some(site_a);
        tech_state.mode = TechnicianMode::EnRoute {
            request_id: RepairRequestId(1),
            charger_entity: Entity::PLACEHOLDER,
            site_id: site_b,
            travel_remaining: travel_time,
            travel_total: travel_time,
            repair_remaining: 0.0,
        };
    }

    // Verify travel progress at start
    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.travel_progress(),
        0.0,
        "Progress should be 0% at start"
    );

    // Simulate half travel
    let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
    tech_state.mode = TechnicianMode::EnRoute {
        request_id: RepairRequestId(1),
        charger_entity: Entity::PLACEHOLDER,
        site_id: site_b,
        travel_remaining: travel_time / 2.0,
        travel_total: travel_time,
        repair_remaining: 0.0,
    };

    let tech_state = app.world().resource::<TechnicianState>();
    assert!(
        (tech_state.travel_progress() - 0.5).abs() < 0.01,
        "Progress should be ~50%"
    );

    // Simulate arrival
    let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
    tech_state.mode = TechnicianMode::EnRoute {
        request_id: RepairRequestId(1),
        charger_entity: Entity::PLACEHOLDER,
        site_id: site_b,
        travel_remaining: 0.0,
        travel_total: travel_time,
        repair_remaining: 0.0,
    };

    let tech_state = app.world().resource::<TechnicianState>();
    assert_eq!(
        tech_state.travel_progress(),
        1.0,
        "Progress should be 100% on arrival"
    );
}

#[test]
fn test_technician_stays_at_site_no_travel() {
    let mut app = create_multi_site_test_app();

    // Rent 1 site
    let site_a = rent_test_site(&mut app, 0);

    // Simulate technician at site A
    let mut tech_state = app.world_mut().resource_mut::<TechnicianState>();
    tech_state.current_site_id = Some(site_a);

    // Get the archetype
    let multi_site = app.world().resource::<MultiSiteManager>();
    let archetype_a = multi_site.get_site(site_a).unwrap().archetype;

    // Calculate travel time to same site
    let travel_time_same_site = calculate_travel_time(archetype_a, archetype_a);

    // Should be quick (5 minutes) for same archetype
    assert_eq!(
        travel_time_same_site,
        5.0 * 60.0,
        "Same archetype travel should be 5 minutes"
    );

    // But if already at the exact same site, dispatch system should set travel to 0
    // This is tested in the dispatch system logic
}

#[test]
fn test_travel_time_matrix() {
    // Test same archetype (quick travel - 5 min)
    let same_archetype_time =
        calculate_travel_time(SiteArchetype::ParkingLot, SiteArchetype::ParkingLot);
    assert_eq!(
        same_archetype_time,
        5.0 * 60.0,
        "Same archetype should be 5 minutes"
    );

    // Test commercial cluster (short - 10 min)
    let commercial_time =
        calculate_travel_time(SiteArchetype::ParkingLot, SiteArchetype::GasStation);
    assert_eq!(
        commercial_time,
        10.0 * 60.0,
        "Commercial cluster should be 10 minutes"
    );

    // Test to/from fleet depot (long - 35 min)
    let to_fleet_time = calculate_travel_time(SiteArchetype::ParkingLot, SiteArchetype::FleetDepot);
    assert_eq!(
        to_fleet_time,
        35.0 * 60.0,
        "To fleet depot should be 35 minutes"
    );

    // Test default (medium - 20 min)
    let default_time = calculate_travel_time(SiteArchetype::GasStation, SiteArchetype::FleetDepot);
    assert_eq!(
        default_time,
        35.0 * 60.0,
        "GasStation to FleetDepot should be 35 minutes"
    );
}

#[test]
fn test_all_site_archetypes_can_be_loaded() {
    let mut app = create_multi_site_test_app();
    let multi_site = app.world().resource::<MultiSiteManager>();

    assert_eq!(
        multi_site.available_sites.len(),
        3,
        "Should have 3 available site archetypes"
    );

    let archetypes: Vec<SiteArchetype> = multi_site
        .available_sites
        .iter()
        .map(|listing| listing.archetype)
        .collect();

    assert!(
        archetypes.contains(&SiteArchetype::ParkingLot),
        "Missing ParkingLot"
    );
    assert!(
        archetypes.contains(&SiteArchetype::GasStation),
        "Missing GasStation"
    );
    assert!(
        archetypes.contains(&SiteArchetype::FleetDepot),
        "Missing FleetDepot"
    );

    let template_cache = SiteTemplateCache::default();
    let tiled_assets = Assets::<TiledMapAsset>::default();
    let game_data = GameDataAssets::default();
    for (i, listing) in multi_site.available_sites.clone().iter().enumerate() {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        let site_id = multi_site
            .rent_site(listing, &template_cache, &tiled_assets, &game_data)
            .unwrap_or_else(|e| panic!("Failed to rent site {} ({}): {}", i, listing.name, e));

        let site = multi_site.get_site(site_id).unwrap();
        assert!(
            site.grid_capacity_kva > 0.0,
            "Site {} has invalid grid capacity: {}",
            listing.name,
            site.grid_capacity_kva
        );
        assert_eq!(
            site.effective_capacity_kva(),
            site.grid_capacity_kva,
            "Site {} should have grid capacity as effective capacity for L2-only sites",
            listing.name
        );
        assert!(
            site.popularity > 0.0,
            "Site {} has invalid popularity: {}",
            listing.name,
            site.popularity
        );
        assert_eq!(
            site.archetype, listing.archetype,
            "Site {} archetype mismatch",
            listing.name
        );
    }

    let multi_site = app.world().resource::<MultiSiteManager>();
    assert_eq!(
        multi_site.owned_sites.len(),
        3,
        "Should have 3 owned sites after renting all"
    );
}

#[test]
fn test_dispatch_limit_uses_grid_capacity_not_transformer() {
    let mut app = create_multi_site_test_app();

    let site_a = rent_test_site(&mut app, 0); // 1500 kVA grid

    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
    let state = multi_site.get_site_mut(site_a).unwrap();

    // Without transformer: dispatch limit = grid capacity
    assert_eq!(
        state.dispatch_limit_kva(),
        1500.0,
        "dispatch_limit_kva should equal grid capacity without transformer"
    );

    // Place a 100 kVA transformer
    state.grid.unlock_tile(1, 1);
    state.grid.unlock_tile(2, 1);
    state.grid.unlock_tile(1, 2);
    state.grid.unlock_tile(2, 2);
    state
        .grid
        .set_tile_content(1, 1, kilowatt_tycoon::resources::TileContent::Grass);
    state
        .grid
        .set_tile_content(2, 1, kilowatt_tycoon::resources::TileContent::Grass);
    state
        .grid
        .set_tile_content(1, 2, kilowatt_tycoon::resources::TileContent::Grass);
    state
        .grid
        .set_tile_content(2, 2, kilowatt_tycoon::resources::TileContent::Grass);
    state.grid.place_transformer(1, 1, 100.0).unwrap();

    // effective_capacity caps at transformer: min(1500, 100) = 100
    assert_eq!(
        state.effective_capacity_kva(),
        100.0,
        "effective_capacity_kva should cap at transformer rating"
    );

    // dispatch_limit should still use grid capacity (transformer overload is thermal)
    assert_eq!(
        state.dispatch_limit_kva(),
        1500.0,
        "dispatch_limit_kva should still equal grid capacity with transformer"
    );
}
