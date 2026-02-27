#!/usr/bin/env python3
"""
Convert all SVG assets to PNG using Inkscape CLI.

Usage:
    python3 tools/convert_svgs_to_pngs.py
    python3 tools/convert_svgs_to_pngs.py --skip-existing

This script:
- Recursively finds all *.svg files under assets/
- Parses each SVG's intrinsic width/height (or viewBox)
- Exports to PNG at 4× the intrinsic size for crisp rendering
- Skips regeneration if PNG exists and SVG hash matches stored hash
- Exits non-zero with helpful message if Inkscape is not installed
- With --skip-existing: skips conversion if PNG exists (for CI)
"""

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path


ASSETS_DIR = Path(__file__).parent.parent / "assets"
SCALE_FACTOR = 1  # Export at native SVG size (64x64)


def check_inkscape() -> str:
    """Check if Inkscape is available and return its path."""
    inkscape_path = shutil.which("inkscape")
    if inkscape_path is None:
        print("ERROR: Inkscape is not installed or not found in PATH.", file=sys.stderr)
        print("", file=sys.stderr)
        print("Please install Inkscape:", file=sys.stderr)
        print("  macOS:   brew install --cask inkscape", file=sys.stderr)
        print("  Ubuntu:  sudo apt install inkscape", file=sys.stderr)
        print("  Windows: https://inkscape.org/release/", file=sys.stderr)
        print("", file=sys.stderr)
        print("After installation, ensure 'inkscape' is in your PATH.", file=sys.stderr)
        sys.exit(1)
    return inkscape_path


def parse_svg_dimensions(svg_path: Path) -> tuple[float, float]:
    """
    Parse width and height from an SVG file.
    
    Tries to read width/height attributes first, then falls back to viewBox.
    Returns (width, height) in pixels (unitless values assumed to be px).
    """
    content = svg_path.read_text(encoding="utf-8")
    
    # Try to find width and height attributes on the root <svg> element
    # Match the opening <svg ...> tag
    svg_match = re.search(r'<svg[^>]*>', content, re.IGNORECASE | re.DOTALL)
    if not svg_match:
        raise ValueError(f"Could not find <svg> element in {svg_path}")
    
    svg_tag = svg_match.group(0)
    
    # Extract width and height attributes
    width_match = re.search(r'width\s*=\s*["\']([^"\']+)["\']', svg_tag)
    height_match = re.search(r'height\s*=\s*["\']([^"\']+)["\']', svg_tag)
    
    width = None
    height = None
    
    if width_match:
        width = parse_dimension(width_match.group(1))
    if height_match:
        height = parse_dimension(height_match.group(1))
    
    # If we got both, return them
    if width is not None and height is not None:
        return (width, height)
    
    # Fall back to viewBox
    viewbox_match = re.search(r'viewBox\s*=\s*["\']([^"\']+)["\']', svg_tag)
    if viewbox_match:
        parts = viewbox_match.group(1).split()
        if len(parts) >= 4:
            vb_width = float(parts[2])
            vb_height = float(parts[3])
            return (vb_width, vb_height)
    
    raise ValueError(f"Could not determine dimensions for {svg_path}")


def parse_dimension(value: str) -> float:
    """Parse a dimension value, stripping units if present."""
    # Remove common units and parse as float
    value = value.strip()
    # Remove px, pt, mm, cm, in, em, ex, % suffixes
    match = re.match(r'^([\d.]+)', value)
    if match:
        return float(match.group(1))
    raise ValueError(f"Could not parse dimension: {value}")


def compute_file_hash(file_path: Path) -> str:
    """
    Compute SHA-256 hash of a file's content.
    
    Returns hex digest string.
    """
    sha256 = hashlib.sha256()
    with open(file_path, 'rb') as f:
        # Read in chunks for memory efficiency
        for chunk in iter(lambda: f.read(8192), b''):
            sha256.update(chunk)
    return sha256.hexdigest()


def read_hash_file(hash_path: Path) -> str | None:
    """
    Read hash value from a .png.hash file.
    
    Returns the hash string or None if file doesn't exist or is corrupt.
    """
    if not hash_path.exists():
        return None
    try:
        content = hash_path.read_text(encoding='utf-8').strip()
        # Basic validation - SHA-256 hashes are 64 hex characters
        if len(content) == 64 and all(c in '0123456789abcdef' for c in content):
            return content
    except Exception:
        pass
    return None


def write_hash_file(hash_path: Path, hash_value: str) -> None:
    """Write hash value to a .png.hash file."""
    hash_path.write_text(hash_value + '\n', encoding='utf-8')


