//! TMX map loading and parsing for gameplay data
//!
//! This module extracts gameplay configuration and layout data from Tiled TMX files,
//! making TMX the single source of truth for level data.
//!
//! ## Architecture
//!
//! - `TiledMapAsset` (from bevy_ecs_tiled) provides access to raw tiled-rs `Map` data
//! - We parse map properties for gameplay config (archetype, capacity, popularity, etc.)
//! - We parse tile layers to build `SiteGrid` with correct `TileContent` types
//! - We parse object layers for zones (parking_area, transformer_zone, etc.)
//!
//! ## Tile ID Mapping
//!
//! TMX tile IDs (with firstgid offset applied) map to `TileContent` via the tileset's
//! `content_type` property. See `assets/tilesets/kilowatt_tiles.tsx` for definitions.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::data::loader::{InitialLayout, LockedTile, SiteTemplateData, SuggestedZone};
use crate::resources::{SiteArchetype, SiteGrid, TileContent};

/// Parse a SiteArchetype from a string (from TMX property)
pub fn parse_archetype(s: &str) -> Option<SiteArchetype> {
    match s.to_lowercase().as_str() {
        "parking_lot" | "parkinglot" => Some(SiteArchetype::ParkingLot),
        "gas_station" | "gasstation" => Some(SiteArchetype::GasStation),
        "fleet_depot" | "fleetdepot" => Some(SiteArchetype::FleetDepot),
        _ => None,
    }
}

/// Parse TileContent from a content_type string (from tileset property)
pub fn parse_tile_content(content_type: &str) -> TileContent {
    match content_type {
        "Empty" => TileContent::Empty,
        "Grass" => TileContent::Grass,
        "Road" => TileContent::Road,
        "Entry" => TileContent::Entry,
        "Exit" => TileContent::Exit,
        "Lot" => TileContent::Lot,
        "ParkingBayNorth" => TileContent::ParkingBayNorth,
        "ParkingBaySouth" => TileContent::ParkingBaySouth,
        "Concrete" => TileContent::Concrete,
        "ChargerPad" => TileContent::ChargerPad,
        "TransformerPad" => TileContent::TransformerPad,
        "TransformerOccupied" => TileContent::TransformerOccupied,
        "SolarPad" => TileContent::SolarPad,
        "SolarOccupied" => TileContent::SolarOccupied,
        "BatteryPad" => TileContent::BatteryPad,
        "BatteryOccupied" => TileContent::BatteryOccupied,
        // Gas station
        "StoreWall" => TileContent::StoreWall,
        "StoreEntrance" => TileContent::StoreEntrance,
        "Storefront" => TileContent::Storefront,
        "PumpIsland" => TileContent::PumpIsland,
        "Canopy" => TileContent::Canopy,
        "FuelCap" => TileContent::FuelCap,
        "DumpsterPad" => TileContent::DumpsterPad,
        "DumpsterOccupied" => TileContent::DumpsterOccupied,
        "CanopyShadow" => TileContent::CanopyShadow,
        "CanopyColumn" => TileContent::CanopyColumn,
        "GasStationSign" => TileContent::GasStationSign,
        "Bollard" => TileContent::Bollard,
        "WheelStop" => TileContent::WheelStop,
        "StreetTree" => TileContent::StreetTree,
        "LightPole" => TileContent::LightPole,
        // Worn asphalt
        "AsphaltWorn" => TileContent::AsphaltWorn,
        "AsphaltSkid" => TileContent::AsphaltSkid,
        // Mall/Garage
        "GarageFloor" => TileContent::GarageFloor,
        "GaragePillar" => TileContent::GaragePillar,
        "MallFacade" => TileContent::MallFacade,
        // Workplace
        "ReservedSpot" => TileContent::ReservedSpot,
        "OfficeBackdrop" => TileContent::OfficeBackdrop,
        // Transit
        "LoadingZone" => TileContent::LoadingZone,
        // Planter
        "Planter" => TileContent::Planter,
        // Amenities
        "AmenityWifiRestrooms" => TileContent::AmenityWifiRestrooms,
        "AmenityLoungeSnacks" => TileContent::AmenityLoungeSnacks,
        "AmenityRestaurant" => TileContent::AmenityRestaurant,
        "AmenityOccupied" => TileContent::AmenityOccupied,
        // Road with yellow line (visually different, same gameplay as Road)
        "RoadYellowLine" => TileContent::Road,
        // Curbs (treat as not driveable grass/concrete edge)
        "CurbAsphaltGrass" | "CurbAsphaltConcrete" => TileContent::Grass,
        _ => {
            warn!(
                "Unknown tile content_type '{}', defaulting to Grass",
                content_type
            );
            TileContent::Grass
        }
    }
}

