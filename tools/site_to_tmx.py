#!/usr/bin/env python3
"""
Convert .site.json level templates to Tiled .tmx format.

This script converts the game's custom JSON site templates to the industry-standard
Tiled map format (.tmx), enabling visual level design using the Tiled map editor.

Usage:
    python tools/site_to_tmx.py assets/data/sites/01_first_street.site.json assets/maps/01_first_street.tmx
    
    # Convert all levels:
    for f in assets/data/sites/*.site.json; do
        name=$(basename "$f" .site.json)
        python tools/site_to_tmx.py "$f" "assets/maps/${name}.tmx"
    done
"""

import json
import sys
import os
from pathlib import Path
import xml.etree.ElementTree as ET
from xml.dom import minidom

# Import generated tile mapping (auto-generated from asset_manifest.json)
from generated_tile_mapping import CONTENT_TO_TILE_ID

# Default tile for unrecognized content types
DEFAULT_TILE_ID = 0  # Grass


def site_json_to_tmx(json_path: str, tmx_path: str) -> None:
    """Convert a .site.json file to Tiled .tmx format."""
    
    with open(json_path, 'r') as f:
        site_data = json.load(f)
    
    # Extract grid dimensions
    grid_size = site_data.get('grid_size', [16, 12])
    width, height = grid_size[0], grid_size[1]
    
    # Create the root map element
    map_elem = ET.Element('map', {
        'version': '1.10',
        'tiledversion': '1.11.0',
        'orientation': 'orthogonal',
        'renderorder': 'right-down',
        'width': str(width),
        'height': str(height),
        'tilewidth': '64',
        'tileheight': '64',
        'infinite': '0',
        'nextlayerid': '3',
        'nextobjectid': '1',
    })
    
    # Add map properties (site metadata)
    map_props = ET.SubElement(map_elem, 'properties')
    ET.SubElement(map_props, 'property', {
        'name': 'archetype',
        'value': site_data.get('archetype', 'parking_lot')
    })
    ET.SubElement(map_props, 'property', {
        'name': 'site_name',
        'value': site_data.get('name', 'Unknown Site')
    })
    ET.SubElement(map_props, 'property', {
        'name': 'grid_capacity_kva',
        'type': 'float',
        'value': str(site_data.get('grid_capacity_kva', 500.0))
    })
    ET.SubElement(map_props, 'property', {
        'name': 'popularity',
        'type': 'float',
        'value': str(site_data.get('popularity', 50.0))
    })
    ET.SubElement(map_props, 'property', {
        'name': 'rent_cost',
        'type': 'float',
        'value': str(site_data.get('rent_cost', 0.0))
    })
    ET.SubElement(map_props, 'property', {
        'name': 'challenge_level',
        'type': 'int',
        'value': str(site_data.get('challenge_level', 1))
    })
    ET.SubElement(map_props, 'property', {
        'name': 'description',
        'value': site_data.get('description', '')
    })
    
    # Add tileset reference
    # firstgid=1 means tile IDs in the layer data are offset by 1 (0 = no tile)
    ET.SubElement(map_elem, 'tileset', {
        'firstgid': '1',
        'source': '../tilesets/kilowatt_tiles.tsx'
    })
    
    # Initialize grid with Grass tiles (tile ID 0)
    # In TMX CSV data, 0 means "no tile", so we use firstgid offset
    # Grass = tile ID 0 in tileset -> 1 in layer data
    grid = [[CONTENT_TO_TILE_ID["Grass"] + 1 for _ in range(width)] for _ in range(height)]
    
    # Initialize a separate locked tiles tracking grid
    locked_grid = [[False for _ in range(width)] for _ in range(height)]
    
    # Process initial layout
    layout = site_data.get('initial_layout', {})
    
    # Fill suggested zones with appropriate base tiles BEFORE placing locked tiles
    # This replicates the exact logic from apply_zone() in loader.rs
    for zone in layout.get('suggested_zones', []):
        zone_type = zone.get('type', 'unknown')
        bounds = zone.get('bounds', [[0, 0], [1, 1]])
        
        x1, y1 = bounds[0]
        x2, y2 = bounds[1]
        min_x, max_x = min(x1, x2), max(x1, x2)
        min_y, max_y = min(y1, y2), max(y1, y2)
        
        if zone_type == 'parking_area':
            # Replicate loader.rs apply_zone() parking_area logic
            
            # 1. Fill main parking area with Lot tiles
            for game_y in range(min_y, max_y + 1):
                for x in range(min_x, max_x + 1):
                    if 0 <= x < width and 0 <= game_y < height:
                        tiled_y = height - 1 - game_y
                        if not locked_grid[tiled_y][x]:
                            grid[tiled_y][x] = CONTENT_TO_TILE_ID["Lot"] + 1
            
            # 2. Fill driveway connecting parking area to road row (y=11)
            if max_y < 11:
                for game_y in range(max_y + 1, 12):  # max_y to 11 inclusive
                    for x in range(min_x, max_x + 1):
                        if 0 <= x < width and 0 <= game_y < height:
                            tiled_y = height - 1 - game_y
                            if not locked_grid[tiled_y][x]:
                                grid[tiled_y][x] = CONTENT_TO_TILE_ID["Lot"] + 1
            
            # 3. Fill complete road row (y=11) with RoadYellowLine tiles to ensure Entry to Exit connectivity
            # This fills any gaps between Entry/Road/Exit locked tiles with proper road visuals (yellow center line)
            road_y = 11
            if 0 <= road_y < height:
                tiled_road_y = height - 1 - road_y
                for x in range(width):
                    if not locked_grid[tiled_road_y][x]:
                        grid[tiled_road_y][x] = CONTENT_TO_TILE_ID["RoadYellowLine"] + 1
            
            # 4. Generate parking bays with ChargerPad tiles above them
            # Every 2 tiles horizontally (starting at min_x+1), every 4 rows vertically
            # This matches the pattern in loader.rs: (min_x+1..max_x).step_by(2)
            for game_y in range(min_y, max_y, 4):
                for x in range(min_x + 1, max_x, 2):
                    if 0 <= x < width and 0 <= game_y < height:
                        tiled_y = height - 1 - game_y
                        # Only place bay if not locked and there's room for charger pad above (y+1)
                        if not locked_grid[tiled_y][x] and game_y < max_y:
                            # Check if y+1 position is also available (for ChargerPad)
                            tiled_y_plus_1 = height - 1 - (game_y + 1)
                            if 0 <= game_y + 1 < height and not locked_grid[tiled_y_plus_1][x]:
                                # Place ParkingBaySouth at (x, y)
                                grid[tiled_y][x] = CONTENT_TO_TILE_ID["ParkingBaySouth"] + 1
                                # Place ChargerPad at (x, y+1) - directly above the parking bay
                                grid[tiled_y_plus_1][x] = CONTENT_TO_TILE_ID["ChargerPad"] + 1
        
        elif zone_type == 'transformer_zone':
            # Fill transformer zone with Grass
            for game_y in range(min_y, max_y + 1):
                for x in range(min_x, max_x + 1):
                    if 0 <= x < width and 0 <= game_y < height:
                        tiled_y = height - 1 - game_y
                        if not locked_grid[tiled_y][x]:
                            grid[tiled_y][x] = CONTENT_TO_TILE_ID["Grass"] + 1
    
    # Place locked tiles (these will override zone fills)
    for tile in layout.get('locked_tiles', []):
        pos = tile.get('pos', [0, 0])
        x, y = pos[0], pos[1]
        content = tile.get('content', 'Grass')
        
        if 0 <= x < width and 0 <= y < height:
            tile_id = CONTENT_TO_TILE_ID.get(content, DEFAULT_TILE_ID)
            # Invert Y coordinate: Tiled uses top-left origin, game uses bottom-left
            tiled_y = height - 1 - y
            grid[tiled_y][x] = tile_id + 1  # +1 for firstgid offset
            locked_grid[tiled_y][x] = True
    
    # Create the base tile layer
    layer = ET.SubElement(map_elem, 'layer', {
        'id': '1',
        'name': 'tiles',
        'width': str(width),
        'height': str(height),
    })
    
    # Convert grid to CSV format
    csv_rows = []
    for row in grid:
        csv_rows.append(','.join(str(cell) for cell in row))
    csv_data = ',\n'.join(csv_rows)
    
    data_elem = ET.SubElement(layer, 'data', {'encoding': 'csv'})
    data_elem.text = '\n' + csv_data + '\n'
    
    # Create object layer for zones
    objectgroup = ET.SubElement(map_elem, 'objectgroup', {
        'id': '2',
        'name': 'zones'
    })
    
    obj_id = 1
    for zone in layout.get('suggested_zones', []):
        zone_type = zone.get('type', 'unknown')
        bounds = zone.get('bounds', [[0, 0], [1, 1]])
        
        x1, y1 = bounds[0]
        x2, y2 = bounds[1]
        
        # Calculate position and size in pixels
        # Note: Tiled Y is inverted from game coordinates
        min_x = min(x1, x2)
        max_x = max(x1, x2)
        min_y = min(y1, y2)
        max_y = max(y1, y2)
        
        # Convert to Tiled coordinates (Y inverted)
        tiled_y = height - 1 - max_y
        
        pixel_x = min_x * 64
        pixel_y = tiled_y * 64
        pixel_width = (max_x - min_x + 1) * 64
        pixel_height = (max_y - min_y + 1) * 64
        
        obj = ET.SubElement(objectgroup, 'object', {
            'id': str(obj_id),
            'type': zone_type,
            'x': str(pixel_x),
            'y': str(pixel_y),
            'width': str(pixel_width),
            'height': str(pixel_height),
        })
        
        # Add zone-specific properties
        if zone_type == 'parking_area':
            props = ET.SubElement(obj, 'properties')
            ET.SubElement(props, 'property', {
                'name': 'generates_bays',
                'type': 'bool',
                'value': 'true'
            })
        elif zone_type == 'transformer_zone':
            props = ET.SubElement(obj, 'properties')
            ET.SubElement(props, 'property', {
                'name': 'suggested_placement',
                'type': 'bool',
                'value': 'true'
            })
        
        obj_id += 1
    
    # Write formatted XML
    rough_string = ET.tostring(map_elem, 'utf-8')
    reparsed = minidom.parseString(rough_string)
    
    # Ensure output directory exists
    os.makedirs(os.path.dirname(tmx_path), exist_ok=True)
    
    with open(tmx_path, 'w') as f:
        f.write(reparsed.toprettyxml(indent='  '))
    
    print(f"Converted: {json_path} -> {tmx_path}")


def main():
    if len(sys.argv) < 2:
        print("Usage: python site_to_tmx.py <input.site.json> <output.tmx>")
        print("       python site_to_tmx.py --all  # Convert all site files")
        sys.exit(1)
    
    if sys.argv[1] == '--all':
        # Convert all site files
        sites_dir = Path('assets/data/sites')
        maps_dir = Path('assets/maps')
        
        if not sites_dir.exists():
            print(f"Error: {sites_dir} not found")
            sys.exit(1)
        
        for json_file in sorted(sites_dir.glob('*.site.json')):
            name = json_file.stem.replace('.site', '')
            tmx_file = maps_dir / f"{name}.tmx"
            site_json_to_tmx(str(json_file), str(tmx_file))
    elif len(sys.argv) >= 3:
        json_path = sys.argv[1]
        tmx_path = sys.argv[2]
        site_json_to_tmx(json_path, tmx_path)
    else:
        print("Usage: python site_to_tmx.py <input.site.json> <output.tmx>")
        print("       python site_to_tmx.py --all  # Convert all site files")
        sys.exit(1)


if __name__ == '__main__':
    main()
