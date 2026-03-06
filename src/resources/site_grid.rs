//! Site grid for tile-based placement

use bevy::prelude::*;
use std::collections::HashMap;

/// Placement info for a transformer on the grid
#[derive(Debug, Clone)]
pub struct TransformerPlacement {
    /// Anchor position (bottom-left of 2x2)
    pub pos: (i32, i32),
    /// Transformer rating in kVA
    pub kva: f32,
    /// Entity reference (set when logic entity is spawned)
    pub entity: Option<Entity>,
}

/// Size of each tile in world pixels
pub const TILE_SIZE: f32 = 64.0;

/// Default grid dimensions (used when no TMX map specifies a size)
pub const GRID_WIDTH: i32 = 16;
pub const GRID_HEIGHT: i32 = 12;

/// Grid offset from world origin (to center the grid in the view)
pub const GRID_OFFSET_X: f32 = 50.0;
pub const GRID_OFFSET_Y: f32 = 50.0;

/// Structure sizes (width x height in tiles)
/// All structures use bottom-left as anchor point
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructureSize {
    /// 1x1 tile (default for single-tile items)
    Single,
    /// 2x2 tiles (transformer, battery)
    TwoByTwo,
    /// 3x2 tiles wide (solar array)
    ThreeByTwo,
    /// 3x3 tiles (WiFi+Restrooms amenity)
    ThreeByThree,
    /// 4x4 tiles (Lounge+Snacks amenity)
    FourByFour,
    /// 5x4 tiles wide (Premium Restaurant amenity)
    FiveByFour,
}

impl StructureSize {
    /// Get (width, height) in tiles
    pub fn dimensions(&self) -> (i32, i32) {
        match self {
            StructureSize::Single => (1, 1),
            StructureSize::TwoByTwo => (2, 2),
            StructureSize::ThreeByTwo => (3, 2),
            StructureSize::ThreeByThree => (3, 3),
            StructureSize::FourByFour => (4, 4),
            StructureSize::FiveByFour => (5, 4),
        }
    }

    /// Get total tile count
    pub fn tile_count(&self) -> i32 {
        let (w, h) = self.dimensions();
        w * h
    }
}

/// Amenity type for placeable amenity buildings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmenityType {
    /// WiFi + Restrooms (Level 1) - 3x3 tiles
    WifiRestrooms,
    /// Lounge + Snacks (Level 2) - 4x4 tiles
    LoungeSnacks,
    /// Premium Restaurant (Level 3) - 5x4 tiles
    Restaurant,
    /// Driver Rest Lounge - 3x3 tiles (dormitory-style rest for gig workers)
    DriverRestLounge,
}

impl AmenityType {
    /// Get the structure size for this amenity
    pub fn size(&self) -> StructureSize {
        match self {
            AmenityType::WifiRestrooms => StructureSize::ThreeByThree,
            AmenityType::LoungeSnacks => StructureSize::FourByFour,
            AmenityType::Restaurant => StructureSize::FiveByFour,
            AmenityType::DriverRestLounge => StructureSize::ThreeByThree,
        }
    }

    /// Get the amenity level (for strategy integration)
    pub fn level(&self) -> u32 {
        match self {
            AmenityType::WifiRestrooms => 1,
            AmenityType::LoungeSnacks => 2,
            AmenityType::Restaurant => 3,
            AmenityType::DriverRestLounge => 4,
        }
    }

    /// Get the TileContent for this amenity's anchor tile
    pub fn tile_content(&self) -> TileContent {
        match self {
            AmenityType::WifiRestrooms => TileContent::AmenityWifiRestrooms,
            AmenityType::LoungeSnacks => TileContent::AmenityLoungeSnacks,
            AmenityType::Restaurant => TileContent::AmenityRestaurant,
            AmenityType::DriverRestLounge => TileContent::AmenityDriverRestLounge,
        }
    }
}

/// What type of content a tile contains
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TileContent {
    #[default]
    Empty,
    Grass,
    Road,            // Public road (edge of site)
    Lot,             // Lot surface (asphalt parking area, driveway)
    ParkingBayNorth, // Parking stall facing up (car enters from south, charger at y-1)
    ParkingBaySouth, // Parking stall facing down (car enters from north, charger at y+1)
    ChargerPad,
    TransformerPad,          // Transformer anchor (2x2)
    TransformerOccupied,     // Other tiles occupied by transformer
    SolarPad,                // Solar array anchor (3x2)
    SolarOccupied,           // Other tiles occupied by solar
    BatteryPad,              // Battery anchor (2x2)
    BatteryOccupied,         // Other tiles occupied by battery
    SecurityPad,             // Security system anchor (2x2)
    SecurityOccupied,        // Other tiles occupied by security system
    AmenityWifiRestrooms,    // WiFi+Restrooms anchor (3x3)
    AmenityLoungeSnacks,     // Lounge+Snacks anchor (4x4)
    AmenityRestaurant,       // Restaurant anchor (5x4)
    AmenityDriverRestLounge, // Driver Rest Lounge anchor (3x3)
    AmenityOccupied,         // Other tiles occupied by amenity
    BoosterPad,              // RF Booster (1x1) — improves site SNR
    Entry,                   // Road tile that's the entry point
    Exit,                    // Road tile that's the exit point
    // Gas station specific tiles
    StoreWall,        // Convenience store wall (not walkable)
    StoreEntrance,    // Convenience store entrance
    Storefront,       // Convenience store front window area
    PumpIsland,       // Gas pump island (disabled pumps)
    Canopy,           // Canopy covered area (driveable)
    FuelCap,          // Covered fuel cap in ground
    DumpsterPad,      // Dumpster area anchor (2x2)
    DumpsterOccupied, // Other tiles occupied by dumpster
    CanopyShadow,     // Shadow cast by canopy edge
    CanopyColumn,     // Structural column for canopy
    GasStationSign,   // Large price sign
    Bollard,          // Protective metal bollard
    WheelStop,        // Parking bay wheel stop
    StreetTree,       // Decorative perimeter tree
    LightPole,        // Area lighting pole
    // Worn asphalt variations
    AsphaltWorn, // Worn asphalt with tire marks
    AsphaltSkid, // Asphalt with skid/braking marks
    // Mall/Garage tiles
    GarageFloor,  // Garage floor surface (driveable)
    GaragePillar, // Structural pillar (not driveable)
    MallFacade,   // Mall building facade (not driveable)
    // Workplace tiles
    ReservedSpot,   // Reserved parking spot marking (driveable)
    OfficeBackdrop, // Office building backdrop (not driveable)
    // Transit tiles
    LoadingZone, // Bus/truck loading zone (driveable)
    // Concrete variation
    Concrete, // Basic concrete surface (driveable)
    Planter,  // Decorative planter (not driveable)
}

