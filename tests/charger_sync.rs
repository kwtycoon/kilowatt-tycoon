//! ECS integration tests for charger sync system.
//!
//! Tests that charger placement on the grid results in proper Charger entity
//! and ChargerSprite entity spawning via sync_chargers_with_grid.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapAsset;
use std::collections::HashMap;

use kilowatt_tycoon::components::BelongsToSite;
use kilowatt_tycoon::components::charger::{Charger, ChargerSprite};
use kilowatt_tycoon::resources::{
    ChargerPadType, GameDataAssets, ImageAssets, MultiSiteManager, SiteArchetype, SiteId,
    SiteListingInfo, SiteTemplateCache, TileContent,
};
use kilowatt_tycoon::systems::scene::{
    ChargerSyncRevision, compute_charger_diff, sync_chargers_with_grid,
};

/// Create a minimal test app with resources needed for charger sync.
fn create_charger_sync_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::prelude::ImagePlugin::default());

    // Initialize required resources
    app.init_resource::<MultiSiteManager>();
    app.init_resource::<ChargerSyncRevision>();
    app.init_resource::<SiteTemplateCache>();
    app.insert_resource(ImageAssets::default());

    app
}

/// Set up a site with a parking bay and lot tile for charger placement.
fn setup_site_with_parking_bay(app: &mut App) -> (SiteId, Entity) {
    // Spawn a site root entity
    let root_entity = app.world_mut().spawn(Transform::default()).id();

    // Create a test site listing
    let listing = SiteListingInfo {
        archetype: SiteArchetype::ParkingLot,
        name: "Test Site".to_string(),
        description: "Test site for unit tests".to_string(),
        rent_cost: 0.0,
        grid_capacity_kva: 500.0,
        popularity: 50.0,
        challenge_level: 1,
        grid_size: (16, 12),
    };

    // Get the MultiSiteManager and add a site
    let template_cache = SiteTemplateCache::default();
    let tiled_assets = Assets::<TiledMapAsset>::default();
    let game_data = GameDataAssets::default();
    let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();

    // Rent the site
    let site_id = multi_site
        .rent_site(&listing, &template_cache, &tiled_assets, &game_data)
        .expect("Should rent site");

    // Get mutable access to the site and set up the grid
    if let Some(site) = multi_site.get_site_mut(site_id) {
        // Set the root entity
        site.root_entity = Some(root_entity);

        // Set up a parking bay at (5, 5) and Lot at (5, 6)
        site.grid
            .set_tile_content(5, 5, TileContent::ParkingBaySouth);
        site.grid.set_tile_content(5, 6, TileContent::Lot);

        // Reset revision to a known value
        site.grid.revision = 1;
    }

    // Switch to this site to make it active
    let _ = multi_site.switch_to_site(site_id);

    (site_id, root_entity)
}

/// Count Charger entities in the world.
fn count_chargers(app: &mut App) -> usize {
    let mut count = 0;
    let mut query = app.world_mut().query::<&Charger>();
    for _ in query.iter(app.world()) {
        count += 1;
    }
    count
}

/// Count ChargerSprite entities in the world.
fn count_charger_sprites(app: &mut App) -> usize {
    let mut count = 0;
    let mut query = app.world_mut().query::<&ChargerSprite>();
    for _ in query.iter(app.world()) {
        count += 1;
    }
    count
}

/// Get the first Charger's grid position.
fn get_first_charger_position(app: &mut App) -> Option<(i32, i32)> {
    let mut query = app.world_mut().query::<&Charger>();
    if let Some(charger) = query.iter(app.world()).next() {
        return charger.grid_position;
    }
    None
}

/// Get the first ChargerSprite's charger entity reference.
fn get_first_charger_sprite_ref(app: &mut App) -> Option<Entity> {
    let mut query = app.world_mut().query::<&ChargerSprite>();
    query
        .iter(app.world())
        .next()
        .map(|sprite| sprite.charger_entity)
}

/// Check if a Charger entity exists with the given site_id.
fn charger_has_site_id(app: &mut App, expected_site_id: SiteId) -> bool {
    let mut query = app.world_mut().query::<(&Charger, &BelongsToSite)>();
    for (_, belongs) in query.iter(app.world()) {
        if belongs.site_id == expected_site_id {
            return true;
        }
    }
    false
}

#[test]
fn compute_charger_diff_finds_chargers_to_spawn() {
    // Create a grid with a ChargerPad
    let mut grid = kilowatt_tycoon::resources::SiteGrid::default();
    grid.set_tile_content(5, 5, TileContent::ParkingBaySouth);
    grid.set_tile_content(5, 6, TileContent::Lot);
    grid.place_charger(5, 5, ChargerPadType::DCFC150).unwrap();

    // No existing charger entities
    let existing_positions: HashMap<(i32, i32), Entity> = HashMap::new();

    // Compute diff
    let diff = compute_charger_diff(&grid, &existing_positions);

    // Should have one charger to spawn at (5, 6) - the ChargerPad position
    assert_eq!(diff.to_spawn.len(), 1);
    assert_eq!(diff.to_spawn[0].0, 5); // x
    assert_eq!(diff.to_spawn[0].1, 6); // y
    assert_eq!(diff.to_spawn[0].2, ChargerPadType::DCFC150);
    assert!(diff.to_despawn.is_empty());
}