def needs_regeneration(svg_path: Path, png_path: Path) -> tuple[bool, str]:
    """
    Check if PNG needs to be regenerated based on SVG content hash.
    
    Returns (needs_regeneration, current_svg_hash).
    """
    # Compute current SVG hash
    current_hash = compute_file_hash(svg_path)
    
    # Check if PNG exists
    if not png_path.exists():
        return (True, current_hash)
    
    # Check if hash file exists and matches
    hash_path = png_path.with_suffix('.png.hash')
    stored_hash = read_hash_file(hash_path)
    
    if stored_hash is None:
        # No hash file or corrupt - need to regenerate
        return (True, current_hash)
    
    # Compare hashes
    if current_hash != stored_hash:
        return (True, current_hash)
    
    # Hashes match - skip regeneration
    return (False, current_hash)


def convert_svg_to_png(
    inkscape_path: str,
    svg_path: Path,
    png_path: Path,
    width: int,
    height: int,
) -> bool:
    """
    Convert an SVG to PNG using Inkscape.
    
    Returns True on success, False on failure.
    """
    # Ensure output directory exists
    png_path.parent.mkdir(parents=True, exist_ok=True)
    
    cmd = [
        inkscape_path,
        str(svg_path),
        "--export-type=png",
        f"--export-filename={png_path}",
        f"--export-width={width}",
        f"--export-height={height}",
    ]
    
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=60,
        )
        if result.returncode != 0:
            print(f"  ERROR: Inkscape failed for {svg_path}", file=sys.stderr)
            print(f"  stderr: {result.stderr}", file=sys.stderr)
            return False
        return True
    except subprocess.TimeoutExpired:
        print(f"  ERROR: Inkscape timed out for {svg_path}", file=sys.stderr)
        return False
    except Exception as e:
        print(f"  ERROR: {e}", file=sys.stderr)
        return False


def main():
    parser = argparse.ArgumentParser(
        description='Convert SVG assets to PNG using Inkscape'
    )
    parser.add_argument(
        '--skip-existing',
        action='store_true',
        help='Skip conversion if PNG exists (for CI to avoid Inkscape version differences)'
    )
    args = parser.parse_args()
    
    print("=" * 60)
    print("SVG to PNG Converter (Inkscape)")
    print("=" * 60)
    print()
    
    # Check for Inkscape
    inkscape_path = check_inkscape()
    print(f"Using Inkscape: {inkscape_path}")
    print(f"Assets directory: {ASSETS_DIR}")
    print(f"Scale factor: {SCALE_FACTOR}×")
    if args.skip_existing:
        print(f"Mode: Skip existing PNGs (CI mode)")
    print()
    
    # Find all SVG files
    svg_files = sorted(ASSETS_DIR.rglob("*.svg"))
    print(f"Found {len(svg_files)} SVG files")
    print()
    
    converted = 0
    skipped = 0
    errors = 0
    
    for svg_path in svg_files:
        # Compute output PNG path (same location, different extension)
        png_path = svg_path.with_suffix(".png")
        hash_path = png_path.with_suffix('.png.hash')
        
        # Skip existing PNGs if flag is set (for CI)
        if args.skip_existing and png_path.exists():
            skipped += 1
            continue
        
        # Check if regeneration is needed
        needs_regen, current_hash = needs_regeneration(svg_path, png_path)
        if not needs_regen:
            skipped += 1
            continue
        
        # Parse dimensions
        try:
            width, height = parse_svg_dimensions(svg_path)
        except ValueError as e:
            print(f"  SKIP (no dimensions): {svg_path.relative_to(ASSETS_DIR)}")
            print(f"        {e}")
            errors += 1
            continue
        
        # Validate tile dimensions (tiles must be 64x64)
        if "world/tiles" in str(svg_path):
            if width != 64 or height != 64:
                rel_path = svg_path.relative_to(ASSETS_DIR)
                print(f"  ERROR: Tile must be 64x64: {rel_path} is {int(width)}×{int(height)}")
                print(f"         Run: python tools/fix_svg_tile_sizes.py")
                errors += 1
                continue
        
        # Compute output size at 4× scale
        out_width = int(width * SCALE_FACTOR)
        out_height = int(height * SCALE_FACTOR)
        
        rel_path = svg_path.relative_to(ASSETS_DIR)
        print(f"  Converting: {rel_path} ({int(width)}×{int(height)} → {out_width}×{out_height})")
        
        if convert_svg_to_png(inkscape_path, svg_path, png_path, out_width, out_height):
            # Save hash file after successful conversion
            write_hash_file(hash_path, current_hash)
            converted += 1
        else:
            errors += 1
    
    print()
    print("=" * 60)
    print(f"Done! Converted: {converted}, Skipped (up-to-date): {skipped}, Errors: {errors}")
    print("=" * 60)
    
    if errors > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()