impl TileContent {
    /// Check if vehicles can drive on this tile
    pub fn is_driveable(&self) -> bool {
        matches!(
            self,
            TileContent::Road
                | TileContent::Lot
                | TileContent::Entry
                | TileContent::Exit
                | TileContent::Canopy
                | TileContent::CanopyShadow
                | TileContent::AsphaltWorn
                | TileContent::AsphaltSkid
                | TileContent::GarageFloor
                | TileContent::ReservedSpot
                | TileContent::LoadingZone
                | TileContent::Concrete
        )
    }

    /// Check if this is public road (not lot surface)
    pub fn is_public_road(&self) -> bool {
        matches!(
            self,
            TileContent::Road | TileContent::Entry | TileContent::Exit
        )
    }

    /// Check if this is a valid parking destination
    pub fn is_parking(&self) -> bool {
        matches!(
            self,
            TileContent::ParkingBayNorth | TileContent::ParkingBaySouth
        )
    }

    /// Check if a charger can be placed adjacent to this tile
    pub fn can_have_charger(&self) -> bool {
        matches!(
            self,
            TileContent::ParkingBayNorth | TileContent::ParkingBaySouth
        )
    }

    /// Get the charger pad offset for this parking bay orientation
    /// Returns (dx, dy) where the charger pad should be placed relative to the bay
    pub fn charger_offset(&self) -> Option<(i32, i32)> {
        match self {
            TileContent::ParkingBayNorth => Some((0, -1)), // Charger below
            TileContent::ParkingBaySouth => Some((0, 1)),  // Charger above
            _ => None,
        }
    }

    /// Check if this is electrical infrastructure
    pub fn is_electrical(&self) -> bool {
        matches!(
            self,
            TileContent::TransformerPad
                | TileContent::TransformerOccupied
                | TileContent::SolarPad
                | TileContent::SolarOccupied
                | TileContent::BatteryPad
                | TileContent::BatteryOccupied
        )
    }

    /// Check if this is an amenity building
    pub fn is_amenity(&self) -> bool {
        matches!(
            self,
            TileContent::AmenityWifiRestrooms
                | TileContent::AmenityLoungeSnacks
                | TileContent::AmenityRestaurant
                | TileContent::AmenityDriverRestLounge
                | TileContent::AmenityOccupied
        )
    }

    /// Check if this is an anchor tile (primary tile of a multi-tile structure)
    pub fn is_anchor(&self) -> bool {
        matches!(
            self,
            TileContent::TransformerPad
                | TileContent::SolarPad
                | TileContent::BatteryPad
                | TileContent::SecurityPad
                | TileContent::DumpsterPad
                | TileContent::AmenityWifiRestrooms
                | TileContent::AmenityLoungeSnacks
                | TileContent::AmenityRestaurant
                | TileContent::AmenityDriverRestLounge
        )
    }

    /// Check if this is an occupied tile (non-anchor tile of a multi-tile structure)
    pub fn is_occupied(&self) -> bool {
        matches!(
            self,
            TileContent::TransformerOccupied
                | TileContent::SolarOccupied
                | TileContent::BatteryOccupied
                | TileContent::SecurityOccupied
                | TileContent::DumpsterOccupied
                | TileContent::AmenityOccupied
        )
    }

    /// Get the structure size for anchor tiles
    pub fn structure_size(&self) -> Option<StructureSize> {
        match self {
            TileContent::TransformerPad => Some(StructureSize::TwoByTwo),
            TileContent::SolarPad => Some(StructureSize::ThreeByTwo),
            TileContent::BatteryPad => Some(StructureSize::TwoByTwo),
            TileContent::SecurityPad => Some(StructureSize::TwoByTwo),
            TileContent::DumpsterPad => Some(StructureSize::TwoByTwo),
            TileContent::AmenityWifiRestrooms => Some(StructureSize::ThreeByThree),
            TileContent::AmenityLoungeSnacks => Some(StructureSize::FourByFour),
            TileContent::AmenityRestaurant => Some(StructureSize::FiveByFour),
            TileContent::AmenityDriverRestLounge => Some(StructureSize::ThreeByThree),
            _ => None,
        }
    }

    /// Get the corresponding occupied tile type for an anchor
    pub fn occupied_variant(&self) -> Option<TileContent> {
        match self {
            TileContent::TransformerPad => Some(TileContent::TransformerOccupied),
            TileContent::SolarPad => Some(TileContent::SolarOccupied),
            TileContent::BatteryPad => Some(TileContent::BatteryOccupied),
            TileContent::SecurityPad => Some(TileContent::SecurityOccupied),
            TileContent::DumpsterPad => Some(TileContent::DumpsterOccupied),
            TileContent::AmenityWifiRestrooms
            | TileContent::AmenityLoungeSnacks
            | TileContent::AmenityRestaurant
            | TileContent::AmenityDriverRestLounge => Some(TileContent::AmenityOccupied),
            _ => None,
        }
    }
}

/// A single tile in the grid
#[derive(Debug, Clone)]
pub struct Tile {
    /// What's on this tile
    pub content: TileContent,
    /// Entity spawned for this tile's visual (if any)
    pub visual_entity: Option<Entity>,
    /// Entity of the charger placed here (if ChargerPad)
    pub charger_entity: Option<Entity>,
    /// Charger type if this is a charger pad
    pub charger_type: Option<ChargerPadType>,
    /// Whether this bay has a charger next to it (DEPRECATED - use linked_charger_pad)
    pub has_adjacent_charger: bool,
    /// Whether this tile is locked (cannot be sold)
    pub is_locked: bool,
    /// For occupied tiles, points to the anchor tile position
    pub anchor_pos: Option<(i32, i32)>,
    /// On ParkingBay: points to linked ChargerPad position
    pub linked_charger_pad: Option<(i32, i32)>,
    /// On ChargerPad: points to linked ParkingBay position
    pub linked_parking_bay: Option<(i32, i32)>,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            content: TileContent::Empty,
            visual_entity: None,
            charger_entity: None,
            charger_type: None,
            has_adjacent_charger: false,
            is_locked: false,
            anchor_pos: None,
            linked_charger_pad: None,
            linked_parking_bay: None,
        }
    }
}

/// Result of a sell operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellResult {
    /// Sold a charger from a parking bay (bay remains)
    SoldCharger(ChargerPadType),
    /// Sold equipment entirely
    SoldEquipment(TileContent),
}

/// Type of charger on a pad
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargerPadType {
    L2,
    DCFC50,  // 50kW budget DCFC
    DCFC100, // 100kW DCFC with built-in video ads
    DCFC150, // 150kW standard DCFC
    DCFC350, // 350kW premium DCFC
}

impl ChargerPadType {
    /// Get the rated power in kW for this charger type
    pub fn power_kw(&self) -> f32 {
        match self {
            ChargerPadType::L2 => 7.0,
            ChargerPadType::DCFC50 => 50.0,
            ChargerPadType::DCFC100 => 100.0,
            ChargerPadType::DCFC150 => 150.0,
            ChargerPadType::DCFC350 => 350.0,
        }
    }

