# Level Design with Tiled

This guide explains how to design levels for Kilowatt Tycoon using the Tiled map editor.

## Architecture Overview

The game uses **TMX files as the single source of truth** for level data:

- **bevy_ecs_tiled** renders the tilemap visuals (from `.tmx` files)
- **SiteGrid** maintains gameplay state (initialized from `.tmx` tile/object data)
- **Entity overlays** for infrastructure with dynamic state (transformers, solar, batteries)

TMX files contain:
- **Tile layer** - All base tiles (terrain, walls, parking, decorations)
- **Object layer** - Gameplay zones (parking areas, transformer suggestions)
- **Map properties** - Gameplay config (archetype, capacity, popularity, rent cost)

This approach enables:
- Visual level design in the Tiled editor (no code changes needed)
- Designer-friendly workflow (edit TMX, reload game)
- Single source of truth (no JSON sync issues)
- Clean separation: Tiled = visuals + layout, SiteGrid = gameplay logic

## Setup

### Install Tiled

1. Download Tiled from https://www.mapeditor.org/
2. Install for your platform (macOS, Windows, Linux)

### Open Existing Level

1. Launch Tiled
2. Open an existing level: `assets/maps/03_westfield_garage.tmx`
3. The tileset should load automatically from `assets/tilesets/kilowatt_tiles.tsx`

## File Structure

```
assets/
├── maps/                    # TMX map files (single source of truth)
│   ├── 01_first_street.tmx
│   ├── 02_quick_charge_express.tmx
│   └── ...
├── tilesets/
│   └── kilowatt_tiles.tsx   # Tileset definition with tile properties
├── data/sites/              # JSON templates (legacy fallback)
│   ├── 01_first_street.site.json  # No longer required
│   └── ...
└── world/tiles/             # PNG tile images
    └── tile_*.png
```

**Note**: `.site.json` files are kept for backward compatibility but are no longer the primary data source. All gameplay config should be in the TMX files.

## Editing a Level

### Basic Tile Painting

1. Select a tile from the tileset palette (right panel)
2. Use the Stamp Brush (B) to paint tiles
3. Use the Bucket Fill (F) for larger areas
4. Use the Eraser (E) to clear tiles

### Working with Layers

The map has two layers:

1. **tiles** - The main tile layer containing floor, walls, etc.
2. **zones** - Object layer for parking areas and transformer zones

### Adding Zones

Zones define gameplay areas:

1. Select the **zones** layer
2. Use Insert → Insert Rectangle
3. Set the object type:
   - `parking_area` - Generates parking bays
   - `transformer_zone` - Suggested transformer placement

### Setting Map Properties

Map properties define gameplay configuration:

1. Map → Map Properties
2. Required properties:
   - `archetype` (string) - Site type: "parking_lot", "mall", "gas_station", etc.
   - `site_name` (string) - Display name
   - `grid_capacity_kva` (float) - Utility connection capacity
   - `popularity` (float) - Base demand level (0-100)
   - `rent_cost` (float) - One-time purchase cost
   - `challenge_level` (int) - Difficulty rating (1-5)
   - `description` (string) - Site description text

### Setting Tile Properties

Each tile type in the tileset has custom properties:

1. Right-click a tile in the tileset → Tile Properties
2. Standard properties (already configured):
   - `content_type` (string) - Maps to TileContent enum in Rust
   - `locked` (bool) - Player cannot sell this tile (set on template tiles)
   - `buildable` (bool) - Player can place structures (grass tiles)
   - `driveable` (bool) - Vehicles can traverse (roads, parking)
   - `is_entry` (bool) - Marks entry point for pathfinding
   - `is_exit` (bool) - Marks exit point for pathfinding
   - `is_parking` (bool) - Marks parking bay tiles

## Coordinate System

**Important**: Tiled uses a top-left origin, while the game uses bottom-left.

The conversion script (`tools/site_to_tmx.py`) handles this automatically:
- Y coordinates are flipped: `game_y = map_height - 1 - tiled_y`

When editing manually, remember:
- Row 0 in Tiled = Top of the map = Bottom of the game grid
- Row 11 in Tiled (for 12-row map) = Bottom of the map = Top of the game grid

## Testing Changes

### Hot-Reload (Development)

With bevy_ecs_tiled's hot-reload feature, changes to TMX files are reflected
without restarting the game (when enabled).

### Manual Testing

1. Save your changes in Tiled (Ctrl+S)
2. Run the game: `cargo run`
3. Navigate to your level
4. Verify the visual appearance

### Checking Gameplay

The game reads all data directly from TMX files:

1. Edit the `.tmx` file in Tiled
2. Save your changes (Ctrl+S / Cmd+S)
3. Run the game to test: `cargo run`
4. Changes to tiles, zones, and properties are all reflected immediately

## Converting Between Formats (Optional)

