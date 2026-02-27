#!/usr/bin/env python3
"""Validate asset_manifest.json for correctness and consistency.

This script checks the asset manifest for common errors:
- Unique IDs
- Valid color references
- Proper filename conventions
- Reasonable dimensions
- Valid Rust identifiers
- Required fields present

Usage:
    python tools/validate_manifest.py
    
Exit codes:
    0 - Validation passed
    1 - Validation failed with errors
"""

import json
import re
import sys
from pathlib import Path
from typing import List, Set


class ManifestValidator:
    """Validates asset manifest."""

    def __init__(self, manifest_path: Path, style_config_path: Path):
        """Initialize validator."""
        self.manifest_path = manifest_path
        self.style_config_path = style_config_path
        self.errors: List[str] = []
        self.warnings: List[str] = []
        
        # Load manifest
        with open(manifest_path) as f:
            self.manifest = json.load(f)
        
        # Load color palette
        self.valid_colors = self._load_color_palette()
    
    def _load_color_palette(self) -> Set[str]:
        """Load valid color names from style_config.json."""
        if not self.style_config_path.exists():
            self.warnings.append(f"Style config not found at {self.style_config_path}")
            return set()
        
        try:
            with open(self.style_config_path) as f:
                style_config = json.load(f)
            
            color_palette = style_config.get("color_palette", {})
            return set(color_palette.keys())
        except (json.JSONDecodeError, KeyError) as e:
            self.warnings.append(f"Error loading color palette: {e}")
            return set()
    
    def error(self, message: str):
        """Add validation error."""
        self.errors.append(f"ERROR: {message}")
    
    def warning(self, message: str):
        """Add validation warning."""
        self.warnings.append(f"WARNING: {message}")
    
    def validate_all(self) -> bool:
        """Run all validations. Returns True if valid."""
        print("=" * 70)
        print("MANIFEST VALIDATION")
        print("=" * 70)
        print(f"Manifest: {self.manifest_path}")
        print(f"Version: {self.manifest.get('version', 'unknown')}")
        print()
        
        # Validate each asset type
        self.validate_tiles()
        self.validate_vehicles()
        self.validate_vfx()
        
        # Print results
        print()
        print("=" * 70)
        
        if self.errors:
            print(f"VALIDATION FAILED ({len(self.errors)} errors)")
            print("=" * 70)
            for error in self.errors:
                print(error)
            if self.warnings:
                print()
                for warning in self.warnings:
                    print(warning)
            return False
        
        if self.warnings:
            print(f"VALIDATION PASSED ({len(self.warnings)} warnings)")
            print("=" * 70)
            for warning in self.warnings:
                print(warning)
        else:
            print("VALIDATION PASSED")
            print("=" * 70)
            print("✓ No errors or warnings found")
        
        print()
        return True
    
    def validate_tiles(self):
        """Validate tile definitions."""
        if "tiles" not in self.manifest:
            self.error("Missing 'tiles' section in manifest")
            return
        
        tiles_config = self.manifest["tiles"]
        tiles = tiles_config.get("entries", [])
        
        print(f"Validating {len(tiles)} tiles...")
        
        # Check IDs are unique
        ids = [t["id"] for t in tiles]
        if len(ids) != len(set(ids)):
            self.error("Tile IDs are not unique")
        
        # Validate each tile
        for tile in tiles:
            tid = tile.get("id", "?")
            name = tile.get("name", "?")
            
            # Required fields
            if "rust_variant" not in tile:
                self.error(f"Tile {tid} ({name}): missing 'rust_variant'")
            if "filename" not in tile:
                self.error(f"Tile {tid} ({name}): missing 'filename'")
            if "properties" not in tile:
                self.error(f"Tile {tid} ({name}): missing 'properties'")
            
            # Validate Rust identifier
            rust_var = tile.get("rust_variant", "")
            if rust_var and not re.match(r'^[A-Z][A-Za-z0-9]*$', rust_var):
                self.error(f"Tile {tid} ({name}): invalid Rust variant '{rust_var}'")
            
            # Validate filename convention
            filename = tile.get("filename", "")
            if filename and not filename.startswith("tile_"):
                self.warning(f"Tile {tid} ({name}): filename doesn't start with 'tile_'")
            if filename and not filename.endswith(".png"):
                self.error(f"Tile {tid} ({name}): filename doesn't end with '.png'")
            
            # Validate colors
            for color in tile.get("colors", []):
                if color not in self.valid_colors:
                    self.error(f"Tile {tid} ({name}): unknown color '{color}'")
            
            # Validate properties
            props = tile.get("properties", {})
            for key in props:
                if key not in ["locked", "driveable", "buildable", "is_entry", "is_exit", "is_parking"]:
                    self.warning(f"Tile {tid} ({name}): unknown property '{key}'")
        
        print(f"  ✓ Validated {len(tiles)} tiles")
    
    def validate_vehicles(self):
        """Validate vehicle definitions."""
        if "vehicles" not in self.manifest:
            self.error("Missing 'vehicles' section in manifest")
            return
        
        vehicles_config = self.manifest["vehicles"]
        vehicles = vehicles_config.get("entries", [])
        
        print(f"Validating {len(vehicles)} vehicles...")
        
        # Check names are unique
        names = [v["name"] for v in vehicles]
        if len(names) != len(set(names)):
            self.error("Vehicle names are not unique")
        
        # Validate each vehicle
        for vehicle in vehicles:
            name = vehicle.get("name", "?")
            
            # Required fields
            if "rust_variant" not in vehicle:
                self.error(f"Vehicle {name}: missing 'rust_variant'")
            if "filename" not in vehicle:
                self.error(f"Vehicle {name}: missing 'filename'")
            if "dimensions" not in vehicle:
                self.error(f"Vehicle {name}: missing 'dimensions'")
            
            # Validate Rust identifier
            rust_var = vehicle.get("rust_variant", "")
            if rust_var and not re.match(r'^[A-Z][A-Za-z0-9]*$', rust_var):
                self.error(f"Vehicle {name}: invalid Rust variant '{rust_var}'")
            
            # Validate filename convention
            filename = vehicle.get("filename", "")
            if filename and not filename.startswith("vehicle_"):
                self.warning(f"Vehicle {name}: filename doesn't start with 'vehicle_'")
            if filename and not filename.endswith(".png"):
                self.error(f"Vehicle {name}: filename doesn't end with '.png'")
            
            # Validate dimensions are reasonable
            dims = vehicle.get("dimensions", {})
            width = dims.get("width", 0)
            height = dims.get("height", 0)
            
            if width < 10 or width > 200:
                self.warning(f"Vehicle {name}: unusual width {width}")
            if height < 10 or height > 300:
                self.warning(f"Vehicle {name}: unusual height {height}")
            
            # Validate footprint
            footprint = vehicle.get("footprint_tiles", 0)
            if footprint < 1 or footprint > 5:
                self.warning(f"Vehicle {name}: unusual footprint {footprint}")
        
        print(f"  ✓ Validated {len(vehicles)} vehicles")
    
    def validate_vfx(self):
        """Validate VFX definitions."""
        if "vfx" not in self.manifest:
            self.error("Missing 'vfx' section in manifest")
            return
        
        vfx_config = self.manifest["vfx"]
        vfx_list = vfx_config.get("entries", [])
        
        print(f"Validating {len(vfx_list)} VFX...")
        
        # Check names are unique
        names = [v["name"] for v in vfx_list]
        if len(names) != len(set(names)):
            self.error("VFX names are not unique")
        
        # Validate each VFX
        for vfx in vfx_list:
            name = vfx.get("name", "?")
            
            # Required fields
            if "rust_variant" not in vfx:
                self.error(f"VFX {name}: missing 'rust_variant'")
            if "filename" not in vfx:
                self.error(f"VFX {name}: missing 'filename'")
            if "used" not in vfx:
                self.warning(f"VFX {name}: missing 'used' flag")
            
            # Validate Rust identifier
            rust_var = vfx.get("rust_variant", "")
            if rust_var and not re.match(r'^[A-Z][A-Za-z0-9]*$', rust_var):
                self.error(f"VFX {name}: invalid Rust variant '{rust_var}'")
            
            # Validate filename convention
            filename = vfx.get("filename", "")
            if filename and not filename.startswith("vfx_"):
                self.warning(f"VFX {name}: filename doesn't start with 'vfx_'")
            if filename and not filename.endswith(".png"):
                self.error(f"VFX {name}: filename doesn't end with '.png'")
        
        print(f"  ✓ Validated {len(vfx_list)} VFX")


def main():
    """Main entry point."""
    script_dir = Path(__file__).parent
    manifest_path = script_dir / "asset_manifest.json"
    style_config_path = script_dir / "style_config.json"
    
    if not manifest_path.exists():
        print(f"Error: Manifest not found at {manifest_path}")
        sys.exit(1)
    
    try:
        validator = ManifestValidator(manifest_path, style_config_path)
        success = validator.validate_all()
        sys.exit(0 if success else 1)
    except Exception as e:
        print(f"Error during validation: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