    /// Check if this is a DCFC (DC Fast Charger) type
    pub fn is_dcfc(&self) -> bool {
        matches!(
            self,
            ChargerPadType::DCFC50
                | ChargerPadType::DCFC100
                | ChargerPadType::DCFC150
                | ChargerPadType::DCFC350
        )
    }

    /// Check if this charger type has built-in video ads
    pub fn has_built_in_video_ads(&self) -> bool {
        matches!(self, ChargerPadType::DCFC100)
    }

    /// Convert to the runtime ChargerType enum
    pub fn to_charger_type(&self) -> crate::components::charger::ChargerType {
        if self.is_dcfc() {
            crate::components::charger::ChargerType::DcFast
        } else {
            crate::components::charger::ChargerType::AcLevel2
        }
    }
}

/// The site grid resource
#[derive(Resource, Debug, Clone)]
pub struct SiteGrid {
    /// Grid width in tiles (set from TMX map or default 16)
    pub width: i32,
    /// Grid height in tiles (set from TMX map or default 12)
    pub height: i32,
    /// Tiles indexed by (x, y) grid coordinates
    tiles: HashMap<(i32, i32), Tile>,
    /// All placed transformers (supports multiple)
    pub transformers: Vec<TransformerPlacement>,
    /// Entry tile position
    pub entry_pos: (i32, i32),
    /// Exit tile position
    pub exit_pos: (i32, i32),
    /// Whether grid visuals need refresh (legacy, use revision instead)
    pub needs_visual_refresh: bool,
    /// Monotonic revision counter - increments on any grid mutation.
    /// Systems track their last processed revision to detect changes.
    pub revision: u64,
    /// Solar array positions (anchor tiles, bottom-left of 3x2)
    pub solar_positions: Vec<(i32, i32)>,
    /// Battery storage positions (anchor tiles, bottom-left of 2x2)
    pub battery_positions: Vec<(i32, i32)>,
    /// Security system positions (anchor tiles, bottom-left of 2x2)
    pub security_system_positions: Vec<(i32, i32)>,
    /// Amenity building positions and types (unlimited per site, effects stack)
    pub amenities: Vec<(i32, i32, AmenityType)>,
    /// Total installed solar capacity (kW)
    pub total_solar_kw: f32,
    /// Total installed battery capacity (kWh)
    pub total_battery_kwh: f32,
    /// Total installed battery power (kW)
    pub total_battery_kw: f32,
}

impl Default for SiteGrid {
    fn default() -> Self {
        Self::new(GRID_WIDTH, GRID_HEIGHT)
    }
}

impl SiteGrid {
    /// Create a new grid with the given dimensions, pre-filled with grass and a road along the top.
    pub fn new(width: i32, height: i32) -> Self {
        let road_y = height - 1;

        let drive_lane = if road_y > 0 { road_y - 1 } else { road_y };

        let mut grid = Self {
            width,
            height,
            tiles: HashMap::new(),
            transformers: Vec::new(),
            entry_pos: (0, drive_lane),
            exit_pos: (width - 1, drive_lane),
            needs_visual_refresh: true,
            revision: 1,
            solar_positions: Vec::new(),
            battery_positions: Vec::new(),
            security_system_positions: Vec::new(),
            amenities: Vec::new(),
            total_solar_kw: 0.0,
            total_battery_kwh: 0.0,
            total_battery_kw: 0.0,
        };

        for x in 0..width {
            for y in 0..height {
                grid.tiles.insert(
                    (x, y),
                    Tile {
                        content: TileContent::Grass,
                        ..default()
                    },
                );
            }
        }

        // Upper road row (center-line side)
        for x in 0..width {
            grid.set_tile_content(x, road_y, TileContent::Road);
        }

        // Lower road row (driving lane) with entry/exit on the right side
        if road_y > 0 {
            for x in 0..width {
                grid.set_tile_content(x, drive_lane, TileContent::Road);
            }
        }

        grid.set_tile_content(grid.entry_pos.0, grid.entry_pos.1, TileContent::Entry);
        grid.set_tile_content(grid.exit_pos.0, grid.exit_pos.1, TileContent::Exit);

        grid
    }

    /// Convert grid coordinates to world position (center of tile)
    pub fn grid_to_world(grid_x: i32, grid_y: i32) -> Vec2 {
        Vec2::new(
            GRID_OFFSET_X + (grid_x as f32 + 0.5) * TILE_SIZE,
            GRID_OFFSET_Y + (grid_y as f32 + 0.5) * TILE_SIZE,
        )
    }

    /// Convert anchor grid coordinates to world position (center of multi-tile structure)
    pub fn multi_tile_center(anchor_x: i32, anchor_y: i32, size: StructureSize) -> Vec2 {
        let (width, height) = size.dimensions();
        Vec2::new(
            GRID_OFFSET_X + (anchor_x as f32 + width as f32 / 2.0) * TILE_SIZE,
            GRID_OFFSET_Y + (anchor_y as f32 + height as f32 / 2.0) * TILE_SIZE,
        )
    }

    /// Convert world position to grid coordinates
    pub fn world_to_grid(world_pos: Vec2) -> (i32, i32) {
        let x = ((world_pos.x - GRID_OFFSET_X) / TILE_SIZE).floor() as i32;
        let y = ((world_pos.y - GRID_OFFSET_Y) / TILE_SIZE).floor() as i32;
        (x, y)
    }

    /// Check if grid coordinates are valid for the default grid size (static method for backward compat)
    pub fn is_valid_coord(x: i32, y: i32) -> bool {
        (0..GRID_WIDTH).contains(&x) && (0..GRID_HEIGHT).contains(&y)
    }

    /// Check if grid coordinates are valid for this grid's dimensions
    pub fn is_valid(&self, x: i32, y: i32) -> bool {
        (0..self.width).contains(&x) && (0..self.height).contains(&y)
    }

