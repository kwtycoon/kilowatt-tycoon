//! Integration with bevy_northstar for spatial pathfinding.
//!
//! This module provides a bridge between our `SiteGrid` (tile content and layout)
//! and bevy_northstar's `Grid` (pathfinding infrastructure).

use bevy::prelude::*;
use bevy_northstar::prelude::*;

use crate::resources::site_grid::{SiteGrid, TileContent};

/// Creates a new bevy_northstar CardinalGrid from a SiteGrid.
///
/// The CardinalGrid is used for efficient spatial pathfinding (HPA*).
/// Vehicles can only move in 4 cardinal directions (no diagonal movement).
pub fn create_northstar_grid(site_grid: &SiteGrid) -> CardinalGrid {
    let settings = GridSettingsBuilder::new_2d(site_grid.width as u32, site_grid.height as u32)
        .chunk_size(4)
        .enable_collision()
        .avoidance_distance(5)
        .build();

    let mut grid = CardinalGrid::new(&settings);

    sync_grid_nav(&mut grid, site_grid);

    grid.build();
    grid
}

/// Syncs the navigation data from a SiteGrid to a CardinalGrid.
///
/// Call this after modifying the SiteGrid layout, then call `grid.build()`
/// to rebuild the HPA* hierarchy.
pub fn sync_grid_nav(grid: &mut CardinalGrid, site_grid: &SiteGrid) {
    for x in 0..site_grid.width {
        for y in 0..site_grid.height {
            let content = site_grid.get_content(x, y);
            let nav = tile_content_to_nav(content);
            grid.set_nav(UVec3::new(x as u32, y as u32, 0), nav);
        }
    }
}

/// Converts a TileContent to a bevy_northstar Nav value.
fn tile_content_to_nav(content: TileContent) -> Nav {
    // Driveable tiles (roads, lot surface, parking bays) are passable
    if content.is_driveable() || content.is_parking() {
        // Use cost of 1 for all passable tiles
        // We could add higher costs for congested areas via NavMask
        Nav::Passable(1)
    } else {
        Nav::Impassable
    }
}

/// Tracks the SiteGrid revision for efficient CardinalGrid rebuilds.
///
/// This component is paired with a `CardinalGrid` on the same entity.
/// The rebuild system checks if `SiteGrid.revision` has changed and
/// only rebuilds the grid when necessary.
#[derive(Component)]
pub struct GridSyncRevision {
    /// The SiteGrid revision we last synced from
    pub last_synced: u64,
}

impl GridSyncRevision {
    /// Creates a new revision tracker.
    pub fn new(revision: u64) -> Self {
        Self {
            last_synced: revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_northstar_grid() {
        let site_grid = SiteGrid::default();
        let grid = create_northstar_grid(&site_grid);

        // Check dimensions match
        assert_eq!(grid.width(), site_grid.width as u32);
        assert_eq!(grid.height(), site_grid.height as u32);

        // Entry and exit should be passable (they're road tiles)
        assert!(grid.is_passable(UVec3::new(
            site_grid.entry_pos.0 as u32,
            site_grid.entry_pos.1 as u32,
            0
        )));
        assert!(grid.is_passable(UVec3::new(
            site_grid.exit_pos.0 as u32,
            site_grid.exit_pos.1 as u32,
            0
        )));
    }

    #[test]
    fn test_tile_content_to_nav() {
        // Driveable tiles should be passable
        assert!(matches!(
            tile_content_to_nav(TileContent::Road),
            Nav::Passable(_)
        ));
        assert!(matches!(
            tile_content_to_nav(TileContent::Lot),
            Nav::Passable(_)
        ));
        assert!(matches!(
            tile_content_to_nav(TileContent::Entry),
            Nav::Passable(_)
        ));
        assert!(matches!(
            tile_content_to_nav(TileContent::Exit),
            Nav::Passable(_)
        ));

        // Parking bays should be passable (vehicles drive into them)
        assert!(matches!(
            tile_content_to_nav(TileContent::ParkingBayNorth),
            Nav::Passable(_)
        ));
        assert!(matches!(
            tile_content_to_nav(TileContent::ParkingBaySouth),
            Nav::Passable(_)
        ));

        // Non-driveable tiles should be impassable
        assert!(matches!(
            tile_content_to_nav(TileContent::Grass),
            Nav::Impassable
        ));
        assert!(matches!(
            tile_content_to_nav(TileContent::TransformerPad),
            Nav::Impassable
        ));
        assert!(matches!(
            tile_content_to_nav(TileContent::StoreWall),
            Nav::Impassable
        ));
    }
}
