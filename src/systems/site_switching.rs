//! Site switching logic - handle viewing different sites

use bevy::prelude::*;

use crate::components::SiteRoot;
use crate::events::{SiteSoldEvent, SiteSwitchEvent};
use crate::resources::{GRID_OFFSET_X, GRID_OFFSET_Y, MultiSiteManager, TILE_SIZE};
use crate::systems::WorldCamera;

/// System to handle site switching events
pub fn handle_site_switch(
    mut switch_events: MessageReader<SiteSwitchEvent>,
    mut multi_site: ResMut<MultiSiteManager>,
) {
    for event in switch_events.read() {
        info!("Switching to site {:?}", event.target_site_id);

        // Switch active site ID
        if let Err(e) = multi_site.switch_to_site(event.target_site_id) {
            error!("Failed to switch site: {}", e);
            continue;
        }

        // Mark the target site's grid for visual refresh to ensure tiles render
        if let Some(site) = multi_site.get_site_mut(event.target_site_id) {
            site.grid.mark_changed();
        }

        info!("Switched to site {:?}", multi_site.viewed_site_id);
    }
}

/// System to clean up entities when a site is sold
///
/// With the site root hierarchy, we only need to despawn the root entity
/// and Bevy automatically despawns all children (tiles, vehicles, chargers, etc.)
pub fn cleanup_sold_site(
    mut commands: Commands,
    mut events: MessageReader<SiteSoldEvent>,
    site_roots: Query<(Entity, &SiteRoot)>,
) {
    for event in events.read() {
        info!("Cleaning up site {:?}", event.site_id);

        // Find and despawn the site root (all children despawn automatically)
        for (entity, site_root) in &site_roots {
            if site_root.site_id == event.site_id {
                commands.entity(entity).try_despawn();
                info!(
                    "Despawned site root for {:?} (all children removed)",
                    event.site_id
                );
                break;
            }
        }
    }
}

/// Update camera position when switching to a different site
pub fn update_camera_for_site(
    multi_site: Res<MultiSiteManager>,
    mut cameras: Query<&mut Transform, With<WorldCamera>>,
) {
    // Only update when active site changes
    if !multi_site.is_changed() {
        return;
    }

    if let Some(site_state) = multi_site.active_site() {
        let world_offset = site_state.world_offset();

        // Calculate grid center in world space
        let grid_width = site_state.grid.width as f32 * TILE_SIZE;
        let grid_height = site_state.grid.height as f32 * TILE_SIZE;
        let grid_center_x = GRID_OFFSET_X + grid_width / 2.0;
        let grid_center_y = GRID_OFFSET_Y + grid_height / 2.0;

        // Pan camera to the site's world position
        let target_x = world_offset.x + grid_center_x;
        let target_y = world_offset.y + grid_center_y;

        for mut transform in &mut cameras {
            transform.translation.x = target_x;
            transform.translation.y = target_y;
        }
    }
}
