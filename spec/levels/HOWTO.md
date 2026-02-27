# Level Design Guide

This document explains how to design and configure site levels for Kilowatt Tycoon.

## File Structure

Each level consists of:
- **Site template**: `assets/data/sites/NN_level_name.site.json`
- **Spec document**: `spec/levels/NN_level_name.md`
- **Screenshot**: `spec/levels/level_NN_level_name.png` (auto-generated)

## Site Template Schema

```json
{
  "archetype": "mall",
  "name": "Westfield Garage",
  "grid_size": [20, 14],
  "rent_cost": 12000.0,
  "popularity": 70,
  "challenge_level": 4,
  "grid_capacity_kva": 250.0,
  "description": "Description shown to player...",
  "initial_layout": {
    "locked_tiles": [...],
    "suggested_zones": [...]
  }
}
```

### Grid Coordinates

- Origin `[0, 0]` is **top-left** of the grid
- `x` increases to the right
- `y` increases downward
- Road/entry/exit are typically at the **bottom** (high y values)

For a 20x14 grid:
```
y=0  ┌────────────────────┐  (Mall facade, buildings)
     │                    │
     │   Parking area     │
     │                    │
y=13 └────────────────────┘  (Entry/Exit road)
     x=0                x=19
```

## Locked Tiles

Locked tiles are pre-placed and **cannot be sold by the player**. They define the visual character of the level.

```json
"locked_tiles": [
  {"pos": [0, 13], "content": "Entry"},
  {"pos": [19, 13], "content": "Exit"},
  {"pos": [5, 0], "content": "StoreEntrance"},
  {"pos": [0, 3], "content": "GaragePillar"}
]
```

### Available Tile Types

| Tile Type | Driveable | Notes |
|-----------|-----------|-------|
| `Entry` | Yes | Vehicle entry point (required) |
| `Exit` | Yes | Vehicle exit point (required) |
| `Road` | Yes | Public road surface |
| `Lot` | Yes | Asphalt parking surface |
| `GarageFloor` | Yes | Concrete garage surface |
| `Concrete` | Yes | Concrete walkway/drive lane |
| `GaragePillar` | **No** | Structural column (blocks placement) |
| `MallFacade` | **No** | Building edge |
| `StoreEntrance` | **No** | Visible store entry |
| `StoreWall` | **No** | Store wall section |
| `Bollard` | **No** | Safety barrier |
| `LightPole` | **No** | Lighting fixture |
| `Planter` | **No** | Decorative planter |
| `BrickSidewalk` | **No** | Brick pedestrian area |
| `Crosswalk` | Yes | Pedestrian crossing |
| `BikeLane` | Yes | Bike lane marking |
| `StreetRoad` | Yes | Urban street |
| `StreetTree` | **No** | Street tree |

Any tile NOT in `locked_tiles` defaults to **Grass** (buildable).

## Suggested Zones

Zones automate initial setup. **Order matters** - zones are processed sequentially.

```json
"suggested_zones": [
  {"type": "parking_area", "bounds": [[5, 2], [18, 5]]},
  {"type": "transformer_zone", "bounds": [[1, 2], [4, 11]]}
]
```

### Zone Types

| Type | Behavior |
|------|----------|
| `parking_area` | Fills with Lot, adds driveway, places parking bays + chargers |
| `transformer_zone` | Hint only - does NOT reserve grass |
| `solar_zone` | Hint only |
| `battery_zone` | Hint only |

## CRITICAL: Parking Area Driveway Fill

The `parking_area` zone has **hidden behavior** that extends beyond its bounds:

```rust
// In src/data/loader.rs apply_zone()
if max_y < 11 {
    grid.fill_lot_area(min_x, max_y, max_x, 11);
}
```

This means a parking area fills a **driveway** from its bottom edge down to y=11 using the **same x range**.

### Example

```json
{"type": "parking_area", "bounds": [[1, 2], [18, 5]]}
```

This fills:
- **Main area**: x=1-18, y=2-5 → Lot
- **Driveway**: x=1-18, y=5-11 → Lot (because 5 < 11)

The driveway covers a MUCH larger area than the specified bounds!

### How to Reserve Space for Transformer

To leave grass for transformer/battery/solar placement:

1. **Both parking areas must start at the same x** to leave a consistent grass column
2. The grass column must be wide enough (2+ tiles for transformer)
3. Account for the driveway fill extending down

```json
"suggested_zones": [
  {"type": "parking_area", "bounds": [[3, 2], [18, 5]]},
  {"type": "parking_area", "bounds": [[3, 7], [13, 12]]},
  {"type": "transformer_zone", "bounds": [[1, 2], [2, 11]]}
]
```

This leaves x=1-2 as grass for the entire height.

## Parking Bay Placement

Within a `parking_area`, bays are placed automatically:
- Every **2 tiles** horizontally (starting at min_x + 1)
- Every **4 tiles** vertically
- Only on Lot tiles
- Chargers are auto-placed on each bay

## Structure Sizes

| Structure | Size | Placement |
|-----------|------|-----------|
| Transformer | 2x2 | Grass only |
| Battery | 2x2 | Grass only |
| Solar Array | 3x2 | Grass only |
| Amenity (L1) | 3x3 | Grass only |
| Amenity (L2) | 4x4 | Grass only |
| Amenity (L3) | 5x4 | Grass only |

## Common Pitfalls

### 1. Driveway Covers Transformer Zone

**Problem**: Upper parking area's driveway fill covers intended transformer area.

**Solution**: Start all parking areas at the same x offset, leaving consistent grass columns.

### 2. Pillars Block Transformer Placement

**Problem**: Pillars at x=0 or in the transformer zone block 2x2 placement.

**Solution**: Keep pillars away from transformer zone. Use x >= parking start for pillars in lower sections.

### 3. Zone Hints Don't Reserve Space

**Problem**: `transformer_zone` doesn't actually keep tiles as grass.

**Solution**: It's just a UI hint. You must ensure no `parking_area` covers those tiles (including driveway fill).

### 4. Locked Tiles in Build Area

**Problem**: Locked tiles like LightPole or Bollard block player building.

**Solution**: Place locked decorative elements at edges (x=0, x=max) or in non-buildable areas.

## Testing a Level

1. Edit the `.site.json` file
2. Run screenshot mode: `cargo run --release -- --screenshot`
3. Check the generated PNG in `spec/levels/`
4. Run the game normally and try to place a transformer in the intended area

## Visual Design Tips

- Use **pillars in a grid pattern** for garage feel (every 4-5 tiles)
- Add **concrete drive lanes** to break up large parking areas
- Include **store entrances** or **building features** at edges
- Place **light poles** at regular intervals for realism
- Use **bollards** around elevator cores or restricted areas
- Keep a clear **entry/exit flow** with road tiles

## Code References

| File | Purpose |
|------|---------|
| `src/data/loader.rs` | Zone processing, tile parsing |
| `src/resources/site_grid.rs` | TileContent enum, placement logic |
| `src/systems/scene.rs` | Tile rendering |
| `src/resources/asset_handles.rs` | Asset loading for tiles |
| `src/states/loading.rs` | Site template loading paths |