    /// Mark the grid as changed by incrementing the revision counter.
    /// Systems that derive state from the grid track their last processed revision
    /// and only run when revision changes, eliminating race conditions.
    pub fn mark_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        // Keep legacy flag in sync for any remaining consumers
        self.needs_visual_refresh = true;
    }

    /// Check if any transformer is placed on the grid
    pub fn has_transformer(&self) -> bool {
        !self.transformers.is_empty()
    }

    /// Get total transformer capacity (sum of all kVA ratings)
    pub fn total_transformer_capacity(&self) -> f32 {
        self.transformers.iter().map(|t| t.kva).sum()
    }

    /// Get transformer count
    pub fn transformer_count(&self) -> usize {
        self.transformers.len()
    }

    /// Find transformer by position
    pub fn get_transformer_at(&self, x: i32, y: i32) -> Option<&TransformerPlacement> {
        self.transformers.iter().find(|t| t.pos == (x, y))
    }

    /// Find transformer by position (mutable)
    pub fn get_transformer_at_mut(&mut self, x: i32, y: i32) -> Option<&mut TransformerPlacement> {
        self.transformers.iter_mut().find(|t| t.pos == (x, y))
    }

    /// Check if a multi-tile footprint fits entirely within the grid
    pub fn is_valid_footprint(&self, anchor_x: i32, anchor_y: i32, size: StructureSize) -> bool {
        let (width, height) = size.dimensions();
        self.is_valid(anchor_x, anchor_y)
            && self.is_valid(anchor_x + width - 1, anchor_y + height - 1)
    }

    /// Get all tile positions for a multi-tile structure
    pub fn get_footprint_tiles(
        anchor_x: i32,
        anchor_y: i32,
        size: StructureSize,
    ) -> Vec<(i32, i32)> {
        let (width, height) = size.dimensions();
        let mut tiles = Vec::with_capacity((width * height) as usize);
        for dx in 0..width {
            for dy in 0..height {
                tiles.push((anchor_x + dx, anchor_y + dy));
            }
        }
        tiles
    }

    /// Check if all tiles in a footprint can be placed on (are grass/empty and not locked)
    pub fn can_place_footprint(
        &self,
        anchor_x: i32,
        anchor_y: i32,
        size: StructureSize,
    ) -> Result<(), String> {
        if !self.is_valid_footprint(anchor_x, anchor_y, size) {
            return Err("Structure doesn't fit within grid bounds".to_string());
        }

        for (x, y) in Self::get_footprint_tiles(anchor_x, anchor_y, size) {
            let content = self.get_content(x, y);

            // Check for locked tiles
            if let Some(tile) = self.get_tile(x, y)
                && tile.is_locked
            {
                return Err(format!("Tile at ({x}, {y}) is locked"));
            }

            // Check for valid placement surface
            if !matches!(content, TileContent::Grass | TileContent::Empty) {
                return Err(format!(
                    "Tile at ({x}, {y}) is not empty (contains {content:?})"
                ));
            }
        }

        Ok(())
    }

    /// Place a multi-tile structure, setting anchor and occupied tiles
    fn place_multi_tile(
        &mut self,
        anchor_x: i32,
        anchor_y: i32,
        anchor_content: TileContent,
        occupied_content: TileContent,
        size: StructureSize,
    ) {
        let tiles = Self::get_footprint_tiles(anchor_x, anchor_y, size);

        for (i, (x, y)) in tiles.iter().enumerate() {
            if i == 0 {
                // First tile is the anchor
                self.set_tile_content(*x, *y, anchor_content);
            } else {
                // Other tiles are occupied and point back to anchor
                self.set_tile_content(*x, *y, occupied_content);
                if let Some(tile) = self.get_tile_mut(*x, *y) {
                    tile.anchor_pos = Some((anchor_x, anchor_y));
                }
            }
        }
    }

    /// Remove a multi-tile structure by its anchor position
    fn remove_multi_tile(&mut self, anchor_x: i32, anchor_y: i32, size: StructureSize) {
        for (x, y) in Self::get_footprint_tiles(anchor_x, anchor_y, size) {
            if let Some(tile) = self.get_tile_mut(x, y) {
                tile.content = TileContent::Grass;
                tile.anchor_pos = None;
            }
        }
        self.mark_changed();
    }

    /// Find the anchor position for a tile (returns self if anchor, or stored anchor_pos)
    pub fn find_anchor(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let tile = self.get_tile(x, y)?;
        if tile.content.is_anchor() {
            Some((x, y))
        } else if tile.content.is_occupied() {
            tile.anchor_pos
        } else {
            None
        }
    }

    /// Get tile at coordinates
    pub fn get_tile(&self, x: i32, y: i32) -> Option<&Tile> {
        self.tiles.get(&(x, y))
    }

    /// Get mutable tile at coordinates
    pub fn get_tile_mut(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        self.tiles.get_mut(&(x, y))
    }

    /// Get tile content at coordinates
    pub fn get_content(&self, x: i32, y: i32) -> TileContent {
        self.tiles
            .get(&(x, y))
            .map(|t| t.content)
            .unwrap_or(TileContent::Empty)
    }

    /// Count how many tiles have the given content type.
    pub fn count_content(&self, target: TileContent) -> u32 {
        self.tiles.values().filter(|t| t.content == target).count() as u32
    }

    /// Set tile content (low-level, doesn't check rules)
    /// Creates the tile if it doesn't exist.
    pub fn set_tile_content(&mut self, x: i32, y: i32, content: TileContent) {
        self.tiles
            .entry((x, y))
            .and_modify(|tile| tile.content = content)
            .or_insert_with(|| Tile {
                content,
                ..Default::default()
            });
        self.mark_changed();
    }

    /// Check if a tile can be placed at this location (single-tile items only)
    pub fn can_place(&self, x: i32, y: i32, content: TileContent) -> Result<(), String> {
        if !self.is_valid(x, y) {
            return Err("Outside grid bounds".to_string());
        }

        // Check if tile is locked
        if let Some(tile) = self.get_tile(x, y)
            && tile.is_locked
        {
            return Err("This tile is locked".to_string());
        }

        let current = self.get_content(x, y);

        // Can't place on entry/exit
        if matches!(current, TileContent::Entry | TileContent::Exit) {
            return Err("Can't build on entry/exit".to_string());
        }

        match content {
            TileContent::Road => {
                // Roads can replace grass or empty
                if !matches!(current, TileContent::Grass | TileContent::Empty) {
                    return Err("Can only place road on grass".to_string());
                }
            }
            TileContent::Lot => {
                // Lot surface can replace grass or empty
                if !matches!(current, TileContent::Grass | TileContent::Empty) {
                    return Err("Can only place lot surface on grass".to_string());
                }
            }
            TileContent::ParkingBayNorth | TileContent::ParkingBaySouth => {
                // Parking bays need adjacent driveable tile (road or lot)
                if !matches!(
                    current,
                    TileContent::Grass | TileContent::Empty | TileContent::Lot
                ) {
                    return Err("Can only place parking bay on grass or lot".to_string());
                }
                if !self.has_adjacent_road(x, y) {
                    return Err("Parking bay needs adjacent road or lot".to_string());
                }
            }
            TileContent::ChargerPad => {
                // Chargers go on parking bays (we'll handle this specially)
                // Actually chargers are placed as entities near bays, not on tiles
                return Err("Use place_charger() for chargers".to_string());
            }
            // Multi-tile structures - use place_* methods instead
            TileContent::TransformerPad => {
                return Err("Use place_transformer() for transformers".to_string());
            }
            TileContent::SolarPad => {
                return Err("Use place_solar() for solar arrays".to_string());
            }
            TileContent::BatteryPad => {
                return Err("Use place_battery() for batteries".to_string());
            }
            TileContent::SecurityPad => {
                return Err("Use place_security_system() for security systems".to_string());
            }
            TileContent::AmenityWifiRestrooms
            | TileContent::AmenityLoungeSnacks
            | TileContent::AmenityRestaurant
            | TileContent::AmenityDriverRestLounge => {
                return Err("Use place_amenity() for amenity buildings".to_string());
            }
            _ => {}
        }

        Ok(())
    }

    /// Check if there's an adjacent road tile
    pub fn has_adjacent_road(&self, x: i32, y: i32) -> bool {
        let neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)];
        for (dx, dy) in neighbors {
            let content = self.get_content(x + dx, y + dy);
            if content.is_driveable() {
                return true;
            }
        }
        false
    }

    /// Place a road tile
    pub fn place_road(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.can_place(x, y, TileContent::Road)?;
        self.set_tile_content(x, y, TileContent::Road);
        Ok(())
    }

    /// Place a lot surface tile (internal driveway / parking area)
    pub fn place_lot(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.can_place(x, y, TileContent::Lot)?;
        self.set_tile_content(x, y, TileContent::Lot);
        Ok(())
    }

    /// Fill a rectangular area with lot surface (ignoring errors for already-placed tiles)
    pub fn fill_lot_area(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        let min_x = x1.min(x2);
        let max_x = x1.max(x2);
        let min_y = y1.min(y2);
        let max_y = y1.max(y2);

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let content = self.get_content(x, y);
                // Only fill grass/empty tiles
                if matches!(content, TileContent::Grass | TileContent::Empty) {
                    self.set_tile_content(x, y, TileContent::Lot);
                }
            }
        }
    }

    /// Place a parking bay (defaults to south-facing, car enters from north)
    pub fn place_parking_bay(&mut self, x: i32, y: i32) -> Result<(), String> {
        self.can_place(x, y, TileContent::ParkingBaySouth)?;
        self.set_tile_content(x, y, TileContent::ParkingBaySouth);
        Ok(())
    }

    /// Place a transformer (2x2 tiles, anchor at bottom-left)
    /// Multiple transformers are allowed - their capacity is summed
    pub fn place_transformer(&mut self, x: i32, y: i32, kva: f32) -> Result<(), String> {
        let size = StructureSize::TwoByTwo;
        self.can_place_footprint(x, y, size)?;

        self.place_multi_tile(
            x,
            y,
            TileContent::TransformerPad,
            TileContent::TransformerOccupied,
            size,
        );
        self.transformers.push(TransformerPlacement {
            pos: (x, y),
            kva,
            entity: None,
        });
        Ok(())
    }

    /// Place a solar array (3x2 tiles, 25 kW per array)
    pub fn place_solar(&mut self, x: i32, y: i32) -> Result<(), String> {
        let size = StructureSize::ThreeByTwo;
        self.can_place_footprint(x, y, size)?;

        self.place_multi_tile(
            x,
            y,
            TileContent::SolarPad,
            TileContent::SolarOccupied,
            size,
        );
        self.solar_positions.push((x, y));
        self.total_solar_kw += 25.0; // 25 kW per 3x2 solar array
        Ok(())
    }

    /// Place a battery storage unit (2x2 tiles, 200 kWh / 100 kW per unit)
    pub fn place_battery(&mut self, x: i32, y: i32) -> Result<(), String> {
        let size = StructureSize::TwoByTwo;
        self.can_place_footprint(x, y, size)?;

        self.place_multi_tile(
            x,
            y,
            TileContent::BatteryPad,
            TileContent::BatteryOccupied,
            size,
        );
        self.battery_positions.push((x, y));
        self.total_battery_kwh += 200.0; // 200 kWh per 2x2 battery
        self.total_battery_kw += 100.0; // 100 kW charge/discharge rate
        Ok(())
    }

    /// Place a security system (2x2 tiles)
    pub fn place_security_system(&mut self, x: i32, y: i32) -> Result<(), String> {
        let size = StructureSize::TwoByTwo;
        self.can_place_footprint(x, y, size)?;

        self.place_multi_tile(
            x,
            y,
            TileContent::SecurityPad,
            TileContent::SecurityOccupied,
            size,
        );
        self.security_system_positions.push((x, y));
        Ok(())
    }

    /// Check if this site has at least one security system installed
    pub fn has_security_system(&self) -> bool {
        !self.security_system_positions.is_empty()
    }

    /// Number of security systems installed on this site
    pub fn security_system_count(&self) -> usize {
        self.security_system_positions.len()
    }

    /// Place an amenity building (size depends on type, unlimited per site)
    pub fn place_amenity(
        &mut self,
        x: i32,
        y: i32,
        amenity_type: AmenityType,
    ) -> Result<(), String> {
        let size = amenity_type.size();
        self.can_place_footprint(x, y, size)?;

        self.place_multi_tile(
            x,
            y,
            amenity_type.tile_content(),
            TileContent::AmenityOccupied,
            size,
        );
        self.amenities.push((x, y, amenity_type));
        Ok(())
    }

    /// Place a charger adjacent to a parking bay
    ///
    /// Creates a ChargerPad tile based on the parking bay's orientation.
    /// - ParkingBayNorth: ChargerPad at (bay_x, bay_y - 1)
    /// - ParkingBaySouth: ChargerPad at (bay_x, bay_y + 1)
    pub fn place_charger(
        &mut self,
        bay_x: i32,
        bay_y: i32,
        charger_type: ChargerPadType,
    ) -> Result<(), String> {
        // 1. Verify (bay_x, bay_y) is a parking bay and get its orientation
        let content = self.get_content(bay_x, bay_y);
        let offset = content
            .charger_offset()
            .ok_or_else(|| "Charger must be placed on a parking bay".to_string())?;

        // 2. Check bay doesn't already have a charger
        if let Some(tile) = self.get_tile(bay_x, bay_y)
            && (tile.linked_charger_pad.is_some() || tile.has_adjacent_charger)
        {
            return Err("This bay already has a charger".to_string());
        }

        // 3. ChargerPad position based on bay orientation
        let pad_x = bay_x + offset.0;
        let pad_y = bay_y + offset.1;

        // 4. Verify pad position is valid and within bounds
        if !self.is_valid(pad_x, pad_y) {
            return Err("Cannot place charger - position is out of bounds".to_string());
        }

        let pad_content = self.get_content(pad_x, pad_y);
        // ChargerPad is allowed because TMX now pre-places these tiles to mark valid spots
        if !matches!(
            pad_content,
            TileContent::ChargerPad
                | TileContent::Lot
                | TileContent::Grass
                | TileContent::Empty
                | TileContent::WheelStop
        ) {
            return Err(format!(
                "Cannot place charger - tile at ({pad_x}, {pad_y}) is {pad_content:?}"
            ));
        }

        // 5. Set ChargerPad content and charger info
        self.set_tile_content(pad_x, pad_y, TileContent::ChargerPad);
        if let Some(tile) = self.get_tile_mut(pad_x, pad_y) {
            tile.charger_type = Some(charger_type);
            tile.linked_parking_bay = Some((bay_x, bay_y));
        }

        // 6. Link bay to pad (also set has_adjacent_charger for backward compat)
        if let Some(tile) = self.get_tile_mut(bay_x, bay_y) {
            tile.linked_charger_pad = Some((pad_x, pad_y));
            tile.has_adjacent_charger = true; // Keep for transition
        }

        self.mark_changed();
        Ok(())
    }

    /// Sell/remove equipment placed by the player
    ///
    /// Priority order:
    /// 1. If the tile has a charger, remove just the charger (keep the parking bay)
    /// 2. If the tile is a road or parking bay, cannot be sold (protected infrastructure)
    /// 3. If the tile is locked (from template), cannot be removed
    /// 4. If the tile is part of a multi-tile structure, remove the entire structure
    /// 5. Otherwise, remove the single tile (revert to grass)
    pub fn sell(&mut self, x: i32, y: i32) -> Result<SellResult, String> {
        let content = self.get_content(x, y);

        // Can't sell entry/exit
        if matches!(content, TileContent::Entry | TileContent::Exit) {
            return Err("Can't sell entry/exit".to_string());
        }

        // Can't sell roads, parking bays, or lot surfaces
        if matches!(
            content,
            TileContent::Road
                | TileContent::ParkingBayNorth
                | TileContent::ParkingBaySouth
                | TileContent::Lot
        ) {
            return Err("Can't sell roads, parking bays, or lot surfaces".to_string());
        }

        // Can't sell grass (nothing to sell)
        if matches!(content, TileContent::Grass | TileContent::Empty) {
            return Err("Nothing to sell".to_string());
        }

        // PRIORITY 1: Remove charger if present (regardless of locked status)

        // Check if this is a ChargerPad being sold directly
        if content == TileContent::ChargerPad
            && let Some(tile) = self.get_tile(x, y)
        {
            let charger_type = tile.charger_type;
            let linked_bay = tile.linked_parking_bay;

            // Revert ChargerPad to Lot
            if let Some(tile_mut) = self.get_tile_mut(x, y) {
                tile_mut.content = TileContent::Lot;
                tile_mut.charger_type = None;
                tile_mut.charger_entity = None;
                tile_mut.linked_parking_bay = None;
            }

            // Unlink the bay
            if let Some((bay_x, bay_y)) = linked_bay
                && let Some(bay_tile) = self.get_tile_mut(bay_x, bay_y)
            {
                bay_tile.linked_charger_pad = None;
                bay_tile.has_adjacent_charger = false;
            }

            self.mark_changed();
            if let Some(ctype) = charger_type {
                return Ok(SellResult::SoldCharger(ctype));
            }
        }

        // Check if this is a ParkingBay with a linked charger
        let linked_pad = self.get_tile(x, y).and_then(|t| t.linked_charger_pad);

        if let Some((pad_x, pad_y)) = linked_pad
            && let Some(pad_tile) = self.get_tile(pad_x, pad_y)
        {
            let charger_type = pad_tile.charger_type;

            // Remove the ChargerPad
            if let Some(pad_tile_mut) = self.get_tile_mut(pad_x, pad_y) {
                pad_tile_mut.content = TileContent::Lot;
                pad_tile_mut.charger_type = None;
                pad_tile_mut.charger_entity = None;
                pad_tile_mut.linked_parking_bay = None;
            }

            // Unlink the bay
            if let Some(bay_tile) = self.get_tile_mut(x, y) {
                bay_tile.linked_charger_pad = None;
                bay_tile.has_adjacent_charger = false;
            }

            self.mark_changed();
            if let Some(ctype) = charger_type {
                return Ok(SellResult::SoldCharger(ctype));
            }
        }

        // Legacy: Old style charger on bay (backward compat)
        let has_charger = self
            .get_tile(x, y)
            .map(|t| t.has_adjacent_charger)
            .unwrap_or(false);

        if has_charger
            && linked_pad.is_none()
            && let Some(tile) = self.get_tile(x, y)
            && let Some(ctype) = tile.charger_type
        {
            // Remove just the charger, keep the bay
            if let Some(tile_mut) = self.get_tile_mut(x, y) {
                tile_mut.has_adjacent_charger = false;
                tile_mut.charger_type = None;
                tile_mut.charger_entity = None;
            }
            self.mark_changed();
            return Ok(SellResult::SoldCharger(ctype));
        }

        // PRIORITY 3: Check if tile is locked (can't remove locked tiles)
        let is_locked = self.get_tile(x, y).map(|t| t.is_locked).unwrap_or(false);
        if is_locked {
            return Err("This tile is locked and cannot be sold".to_string());
        }

        // PRIORITY 4: Handle multi-tile structures - find anchor and remove entire structure
        if content.is_anchor() || content.is_occupied() {
            let (anchor_x, anchor_y) = if content.is_anchor() {
                (x, y)
            } else {
                // Get anchor from tile
                self.get_tile(x, y)
                    .and_then(|t| t.anchor_pos)
                    .ok_or_else(|| "Occupied tile missing anchor reference".to_string())?
            };

            let anchor_content = self.get_content(anchor_x, anchor_y);

            // Handle transformer removal (2x2)
            if anchor_content == TileContent::TransformerPad {
                self.remove_multi_tile(anchor_x, anchor_y, StructureSize::TwoByTwo);
                self.transformers.retain(|t| t.pos != (anchor_x, anchor_y));
                return Ok(SellResult::SoldEquipment(anchor_content));
            }

            // Handle solar removal (3x2)
            if anchor_content == TileContent::SolarPad {
                self.remove_multi_tile(anchor_x, anchor_y, StructureSize::ThreeByTwo);
                self.solar_positions
                    .retain(|pos| *pos != (anchor_x, anchor_y));
                self.total_solar_kw = (self.total_solar_kw - 25.0).max(0.0);
                return Ok(SellResult::SoldEquipment(anchor_content));
            }

            // Handle battery removal (2x2)
            if anchor_content == TileContent::BatteryPad {
                self.remove_multi_tile(anchor_x, anchor_y, StructureSize::TwoByTwo);
                self.battery_positions
                    .retain(|pos| *pos != (anchor_x, anchor_y));
                self.total_battery_kwh = (self.total_battery_kwh - 200.0).max(0.0);
                self.total_battery_kw = (self.total_battery_kw - 100.0).max(0.0);
                return Ok(SellResult::SoldEquipment(anchor_content));
            }

            // Handle security system removal (2x2)
            if anchor_content == TileContent::SecurityPad {
                self.remove_multi_tile(anchor_x, anchor_y, StructureSize::TwoByTwo);
                self.security_system_positions
                    .retain(|pos| *pos != (anchor_x, anchor_y));
                return Ok(SellResult::SoldEquipment(anchor_content));
            }

            // Handle amenity removal
            if anchor_content.is_amenity()
                && let Some(idx) = self
                    .amenities
                    .iter()
                    .position(|(ax, ay, _)| *ax == anchor_x && *ay == anchor_y)
            {
                let (_, _, amenity_type) = self.amenities.remove(idx);
                self.remove_multi_tile(anchor_x, anchor_y, amenity_type.size());
                return Ok(SellResult::SoldEquipment(anchor_content));
            }
        }

        // PRIORITY 5: Remove single tile (revert to grass)
        let removed_content = content;
        if let Some(tile) = self.get_tile_mut(x, y) {
            tile.content = TileContent::Grass;
            tile.charger_entity = None;
            tile.charger_type = None;
            tile.has_adjacent_charger = false;
            tile.anchor_pos = None;
        }
        self.mark_changed();

        Ok(SellResult::SoldEquipment(removed_content))
    }

    /// Lock a tile to prevent selling
    pub fn lock_tile(&mut self, x: i32, y: i32) {
        if let Some(tile) = self.get_tile_mut(x, y) {
            tile.is_locked = true;
        }
    }

    /// Unlock a tile to allow selling
    pub fn unlock_tile(&mut self, x: i32, y: i32) {
        if let Some(tile) = self.get_tile_mut(x, y) {
            tile.is_locked = false;
        }
    }

    /// Check if a tile is locked
    pub fn is_tile_locked(&self, x: i32, y: i32) -> bool {
        self.get_tile(x, y).map(|t| t.is_locked).unwrap_or(false)
    }

    /// Get all parking bays with chargers
    pub fn get_charger_bays(&self) -> Vec<(i32, i32, ChargerPadType)> {
        let mut bays = Vec::new();
        for ((x, y), tile) in &self.tiles {
            if tile.content.is_parking() {
                // Check for linked ChargerPad (new way)
                if let Some((pad_x, pad_y)) = tile.linked_charger_pad {
                    if let Some(pad_tile) = self.get_tile(pad_x, pad_y)
                        && let Some(charger_type) = pad_tile.charger_type
                    {
                        bays.push((*x, *y, charger_type));
                    }
                } else if tile.has_adjacent_charger {
                    // Fallback for old style (backward compat during transition)
                    if let Some(charger_type) = tile.charger_type {
                        bays.push((*x, *y, charger_type));
                    }
                }
            }
        }
        bays
    }

    /// Get all road tiles (for pathfinding)
    pub fn get_road_tiles(&self) -> Vec<(i32, i32)> {
        let mut roads = Vec::new();
        for ((x, y), tile) in &self.tiles {
            if tile.content.is_driveable() {
                roads.push((*x, *y));
            }
        }
        roads
    }

    /// Get all parking bays
    pub fn get_parking_bays(&self) -> Vec<(i32, i32)> {
        let mut bays = Vec::new();
        for ((x, y), tile) in &self.tiles {
            if tile.content.is_parking() {
                bays.push((*x, *y));
            }
        }
        bays
    }

    /// Find a driveable tile near the charging area for a vehicle to wait on.
    ///
    /// BFS outward from the supplied `charger_bay_positions` (only bays with
    /// linked chargers). Returns the best candidate by priority:
    /// `Lot` > other driveable > `Road`.
    /// Tiles in `occupied_waiting` or `occupied_bays` are excluded.
    pub fn find_waiting_tile(
        &self,
        occupied_waiting: &[(i32, i32)],
        occupied_bays: &[(i32, i32)],
        charger_bay_positions: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        use std::collections::{HashSet, VecDeque};

        const MAX_DEPTH: u32 = 5;
        const CARDINAL: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

        let occupied_set: HashSet<(i32, i32)> = occupied_waiting
            .iter()
            .chain(occupied_bays.iter())
            .copied()
            .collect();

        let mut visited: HashSet<(i32, i32)> = HashSet::new();
        let mut queue: VecDeque<((i32, i32), u32)> = VecDeque::new();

        // Seed BFS only from parking bays that have linked chargers
        for &(x, y) in charger_bay_positions {
            visited.insert((x, y));
            queue.push_back(((x, y), 0));
        }

        // Sort seeds for deterministic BFS order across runs
        let mut seeds: Vec<_> = queue.drain(..).collect();
        seeds.sort_by_key(|&((x, y), _)| (x, y));
        queue.extend(seeds);

        let mut lot_candidate: Option<(i32, i32)> = None;
        let mut other_candidate: Option<(i32, i32)> = None;
        let mut road_candidate: Option<(i32, i32)> = None;

        while let Some(((x, y), depth)) = queue.pop_front() {
            if depth >= MAX_DEPTH {
                continue;
            }

            for (dx, dy) in CARDINAL {
                let nx = x + dx;
                let ny = y + dy;

                if !self.is_valid(nx, ny) || !visited.insert((nx, ny)) {
                    continue;
                }

                if occupied_set.contains(&(nx, ny)) {
                    queue.push_back(((nx, ny), depth + 1));
                    continue;
                }

                let content = self.get_content(nx, ny);

                if content == TileContent::Lot && lot_candidate.is_none() {
                    lot_candidate = Some((nx, ny));
                } else if content.is_driveable()
                    && !content.is_parking()
                    && !content.is_public_road()
                    && other_candidate.is_none()
                {
                    other_candidate = Some((nx, ny));
                } else if content == TileContent::Road && road_candidate.is_none() {
                    road_candidate = Some((nx, ny));
                }

                if lot_candidate.is_some() {
                    return lot_candidate;
                }

                queue.push_back(((nx, ny), depth + 1));
            }
        }

        lot_candidate.or(other_candidate).or(road_candidate)
    }

    /// Validate if station can open
    pub fn validate_for_open(&self) -> super::build_state::OpenValidation {
        let mut issues = Vec::new();

        // Must have at least 1 charger
        let charger_bays = self.get_charger_bays();
        if charger_bays.is_empty() {
            issues.push("Need at least 1 charger".to_string());
        }

        // Transformer only required for DCFC chargers
        // L2 chargers use split-phase 240V which is ubiquitous in North America
        let has_dcfc = charger_bays.iter().any(|(_, _, ct)| ct.is_dcfc());
        if has_dcfc && !self.has_transformer() {
            issues.push("Need a transformer for DCFC chargers".to_string());
        }

        // Each charger bay must be reachable from entry
        // (simplified: just check there's a road path exists)
        if !self.has_road_from_entry() {
            issues.push("Need road connected to entry".to_string());
        }

        if issues.is_empty() {
            super::build_state::OpenValidation::valid()
        } else {
            super::build_state::OpenValidation::invalid(issues)
        }
    }

    /// Check if there's a road path from entry
    fn has_road_from_entry(&self) -> bool {
        // Simple check: is entry adjacent to at least one road?
        let (ex, ey) = self.entry_pos;
        self.has_adjacent_road(ex, ey) || self.get_content(ex + 1, ey).is_driveable()
    }

    /// Iterator over all tiles
    pub fn iter_tiles(&self) -> impl Iterator<Item = ((i32, i32), &Tile)> {
        self.tiles.iter().map(|((x, y), tile)| ((*x, *y), tile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test grid with a south-facing parking bay at (5, 5) for charger placement tests.
    fn create_test_grid_with_parking_bay() -> SiteGrid {
        let mut grid = SiteGrid::default();
        // Set up a south-facing parking bay at (5, 5) and a Lot tile above it at (5, 6)
        // South-facing means car enters from north, charger goes at y+1
        grid.set_tile_content(5, 5, TileContent::ParkingBaySouth);
        grid.set_tile_content(5, 6, TileContent::Lot);
        // Reset revision to known value for testing
        grid.revision = 1;
        grid
    }

    #[test]
    fn place_charger_creates_charger_pad() {
        let mut grid = create_test_grid_with_parking_bay();

        // Place charger on the parking bay at (5, 5)
        let result = grid.place_charger(5, 5, ChargerPadType::DCFC150);
        assert!(result.is_ok(), "place_charger should succeed");

        // Verify ChargerPad was created at (5, 6) - one tile above the bay
        let pad_tile = grid.get_tile(5, 6).expect("Pad tile should exist");
        assert_eq!(pad_tile.content, TileContent::ChargerPad);
        assert_eq!(pad_tile.charger_type, Some(ChargerPadType::DCFC150));
    }

    #[test]
    fn place_charger_links_bay_and_pad() {
        let mut grid = create_test_grid_with_parking_bay();

        grid.place_charger(5, 5, ChargerPadType::L2).unwrap();

        // Verify bidirectional linking
        let bay_tile = grid.get_tile(5, 5).expect("Bay tile should exist");
        assert_eq!(bay_tile.linked_charger_pad, Some((5, 6)));
        assert!(bay_tile.has_adjacent_charger);

        let pad_tile = grid.get_tile(5, 6).expect("Pad tile should exist");
        assert_eq!(pad_tile.linked_parking_bay, Some((5, 5)));
    }

    #[test]
    fn place_charger_increments_revision() {
        let mut grid = create_test_grid_with_parking_bay();
        let initial_revision = grid.revision;

        grid.place_charger(5, 5, ChargerPadType::DCFC50).unwrap();

        assert!(
            grid.revision > initial_revision,
            "Revision should increment after place_charger"
        );
    }

    #[test]
    fn place_charger_fails_on_non_parking_bay() {
        let mut grid = SiteGrid::default();
        grid.set_tile_content(5, 5, TileContent::Grass);
        grid.set_tile_content(5, 6, TileContent::Lot);

        let result = grid.place_charger(5, 5, ChargerPadType::L2);
        assert!(result.is_err(), "Should fail when not on parking bay");
    }

    #[test]
    fn place_charger_fails_if_bay_already_has_charger() {
        let mut grid = create_test_grid_with_parking_bay();

        // Place first charger
        grid.place_charger(5, 5, ChargerPadType::L2).unwrap();

        // Try to place another charger on same bay
        let result = grid.place_charger(5, 5, ChargerPadType::DCFC150);
        assert!(result.is_err(), "Should fail if bay already has charger");
    }

    #[test]
    fn sell_charger_increments_revision() {
        let mut grid = create_test_grid_with_parking_bay();
        grid.place_charger(5, 5, ChargerPadType::L2).unwrap();

        let revision_before_sell = grid.revision;
        let result = grid.sell(5, 6); // Sell the ChargerPad at (5, 6)

        assert!(result.is_ok(), "Selling charger should succeed");
        assert!(
            grid.revision > revision_before_sell,
            "Revision should increment after sell"
        );
    }

    #[test]
    fn set_tile_content_increments_revision() {
        let mut grid = SiteGrid::default();
        let initial_revision = grid.revision;

        grid.set_tile_content(3, 3, TileContent::Road);

        assert!(
            grid.revision > initial_revision,
            "Revision should increment after set_tile_content"
        );
    }

    #[test]
    fn find_waiting_tile_prefers_lot() {
        let mut grid = SiteGrid::default();
        // Road row at y=4
        for x in 0..6 {
            grid.set_tile_content(x, 4, TileContent::Road);
        }
        // Lot tiles adjacent to bays
        grid.set_tile_content(2, 3, TileContent::Lot);
        grid.set_tile_content(3, 3, TileContent::Lot);
        grid.set_tile_content(4, 3, TileContent::Lot);
        // Parking bay with charger
        grid.set_tile_content(3, 2, TileContent::ParkingBaySouth);
        if let Some(tile) = grid.get_tile_mut(3, 2) {
            tile.linked_charger_pad = Some((3, 3));
        }

        let bays = vec![(3, 2)];
        let result = grid.find_waiting_tile(&[], &[], &bays);
        assert!(result.is_some(), "Should find a waiting tile");
        let (rx, ry) = result.unwrap();
        let content = grid.get_content(rx, ry);
        assert_eq!(content, TileContent::Lot, "Should prefer Lot tiles");
    }

    #[test]
    fn find_waiting_tile_skips_occupied() {
        let mut grid = SiteGrid::default();
        grid.set_tile_content(3, 3, TileContent::Lot);
        grid.set_tile_content(4, 3, TileContent::Lot);
        grid.set_tile_content(3, 2, TileContent::ParkingBaySouth);

        let bays = vec![(3, 2)];
        // Occupy the closest lot tile
        let occupied_waiting = vec![(3, 3)];
        let result = grid.find_waiting_tile(&occupied_waiting, &[], &bays);
        // Should return the other lot tile
        assert_eq!(result, Some((4, 3)));
    }

    #[test]
    fn find_waiting_tile_returns_none_on_empty_grid() {
        let grid = SiteGrid::default();
        assert_eq!(grid.find_waiting_tile(&[], &[], &[]), None);
    }

    #[test]
    fn find_waiting_tile_ignores_bays_without_chargers() {
        let mut grid = SiteGrid::default();

        // Upper row: parking bay at (4,8) with a charger (equipped)
        grid.set_tile_content(3, 8, TileContent::Lot);
        grid.set_tile_content(4, 8, TileContent::ParkingBaySouth);
        grid.set_tile_content(5, 8, TileContent::Lot);

        // Lower row: parking bay at (4,4) with NO charger linked
        grid.set_tile_content(3, 4, TileContent::Lot);
        grid.set_tile_content(4, 4, TileContent::ParkingBaySouth);
        grid.set_tile_content(5, 4, TileContent::Lot);

        // Lot filler between rows so BFS could reach both if seeded from all bays
        for y in 5..8 {
            grid.set_tile_content(4, y, TileContent::Lot);
        }

        // Only the upper bay has a charger
        let equipped_bays = vec![(4, 8)];
        let result = grid.find_waiting_tile(&[], &[], &equipped_bays);

        assert!(result.is_some(), "Should find a waiting tile");
        let (_, ry) = result.unwrap();
        assert!(
            ry >= 7,
            "Waiting tile should be near the equipped upper row (y >= 7), got y={ry}"
        );
    }

    #[test]
    fn apac_surface_driveability_matches_intended_semantics() {
        assert!(!TileContent::Grass.is_driveable());
        assert!(!TileContent::Planter.is_driveable());
        assert!(TileContent::Concrete.is_driveable());
    }
}
