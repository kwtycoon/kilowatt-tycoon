//! Tiled map integration bridge
//!
//! This module provides a bridge between bevy_ecs_tiled visual rendering
//! and the game's existing SiteGrid-based game logic.
//!
//! ## Architecture
//!
//! The game uses a **hybrid approach**:
//! - **bevy_ecs_tiled** renders the tilemap visuals (from .tmx files)
//! - **SiteGrid** maintains gameplay state (from .site.json files)
//!
//! This separation allows:
//! - Visual level design in Tiled editor
//! - Existing game logic to work unchanged
//! - Player-placed objects (chargers, transformers) to overlay the base tilemap
//!
//! ## Usage
//!
//! When a site is rented, the system:
//! 1. Loads the JSON template into SiteGrid (existing flow)
//! 2. Spawns the corresponding Tiled map for visuals
//! 3. Positions the Tiled map at the site's world offset
//!
//! The SiteGrid remains the source of truth for:
//! - Building placement validation
//! - Charger positions
//! - Pathfinding/navigation
//! - Equipment tracking

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::resources::SiteId;

/// Component marker for Tiled map entities associated with a site
#[derive(Component)]
pub struct SiteTiledMap {
    /// The site this Tiled map belongs to
    pub site_id: SiteId,
}

/// Resource tracking Tiled map entities per site
#[derive(Resource, Default)]
pub struct TiledMapRegistry {
    /// Maps SiteId to the Tiled map entity
    pub maps: std::collections::HashMap<SiteId, Entity>,
}

impl TiledMapRegistry {
    /// Register a Tiled map entity for a site
    pub fn register(&mut self, site_id: SiteId, entity: Entity) {
        self.maps.insert(site_id, entity);
    }

    /// Get the Tiled map entity for a site
    pub fn get(&self, site_id: SiteId) -> Option<Entity> {
        self.maps.get(&site_id).copied()
    }

    /// Remove and return the Tiled map entity for a site
    pub fn remove(&mut self, site_id: SiteId) -> Option<Entity> {
        self.maps.remove(&site_id)
    }

    /// Check if a site has a Tiled map registered
    pub fn contains(&self, site_id: SiteId) -> bool {
        self.maps.contains_key(&site_id)
    }

    /// Iterator over all registered sites and their map entities
    pub fn iter(&self) -> impl Iterator<Item = (&SiteId, &Entity)> {
        self.maps.iter()
    }
}

/// Spawn a Tiled map for a site
///
/// The map is spawned as a child of the site's root entity if provided,
/// or at the site's world offset otherwise.
pub fn spawn_tiled_map_for_site(
    commands: &mut Commands,
    map_handle: Handle<TiledMapAsset>,
    site_id: SiteId,
    site_root: Option<Entity>,
    world_offset: Vec2,
    registry: &mut TiledMapRegistry,
) -> Entity {
    // Spawn the Tiled map entity
    let map_entity = commands
        .spawn((
            TiledMap(map_handle),
            SiteTiledMap { site_id },
            // Position at the site's world offset
            // Note: bevy_ecs_tiled uses bottom-left origin by default,
            // which matches our game's coordinate system
            Transform::from_translation(Vec3::new(
                world_offset.x + crate::resources::GRID_OFFSET_X,
                world_offset.y + crate::resources::GRID_OFFSET_Y,
                -10.0, // Behind game objects
            )),
            // Start hidden - site visibility system will show/hide
            Visibility::Hidden,
        ))
        .id();

    // If we have a site root, make the map a child
    if let Some(root) = site_root {
        commands.entity(root).add_child(map_entity);
    }

    // Register the map
    registry.register(site_id, map_entity);

    info!("Spawned Tiled map for site {:?}", site_id);
    map_entity
}

/// Despawn the Tiled map for a site
pub fn despawn_tiled_map_for_site(
    commands: &mut Commands,
    site_id: SiteId,
    registry: &mut TiledMapRegistry,
) {
    if let Some(map_entity) = registry.remove(site_id) {
        commands.entity(map_entity).try_despawn();
        info!("Despawned Tiled map for site {:?}", site_id);
    }
}

/// System to sync Tiled map visibility with site visibility
///
/// Shows the Tiled map when its site is the active/viewed site,
/// hides it otherwise.
pub fn sync_tiled_map_visibility(
    multi_site: Res<crate::resources::MultiSiteManager>,
    registry: Res<TiledMapRegistry>,
    mut visibility_query: Query<&mut Visibility, With<SiteTiledMap>>,
) {
    let viewed_site_id = multi_site.viewed_site_id;

    for (site_id, &map_entity) in registry.maps.iter() {
        if let Ok(mut visibility) = visibility_query.get_mut(map_entity) {
            *visibility = if Some(*site_id) == viewed_site_id {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}
