//! Site root entity management
//!
//! This module handles spawning and managing site root entities, which act as
//! parents for all entities belonging to a site. This enables automatic transform
//! propagation without manual world_offset calculations.

use bevy::prelude::*;
use bevy_northstar::prelude::*;

use crate::components::SiteRoot;
use crate::resources::MultiSiteManager;
use crate::resources::northstar_grid::{GridSyncRevision, create_northstar_grid, sync_grid_nav};
use crate::resources::site_grid::{GRID_OFFSET_X, GRID_OFFSET_Y, TILE_SIZE};

/// Spawn root entities for sites that don't have one yet
///
/// This system runs during the Playing state and creates a root entity for each
/// owned site that doesn't already have one. The root entity is positioned at
/// the site's world_offset and acts as a parent for all site entities.
///
/// Only the currently viewed site gets a CardinalGrid for pathfinding, since
/// bevy_northstar's built-in systems expect exactly one grid in the world.
pub fn spawn_missing_site_roots(mut commands: Commands, mut multi_site: ResMut<MultiSiteManager>) {
    let viewed_site_id = multi_site.viewed_site_id;

    for (site_id, site_state) in multi_site.owned_sites.iter_mut() {
        // Skip if site already has a root entity
        if site_state.root_entity.is_some() {
            continue;
        }

        // Calculate world offset for this site
        let world_offset = site_state.world_offset();

        // Only create CardinalGrid for the currently viewed site
        // bevy_northstar uses Single<&Grid<N>> which requires exactly one grid
        let is_viewed = viewed_site_id == Some(*site_id);

        let root_entity = if is_viewed {
            // Create the bevy_northstar pathfinding grid from our SiteGrid
            let grid = create_northstar_grid(&site_state.grid);
            let revision = GridSyncRevision::new(site_state.grid.revision);

            commands
                .spawn((
                    SiteRoot::new(*site_id),
                    grid,     // CardinalGrid - only for viewed site
                    revision, // Revision tracker for efficient rebuilds
                    Transform::from_xyz(world_offset.x, world_offset.y, 0.0),
                    GlobalTransform::default(),
                    Visibility::Visible,
                    InheritedVisibility::default(),
                ))
                .id()
        } else {
            // Non-viewed sites get root entity without CardinalGrid
            commands
                .spawn((
                    SiteRoot::new(*site_id),
                    Transform::from_xyz(world_offset.x, world_offset.y, 0.0),
                    GlobalTransform::default(),
                    Visibility::Visible,
                    InheritedVisibility::default(),
                ))
                .id()
        };

        // Store the root entity in the site state
        site_state.root_entity = Some(root_entity);

        if is_viewed {
            attach_debug_grid(&mut commands, root_entity, site_state);
        }

        info!(
            "Spawned root entity for site {:?} ({}) at offset ({:.0}, {:.0}){}",
            site_id,
            site_state.name,
            world_offset.x,
            world_offset.y,
            if is_viewed {
                " [with pathfinding grid]"
            } else {
                ""
            }
        );
    }
}

/// Rebuild pathfinding grids when site grids change.
///
/// This system checks if the SiteGrid has been modified (by tracking revision)
/// and rebuilds the bevy_northstar CardinalGrid only when necessary.
pub fn rebuild_site_pathfinding_grids(
    multi_site: Res<MultiSiteManager>,
    mut grids: Query<(&SiteRoot, &mut CardinalGrid, &mut GridSyncRevision)>,
) {
    for (site_root, mut grid, mut revision) in grids.iter_mut() {
        if let Some(site_state) = multi_site.get_site(site_root.site_id) {
            // Only rebuild if the SiteGrid revision has changed
            if site_state.grid.revision != revision.last_synced {
                sync_grid_nav(&mut grid, &site_state.grid);
                grid.build();
                revision.last_synced = site_state.grid.revision;
            }
        }
    }
}

/// Resource to track which site currently has the pathfinding grid.
#[derive(Resource, Default)]
pub struct ActivePathfindingGrid {
    pub site_id: Option<crate::resources::SiteId>,
}

/// Transfer the pathfinding grid when switching sites.
///
/// bevy_northstar requires exactly one CardinalGrid in the world. When the player
/// switches to view a different site, we remove the grid from the old site's root
/// entity and create a new one on the new site's root entity.
pub fn transfer_pathfinding_grid_on_site_switch(
    mut commands: Commands,
    multi_site: Res<MultiSiteManager>,
    mut active_grid: ResMut<ActivePathfindingGrid>,
    site_roots: Query<(Entity, &SiteRoot)>,
    grids: Query<Entity, With<CardinalGrid>>,
    children: Query<&Children>,
    debug_grids: Query<Entity, With<DebugGrid>>,
) {
    let viewed_site_id = multi_site.viewed_site_id;

    // No change in viewed site
    if active_grid.site_id == viewed_site_id {
        return;
    }

    // Remove CardinalGrid from old site root (if any exists)
    for grid_entity in grids.iter() {
        commands
            .entity(grid_entity)
            .remove::<CardinalGrid>()
            .remove::<GridSyncRevision>();
    }

    // Add CardinalGrid to new site root
    if let Some(new_site_id) = viewed_site_id {
        // Find the root entity for the new site
        let new_root = site_roots
            .iter()
            .find(|(_, root)| root.site_id == new_site_id)
            .map(|(entity, _)| entity);

        if let Some(root_entity) = new_root {
            // Get the site's grid to create the CardinalGrid
            if let Some(site_state) = multi_site.get_site(new_site_id) {
                let grid = create_northstar_grid(&site_state.grid);
                let revision = GridSyncRevision::new(site_state.grid.revision);

                commands.entity(root_entity).insert((grid, revision));

                attach_debug_grid_checked(
                    &mut commands,
                    root_entity,
                    site_state,
                    &children,
                    &debug_grids,
                );

                info!("Transferred pathfinding grid to site {:?}", new_site_id);
            }
        }
    }

    // Update tracking
    active_grid.site_id = viewed_site_id;
}

fn attach_debug_grid(
    commands: &mut Commands,
    root_entity: Entity,
    site_state: &crate::resources::SiteState,
) {
    let offset = site_state.world_offset()
        + Vec2::new(
            GRID_OFFSET_X + TILE_SIZE * 0.5,
            GRID_OFFSET_Y + TILE_SIZE * 0.5,
        );

    commands.entity(root_entity).with_children(|parent| {
        parent.spawn((
            DebugGridBuilder::new(TILE_SIZE as u32, TILE_SIZE as u32)
                .enable_cells()
                .enable_entrances()
                .build(),
            DebugOffset(offset.extend(0.0)),
        ));
    });
}

fn attach_debug_grid_checked(
    commands: &mut Commands,
    root_entity: Entity,
    site_state: &crate::resources::SiteState,
    children: &Query<&Children>,
    debug_grids: &Query<Entity, With<DebugGrid>>,
) {
    if let Ok(children) = children.get(root_entity)
        && children.iter().any(|child| debug_grids.get(child).is_ok())
    {
        return;
    }

    attach_debug_grid(commands, root_entity, site_state);
}