/// Map raw tile ID (0-indexed from tileset) to TileContent
/// Uses the tileset definition to look up content_type property
pub fn tile_id_to_content(tile_id: u32) -> TileContent {
    // Tile IDs from kilowatt_tiles.tsx (0-indexed)
    match tile_id {
        0 => TileContent::Grass,
        1 => TileContent::Road,
        2 => TileContent::Entry,
        3 => TileContent::Exit,
        4 => TileContent::Lot,
        5 => TileContent::ParkingBayNorth,
        6 => TileContent::ParkingBaySouth,
        7 => TileContent::Concrete,
        8 => TileContent::GarageFloor,
        9 => TileContent::GaragePillar,
        10 => TileContent::MallFacade,
        11 => TileContent::StoreWall,
        12 => TileContent::StoreEntrance,
        13 => TileContent::Storefront,
        14 => TileContent::PumpIsland,
        15 => TileContent::Canopy,
        16 => TileContent::FuelCap,
        17 => TileContent::CanopyShadow,
        18 => TileContent::Grass,
        19 => TileContent::Grass,
        20 => TileContent::Road,
        21 => TileContent::Road,
        22 => TileContent::ReservedSpot,
        23 => TileContent::OfficeBackdrop,
        24 => TileContent::Concrete,
        25 => TileContent::Road,
        26 => TileContent::Concrete,
        27 => TileContent::Grass,
        28 => TileContent::Grass,
        29 => TileContent::Concrete,
        30 => TileContent::LoadingZone,
        31 => TileContent::AsphaltWorn,
        32 => TileContent::AsphaltSkid,
        33 => TileContent::Planter,
        34 | 35 => TileContent::Grass, // Curbs
        36 => TileContent::ChargerPad,
        37 => TileContent::TransformerPad,
        38 => TileContent::SolarPad,
        39 => TileContent::BatteryPad,
        40 => TileContent::Empty,
        41 => TileContent::Bollard,
        42 => TileContent::WheelStop,
        43 => TileContent::StreetTree,
        44 => TileContent::LightPole,
        45 => TileContent::CanopyColumn,
        46 => TileContent::GasStationSign,
        47 => TileContent::DumpsterPad,
        48 => TileContent::DumpsterOccupied,
        49 => TileContent::TransformerOccupied,
        50 => TileContent::SolarOccupied,
        51 => TileContent::BatteryOccupied,
        52 => TileContent::AmenityWifiRestrooms,
        53 => TileContent::AmenityLoungeSnacks,
        54 => TileContent::AmenityRestaurant,
        55 => TileContent::AmenityOccupied,
        56 => TileContent::Road, // RoadYellowLine - same gameplay as Road
        57 => TileContent::Road, // Decorative road - same gameplay as Road
        _ => {
            warn!("Unknown tile ID {}, defaulting to Grass", tile_id);
            TileContent::Grass
        }
    }
}

/// Get a string property from tiled properties
fn get_string_property(properties: &tiled::Properties, name: &str) -> Option<String> {
    properties.get(name).and_then(|v| match v {
        tiled::PropertyValue::StringValue(s) => Some(s.clone()),
        _ => None,
    })
}

/// Get a float property from tiled properties
fn get_float_property(properties: &tiled::Properties, name: &str) -> Option<f32> {
    properties.get(name).and_then(|v| match v {
        tiled::PropertyValue::FloatValue(f) => Some(*f),
        tiled::PropertyValue::IntValue(i) => Some(*i as f32),
        _ => None,
    })
}

/// Get an int property from tiled properties
fn get_int_property(properties: &tiled::Properties, name: &str) -> Option<i32> {
    properties.get(name).and_then(|v| match v {
        tiled::PropertyValue::IntValue(i) => Some(*i),
        tiled::PropertyValue::FloatValue(f) => Some(*f as i32),
        _ => None,
    })
}

