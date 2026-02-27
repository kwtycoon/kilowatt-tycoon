//! Vehicle traffic components (grid route + footprint).

use bevy::prelude::*;

/// Size of a vehicle in grid tiles, along its travel direction.
#[derive(Component, Debug, Clone, Copy)]
pub struct VehicleFootprint {
    pub length_tiles: u8,
}

/// Grid tiles corresponding to each `VehicleMovement.waypoints` entry.
///
/// - `Some((x,y))` means the waypoint is the center of a grid tile.
/// - `None` means the waypoint is off-grid (e.g., padding before entry / after exit).
#[derive(Component, Debug, Clone)]
pub struct VehicleTileRoute {
    pub waypoint_tiles: Vec<Option<(i32, i32)>>,
}
