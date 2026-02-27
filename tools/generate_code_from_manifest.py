#!/usr/bin/env python3
"""Generate code and mappings from asset_manifest.json.

This script is the single source of code generation. It reads the unified
asset manifest and generates:
- Rust enum definitions
- Rust parsing functions (string → enum)
- Rust serialization functions (enum → string)  
- Python ID mappings for tools
- Tiled TSX tileset file

Usage:
    python tools/generate_code_from_manifest.py
"""

import json
import sys
from pathlib import Path
from typing import Any


class CodeGenerator:
    """Generates code from asset manifest."""

    def __init__(self, manifest_path: Path, workspace_root: Path):
        """Initialize code generator."""
        self.manifest_path = manifest_path
        self.workspace_root = workspace_root
        
        with open(manifest_path) as f:
            self.manifest = json.load(f)
    
    def generate_all(self):
        """Generate all code and mappings."""
        print("=" * 70)
        print("CODE GENERATION FROM MANIFEST")
        print("=" * 70)
        print(f"Manifest: {self.manifest_path}")
        print(f"Version: {self.manifest.get('version', 'unknown')}")
        print()
        
        # Generate each output
        self.generate_tile_id_mapping_python()
        self.generate_tileset_tsx()
        
        print()
        print("=" * 70)
        print("GENERATION COMPLETE")
        print("=" * 70)
        print()
        print("Generated files:")
        print("  - tools/generated_tile_mapping.py")
        print("  - assets/tilesets/kilowatt_tiles.tsx")
        print()
        print("Note: Rust code generation will be added in future updates.")
        print("For now, manual Rust files remain authoritative.")
    
    def generate_tile_id_mapping_python(self):
        """Generate Python tile ID mapping for site_to_tmx.py."""
        tiles = self.manifest["tiles"]["entries"]
        
        # Build mapping
        lines = [
            "# AUTO-GENERATED from asset_manifest.json",
            "# Do not edit manually! Run: python tools/generate_code_from_manifest.py",
            "",
            "\"\"\"Tile content name to tile ID mapping.\"\"\"",
            "",
            "CONTENT_TO_TILE_ID = {",
        ]
        
        for tile in tiles:
            name = tile["name"]
            tile_id = tile["id"]
            lines.append(f'    "{name}": {tile_id},')
        
        lines.append("}")
        lines.append("")
        
        output_path = self.workspace_root / "tools" / "generated_tile_mapping.py"
        output_path.write_text("\n".join(lines))
        print(f"✓ Generated: {output_path.relative_to(self.workspace_root)}")
    
    def generate_tileset_tsx(self):
        """Generate Tiled TSX tileset file."""
        tiles = self.manifest["tiles"]["entries"]
        fixed_tiles = self.manifest["tiles"].get("fixed_tiles", [])
        tile_config = self.manifest["tiles"]
        width = tile_config["dimensions"]["width"]
        height = tile_config["dimensions"]["height"]
        total_count = len(tiles) + len(fixed_tiles)
        
        lines = [
            '<?xml version="1.0" encoding="UTF-8"?>',
            f'<tileset version="1.10" tiledversion="1.11.0" name="kilowatt_tiles" tilewidth="{width}" tileheight="{height}" tilecount="{total_count}" columns="0">',
            ' <grid orientation="orthogonal" width="1" height="1"/>',
            '',
        ]
        
        for tile in tiles:
            tile_id = tile["id"]
            name = tile["name"]
            filename = tile["filename"]
            props = tile["properties"]
            
            # Tile element
            lines.append(f' <tile id="{tile_id}" type="{name}">')
            
            # Properties
            lines.append('  <properties>')
            lines.append(f'   <property name="content_type" value="{name}"/>')
            
            if props.get("locked", False):
                lines.append('   <property name="locked" type="bool" value="true"/>')
            if props.get("driveable", False):
                lines.append('   <property name="driveable" type="bool" value="true"/>')
            if props.get("buildable", False):
                lines.append('   <property name="buildable" type="bool" value="true"/>')
            if props.get("is_entry", False):
                lines.append('   <property name="is_entry" type="bool" value="true"/>')
            if props.get("is_exit", False):
                lines.append('   <property name="is_exit" type="bool" value="true"/>')
            if props.get("is_parking", False):
                lines.append('   <property name="is_parking" type="bool" value="true"/>')
            
            lines.append('  </properties>')
            
            # Image reference
            lines.append(f'  <image width="{width}" height="{height}" source="../world/tiles/{filename}"/>')
            lines.append(' </tile>')
            lines.append('')
        
        for tile in fixed_tiles:
            tile_id = tile["id"]
            filename = tile["filename"]
            lines.append(f' <tile id="{tile_id}">')
            lines.append(f'  <image width="{width}" height="{height}" source="../fixed/{filename}"/>')
            lines.append(' </tile>')
            lines.append('')
        
        lines.append('</tileset>')
        
        output_path = self.workspace_root / "assets" / "tilesets" / "kilowatt_tiles.tsx"
        output_path.write_text("\n".join(lines))
        print(f"✓ Generated: {output_path.relative_to(self.workspace_root)}")


def main():
    """Main entry point."""
    script_dir = Path(__file__).parent
    workspace_root = script_dir.parent
    manifest_path = script_dir / "asset_manifest.json"
    
    if not manifest_path.exists():
        print(f"Error: Manifest not found at {manifest_path}")
        sys.exit(1)
    
    try:
        generator = CodeGenerator(manifest_path, workspace_root)
        generator.generate_all()
    except Exception as e:
        print(f"Error during code generation: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
