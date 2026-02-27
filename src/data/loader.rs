//! JSON data types and layout application
//!
//! This module contains data structures for game data loaded from JSON
//! and utilities for applying template layouts to grids.
//!
//! Note: JSON loading is now handled via Bevy's AssetServer (see json_assets.rs).
//! The types here are used by the asset loaders and for runtime data manipulation.

use bevy::prelude::*;

use crate::resources::{ChargerData, SiteGrid, TileContent};

/// Loaded charger data (temporary storage before spawning)
#[derive(Resource, Debug, Clone, Default)]
pub struct LoadedChargers(pub Vec<ChargerData>);

/// Result of loading scenario data
#[derive(Resource, Debug, Clone, Default)]
pub struct ScenarioLoadResult {
    pub site_loaded: bool,
    pub chargers_loaded: usize,
    pub drivers_loaded: usize,
    pub errors: Vec<String>,
}

/// Site template data loaded from JSON
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SiteTemplateData {
    pub archetype: String,
    pub name: String,
    pub grid_size: [i32; 2],
    pub rent_cost: f32,
    pub popularity: f32,
    pub challenge_level: u8,
    pub grid_capacity_kva: f32,
    pub description: String,
    pub initial_layout: Option<InitialLayout>,
}

/// Initial layout configuration for a site template
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitialLayout {
    pub locked_tiles: Vec<LockedTile>,
    pub suggested_zones: Vec<SuggestedZone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_pos: Option<(i32, i32)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_pos: Option<(i32, i32)>,
}

/// A locked tile in the initial layout
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockedTile {
    pub pos: [i32; 2],
    pub content: String, // "Entry", "Exit", "Road", "ParkingBay", etc.
}

/// A suggested zone for initial placement
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuggestedZone {
    #[serde(rename = "type")]
    pub zone_type: String,
    pub bounds: [[i32; 2]; 2], // [[x1,y1], [x2,y2]]
}

/// Parse tile content string to TileContent enum
fn parse_tile_content(content_str: &str) -> TileContent {
    match content_str {
        "Empty" => TileContent::Empty,
        "Grass" => TileContent::Grass,
        "Road" => TileContent::Road,
        "Lot" => TileContent::Lot,
        "ParkingBay" => TileContent::ParkingBaySouth, // Legacy: default to south-facing
        "ParkingBayNorth" => TileContent::ParkingBayNorth,
        "ParkingBaySouth" => TileContent::ParkingBaySouth,
        "ChargerPad" => TileContent::ChargerPad,
        "Entry" => TileContent::Entry,
        "Exit" => TileContent::Exit,
        // Gas station specific tiles
        "StoreWall" => TileContent::StoreWall,
        "StoreEntrance" => TileContent::StoreEntrance,
        "Storefront" => TileContent::Storefront,
        "PumpIsland" => TileContent::PumpIsland,
        "Canopy" => TileContent::Canopy,
        "FuelCap" => TileContent::FuelCap,
        "Dumpster" => TileContent::DumpsterPad,
        "DumpsterPad" => TileContent::DumpsterPad,
        "DumpsterOccupied" => TileContent::DumpsterOccupied,
        "CanopyShadow" => TileContent::CanopyShadow,
        "CanopyColumn" => TileContent::CanopyColumn,
        "GasStationSign" => TileContent::GasStationSign,
        "Bollard" => TileContent::Bollard,
        "WheelStop" => TileContent::WheelStop,
        "StreetTree" => TileContent::StreetTree,
        "LightPole" => TileContent::LightPole,
        // Worn asphalt variations
        "AsphaltWorn" => TileContent::AsphaltWorn,
        "AsphaltSkid" => TileContent::AsphaltSkid,
        // Mall/Garage tiles
        "GarageFloor" => TileContent::GarageFloor,
        "GaragePillar" => TileContent::GaragePillar,
        "MallFacade" => TileContent::MallFacade,
        // Workplace tiles
        "ReservedSpot" => TileContent::ReservedSpot,
        "OfficeBackdrop" => TileContent::OfficeBackdrop,
        // Transit tiles
        "LoadingZone" => TileContent::LoadingZone,
        // Other tiles
        "Concrete" => TileContent::Concrete,
        "Planter" => TileContent::Planter,
        _ => {
            warn!(
                "Unknown tile content type '{}', defaulting to Grass",
                content_str
            );
            TileContent::Grass
        }
    }
}