/// Get a bool property from tiled properties
#[allow(dead_code)]
fn get_bool_property(properties: &tiled::Properties, name: &str) -> Option<bool> {
    properties.get(name).and_then(|v| match v {
        tiled::PropertyValue::BoolValue(b) => Some(*b),
        _ => None,
    })
}

/// Extract SiteTemplateData from a TiledMapAsset's raw map
///
/// Reads map properties to populate gameplay config. Returns None if required
/// properties are missing.
pub fn extract_template_from_map(map: &tiled::Map) -> Option<SiteTemplateData> {
    let props = &map.properties;

    // Required properties
    let archetype_str = get_string_property(props, "archetype")?;
    let name = get_string_property(props, "site_name")?;

    // Optional properties with defaults
    let grid_capacity_kva = get_float_property(props, "grid_capacity_kva").unwrap_or(500.0);
    let popularity = get_float_property(props, "popularity").unwrap_or(50.0);
    let rent_cost = get_float_property(props, "rent_cost").unwrap_or(0.0);
    let challenge_level = get_int_property(props, "challenge_level").unwrap_or(1) as u8;
    let description = get_string_property(props, "description").unwrap_or_default();

    // Grid size from map dimensions
    let grid_size = [map.width as i32, map.height as i32];

    // Build initial layout from tile data and object zones
    let initial_layout = extract_initial_layout(map);

    Some(SiteTemplateData {
        archetype: archetype_str,
        name,
        grid_size,
        rent_cost,
        popularity,
        challenge_level,
        grid_capacity_kva,
        description,
        initial_layout: Some(initial_layout),
    })
}

/// Extract InitialLayout from TMX tile layer and object layer
fn extract_initial_layout(map: &tiled::Map) -> InitialLayout {
    let mut locked_tiles = Vec::new();
    let mut suggested_zones = Vec::new();
    let mut entry_pos = None;
    let mut exit_pos = None;

    let map_height = map.height as i32;

    // Find the tile layer named "tiles"
    for layer in map.layers() {
        match layer.layer_type() {
            tiled::LayerType::Tiles(tile_layer) => {
                if layer.name == "tiles" {
                    // Extract locked tiles from tile data
                    extract_locked_tiles_from_layer(tile_layer, map, map_height, &mut locked_tiles);
                }
            }
            tiled::LayerType::Objects(object_layer) => {
                if layer.name == "zones" {
                    // Extract zones from object layer
                    extract_zones_from_layer(
                        &object_layer,
                        map_height,
                        &mut suggested_zones,
                        &mut entry_pos,
                        &mut exit_pos,
                    );
                }
            }
            _ => {}
        }
    }

    InitialLayout {
        locked_tiles,
        suggested_zones,
        entry_pos,
        exit_pos,
    }
}

/// Extract locked tiles from a tile layer
fn extract_locked_tiles_from_layer(
    tile_layer: tiled::TileLayer,
    map: &tiled::Map,
    map_height: i32,
    locked_tiles: &mut Vec<LockedTile>,
) {
    let width = map.width;
    let height = map.height;

    for y in 0..height {
        for x in 0..width {
            if let Some(tile) = tile_layer.get_tile(x as i32, y as i32) {
                let tile_id = tile.id();
                let content = tile_id_to_content(tile_id);

                // Check if this tile should be locked (from tileset properties)
                let is_locked = should_tile_be_locked(tile_id, &content);

                if is_locked {
                    // Convert Tiled coordinates (top-left origin) to game coordinates (bottom-left origin)
                    let game_y = map_height - 1 - y as i32;
                    let content_str = tile_content_to_string(&content);

                    locked_tiles.push(LockedTile {
                        pos: [x as i32, game_y],
                        content: content_str,
                    });
                }
            }
        }
    }
}

