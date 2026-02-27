//! Pre-built lot templates

use crate::resources::{GameState, SiteGrid, StructureSize, TileContent};
use bevy::prelude::*;

/// Available lot templates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LotTemplate {
    Small,
    Medium,
    Large,
}

impl LotTemplate {
    pub fn display_name(&self) -> &'static str {
        match self {
            LotTemplate::Small => "Small (4 bays)",
            LotTemplate::Medium => "Medium (6 bays)",
            LotTemplate::Large => "Large (8 bays)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            LotTemplate::Small => "Compact setup for quick start",
            LotTemplate::Medium => "Balanced layout with room to grow",
            LotTemplate::Large => "Maximum capacity, needs careful management",
        }
    }

    pub fn starting_budget(&self) -> i32 {
        1000000
    }

    pub fn bay_count(&self) -> usize {
        match self {
            LotTemplate::Small => 4,
            LotTemplate::Medium => 6,
            LotTemplate::Large => 8,
        }
    }

    pub fn transformer_capacity(&self) -> f32 {
        match self {
            LotTemplate::Small => 100.0,
            LotTemplate::Medium => 150.0,
            LotTemplate::Large => 200.0,
        }
    }

    /// Build the grid from this template
    pub fn build_grid(&self) -> SiteGrid {
        let mut grid = SiteGrid::default();

        match self {
            LotTemplate::Small => build_small_lot(&mut grid),
            LotTemplate::Medium => build_medium_lot(&mut grid),
            LotTemplate::Large => build_large_lot(&mut grid),
        }

        // Transformer capacity is now set via place_transformer() calls in the build functions
        grid
    }
}

/// Build small lot layout (4 bays in a row)
///
/// Layout (16x12 grid, y=0 is bottom, y=11 is top):
/// - Road runs along top edge (y=11): Entry at (0,11), Exit at (15,11)
/// - Driveway at x=3, from y=10 down to lot
/// - Lot area: x=3-10, y=6-10 (extended to accommodate ChargerPads)
/// - Parking bays: 4 stalls in a row at y=6 (ChargerPads will be at y=7)
/// - Transformer pre-placed at (11, 6) - 2x2 tiles
fn build_small_lot(grid: &mut SiteGrid) {
    // SiteGrid::default() provides public road at y=11

    // Pre-place transformer BEFORE filling lot (needs grass tile) - 2x2 at (11,6)
    // Moved down one tile to make room for parking layout
    let _ = grid.place_transformer(11, 6, 500.0); // 500 kVA default
    // Lock all 4 tiles of the 2x2 transformer
    for (x, y) in SiteGrid::get_footprint_tiles(11, 6, StructureSize::TwoByTwo) {
        grid.lock_tile(x, y);
    }

    // Fill the lot area with asphalt surface (extended to y=6 for bays, y=7 for charger pads)
    grid.fill_lot_area(3, 6, 10, 10);

    // Parking bays in a row (4 bays at y=6, charger pads will go at y=7 when chargers placed)
    let bay_positions = [
        (4, 6),  // Bay 1
        (6, 6),  // Bay 2
        (8, 6),  // Bay 3
        (10, 6), // Bay 4
    ];

    for (x, y) in bay_positions {
        grid.set_tile_content(x, y, TileContent::ParkingBaySouth);
        // Lock the parking bay so it can't be sold
        grid.lock_tile(x, y);
    }
}

/// Build medium lot layout (6 bays in two rows)
///
/// Layout:
/// - Road at top edge (y=11)
/// - Larger lot area with 6 parking bays in two rows
/// - Row 1: bays at y=5, charger pads at y=6
/// - Row 2: bays at y=7, charger pads at y=8
/// - Transformer pre-placed at (11, 4) - 2x2 tiles
fn build_medium_lot(grid: &mut SiteGrid) {
    // SiteGrid::default() provides public road at y=11

    // Pre-place transformer BEFORE filling lot (needs grass tile) - 2x2 at (11,4)
    // Moved down to accommodate new bay positions
    let _ = grid.place_transformer(11, 4, 500.0); // 500 kVA default
    // Lock all 4 tiles of the 2x2 transformer
    for (x, y) in SiteGrid::get_footprint_tiles(11, 4, StructureSize::TwoByTwo) {
        grid.lock_tile(x, y);
    }

    // Fill the lot area with asphalt surface (extended to accommodate ChargerPads)
    grid.fill_lot_area(2, 5, 10, 10);

    // Parking bays in two rows (6 bays total)
    // Row 1: y=5 (charger pads at y=6)
    // Row 2: y=7 (charger pads at y=8)
    let bay_positions = [
        (3, 5), // Bay 1 - Row 1
        (5, 5), // Bay 2 - Row 1
        (7, 5), // Bay 3 - Row 1
        (9, 5), // Bay 4 - Row 1
        (4, 7), // Bay 5 - Row 2
        (6, 7), // Bay 6 - Row 2
    ];

    for (x, y) in bay_positions {
        grid.set_tile_content(x, y, TileContent::ParkingBaySouth);
        // Lock the parking bay so it can't be sold
        grid.lock_tile(x, y);
    }
}

/// Build large lot layout (8 bays, two rows of 4)
///
/// Layout:
/// - Road at top edge (y=11)
/// - Large lot with two rows of parking bays
/// - Top row: bays at y=7, charger pads at y=8
/// - Bottom row: bays at y=4, charger pads at y=5
/// - Transformer pre-placed at (13, 0) - 2x2 tiles
fn build_large_lot(grid: &mut SiteGrid) {
    // SiteGrid::default() provides public road at y=11

    // Pre-place transformer BEFORE filling lot (needs grass tile) - 2x2 at (13,0)
    let _ = grid.place_transformer(13, 0, 500.0); // 500 kVA default
    // Lock all 4 tiles of the 2x2 transformer
    for (x, y) in SiteGrid::get_footprint_tiles(13, 0, StructureSize::TwoByTwo) {
        grid.lock_tile(x, y);
    }

    // Fill the lot area with asphalt surface (extended to accommodate ChargerPads)
    grid.fill_lot_area(2, 4, 11, 10);

    // Two rows of parking bays (8 bays total)
    // Top row: y=7 (charger pads at y=8)
    // Bottom row: y=4 (charger pads at y=5)
    let bay_positions = [
        // Top row (closer to road)
        (3, 7), // Bay 1
        (5, 7), // Bay 2
        (7, 7), // Bay 3
        (9, 7), // Bay 4
        // Bottom row
        (4, 4),  // Bay 5
        (6, 4),  // Bay 6
        (8, 4),  // Bay 7
        (10, 4), // Bay 8
    ];

    for (x, y) in bay_positions {
        grid.set_tile_content(x, y, TileContent::ParkingBaySouth);
        // Lock the parking bay so it can't be sold
        grid.lock_tile(x, y);
    }
}

/// Resource tracking the selected template (before game starts)
#[derive(Resource, Default, Debug, Clone)]
pub struct SelectedTemplate {
    pub template: Option<LotTemplate>,
}

/// Apply a template to the grid and game state
pub fn apply_template(template: LotTemplate, grid: &mut SiteGrid, game_state: &mut GameState) {
    *grid = template.build_grid();
    game_state.cash = template.starting_budget() as f32;
}
