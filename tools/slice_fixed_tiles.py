#!/usr/bin/env python3
"""Slice large images into 64x64 fixed tiles and register them in the asset manifest.

Resizes source images to a target tile grid (keeping aspect ratio), slices into
64x64 tiles, saves to assets/fixed/, adds entries to asset_manifest.json, and
regenerates the tileset via generate_code_from_manifest.py.

Usage:
    python tools/slice_fixed_tiles.py
"""

import json
import subprocess
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Error: Pillow is required. Install with: pip install Pillow")
    sys.exit(1)

TILE_SIZE = 64

WORKSPACE_ROOT = Path(__file__).parent.parent
FIXED_DIR = WORKSPACE_ROOT / "assets" / "fixed"
MANIFEST_PATH = WORKSPACE_ROOT / "tools" / "asset_manifest.json"

IMAGES_TO_SLICE = [
    {
        "source": "quickcharge_express.png",
        "prefix": "qce",
        "cols": 4,
        "rows": 3,
    },
    {
        "source": "quickcharge_express_gas.png",
        "prefix": "qceg",
        "cols": 8,
        "rows": 4,
    },
]


def slice_image(source_path: Path, prefix: str, cols: int, rows: int) -> list[str]:
    """Resize an image to cols*64 x rows*64 and slice into 64x64 tiles.

    Returns a list of output filenames in row-major order.
    """
    target_w = cols * TILE_SIZE
    target_h = rows * TILE_SIZE

    img = Image.open(source_path).convert("RGBA")
    orig_w, orig_h = img.size
    print(f"  Source: {orig_w}x{orig_h} -> Target: {target_w}x{target_h} ({cols}x{rows} tiles)")

    scale = max(target_w / orig_w, target_h / orig_h)
    scaled_w = round(orig_w * scale)
    scaled_h = round(orig_h * scale)
    resized = img.resize((scaled_w, scaled_h), Image.LANCZOS)

    crop_x = (scaled_w - target_w) // 2
    crop_y = (scaled_h - target_h) // 2
    canvas = resized.crop((crop_x, crop_y, crop_x + target_w, crop_y + target_h))

    filenames = []
    for row in range(rows):
        for col in range(cols):
            x = col * TILE_SIZE
            y = row * TILE_SIZE
            tile = canvas.crop((x, y, x + TILE_SIZE, y + TILE_SIZE))
            filename = f"{prefix}_r{row}c{col}.png"
            tile.save(FIXED_DIR / filename)
            filenames.append(filename)

    print(f"  Saved {len(filenames)} tiles with prefix '{prefix}_'")
    return filenames


def next_fixed_id(manifest: dict) -> int:
    """Find the next available fixed tile ID."""
    used_ids = set()
    for entry in manifest["tiles"]["entries"]:
        used_ids.add(entry["id"])
    for entry in manifest["tiles"]["fixed_tiles"]:
        used_ids.add(entry["id"])
    candidate = max(used_ids) + 1
    return candidate


def update_manifest(all_new_tiles: list[dict]):
    """Add new fixed tile entries to the asset manifest, replacing any previous run."""
    with open(MANIFEST_PATH) as f:
        manifest = json.load(f)

    prefixes = {t["prefix"] for t in all_new_tiles}
    old_fixed = manifest["tiles"]["fixed_tiles"]
    manifest["tiles"]["fixed_tiles"] = [
        ft for ft in old_fixed
        if not any(ft["filename"].startswith(p + "_") for p in prefixes)
    ]

    start_id = next_fixed_id(manifest)
    print(f"\nAssigning IDs starting from {start_id}")

    for i, tile_info in enumerate(all_new_tiles):
        tile_id = start_id + i
        tile_info["id"] = tile_id
        manifest["tiles"]["fixed_tiles"].append({
            "id": tile_id,
            "filename": tile_info["filename"],
        })

    with open(MANIFEST_PATH, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")

    print(f"Updated {MANIFEST_PATH.name} with {len(all_new_tiles)} new fixed tiles")
    return all_new_tiles


def print_tile_grid(prefix: str, cols: int, rows: int, tiles: list[dict]):
    """Print a visual grid of tile IDs for use in tilemaps."""
    print(f"\n  Tile ID grid for '{prefix}' (for TMX, add firstgid offset):")
    idx = 0
    for row in range(rows):
        row_ids = []
        for col in range(cols):
            row_ids.append(str(tiles[idx]["id"]))
            idx += 1
        print(f"    {','.join(row_ids)}")


def regenerate_tileset():
    """Run generate_code_from_manifest.py to rebuild the tileset."""
    script = WORKSPACE_ROOT / "tools" / "generate_code_from_manifest.py"
    print(f"\nRegenerating tileset...")
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=str(WORKSPACE_ROOT),
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Error regenerating tileset:\n{result.stderr}")
        sys.exit(1)
    print(result.stdout)


def main():
    print("=" * 60)
    print("SLICE FIXED TILES")
    print("=" * 60)

    all_new_tiles = []

    for spec in IMAGES_TO_SLICE:
        source_path = FIXED_DIR / spec["source"]
        if not source_path.exists():
            print(f"Error: {source_path} not found")
            sys.exit(1)

        print(f"\nProcessing: {spec['source']}")
        filenames = slice_image(
            source_path,
            prefix=spec["prefix"],
            cols=spec["cols"],
            rows=spec["rows"],
        )
        for filename in filenames:
            all_new_tiles.append({"filename": filename, "prefix": spec["prefix"]})

    tiles_with_ids = update_manifest(all_new_tiles)

    # Print grids grouped by image
    offset = 0
    for spec in IMAGES_TO_SLICE:
        count = spec["cols"] * spec["rows"]
        group = tiles_with_ids[offset : offset + count]
        print_tile_grid(spec["prefix"], spec["cols"], spec["rows"], group)
        offset += count

    regenerate_tileset()

    print("Done! Use the tile ID grids above to place tiles in your TMX maps.")
    print("Remember: TMX tile value = tileset_id + firstgid (usually +1)")


if __name__ == "__main__":
    main()