/// Determine if a tile should be locked based on its ID and content
fn should_tile_be_locked(_tile_id: u32, content: &TileContent) -> bool {
    // Tiles that are typically locked (from template, not player-placed)
    // Note: Entry/Exit are no longer locked tiles - they're defined via zone objects
    matches!(
        content,
        TileContent::Road
            | TileContent::StoreWall
            | TileContent::StoreEntrance
            | TileContent::Storefront
            | TileContent::PumpIsland
            | TileContent::Canopy
            | TileContent::FuelCap
            | TileContent::CanopyShadow
            | TileContent::CanopyColumn
            | TileContent::GasStationSign
            | TileContent::DumpsterPad
            | TileContent::DumpsterOccupied
            | TileContent::GarageFloor
            | TileContent::GaragePillar
            | TileContent::MallFacade
            | TileContent::ReservedSpot
            | TileContent::OfficeBackdrop
            | TileContent::LoadingZone
            | TileContent::Concrete
            | TileContent::Bollard
            | TileContent::WheelStop
            | TileContent::StreetTree
            | TileContent::LightPole
            | TileContent::AsphaltWorn
            | TileContent::AsphaltSkid
            | TileContent::Planter
    )
}

/// Convert TileContent to string for serialization
fn tile_content_to_string(content: &TileContent) -> String {
    match content {
        TileContent::Empty => "Empty",
        TileContent::Grass => "Grass",
        TileContent::Road => "Road",
        TileContent::Entry => "Entry",
        TileContent::Exit => "Exit",
        TileContent::Lot => "Lot",
        TileContent::ParkingBayNorth => "ParkingBayNorth",
        TileContent::ParkingBaySouth => "ParkingBaySouth",
        TileContent::Concrete => "Concrete",
        TileContent::ChargerPad => "ChargerPad",
        TileContent::TransformerPad => "TransformerPad",
        TileContent::TransformerOccupied => "TransformerOccupied",
        TileContent::SolarPad => "SolarPad",
        TileContent::SolarOccupied => "SolarOccupied",
        TileContent::BatteryPad => "BatteryPad",
        TileContent::BatteryOccupied => "BatteryOccupied",
        TileContent::SecurityPad => "SecurityPad",
        TileContent::SecurityOccupied => "SecurityOccupied",
        TileContent::StoreWall => "StoreWall",
        TileContent::StoreEntrance => "StoreEntrance",
        TileContent::Storefront => "Storefront",
        TileContent::PumpIsland => "PumpIsland",
        TileContent::Canopy => "Canopy",
        TileContent::FuelCap => "FuelCap",
        TileContent::DumpsterPad => "DumpsterPad",
        TileContent::DumpsterOccupied => "DumpsterOccupied",
        TileContent::CanopyShadow => "CanopyShadow",
        TileContent::CanopyColumn => "CanopyColumn",
        TileContent::GasStationSign => "GasStationSign",
        TileContent::Bollard => "Bollard",
        TileContent::WheelStop => "WheelStop",
        TileContent::StreetTree => "StreetTree",
        TileContent::LightPole => "LightPole",
        TileContent::AsphaltWorn => "AsphaltWorn",
        TileContent::AsphaltSkid => "AsphaltSkid",
        TileContent::GarageFloor => "GarageFloor",
        TileContent::GaragePillar => "GaragePillar",
        TileContent::MallFacade => "MallFacade",
        TileContent::ReservedSpot => "ReservedSpot",
        TileContent::OfficeBackdrop => "OfficeBackdrop",
        TileContent::LoadingZone => "LoadingZone",
        TileContent::Planter => "Planter",
        TileContent::AmenityWifiRestrooms => "AmenityWifiRestrooms",
        TileContent::AmenityLoungeSnacks => "AmenityLoungeSnacks",
        TileContent::AmenityRestaurant => "AmenityRestaurant",
        TileContent::AmenityOccupied => "AmenityOccupied",
    }
    .to_string()
}

