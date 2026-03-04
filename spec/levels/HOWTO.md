# Level Design Guide

This document explains how to design and configure site levels for Kilowatt Tycoon.

For detailed Tiled editor instructions, see [TILED_WORKFLOW.md](TILED_WORKFLOW.md).

## Architecture

**TMX files are the single source of truth** for level data. The legacy `.site.json` format has been removed.

Each level is defined by:
- **TMX map** (`assets/maps/NN_level_name.tmx`) - Tile layout, zone objects, and map properties
- **Scenario JSON** (`assets/data/scenarios/*.scenario.json`) - Driver schedule and scripted events
- **Charger JSON** (`assets/data/chargers/*.chargers.json`) - Pre-placed charger definitions
- **Spec document** (`spec/levels/NN_level_name.md`) - Design notes and strategy guide
- **Zones JSON** (`spec/levels/NN_level_name.zones.json`) - Zone layout reference (mirrors TMX zones)

## Current Levels

| # | File | Archetype | Grid | kVA | Rent | Pop | Climate |
|---|------|-----------|------|-----|------|-----|---------|
| 1 | `01_first_street.tmx` | `ParkingLot` | 16x12 | 1500 | $0 | 60 | Mild |
| 2 | `02_quick_charge_express.tmx` | `GasStation` | 16x12 | 500 | $5,000 | 85 | Cold (NJ winter) |
| 3 | `03_central_fleet_plaza.tmx` | `FleetDepot` | 30x20 | 3000 | $35,000 | 75 | Warm |
| 4 | `04_scooter_alley.tmx` | `ScooterHub` | 30x20 | 800 | $28,000 | 95 | Hot & Humid (HCMC) |

## File Structure

```
assets/
├── maps/                              # TMX maps (single source of truth)
│   ├── 01_first_street.tmx
│   ├── 02_quick_charge_express.tmx
│   ├── 03_central_fleet_plaza.tmx
│   └── 04_scooter_alley.tmx
├── tilesets/
│   └── kilowatt_tiles.tsx             # Shared tileset definition
├── data/
│   ├── scenarios/
│   │   ├── mvp_drivers.scenario.json  # Default scenario
│   │   └── hcmc_scooters.scenario.json # Scooter-focused scenario
│   └── chargers/
│       └── mvp_chargers.chargers.json # Charger definitions
└── world/tiles/                       # PNG tile images (64x64)
    └── tile_*.png

spec/levels/
├── HOWTO.md                           # This file
├── TILED_WORKFLOW.md                  # Tiled editor guide
├── 01_first_street.md                 # Level design docs
├── 01_first_street.zones.json
├── 02_quick_charge_express.md
├── 02_quick_charge_express.zones.json
├── 03_central_fleet_plaza.md
├── 03_central_fleet_plaza.zones.json
├── 04_scooter_alley.md
└── 04_scooter_alley.zones.json
```

## TMX Map Properties

Every TMX file must have these map properties (set in Tiled via Map > Map Properties):

| Property | Type | Required | Notes |
|----------|------|----------|-------|
| `archetype` | string | Yes | One of: `parking_lot`, `gas_station`, `fleet_depot`, `scooter_hub` |
| `site_name` | string | Yes | Display name shown to the player |
| `grid_capacity_kva` | float | Yes | Utility connection hard limit |
| `popularity` | float | Yes | Base demand level (0-100) |
| `rent_cost` | float | Yes | One-time purchase cost |
| `challenge_level` | int | Yes | Difficulty rating (1-5) |
| `description` | string | Yes | Site card description text |

## Archetypes

| Enum Variant | TMX String | Climate Offset | Notes |
|-------------|------------|----------------|-------|
| `ParkingLot` | `parking_lot` | +5 F | Mild climate, beginner-friendly |
| `GasStation` | `gas_station` | -35 F | Cold NJ winters slow charging |
| `FleetDepot` | `fleet_depot` | +15 F | Hot industrial, high power |
| `ScooterHub` | `scooter_hub` | +15 F | Tropical HCMC, 97% two-wheelers |

Archetype strings are parsed in `src/data/tiled_loader.rs` (`parse_archetype`). Each archetype controls temperature offset, ambient traffic mix, procedural vehicle distribution, and site tab color.

## TMX Layers

### Tile Layer (`tiles`)

Contains all base terrain. Each tile ID maps to a `TileContent` variant via `tile_id_to_content()` in `src/data/tiled_loader.rs`. Common tile IDs (0-indexed from tileset):

| ID | Content | Buildable | Driveable |
|----|---------|-----------|-----------|
| 0 | Grass | Yes | No |
| 1 | Road | No | Yes |
| 2 | Entry | No | Yes |
| 3 | Exit | No | Yes |
| 4 | Lot | No | Yes |
| 5 | ParkingBayNorth | No | Yes |
| 6 | ParkingBaySouth | No | Yes |
| 7 | Concrete | No | Yes |
| 36 | ChargerPad | No | Yes |
| 37 | TransformerPad | No | Yes |
| 38 | SolarPad | No | Yes |
| 39 | BatteryPad | No | Yes |

See `assets/tilesets/kilowatt_tiles.tsx` for the full list.

### Object Layer (`zones`)

Rectangle objects defining gameplay areas. The `type` attribute determines behavior:

| Zone Type | Purpose |
|-----------|---------|
| `entry` | Vehicle entry point (exactly one required) |
| `exit` | Vehicle exit point (exactly one required) |
| `parking_area` | Auto-generates parking bays and lot surface |
| `transformer_zone` | Hint for transformer placement (grass preserved) |
| `canopy_zone` | Marks overhead canopy area |

Zone coordinates are in **pixels** (grid * 64). Y is flipped when converted to game coordinates.

