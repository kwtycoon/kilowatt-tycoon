#!/usr/bin/env python3
"""
Unified asset build pipeline - Single source of truth: asset_manifest.json

Usage:
    python tools/build_assets.py
    python tools/build_assets.py --skip-existing
    
This script:
    1. Validates manifest structure
    2. Checks that SVG source files exist
    3. Converts SVGs to PNGs (via Inkscape)
    4. Generates Tiled TSX tileset
    5. Generates Python tile mapping
    6. Generates HTML preview

All asset definitions come from asset_manifest.json.

Options:
    --skip-existing: Skip PNG conversion if PNG already exists (for CI)
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Set


class AssetBuilder:
    """Unified asset build pipeline driven by manifest."""
    
    def __init__(self, workspace_root: Path, skip_existing: bool = False):
        self.workspace_root = workspace_root
        self.manifest_path = workspace_root / "tools" / "asset_manifest.json"
        self.assets_dir = workspace_root / "assets"
        self.skip_existing = skip_existing
        self.manifest = None
        self.missing_svgs: List[str] = []
        self.stats = {
            "assets_checked": 0,
            "svgs_converted": 0,
            "errors": 0
        }
    
    def run(self):
        """Execute the complete build pipeline."""
        print("=" * 70)
        print("ASSET BUILD PIPELINE")
        print("=" * 70)
        print(f"Manifest: {self.manifest_path}")
        print(f"Assets: {self.assets_dir}")
        print()
        
        # Step 1: Load and validate manifest
        if not self.load_manifest():
            return False
        
        # Step 2: Check SVG sources exist
        self.check_svg_files()
        
        # Step 2b: Check for orphaned assets
        self.check_for_orphans()
        
        # Step 3: Convert SVGs to PNGs
        if not self.convert_svgs_to_pngs():
            return False
        
        # Step 4: Generate outputs from manifest
        if not self.generate_outputs():
            return False
        
        # Step 5: Report summary
        self.print_summary()
        
        return self.stats["errors"] == 0
    
    def load_manifest(self) -> bool:
        """Load and validate the asset manifest."""
        print("[1/5] Loading manifest...")
        
        if not self.manifest_path.exists():
            print(f"ERROR: Manifest not found at {self.manifest_path}")
            return False
        
        try:
            with open(self.manifest_path) as f:
                self.manifest = json.load(f)
            
            version = self.manifest.get("version", "unknown")
            print(f"  ✓ Loaded manifest version {version}")
            
            # Check required sections
            required_sections = ["tiles"]
            for section in required_sections:
                if section not in self.manifest:
                    print(f"  ✗ Missing required section: {section}")
                    return False
            
            return True
        
        except json.JSONDecodeError as e:
            print(f"ERROR: Invalid JSON in manifest: {e}")
            return False
    
    def check_svg_files(self):
        """Verify that asset files exist (PNG or SVG) for all assets in manifest."""
        print("\n[2/5] Checking asset files...")
        
        missing_assets = []
        section_counts = {}
        
        # Check all sections in manifest that have entries
        for section_name, section_data in self.manifest.items():
            if not isinstance(section_data, dict) or "entries" not in section_data:
                continue
            
            asset_dir = section_data.get("asset_dir", section_name)
            entries = section_data.get("entries", [])
            section_counts[section_name] = len(entries)
            
            for entry in entries:
                self.stats["assets_checked"] += 1
                filename = entry.get("filename", "")
                
                if not filename:
                    continue
                
                png_path = self.assets_dir / asset_dir / filename
                svg_filename = filename.replace(".png", ".svg")
                svg_path = self.assets_dir / asset_dir / svg_filename
                
                # Check if PNG exists (required for game)
                if not png_path.exists():
                    # PNG missing - check if SVG exists (can be converted)
                    if svg_path.exists():
                        self.missing_svgs.append(str(svg_path.relative_to(self.workspace_root)))
                    else:
                        # Both missing - this is an error
                        missing_assets.append(str(png_path.relative_to(self.workspace_root)))
                # else: PNG exists, we're good (SVG is optional)
        
        # Print summary by section
        print(f"  Checked {self.stats['assets_checked']} assets across {len(section_counts)} sections:")
        for section, count in section_counts.items():
            print(f"    - {section}: {count} assets")
        
        if missing_assets:
            print(f"  ✗ Error: {len(missing_assets)} assets missing (no PNG or SVG)")
            for missing in missing_assets[:5]:
                print(f"    - {missing}")
            if len(missing_assets) > 5:
                print(f"    ... and {len(missing_assets) - 5} more")
            self.stats["errors"] += len(missing_assets)
        else:
            print(f"  ✓ All assets found")
    
    def check_for_orphans(self):
        """Check for assets on disk that are not in the manifest."""
        print("\n[2b/5] Checking for orphaned assets...")
        
        # Build set of expected files from manifest
        manifest_files = set()
        
        for section_name, section_data in self.manifest.items():
            if not isinstance(section_data, dict) or "entries" not in section_data:
                continue
            
            asset_dir = section_data.get("asset_dir", section_name)
            for entry in section_data.get("entries", []):
                filename = entry.get("filename", "")
                if filename:
                    # Store both PNG and SVG as valid
                    manifest_files.add(f"{asset_dir}/{filename}")
                    svg_filename = filename.replace(".png", ".svg")
                    manifest_files.add(f"{asset_dir}/{svg_filename}")
        
        # Scan filesystem for actual asset files
        filesystem_files = set()
        asset_patterns = [
            ("world/tiles", "tile_*.png"),
            ("world/tiles", "tile_*.svg"),
            ("chargers", "**/charger_*.png"),
            ("chargers", "**/charger_*.svg"),
            ("vehicles", "vehicle_*.png"),
            ("vehicles", "vehicle_*.svg"),
            ("vfx", "vfx_*.png"),
            ("vfx", "vfx_*.svg"),
            ("props", "prop_*.png"),
            ("props", "prop_*.svg"),
            ("characters", "character_*.png"),
            ("characters", "character_*.svg"),
            ("ui/icons", "icon_*.png"),
            ("ui/icons", "icon_*.svg"),
        ]
        
        for subdir, pattern in asset_patterns:
            search_dir = self.assets_dir / subdir
            if search_dir.exists():
                for file_path in search_dir.glob(pattern):
                    rel_path = file_path.relative_to(self.assets_dir)
                    # Exclude hash files
                    if not str(rel_path).endswith('.hash'):
                        filesystem_files.add(str(rel_path))
        
        # Find orphans (on disk but not in manifest)
        orphans = filesystem_files - manifest_files
        
        if orphans:
            print(f"  ✗ Error: {len(orphans)} orphaned assets found")
            print("  (Assets exist on disk but not in manifest)")
            orphan_list = sorted(orphans)
            for orphan in orphan_list[:10]:
                print(f"    - {orphan}")
            if len(orphans) > 10:
                print(f"    ... and {len(orphans) - 10} more")
            self.stats["errors"] += 1
        else:
            print(f"  ✓ No orphaned assets (all files in manifest)")
    
    def convert_svgs_to_pngs(self) -> bool:
        """Run SVG to PNG conversion script."""
        print("\n[3/5] Converting SVGs to PNGs...")
        
        converter_script = self.workspace_root / "tools" / "convert_svgs_to_pngs.py"
        
        if not converter_script.exists():
            print(f"ERROR: Converter script not found: {converter_script}")
            return False
        
        try:
            cmd = [sys.executable, str(converter_script)]
            if self.skip_existing:
                cmd.append('--skip-existing')
            
            result = subprocess.run(
                cmd,
                cwd=self.workspace_root,
                capture_output=True,
                text=True,
                timeout=300
            )
            
            # Parse output to get conversion count
            for line in result.stdout.split('\n'):
                if "Converted:" in line:
                    # Extract number from "Converted: N, Skipped: M"
                    parts = line.split(',')
                    if parts:
                        converted = parts[0].split(':')[-1].strip()
                        try:
                            self.stats["svgs_converted"] = int(converted)
                        except ValueError:
                            pass
            
            if result.returncode != 0:
                print(f"  ✗ Conversion failed with exit code {result.returncode}")
                print(result.stderr)
                self.stats["errors"] += 1
                return False
            
            print(f"  ✓ Converted {self.stats['svgs_converted']} SVG(s) to PNG")
            return True
        
        except subprocess.TimeoutExpired:
            print("  ✗ Conversion timed out after 5 minutes")
            self.stats["errors"] += 1
            return False
        except Exception as e:
            print(f"  ✗ Conversion failed: {e}")
            self.stats["errors"] += 1
            return False
    
    def generate_outputs(self) -> bool:
        """Generate all output files from manifest."""
        print("\n[4/5] Generating outputs from manifest...")
        
        success = True
        
        # Generate TSX tileset
        if not self.generate_tsx():
            success = False
        
        # Generate Python tile mapping
        if not self.generate_python_mapping():
            success = False
        
        # Generate HTML preview
        if not self.generate_html_preview():
            success = False
        
        return success
    
    def generate_tsx(self) -> bool:
        """Generate Tiled TSX tileset."""
        codegen_script = self.workspace_root / "tools" / "generate_code_from_manifest.py"
        
        if not codegen_script.exists():
            print(f"  ✗ Code generator not found: {codegen_script}")
            self.stats["errors"] += 1
            return False
        
        try:
            result = subprocess.run(
                [sys.executable, str(codegen_script)],
                cwd=self.workspace_root,
                capture_output=True,
                text=True,
                timeout=30
            )
            
            if result.returncode != 0:
                print(f"  ✗ TSX generation failed")
                print(result.stderr)
                self.stats["errors"] += 1
                return False
            
            print("  ✓ Generated Tiled TSX tileset")
            print("  ✓ Generated Python tile mapping")
            return True
        
        except Exception as e:
            print(f"  ✗ TSX generation failed: {e}")
            self.stats["errors"] += 1
            return False
    
    def generate_python_mapping(self) -> bool:
        """Generate Python tile ID mapping (done by TSX generator)."""
        # This is handled by generate_code_from_manifest.py
        return True
    
    def generate_html_preview(self) -> bool:
        """Generate HTML asset preview."""
        print("  ✓ Generating HTML preview...")
        
        # Simple HTML preview that scans actual files
        html_content = self.build_html_preview()
        
        output_path = self.assets_dir / "asset_preview.html"
        try:
            output_path.write_text(html_content, encoding='utf-8')
            print(f"  ✓ Generated HTML preview: {output_path.relative_to(self.workspace_root)}")
            return True
        except Exception as e:
            print(f"  ✗ HTML preview generation failed: {e}")
            self.stats["errors"] += 1
            return False
    
    def build_html_preview(self) -> str:
        """Build HTML preview content by scanning filesystem."""
        import glob
        
        # Scan for tile PNGs
        tiles_dir = self.assets_dir / "world" / "tiles"
        tile_files = sorted(tiles_dir.glob("tile_*.png")) if tiles_dir.exists() else []
        
        # Scan for prop PNGs
        props_dir = self.assets_dir / "props"
        prop_files = sorted(props_dir.glob("prop_*.png")) if props_dir.exists() else []
        
        # Scan for charger PNGs (recursively in subdirectories)
        chargers_dir = self.assets_dir / "chargers"
        charger_files = sorted(chargers_dir.glob("**/charger_*.png")) if chargers_dir.exists() else []
        
        # Scan for vehicle PNGs
        vehicles_dir = self.assets_dir / "vehicles"
        vehicle_files = sorted(vehicles_dir.glob("vehicle_*.png")) if vehicles_dir.exists() else []
        
        # Scan for character PNGs
        characters_dir = self.assets_dir / "characters"
        character_files = sorted(characters_dir.glob("character_*.png")) if characters_dir.exists() else []
        
        # Scan for VFX PNGs
        vfx_dir = self.assets_dir / "vfx"
        vfx_files = sorted(vfx_dir.glob("vfx_*.png")) if vfx_dir.exists() else []
        
        # Scan for UI icons
        ui_icons_dir = self.assets_dir / "ui" / "icons"
        icon_files = sorted(ui_icons_dir.glob("icon_*.png")) if ui_icons_dir.exists() else []
        
        html = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Kilowatt Tycoon - Asset Preview</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background: #1a1a1a;
            color: #e0e0e0;
        }}
        .container {{ max-width: 1400px; margin: 0 auto; }}
        h1 {{ color: #4a9eff; margin-bottom: 10px; }}
        .subtitle {{ color: #888; margin-bottom: 30px; }}
        h2 {{ color: #66d9ff; margin-top: 40px; border-bottom: 2px solid #333; padding-bottom: 10px; }}
        .grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
            gap: 20px;
            margin: 20px 0;
        }}
        .asset-card {{
            background: #2a2a2a;
            border: 1px solid #3a3a3a;
            border-radius: 8px;
            padding: 15px;
            text-align: center;
            transition: all 0.2s;
        }}
        .asset-card:hover {{
            background: #333;
            border-color: #4a9eff;
            transform: translateY(-2px);
        }}
        .asset-card img {{
            max-width: 100%;
            height: auto;
            image-rendering: pixelated;
            background: #1a1a1a;
            padding: 10px;
            border-radius: 4px;
        }}
        .asset-name {{
            margin-top: 10px;
            font-size: 12px;
            color: #aaa;
            word-break: break-word;
        }}
        .count {{ color: #4a9eff; font-weight: bold; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Kilowatt Tycoon - Asset Preview</h1>
        <p class="subtitle">Auto-generated from asset_manifest.json</p>
        
        <h2>World Tiles (<span class="count">{len(tile_files)}</span>)</h2>
        <div class="grid">
"""
        
        for tile_file in tile_files:
            name = tile_file.stem.replace('tile_', '').replace('_', ' ').title()
            rel_path = tile_file.relative_to(self.assets_dir)
            html += f"""            <div class="asset-card">
                <img src="{rel_path}" alt="{name}">
                <div class="asset-name">{name}</div>
            </div>
"""
        
        html += f"""        </div>
        
        <h2>Props (<span class="count">{len(prop_files)}</span>)</h2>
        <div class="grid">
"""
        
        for prop_file in prop_files:
            name = prop_file.stem.replace('prop_', '').replace('_', ' ').title()
            rel_path = prop_file.relative_to(self.assets_dir)
            html += f"""            <div class="asset-card">
                <img src="{rel_path}" alt="{name}">
                <div class="asset-name">{name}</div>
            </div>
"""
        
        html += f"""        </div>
        
        <h2>Chargers (<span class="count">{len(charger_files)}</span>)</h2>
        <div class="grid">
"""
        
        for charger_file in charger_files:
            name = charger_file.stem.replace('charger_', '').replace('_', ' ').title()
            rel_path = charger_file.relative_to(self.assets_dir)
            html += f"""            <div class="asset-card">
                <img src="{rel_path}" alt="{name}">
                <div class="asset-name">{name}</div>
            </div>
"""
        
        html += f"""        </div>
        
        <h2>Vehicles (<span class="count">{len(vehicle_files)}</span>)</h2>
        <div class="grid">
"""
        
        for vehicle_file in vehicle_files:
            name = vehicle_file.stem.replace('vehicle_', '').replace('_', ' ').title()
            rel_path = vehicle_file.relative_to(self.assets_dir)
            html += f"""            <div class="asset-card">
                <img src="{rel_path}" alt="{name}">
                <div class="asset-name">{name}</div>
            </div>
"""
        
        html += f"""        </div>
        
        <h2>Characters (<span class="count">{len(character_files)}</span>)</h2>
        <div class="grid">
"""
        
        for character_file in character_files:
            name = character_file.stem.replace('character_', '').replace('_', ' ').title()
            rel_path = character_file.relative_to(self.assets_dir)
            html += f"""            <div class="asset-card">
                <img src="{rel_path}" alt="{name}">
                <div class="asset-name">{name}</div>
            </div>
"""
        
        html += f"""        </div>
        
        <h2>VFX (<span class="count">{len(vfx_files)}</span>)</h2>
        <div class="grid">
"""
        
        for vfx_file in vfx_files:
            name = vfx_file.stem.replace('vfx_', '').replace('_', ' ').title()
            rel_path = vfx_file.relative_to(self.assets_dir)
            html += f"""            <div class="asset-card">
                <img src="{rel_path}" alt="{name}">
                <div class="asset-name">{name}</div>
            </div>
"""
        
        html += f"""        </div>
        
        <h2>UI Icons (<span class="count">{len(icon_files)}</span>)</h2>
        <div class="grid">
"""
        
        for icon_file in icon_files:
            name = icon_file.stem.replace('icon_', '').replace('_', ' ').title()
            rel_path = icon_file.relative_to(self.assets_dir)
            html += f"""            <div class="asset-card">
                <img src="{rel_path}" alt="{name}">
                <div class="asset-name">{name}</div>
            </div>
"""
        
        html += """        </div>
    </div>
</body>
</html>
"""
        return html
    
    def print_summary(self):
        """Print build summary."""
        print("\n[5/5] Build Summary")
        print("=" * 70)
        print(f"Assets checked:    {self.stats['assets_checked']}")
        print(f"SVGs converted:    {self.stats['svgs_converted']}")
        print(f"Errors:            {self.stats['errors']}")
        print("=" * 70)
        
        if self.stats["errors"] == 0:
            print("✓ Build completed successfully!")
        else:
            print(f"✗ Build completed with {self.stats['errors']} error(s)")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description='Build all assets from manifest'
    )
    parser.add_argument(
        '--skip-existing',
        action='store_true',
        help='Skip PNG conversion if PNG already exists (for CI)'
    )
    args = parser.parse_args()
    
    workspace_root = Path(__file__).parent.parent
    
    builder = AssetBuilder(workspace_root, skip_existing=args.skip_existing)
    success = builder.run()
    
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