/// Extract zones from an object layer
fn extract_zones_from_layer(
    object_layer: &tiled::ObjectLayer,
    map_height: i32,
    zones: &mut Vec<SuggestedZone>,
    entry_pos: &mut Option<(i32, i32)>,
    exit_pos: &mut Option<(i32, i32)>,
) {
    let tile_size = 64.0; // Standard tile size

    for object in object_layer.objects() {
        // Object type determines zone type
        let zone_type = object.user_type.clone();
        if zone_type.is_empty() {
            continue;
        }

        // Get object shape - we only handle rectangles for zones
        let tiled::ObjectShape::Rect { width, height } = object.shape else {
            continue;
        };

        // Convert pixel coordinates to grid coordinates
        // Tiled uses top-left origin, game uses bottom-left
        let x1 = (object.x / tile_size) as i32;
        let y1_tiled = (object.y / tile_size) as i32;
        // Ensure at least 1 tile for zones that are smaller than a full tile
        let width_tiles = ((width / tile_size) as i32).max(1);
        let height_tiles = ((height / tile_size) as i32).max(1);

        // Convert to game coordinates (flip Y)
        // y1_tiled is the TOP of the object in Tiled (row index, 0 = top)
        // In game coords, Y=0 is bottom, Y=map_height-1 is top
        // So Tiled row 0 → game row (map_height - 1)
        let y_top_game = map_height - 1 - y1_tiled; // Top of zone in game coords
        let y1_game = y_top_game - height_tiles + 1; // Bottom of zone in game coords

        let x2 = x1 + width_tiles - 1;
        let y2 = y1_game + height_tiles - 1;

        // Handle special zone types for entry/exit
        match zone_type.as_str() {
            "entry" => {
                *entry_pos = Some((x1, y1_game));
                info!("Entry zone found at ({}, {})", x1, y1_game);
            }
            "exit" => {
                *exit_pos = Some((x1, y1_game));
                info!("Exit zone found at ({}, {})", x1, y1_game);
            }
            _ => {
                // Regular zones go into the suggested_zones list
                zones.push(SuggestedZone {
                    zone_type,
                    bounds: [[x1, y1_game], [x2, y2]],
                });
            }
        }
    }
}

/// Build a SiteGrid from TMX tile data
///
/// This creates a fully populated SiteGrid from the TMX file, including:
/// - All tile content types
/// - Locked tile flags
/// - Entry/exit positions (from zone objects only)
pub fn build_site_grid_from_map(
    map: &tiled::Map,
    entry_pos: (i32, i32),
    exit_pos: (i32, i32),
) -> SiteGrid {
    let width = map.width as i32;
    let height = map.height as i32;

    let mut grid = SiteGrid::new(width, height);

    // Find the tile layer named "tiles"
    for layer in map.layers() {
        if let tiled::LayerType::Tiles(tile_layer) = layer.layer_type()
            && layer.name == "tiles"
        {
            for tiled_y in 0..height {
                for x in 0..width {
                    if let Some(tile) = tile_layer.get_tile(x, tiled_y) {
                        let tile_id = tile.id();
                        let content = tile_id_to_content(tile_id);

                        // Convert Tiled Y to game Y (flip)
                        let game_y = height - 1 - tiled_y;

                        // Set tile content
                        grid.set_tile_content(x, game_y, content);

                        // Check for locked tiles
                        if should_tile_be_locked(tile_id, &content) {
                            grid.lock_tile(x, game_y);
                        }
                    }
                }
            }
        }
    }

    // Set entry/exit positions from zones
    grid.entry_pos = entry_pos;
    grid.exit_pos = exit_pos;

    grid.mark_changed();
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_archetype() {
        assert_eq!(
            parse_archetype("parking_lot"),
            Some(SiteArchetype::ParkingLot)
        );
        assert_eq!(
            parse_archetype("gas_station"),
            Some(SiteArchetype::GasStation)
        );
        assert_eq!(parse_archetype("unknown"), None);
    }

    #[test]
    fn test_tile_id_to_content() {
        assert_eq!(tile_id_to_content(0), TileContent::Grass);
        assert_eq!(tile_id_to_content(1), TileContent::Road);
        assert_eq!(tile_id_to_content(2), TileContent::Entry);
        assert_eq!(tile_id_to_content(3), TileContent::Exit);
        assert_eq!(tile_id_to_content(4), TileContent::Lot);
        assert_eq!(tile_id_to_content(8), TileContent::GarageFloor);
    }

    #[test]
    fn test_tile_content_round_trip() {
        let contents = [
            TileContent::Grass,
            TileContent::Road,
            TileContent::Entry,
            TileContent::Exit,
            TileContent::GarageFloor,
            TileContent::MallFacade,
        ];

        for content in contents {
            let s = tile_content_to_string(&content);
            let parsed = parse_tile_content(&s);
            assert_eq!(parsed, content);
        }
    }
}