#[test]
fn compute_charger_diff_finds_chargers_to_despawn() {
    // Create a grid with no ChargerPads
    let grid = kilowatt_tycoon::resources::SiteGrid::default();

    // Simulate an existing charger entity at a position
    let fake_entity = Entity::from_bits(42);
    let mut existing_positions: HashMap<(i32, i32), Entity> = HashMap::new();
    existing_positions.insert((5, 6), fake_entity);

    // Compute diff
    let diff = compute_charger_diff(&grid, &existing_positions);

    // Should have one charger to despawn
    assert!(diff.to_spawn.is_empty());
    assert_eq!(diff.to_despawn.len(), 1);
    assert_eq!(diff.to_despawn[0].0, (5, 6));
    assert_eq!(diff.to_despawn[0].1, fake_entity);
}

#[test]
fn compute_charger_diff_no_changes_when_in_sync() {
    // Create a grid with a ChargerPad
    let mut grid = kilowatt_tycoon::resources::SiteGrid::default();
    grid.set_tile_content(5, 5, TileContent::ParkingBaySouth);
    grid.set_tile_content(5, 6, TileContent::Lot);
    grid.place_charger(5, 5, ChargerPadType::L2).unwrap();

    // Existing charger entity at the same position
    let fake_entity = Entity::from_bits(42);
    let mut existing_positions: HashMap<(i32, i32), Entity> = HashMap::new();
    existing_positions.insert((5, 6), fake_entity);

    // Compute diff
    let diff = compute_charger_diff(&grid, &existing_positions);

    // No changes needed
    assert!(diff.to_spawn.is_empty());
    assert!(diff.to_despawn.is_empty());
}

#[test]
fn sync_chargers_spawns_charger_entity_after_placement() {
    let mut app = create_charger_sync_test_app();
    let (site_id, _root_entity) = setup_site_with_parking_bay(&mut app);

    // Place a charger on the grid
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        if let Some(site) = multi_site.get_site_mut(site_id) {
            site.grid
                .place_charger(5, 5, ChargerPadType::DCFC150)
                .expect("Should place charger");
        }
    }

    // Add the sync system and run it
    app.add_systems(Update, sync_chargers_with_grid);
    app.update();

    // Verify charger was spawned
    assert_eq!(
        count_chargers(&mut app),
        1,
        "Should have spawned exactly one Charger"
    );
    assert_eq!(
        get_first_charger_position(&mut app),
        Some((5, 6)),
        "Charger should be at ChargerPad position"
    );
}

#[test]
fn sync_chargers_spawns_charger_sprite() {
    let mut app = create_charger_sync_test_app();
    let (site_id, _root_entity) = setup_site_with_parking_bay(&mut app);

    // Place a charger on the grid
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        if let Some(site) = multi_site.get_site_mut(site_id) {
            site.grid.place_charger(5, 5, ChargerPadType::L2).unwrap();
        }
    }

    // Add the sync system and run it
    app.add_systems(Update, sync_chargers_with_grid);
    app.update();

    // Verify sprite was spawned
    assert_eq!(
        count_charger_sprites(&mut app),
        1,
        "Should have spawned exactly one ChargerSprite"
    );

    // Verify the sprite references a valid charger entity
    let charger_entity = get_first_charger_sprite_ref(&mut app).expect("Should have ChargerSprite");
    let charger = app.world().get::<Charger>(charger_entity);
    assert!(
        charger.is_some(),
        "ChargerSprite should reference a valid Charger entity"
    );
}

#[test]
fn sync_chargers_skips_when_revision_unchanged() {
    let mut app = create_charger_sync_test_app();
    let (site_id, _root_entity) = setup_site_with_parking_bay(&mut app);

    // Place a charger and run sync once
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        if let Some(site) = multi_site.get_site_mut(site_id) {
            site.grid
                .place_charger(5, 5, ChargerPadType::DCFC50)
                .unwrap();
        }
    }

    app.add_systems(Update, sync_chargers_with_grid);
    app.update();

    // Count chargers after first sync
    let charger_count_1 = count_chargers(&mut app);
    assert_eq!(charger_count_1, 1);

    // Run update again without grid changes
    app.update();

    // Should still have exactly one charger (no duplicate spawns)
    let charger_count_2 = count_chargers(&mut app);
    assert_eq!(
        charger_count_2, 1,
        "Should not spawn duplicate chargers when revision unchanged"
    );
}

#[test]
fn sync_chargers_belongs_to_site_marker() {
    let mut app = create_charger_sync_test_app();
    let (site_id, _root_entity) = setup_site_with_parking_bay(&mut app);

    // Place a charger
    {
        let mut multi_site = app.world_mut().resource_mut::<MultiSiteManager>();
        if let Some(site) = multi_site.get_site_mut(site_id) {
            site.grid.place_charger(5, 5, ChargerPadType::L2).unwrap();
        }
    }

    app.add_systems(Update, sync_chargers_with_grid);
    app.update();

    // Verify charger has correct site_id
    assert!(
        charger_has_site_id(&mut app, site_id),
        "Charger should have correct site_id"
    );
}
