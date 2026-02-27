//! Systems for managing Tiled map integration
//!
//! This module provides systems that spawn and manage Tiled maps for sites.
//! The Tiled maps provide the visual tilemap rendering while the SiteGrid
//! maintains gameplay state.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::resources::{
    GRID_OFFSET_X, GRID_OFFSET_Y, GameDataAssets, MultiSiteManager, SiteId, TILE_SIZE,
    TiledMapRegistry,
};

/// Marker component for Tiled map entities associated with a site
#[derive(Component)]
pub struct SiteTiledMap {
    /// The site this Tiled map belongs to
    pub site_id: SiteId,
}

/// System to spawn Tiled maps for newly rented sites
///
/// This system checks all owned sites and spawns a Tiled map for any
/// that don't have one yet. It runs during Update and uses the
/// TiledMapRegistry to track which sites have maps.
pub fn spawn_site_tiled_maps(
    mut commands: Commands,
    mut multi_site: ResMut<MultiSiteManager>,
    mut registry: ResMut<TiledMapRegistry>,
    game_data: Res<GameDataAssets>,
) {
    // Collect site IDs and archetypes that need Tiled maps
    let sites_to_spawn: Vec<_> = multi_site
        .owned_sites
        .iter()
        .filter_map(|(&site_id, site_state)| {
            // Skip if already has a Tiled map
            if site_state.tiled_map_entity.is_some() || registry.contains(site_id) {
                return None;
            }

            // Get the Tiled map handle for this archetype
            let map_handle = game_data.tiled_maps.get(&site_state.archetype)?;

            Some((
                site_id,
                site_state.archetype,
                site_state.world_offset(),
                site_state.root_entity,
                map_handle.clone(),
            ))
        })
        .collect();

    // Spawn Tiled maps
    for (site_id, archetype, world_offset, _root_entity, map_handle) in sites_to_spawn {
        // Spawn the Tiled map entity
        // NOTE: We don't make it a child of the site root because bevy_ecs_tilemap
        // handles its own entity hierarchy and parenting can cause rendering issues
        let map_entity = commands
            .spawn((
                TiledMap(map_handle),
                SiteTiledMap { site_id },
                // Position at the site's world offset
                // Add grid offset to align with existing sprite rendering
                // Also add half-tile offset because bevy_ecs_tilemap anchors tiles at corner
                // while our grid_to_world returns tile centers
                Transform::from_translation(Vec3::new(
                    world_offset.x + GRID_OFFSET_X + TILE_SIZE / 2.0,
                    world_offset.y + GRID_OFFSET_Y + TILE_SIZE / 2.0,
                    -1.0, // Just behind game objects (they use z ~0-10)
                )),
                // Start visible - sync system will hide other sites
                Visibility::Inherited,
                // Ensure on layer 0 (world layer) so WorldCamera can see it
                RenderLayers::layer(0),
            ))
            .id();

        // Register in the registry
        registry.register(site_id, map_entity);

        // Update the site state
        if let Some(site_state) = multi_site.owned_sites.get_mut(&site_id) {
            site_state.tiled_map_entity = Some(map_entity);
        }

        info!("Spawned Tiled map for site {:?} ({:?})", site_id, archetype);
    }
}

/// System to sync Tiled map visibility with site switching
///
/// Shows the Tiled map for the currently viewed site, hides all others.
pub fn sync_tiled_map_visibility(
    multi_site: Res<MultiSiteManager>,
    registry: Res<TiledMapRegistry>,
    mut visibility_query: Query<&mut Visibility, With<SiteTiledMap>>,
) {
    let viewed_site_id = multi_site.viewed_site_id;

    for (&site_id, &map_entity) in registry.maps.iter() {
        if let Ok(mut visibility) = visibility_query.get_mut(map_entity) {
            *visibility = if Some(site_id) == viewed_site_id {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

/// System to clean up Tiled maps when sites are sold
pub fn despawn_sold_site_tiled_maps(
    mut commands: Commands,
    mut registry: ResMut<TiledMapRegistry>,
    multi_site: Res<MultiSiteManager>,
) {
    // Find maps for sites that no longer exist
    let maps_to_remove: Vec<_> = registry
        .maps
        .keys()
        .filter(|site_id| !multi_site.owned_sites.contains_key(site_id))
        .copied()
        .collect();

    // Despawn them
    for site_id in maps_to_remove {
        if let Some(map_entity) = registry.remove(site_id) {
            commands.entity(map_entity).try_despawn();
            info!("Despawned Tiled map for sold site {:?}", site_id);
        }
    }
}