/// Apply a zone to the grid
fn apply_zone(grid: &mut SiteGrid, zone: &SuggestedZone) {
    let [[x1, y1], [x2, y2]] = zone.bounds;

    match zone.zone_type.as_str() {
        "parking_area" => {
            let min_x = x1.min(x2);
            let max_x = x1.max(x2);
            let min_y = y1.min(y2);
            let max_y = y1.max(y2);

            // Fill main parking area with lot surface
            grid.fill_lot_area(min_x, min_y, max_x, max_y);

            // Connect parking area to road (road is typically at y=11, top edge)
            // Fill driveway from parking area up to road
            if max_y < 11 {
                grid.fill_lot_area(min_x, max_y, max_x, 11);
            }

            // Place parking bays with room for charger pads above
            // Every 2 tiles horizontally, every 4 rows vertically (was 3)
            // This leaves room for ChargerPad tiles at (x, y+1) for each bay at (x, y)
            for y in (min_y..max_y).step_by(4) {
                for x in (min_x + 1..max_x).step_by(2) {
                    // Skip if not on lot surface
                    if grid.get_content(x, y) == TileContent::Lot {
                        // Ensure there's room for ChargerPad at (x, y+1)
                        if y < max_y && grid.get_content(x, y + 1) == TileContent::Lot {
                            let _ = grid.place_parking_bay(x, y);
                        }
                    }
                }
            }
        }
        "transformer_zone" | "solar_zone" | "battery_zone" | "canopy_zone" | "store_wall" => {
            // These are suggestions for player placement, not auto-placed
            // Just log them for now
            info!("Template suggests {} at {:?}", zone.zone_type, zone.bounds);
        }
        _ => {
            warn!("Unknown zone type: {}", zone.zone_type);
        }
    }
}

/// Apply initial layout from template to a grid
pub fn apply_initial_layout(grid: &mut SiteGrid, layout: &InitialLayout) {
    info!(
        "Applying initial layout with {} locked tiles, {} zones",
        layout.locked_tiles.len(),
        layout.suggested_zones.len()
    );

    // Apply locked tiles first
    for locked in &layout.locked_tiles {
        let [x, y] = locked.pos;
        let content = parse_tile_content(&locked.content);
        grid.set_tile_content(x, y, content);
        grid.lock_tile(x, y);
        info!("  Locked tile at ({}, {}): {:?}", x, y, content);
    }

    // Apply suggested zones
    for zone in &layout.suggested_zones {
        info!("  Applying zone: {} at {:?}", zone.zone_type, zone.bounds);
        apply_zone(grid, zone);
    }

    // Mark grid as changed (updates revision counter + legacy flag)
    grid.mark_changed();

    // Count parking bays
    let parking_bay_count = grid.get_parking_bays().len();
    info!(
        "Applied initial layout: {} parking bays created, revision={}",
        parking_bay_count, grid.revision
    );
}

/// A fallible helper function: safely get charger by ID
pub fn get_charger_by_id<'a>(
    chargers: impl Iterator<Item = &'a crate::components::charger::Charger>,
    id: &str,
) -> crate::errors::ChargeOpsResult<&'a crate::components::charger::Charger> {
    use crate::errors::ChargeOpsError;
    chargers
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| ChargeOpsError::entity_not_found("Charger", id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_load_result_default() {
        let result = ScenarioLoadResult::default();
        assert!(!result.site_loaded);
        assert_eq!(result.chargers_loaded, 0);
        assert_eq!(result.drivers_loaded, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_parse_tile_content() {
        assert_eq!(parse_tile_content("Grass"), TileContent::Grass);
        assert_eq!(parse_tile_content("Road"), TileContent::Road);
        assert_eq!(parse_tile_content("Entry"), TileContent::Entry);
        assert_eq!(parse_tile_content("Unknown"), TileContent::Grass); // defaults to Grass
    }
}