The `tools/site_to_tmx.py` script can generate TMX from JSON (for migration):

```bash
# Convert a single level (JSON → TMX)
python tools/site_to_tmx.py assets/data/sites/03_westfield_garage.site.json assets/maps/03_westfield_garage.tmx

# Convert all levels
python tools/site_to_tmx.py --all
```

**Note**: With TMX as the primary source, you typically edit TMX directly in Tiled rather than generating from JSON.

## Tile Reference

### Base Tiles

| Tile | ID | Content Type | Buildable | Driveable |
|------|-------|--------------|-----------|-----------|
| Grass | 0 | Grass | ✓ | ✗ |
| Road | 1 | Road | ✗ | ✓ |
| Entry | 2 | Entry | ✗ | ✓ |
| Exit | 3 | Exit | ✗ | ✓ |
| Lot | 4 | Lot | ✗ | ✓ |
| Parking Bay (N) | 5 | ParkingBayNorth | ✗ | ✓ |
| Parking Bay (S) | 6 | ParkingBaySouth | ✗ | ✓ |
| Concrete | 7 | Concrete | ✗ | ✓ |

### Structure Tiles

| Tile | ID | Content Type | Notes |
|------|-------|--------------|-------|
| Garage Floor | 8 | GarageFloor | Indoor surface |
| Garage Pillar | 9 | GaragePillar | Obstacle |
| Mall Facade | 10 | MallFacade | Building exterior |
| Store Wall | 11 | StoreWall | Building wall |
| Store Entrance | 12 | StoreEntrance | Door |

See `assets/tilesets/kilowatt_tiles.tsx` for the complete list.

## Zone Types

### parking_area

Defines a rectangular area where parking bays are generated.

Properties:
- `generates_bays` (bool) - Set to true

The game fills this area with parking bays and surrounding lot surface.

### transformer_zone

Marks a suggested location for transformer placement.

Properties:
- `suggested_placement` (bool) - Set to true

This is a hint for players - the area should have buildable grass tiles.

## Common Pitfalls

### 1. Coordinate Mismatch

Remember Y coordinates are inverted between Tiled and the game.

### 2. Missing Tiles in Tileset

If a tile shows as a red "X", ensure:
- The tileset file path is correct
- The PNG image exists in `assets/world/tiles/`

### 3. Gameplay and Visual Integration

TMX files control both visuals AND gameplay:
- Tile layer defines terrain, walls, parking (rendered by Tiled + used for pathfinding)
- Object layer defines zones for gameplay (parking generation, transformer hints)
- Map properties define gameplay config (capacity, popularity, rent cost)

When you edit a TMX file, both visuals and gameplay logic are updated.

### 4. Object Layer Positioning

Zone objects use pixel coordinates. Multiply grid coordinates by 64:
- Grid (3, 4) → Pixels (192, 256)

### 5. Entry/Exit Zone Misalignment

**Entry and exit zone objects must have pixel coordinates that are exact multiples of 64.** The loader converts pixels to grid indices with integer truncation: `(pixel / 64) as i32`. Tiled's drag-and-drop often produces fractional positions (e.g. `y=1086.32`), and `1086 / 64 = 16` instead of the intended `17` (`1088 / 64`). If the truncated position falls on a non-driveable tile (grass, building), pathfinding to the exit fails silently — vehicles get stuck for 5 seconds (`MAX_STUCK_TIME`) then force-despawn.

**Symptom**: Vehicles disappear in place instead of driving to the exit.
**Fix**: Snap entry/exit zone x and y to multiples of 64 (e.g. `y=1088`, not `y=1086.32`).
**Prevention**: After placing entry/exit objects, check their pixel coordinates in Tiled's property panel and round to the nearest `N * 64`.

## Best Practices

1. **Edit TMX directly** - TMX is the authoritative source. No need to edit JSON files.

2. **Test frequently** - Run the game after major changes to catch issues early

3. **Use meaningful zone names** - Add names to zone objects for easier identification

4. **Leave grass for utilities** - Ensure players have space for transformers and solar panels

5. **Consider traffic flow** - Entry/exit positions affect how vehicles navigate

6. **Validate map properties** - Ensure all required map properties are set before testing

7. **Use locked tiles** - Set `locked="true"` in tileset for template tiles (walls, roads, etc.)

## Troubleshooting

### Map doesn't load

- Check the console for asset loading errors
- Verify the TMX file path matches what's in `loading.rs`
- Ensure the tileset path is correct (relative to TMX location)
- Verify all required map properties are present (archetype, site_name, etc.)

### Tiles look wrong

- Verify tile IDs match between TMX and tileset
- Check that PNG images are 64x64 pixels
- Ensure firstgid offset is applied correctly (CSV data uses ID+1)

### Zones don't work

- Object layer must be named "zones"
- Zone `type` attribute must match exactly (e.g., "parking_area")
- Coordinates must be in pixels, not grid cells