**Entry/exit zones must be pixel-aligned to tile boundaries** (exact multiples of 64). The loader truncates pixel coordinates to grid indices via `(pixel / 64) as i32`. A value like `y=1086` truncates to row 16 instead of the intended row 17 (`1088 / 64`). If the truncated position lands on a non-driveable tile (e.g. grass), pathfinding to the exit silently fails and vehicles force-despawn after 5 seconds instead of driving away. Symptom: vehicles vanish in place rather than animating toward the exit.

## Grid Coordinates

Tiled uses top-left origin; the game uses bottom-left.

```
Tiled row 0   = Game row (height - 1)   [top of map]
Tiled row N   = Game row (height - 1 - N)
```

For a 30x20 map: Tiled row 9 (entry at pixel y=576) becomes game row 10.

## Locked Tiles

Tiles that are automatically locked (player cannot sell) are determined by content type in `should_tile_be_locked()`. Road, StoreWall, Canopy, Bollard, StreetTree, Concrete, and similar structural tiles are locked. Grass tiles are always buildable.

## Parking Area Behavior

The `parking_area` zone has driveway fill logic in `src/data/loader.rs`:

```rust
if max_y < 11 {
    grid.fill_lot_area(min_x, max_y, max_x, 11);
}
```

This fills a driveway from the parking area's bottom edge down to y=11. Account for this when placing parking zones above the road -- the fill extends well beyond the zone bounds.

## Structure Sizes

| Structure | Size | Placement |
|-----------|------|-----------|
| Transformer | 2x2 | Grass only |
| Battery | 2x2 | Grass only |
| Solar Array | 3x2 | Grass only |
| Amenity (L1) | 3x3 | Grass only |
| Amenity (L2) | 4x4 | Grass only |
| Amenity (L3) | 5x4 | Grass only |

## Testing a Level

### Standard run

```bash
cargo run
```

Navigate to the Rent tab and select your level.

### Screenshot mode

```bash
cargo run --release -- --screenshot
```

Auto-captures a PNG of each level into `spec/levels/`.

### Scooter scenario

```bash
KWT_SCENARIO=hcmc_scooters cargo run
```

Loads the HCMC scooter driver schedule instead of the default MVP scenario.

## New Level Checklist

When adding a new level (as was done for Level 4 - Scooter Alley), touch all of these:

### Assets
- [ ] `assets/maps/NN_level_name.tmx` -- TMX map with tile layer, zones layer, and map properties
- [ ] `assets/data/scenarios/*.scenario.json` -- new or updated driver schedule (optional)
- [ ] `assets/data/chargers/*.chargers.json` -- new charger entries if needed

### Rust Code
- [ ] `src/resources/multi_site.rs` -- add variant to `SiteArchetype` enum; update `display_name()`, `description()`, `all_variants()`, `temperature_offset_f()`; optionally update `climate_warning()`
- [ ] `src/data/tiled_loader.rs` -- add string mapping in `parse_archetype()`; add test case
- [ ] `src/states/loading.rs` -- add `(SiteArchetype::NewVariant, "maps/NN_level_name.tmx")` to `tiled_maps` list; update asset count comment
- [ ] `src/systems/screenshot.rs` -- add filename mapping in `get_screenshot_filename()`
- [ ] `src/ui/site_tabs.rs` -- add tab color in `get_archetype_color()`
- [ ] `src/resources/technician.rs` -- add travel time entries in `calculate_travel_time()` if cross-site distances differ
- [ ] `src/systems/ambient_traffic.rs` -- add per-archetype vehicle spawn weights if traffic mix differs
- [ ] `src/resources/demand.rs` -- add per-archetype procedural vehicle distribution in `random_vehicle_type_for_site()` if needed

### Spec Files
- [ ] `spec/levels/NN_level_name.md` -- design document
- [ ] `spec/levels/NN_level_name.zones.json` -- zone layout reference
- [ ] `spec/levels/HOWTO.md` -- update Current Levels table

### Verification
- [ ] `cargo fmt && cargo check` -- compiles cleanly
- [ ] `cargo clippy --all --benches --tests --examples --all-features` -- no warnings
- [ ] `cargo test` -- all tests pass (including `test_parse_archetype`)
- [ ] Run the game and rent the new site -- verify tiles, zones, entry/exit, and traffic

## Code References

| File | Purpose |
|------|---------|
| `src/data/tiled_loader.rs` | TMX parsing, archetype recognition, tile ID mapping |
| `src/data/loader.rs` | Zone processing, tile content parsing, layout application |
| `src/states/loading.rs` | Asset loading, TMX map registration, scenario selection |
| `src/resources/multi_site.rs` | `SiteArchetype` enum, climate offsets, site state |
| `src/resources/site_grid.rs` | `TileContent` enum, grid placement logic |
| `src/resources/demand.rs` | Procedural driver generation, per-archetype vehicle mix |
| `src/systems/ambient_traffic.rs` | Ambient traffic spawning, per-archetype vehicle weights |
| `src/systems/driver.rs` | Driver spawn system, scheduled + procedural arrivals |
| `src/resources/asset_handles.rs` | Image asset loading |
| `src/ui/site_tabs.rs` | Site tab colors per archetype |
| `src/systems/screenshot.rs` | Screenshot filename mapping per archetype |
| `src/resources/technician.rs` | Cross-site travel time calculation |

## Visual Design Tips

- Use **pillars in a grid pattern** for garage feel (every 4-5 tiles)
- Add **concrete drive lanes** to break up large parking areas
- Include **store entrances** or **building features** at edges
- Place **light poles** at regular intervals for realism
- Use **bollards** around elevator cores or restricted areas
- Keep a clear **entry/exit flow** with road tiles
- For scooter hubs: pack bays as densely as possible with narrow service lanes
