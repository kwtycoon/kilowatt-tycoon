#!/usr/bin/env python3
"""Generate tile assets by extracting them from AI-generated category scenes.

Instead of generating individual 64x64 tiles (which AI models struggle with),
this script generates larger cohesive scenes for each tile category, then
extracts tiles from grid positions within those scenes.

Usage:
    python generate_scene_tiles_ai.py --all                # Generate all categories
    python generate_scene_tiles_ai.py --category base_parking  # One category
    python generate_scene_tiles_ai.py --extract-only       # Re-crop without API calls
    python generate_scene_tiles_ai.py --all --dry-run      # Preview prompts only
"""

import argparse
import base64
import json
import os
import sys
import threading
import time
from pathlib import Path
from typing import Optional, Dict, List, Tuple

GEMINI_AI_AVAILABLE = False
PILLOW_AVAILABLE = False

try:
    import google.generativeai as genai
    GEMINI_AI_AVAILABLE = True
except ImportError:
    pass

try:
    from PIL import Image
    import io
    PILLOW_AVAILABLE = True
except ImportError:
    pass


GTA2_STYLE_DIRECTIVES = """ART STYLE (CRITICAL - MUST FOLLOW):
- GTA 2 aesthetic: top-down pixel art with a gritty, urban feel
- PIXELATED look - visible pixels, chunky details, retro game art
- TRUE TOP-DOWN camera angle (90 degrees straight down), just like GTA 2
- Rich, saturated colors with strong contrast between elements
- Dark asphalt roads with painted lane markings
- Buildings shown as solid rooftop shapes (you're looking down at them)
- Small environmental details: cracks, stains, worn paint, tire marks
- Dithered shading instead of smooth gradients

COLOR PALETTE:
- Asphalt/roads: dark grays and charcoals (#3A3F4B) with subtle variation and wear
- Parking line markings: bright white (#FFFFFF) or yellow (#F5C542) paint
- Sidewalks/concrete: lighter warm gray (#E0DCD6) with pixel-level cracks
- Grass/landscaping: rich greens (#5D7F4A), varying shades to suggest texture
- Buildings: muted brick reds (#8B5A3C), concrete grays, industrial browns - flat rooftops
- Accent colors: neon greens and electric blues for EV charging equipment
- Streetlights cast warm yellow circles on the ground

WHAT TO AVOID:
- NO vector art, NO clean/flat UI style
- NO isometric or 3D perspective
- NO smooth gradients - use dithering or flat pixel shading
- NO photorealistic rendering
- NO high-resolution smooth edges - visible pixel stepping"""


SCENE_CATEGORIES: Dict[str, dict] = {
    "base_parking": {
        "grid": (3, 3),
        "description": "an open parking lot with grass borders, asphalt surface, parking bays, and worn pavement areas",
        "tiles": {
            "Grass":           (0, 0, "lush green grass lawn, untrimmed edges"),
            "Empty":           (1, 0, "wild untended grass with weeds, empty lot"),
            "Lot":             (2, 0, "clean asphalt parking lot surface, dark gray"),
            "ParkingBayNorth": (0, 1, "parking space with white lines, bay opens north (top)"),
            "ParkingBaySouth": (1, 1, "parking space with white lines, bay opens south (bottom)"),
            "ReservedSpot":    (2, 1, "reserved parking spot on concrete with markings"),
            "AsphaltWorn":     (0, 2, "weathered cracked asphalt with potholes and patches"),
            "AsphaltSkid":     (1, 2, "asphalt with dark tire skid marks and rubber streaks"),
            "Concrete":        (2, 2, "concrete sidewalk with expansion joints"),
        },
    },
    "road": {
        "grid": (3, 3),
        "description": "a city street intersection with lanes, crosswalks, bike lanes, and road markings",
        "tiles": {
            "Road":            (0, 0, "dark asphalt road surface, urban street"),
            "StreetRoad":      (1, 0, "city street pavement, worn urban road"),
            "RoadYellowLine":  (2, 0, "asphalt with yellow center line stripe"),
            "Crosswalk":       (0, 1, "crosswalk with thick white zebra stripes on asphalt"),
            "BikeLane":        (1, 1, "green painted bike lane on asphalt"),
            "FireLane":        (2, 1, "fire lane with red curb markings on asphalt"),
            "Entry":           (0, 2, "driveway entry apron, asphalt transitioning to lot"),
            "BrickSidewalk":   (1, 2, "red brick paver sidewalk, herringbone pattern"),
            "Cobblestone":     (2, 2, "cobblestone pavement, rounded stone pattern"),
        },
    },
    "gas_station": {
        "grid": (3, 3),
        "description": "a gas station with red canopy, pump islands, convenience store, and surrounding asphalt",
        "tiles": {
            "Canopy":          (0, 0, "asphalt under gas station canopy, slightly brighter lighting"),
            "CanopyShadow":    (1, 0, "asphalt with dithered shadow from canopy edge"),
            "CanopyColumn":    (2, 0, "canopy support column on asphalt, centered post"),
            "PumpIsland":      (0, 1, "concrete pump island with fuel dispenser fixtures"),
            "FuelCap":         (1, 1, "concrete with circular metal fuel tank access cap"),
            "GasStationSign":  (2, 1, "grass with gas station sign pole base"),
            "StoreWall":       (0, 2, "blue QuickMart store rooftop from above"),
            "StoreEntrance":   (1, 2, "store entrance awning from above, blue frame"),
            "Storefront":      (2, 2, "storefront window awning, glass with blue frame"),
        },
    },
    "mall_garage": {
        "grid": (3, 2),
        "description": "an underground parking garage with concrete pillars, floor markings, ramps, and level signage",
        "tiles": {
            "GarageFloor":     (0, 0, "dark polished concrete garage floor with tire marks"),
            "GaragePillar":    (1, 0, "concrete pillar centered on dark garage floor"),
            "MallFacade":      (2, 0, "mall building flat rooftop from above with AC vents"),
            "GarageCeiling":   (0, 1, "garage ceiling with exposed pipes and light strips"),
            "GarageLevel1":    (1, 1, "garage floor with large L1 level marking in white"),
            "GarageRamp":      (2, 1, "garage ramp with directional arrows and yellow stripes"),
        },
    },
    "streetside_decorative": {
        "grid": (6, 6),
        "description": "an urban streetside scene with sidewalks, trees, benches, hydrants, trash cans, lamp posts, bollards, planters, and various street furniture viewed from directly above",
        "tiles": {
            "CurbAsphaltGrass":    (0, 0, "curb edge: asphalt on left, grass on right, concrete curb between"),
            "CurbAsphaltConcrete": (1, 0, "curb edge: asphalt on left, concrete on right, curb between"),
            "Planter":             (2, 0, "raised planter box with green shrubs in concrete container"),
            "PlanterUrn":          (3, 0, "ornamental planter urn with green foliage, round pot"),
            "Bollard":             (4, 0, "metal bollard post on concrete, small round post"),
            "WheelStop":           (5, 0, "concrete wheel stop with yellow stripe on asphalt"),
            "StreetTree":          (0, 1, "tree grate on concrete, metal grate around trunk opening"),
            "StreetTreeTile":      (1, 1, "tree canopy from above, round green foliage with trunk center"),
            "LightPole":           (2, 1, "lamp post from above on concrete, warm yellow light pool"),
            "StreetLamp":          (3, 1, "vintage street lamp, ornate pole with light pool"),
            "FireHydrant":         (4, 1, "red fire hydrant from above on concrete"),
            "TrashCan":            (5, 1, "trash can from above on sidewalk, round bin with lid"),
            "Bench":               (0, 2, "wooden bench from above on concrete, slatted seat"),
            "NewspaperBox":        (1, 2, "newspaper box from above on sidewalk"),
            "ParkingMeter":        (2, 2, "parking meter from above on sidewalk"),
            "CartReturn":          (3, 2, "cart corral from above on asphalt, metal enclosure"),
            "AirVacuum":           (4, 2, "air vacuum machine from above on concrete"),
            "OutdoorHeater":       (5, 2, "patio heater from above, circular heat element"),
            "SpeedBump":           (0, 3, "yellow speed bump across asphalt, striped hump"),
            "RopeBarrier":         (1, 3, "rope stanchion from above, velvet rope between posts"),
            "ReservedSign":        (2, 3, "reserved sign post from above on asphalt"),
            "ExitSign":            (3, 3, "illuminated exit sign from above, green glow"),
            "MallDirectory":       (4, 3, "directory kiosk from above, rectangular info board"),
            "UtilityCabinet":      (5, 3, "utility cabinet box from above, gray metal box"),
            "WheelStopTile":       (0, 4, "concrete wheel stop bumper on asphalt"),
            "GasPumpDisabled":     (1, 4, "disabled gas pump from above, bagged nozzle"),
            "QuickmartFacade":     (2, 4, "QuickMart storefront awning from above, blue branded"),
            "ValetStand":          (3, 4, "valet podium from above on concrete"),
            "BusStop":             (4, 4, "bus stop shelter from above, glass canopy with bench"),
            "Gutter":              (5, 4, "street gutter drain, narrow channel along curb"),
            "Manhole":             (0, 5, "manhole cover from above on asphalt, round metal"),
            "StreetCorner":        (1, 5, "street corner curb radius, concrete and asphalt"),
            "MeterZone":           (2, 5, "metered parking zone, asphalt with green marking"),
            "OfficeBackdrop":      (3, 5, "office building rooftop from above, glass and steel"),
            "DumpsterPad":         (4, 5, "empty dumpster pad on concrete with stains"),
            "DumpsterOccupied":    (5, 5, "dumpster from above, green metal container with lid"),
        },
    },
    "hotel_destination": {
        "grid": (3, 3),
        "description": "a luxury hotel entrance area with covered driveway, valet lane, fountain, gardens, stone paths, and pool deck",
        "tiles": {
            "PorteCochere":    (0, 0, "hotel porte-cochere canopy from above, beige patterned ground"),
            "ValetLane":       (1, 0, "valet lane, purple paint on asphalt"),
            "HotelEntrance":   (2, 0, "hotel entrance canopy from above, beige awning"),
            "FountainBase":    (0, 1, "fountain from above, circular stone basin with blue water"),
            "GardenBed":       (1, 1, "garden bed from above, dark soil with green shrubs"),
            "PathwayStone":    (2, 1, "stone pathway, decorative flagstone pavers"),
            "PoolDeck":        (0, 2, "pool deck, light blue-gray wet surface tiles"),
            "LoadingZone":     (1, 2, "loading zone, yellow paint on concrete"),
            "ElevatorLobby":   (2, 2, "elevator lobby floor, polished tile with door frames"),
        },
    },
    "infrastructure": {
        "grid": (3, 3),
        "description": "a utility and infrastructure area with charger pads, transformer, solar panels, battery storage, and cable trenches for an EV charging station",
        "tiles": {
            "ChargerPad":          (0, 0, "EV charger mounting pad on asphalt with green EV marking"),
            "TransformerPad":      (1, 0, "reinforced concrete transformer pad with bolt pattern"),
            "SolarPad":            (2, 0, "cleared grass patch with solar mounting brackets"),
            "BatteryPad":          (0, 1, "reinforced concrete battery pad with cable conduits"),
            "TransformerOccupied": (1, 1, "green utility transformer box on concrete, warning markings"),
            "SolarOccupied":       (2, 1, "dark blue solar panels in grid on grass"),
            "BatteryOccupied":     (0, 2, "white battery container on concrete with blue EV accent"),
            "UtilityTrench":       (1, 2, "utility trench with metal grating over cable conduit"),
            "ExecutiveSpot":       (2, 2, "premium parking spot with gold EXEC stencil marking"),
        },
    },
    "amenities": {
        "grid": (2, 2),
        "description": "a cluster of small amenity buildings at a charging station: WiFi/restroom hut, lounge/snack cafe, restaurant, seen from directly above",
        "tiles": {
            "AmenityWifiRestrooms": (0, 0, "small amenity building rooftop with WiFi antenna and vents"),
            "AmenityLoungeSnacks":  (1, 0, "lounge building rooftop with cafe awning"),
            "AmenityRestaurant":    (0, 1, "restaurant rooftop with kitchen vents and patio"),
            "AmenityOccupied":      (1, 1, "generic amenity building rooftop, service structure"),
        },
    },
}

TILE_FILENAMES = {
    "Grass": "tile_grass.png",
    "Road": "tile_street_road.png",
    "Entry": "tile_driveway_apron.png",
    "Lot": "tile_asphalt_clean.png",
    "ParkingBayNorth": "tile_parking_bay_north.png",
    "ParkingBaySouth": "tile_parking_bay.png",
    "Concrete": "tile_concrete.png",
    "GarageFloor": "tile_garage_floor.png",
    "GaragePillar": "tile_garage_pillar.png",
    "MallFacade": "tile_mall_facade.png",
    "StoreWall": "tile_store_wall.png",
    "StoreEntrance": "tile_store_entrance.png",
    "Storefront": "tile_storefront.png",
    "PumpIsland": "tile_pump_island.png",
    "Canopy": "tile_canopy_floor.png",
    "FuelCap": "tile_fuel_cap_covered.png",
    "CanopyShadow": "tile_canopy_shadow.png",
    "BrickSidewalk": "tile_brick_sidewalk.png",
    "BikeLane": "tile_bike_lane.png",
    "StreetRoad": "tile_street_road.png",
    "Crosswalk": "tile_crosswalk.png",
    "ReservedSpot": "tile_reserved_spot.png",
    "OfficeBackdrop": "tile_office_backdrop.png",
    "PorteCochere": "tile_porte_cochere.png",
    "ValetLane": "tile_valet_lane.png",
    "HotelEntrance": "tile_hotel_entrance.png",
    "FountainBase": "tile_fountain_base.png",
    "GardenBed": "tile_garden_bed.png",
    "Cobblestone": "tile_cobblestone.png",
    "LoadingZone": "tile_loading_zone.png",
    "AsphaltWorn": "tile_asphalt_worn.png",
    "AsphaltSkid": "tile_asphalt_skid.png",
    "Planter": "tile_planter.png",
    "CurbAsphaltGrass": "tile_curb_asphalt_grass.png",
    "CurbAsphaltConcrete": "tile_curb_asphalt_concrete.png",
    "ChargerPad": "tile_charger_pad.png",
    "TransformerPad": "tile_transformer_pad.png",
    "SolarPad": "tile_solar_pad.png",
    "BatteryPad": "tile_battery_pad.png",
    "Empty": "tile_empty.png",
    "Bollard": "tile_bollard.png",
    "WheelStop": "tile_asphalt_lines.png",
    "StreetTree": "tile_tree_grate.png",
    "LightPole": "tile_light_pole.png",
    "CanopyColumn": "tile_canopy_column.png",
    "GasStationSign": "tile_gas_station_sign.png",
    "DumpsterPad": "tile_dumpster_pad.png",
    "DumpsterOccupied": "tile_dumpster_occupied.png",
    "TransformerOccupied": "tile_transformer_occupied.png",
    "SolarOccupied": "tile_solar_occupied.png",
    "BatteryOccupied": "tile_battery_occupied.png",
    "AmenityWifiRestrooms": "tile_amenity_wifi_restrooms.png",
    "AmenityLoungeSnacks": "tile_amenity_lounge_snacks.png",
    "AmenityRestaurant": "tile_amenity_restaurant.png",
    "AmenityOccupied": "tile_amenity_occupied.png",
    "RoadYellowLine": "tile_road_yellow_line.png",
    "AirVacuum": "tile_air_vacuum.png",
    "Bench": "tile_bench.png",
    "CartReturn": "tile_cart_return.png",
    "ExitSign": "tile_exit_sign.png",
    "FireHydrant": "tile_fire_hydrant.png",
    "GasPumpDisabled": "tile_gas_pump_disabled.png",
    "MallDirectory": "tile_mall_directory.png",
    "NewspaperBox": "tile_newspaper_box.png",
    "OutdoorHeater": "tile_outdoor_heater.png",
    "ParkingMeter": "tile_parking_meter.png",
    "PlanterUrn": "tile_planter_urn.png",
    "QuickmartFacade": "tile_quickmart_facade.png",
    "ReservedSign": "tile_reserved_sign.png",
    "RopeBarrier": "tile_rope_barrier.png",
    "SpeedBump": "tile_speed_bump.png",
    "StreetLamp": "tile_street_lamp.png",
    "TrashCan": "tile_trash_can.png",
    "UtilityCabinet": "tile_utility_cabinet.png",
    "ValetStand": "tile_valet_stand.png",
    "BusStop": "tile_bus_stop.png",
    "ElevatorLobby": "tile_elevator_lobby.png",
    "ExecutiveSpot": "tile_executive_spot.png",
    "FireLane": "tile_fire_lane.png",
    "GarageCeiling": "tile_garage_ceiling.png",
    "GarageLevel1": "tile_garage_level1.png",
    "GarageRamp": "tile_garage_ramp.png",
    "Gutter": "tile_gutter.png",
    "Manhole": "tile_manhole.png",
    "MeterZone": "tile_meter_zone.png",
    "PathwayStone": "tile_pathway_stone.png",
    "PoolDeck": "tile_pool_deck.png",
    "StreetCorner": "tile_street_corner.png",
    "StreetTreeTile": "tile_street_tree.png",
    "UtilityTrench": "tile_utility_trench.png",
    "WheelStopTile": "tile_wheel_stop.png",
}


class SceneTileGenerator:
    """Generates category scenes and extracts tiles from them."""

    DEFAULT_MODEL = "nano-banana-pro-preview"

    def __init__(
        self,
        workspace_root: str,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        dry_run: bool = False,
        extract_only: bool = False,
        verbose: bool = False,
    ):
        self.workspace_root = Path(workspace_root)
        self.model_name = model or self.DEFAULT_MODEL
        self.dry_run = dry_run
        self.extract_only = extract_only
        self.verbose = verbose

        env_file = self.workspace_root / ".env"
        if env_file.exists():
            with open(env_file, "r") as f:
                for line in f:
                    line = line.strip()
                    if not line or line.startswith("#") or "=" not in line:
                        continue
                    key, _, value = line.partition("=")
                    os.environ.setdefault(key.strip(), value.strip())

        self.api_key = api_key or os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY")

        if not dry_run and not extract_only and not self.api_key:
            raise ValueError(
                "Google AI Studio API key required. "
                "Set GEMINI_API_KEY in .env or environment, or use --api-key."
            )

        if not dry_run and not extract_only:
            if not GEMINI_AI_AVAILABLE:
                print("Error: google-generativeai not installed.")
                print("Install with: pip install google-generativeai")
                sys.exit(1)
        if not dry_run:
            if not PILLOW_AVAILABLE:
                print("Error: Pillow not installed.")
                print("Install with: pip install Pillow")
                sys.exit(1)

        self.scenes_dir = self.workspace_root / "spec" / "tiles" / "scenes"
        self.tiles_dir = self.workspace_root / "assets" / "world" / "tiles"
        self.scenes_dir.mkdir(parents=True, exist_ok=True)

        self.stats = {"scenes_generated": 0, "tiles_extracted": 0, "failed": 0, "skipped": 0}

        if not dry_run and not extract_only:
            self._init_gemini()

    def _init_gemini(self):
        """Initialize Gemini AI client with model validation."""
        print("Initializing Google AI Studio (Gemini)...")
        print(f"  API Key: {self.api_key[:8]}...{self.api_key[-4:]}")

        try:
            genai.configure(api_key=self.api_key)
            self._validate_model()
            self.model = genai.GenerativeModel(self.model_name)
            print(f"  Model: {self.model_name}")
            print()
        except SystemExit:
            raise
        except Exception as e:
            print(f"Error initializing Gemini: {e}")
            sys.exit(1)

    def _validate_model(self):
        """Check that the requested model exists, listing alternatives if not."""
        available = []
        try:
            for m in genai.list_models():
                if "generateContent" in (m.supported_generation_methods or []):
                    available.append(m.name.removeprefix("models/"))
        except Exception as e:
            print(f"  Warning: could not list models ({e}), proceeding anyway...")
            return

        if self.model_name not in available:
            print(f"\nError: model '{self.model_name}' not found.\n")
            print("Available models that support generateContent:")
            for name in sorted(available):
                print(f"  - {name}")
            print(f"\nUse --model <name> to pick one.")
            sys.exit(1)

    def build_scene_prompt(self, category_name: str, category: dict) -> str:
        """Build a prompt for generating a category scene."""
        cols, rows = category["grid"]
        tile_entries = category["tiles"]

        grid_descriptions = []
        for tile_name, (col, row, desc) in tile_entries.items():
            grid_descriptions.append(f"- Grid position ({col},{row}): {desc}")

        prompt = f"""You are a pixel artist creating a top-down game tile sheet in the style of GTA 2 for "Kilowatt Tycoon", an EV charging station tycoon game.

{GTA2_STYLE_DIRECTIVES}

SCENE: Generate a top-down tile sheet showing a {category["description"]}.

The image must be divided into a {cols}x{rows} GRID of clearly distinct tile areas.
Each grid cell represents one game tile. The cells should be visually distinct from each other
but share a cohesive pixel art style and color palette.

GRID LAYOUT (column, row) starting from top-left (0,0):
{chr(10).join(grid_descriptions)}

CRITICAL REQUIREMENTS:
1. The grid divisions must be CLEARLY VISIBLE - each cell is a distinct tile
2. Each cell should fill its entire area with the described content
3. Use consistent pixel art style across all cells
4. Adjacent cells should have compatible edges where they would naturally border each other
5. The overall image should read as a coherent tile sheet

Generate a single pixel art tile sheet image with the {cols}x{rows} grid layout described above."""

        return prompt

    def generate_scene(self, category_name: str, category: dict) -> Optional[Path]:
        """Generate a scene image for a category via the Gemini API."""
        prompt = self.build_scene_prompt(category_name, category)

        print(f"\n  Prompt:\n{'- ' * 35}")
        print(prompt)
        print(f"{'- ' * 35}\n")

        if self.dry_run:
            print("  [DRY RUN] Skipping API call")
            self.stats["skipped"] += 1
            return None

        stop_spinner = threading.Event()

        def _spinner():
            start = time.time()
            while not stop_spinner.is_set():
                elapsed = time.time() - start
                print(f"\r  Generating... {elapsed:.0f}s ", end="", flush=True)
                stop_spinner.wait(1.0)

        spinner = threading.Thread(target=_spinner, daemon=True)
        spinner.start()

        try:
            t0 = time.time()
            response = self.model.generate_content(prompt)
            elapsed = time.time() - t0
        finally:
            stop_spinner.set()
            spinner.join()
            print(f"\r  Generation complete ({elapsed:.1f}s)          ")

        try:
            if not response or not response.candidates:
                print("  WARNING: No response from API")
                self.stats["failed"] += 1
                return None

            candidate = response.candidates[0]
            if not candidate.content or not candidate.content.parts:
                print("  WARNING: No content in response")
                self.stats["failed"] += 1
                return None

            for part in candidate.content.parts:
                if hasattr(part, "text") and part.text:
                    print(f"\n  Model thinking:\n{'- ' * 35}")
                    print(f"  {part.text}")
                    print(f"{'- ' * 35}\n")

            image_data = None
            for part in candidate.content.parts:
                if hasattr(part, "inline_data") and part.inline_data:
                    inline_data = part.inline_data
                    mime_type = getattr(inline_data, "mime_type", "")
                    if mime_type.startswith("image/"):
                        data = getattr(inline_data, "data", None)
                        if data is not None:
                            if isinstance(data, str):
                                image_data = base64.b64decode(data)
                            elif isinstance(data, bytes):
                                image_data = data
                            break

            if not image_data:
                print("  WARNING: No image data in response")
                text = getattr(response, "text", "No text")
                print(f"  Response text: {text[:200]}")
                self.stats["failed"] += 1
                return None

            scene_file = self.scenes_dir / f"{category_name}_scene.png"
            img = Image.open(io.BytesIO(image_data))
            img.save(scene_file, "PNG")
            print(f"  Saved scene: {scene_file} ({img.width}x{img.height})")
            self.stats["scenes_generated"] += 1

            time.sleep(2)
            return scene_file

        except Exception as e:
            print(f"  ERROR generating scene: {e}")
            import traceback
            traceback.print_exc()
            self.stats["failed"] += 1
            return None

    def extract_tiles(self, category_name: str, category: dict) -> int:
        """Extract individual tiles from a scene image using the grid layout."""
        scene_file = self.scenes_dir / f"{category_name}_scene.png"
        if not scene_file.exists():
            print(f"  WARNING: Scene file not found: {scene_file}")
            return 0

        img = Image.open(scene_file)
        cols, rows = category["grid"]
        cell_w = img.width // cols
        cell_h = img.height // rows

        print(f"  Scene: {img.width}x{img.height}, grid: {cols}x{rows}, cell: {cell_w}x{cell_h}")

        count = 0
        for tile_name, (col, row, _desc) in category["tiles"].items():
            filename = TILE_FILENAMES.get(tile_name)
            if not filename:
                print(f"  WARNING: No filename mapping for tile '{tile_name}', skipping")
                continue

            x = col * cell_w
            y = row * cell_h
            cell = img.crop((x, y, x + cell_w, y + cell_h))

            tile = cell.resize((64, 64), Image.NEAREST)

            output_path = self.tiles_dir / filename
            tile.save(output_path, "PNG")
            count += 1

        print(f"  Extracted {count} tiles to {self.tiles_dir}")
        self.stats["tiles_extracted"] += count
        return count

    def process_category(self, category_name: str) -> bool:
        """Generate scene and extract tiles for one category."""
        if category_name not in SCENE_CATEGORIES:
            print(f"Error: Unknown category '{category_name}'")
            print(f"Available: {', '.join(sorted(SCENE_CATEGORIES.keys()))}")
            return False

        category = SCENE_CATEGORIES[category_name]
        cols, rows = category["grid"]
        tile_count = len(category["tiles"])

        print(f"\n{'='*70}")
        print(f"CATEGORY: {category_name} ({cols}x{rows} grid, {tile_count} tiles)")
        print(f"{'='*70}")

        if not self.extract_only:
            scene_file = self.generate_scene(category_name, category)
            if not scene_file and not self.dry_run:
                return False

        if not self.dry_run:
            self.extract_tiles(category_name, category)

        return True

    def process_all(self, categories: Optional[List[str]] = None):
        """Process multiple categories."""
        if categories is None:
            categories = list(SCENE_CATEGORIES.keys())

        print(f"{'='*70}")
        mode = "(DRY RUN)" if self.dry_run else "(EXTRACT ONLY)" if self.extract_only else ""
        print(f"SCENE-BASED TILE GENERATION {mode}")
        print(f"{'='*70}")
        print(f"Categories: {', '.join(categories)}")
        print(f"Scenes dir: {self.scenes_dir}")
        print(f"Tiles dir:  {self.tiles_dir}")
        print()

        for cat_name in categories:
            self.process_category(cat_name)

        self._print_summary()

    def _print_summary(self):
        """Print generation summary."""
        print(f"\n{'='*70}")
        print("SUMMARY")
        print(f"{'='*70}")
        print(f"  Scenes generated: {self.stats['scenes_generated']}")
        print(f"  Tiles extracted:  {self.stats['tiles_extracted']}")
        print(f"  Failed:           {self.stats['failed']}")
        print(f"  Skipped:          {self.stats['skipped']}")
        print(f"{'='*70}\n")


def main():
    parser = argparse.ArgumentParser(
        description="Generate tile assets by extracting from AI-generated category scenes",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=f"""
Examples:
  python generate_scene_tiles_ai.py --all                    # Generate all categories
  python generate_scene_tiles_ai.py --category base_parking  # One category
  python generate_scene_tiles_ai.py --extract-only --all     # Re-crop existing scenes
  python generate_scene_tiles_ai.py --all --dry-run          # Preview prompts

Available categories:
  {', '.join(sorted(SCENE_CATEGORIES.keys()))}
        """,
    )

    parser.add_argument("--category", type=str, nargs="+", metavar="CAT",
                        help="Generate specific category/categories")
    parser.add_argument("--all", action="store_true",
                        help="Generate all categories")
    parser.add_argument("--extract-only", action="store_true",
                        help="Only extract tiles from existing scene images (no API calls)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Preview prompts without calling API")
    parser.add_argument("--verbose", "-v", action="store_true",
                        help="Show full prompts")
    parser.add_argument("--model", type=str, metavar="MODEL", default=None,
                        help=f"Gemini model (default: {SceneTileGenerator.DEFAULT_MODEL})")
    parser.add_argument("--api-key", type=str, metavar="KEY",
                        help="Google AI Studio API key")

    args = parser.parse_args()

    if not args.category and not args.all:
        parser.error("Must specify --category or --all")
    if args.category and args.all:
        parser.error("Cannot specify both --category and --all")

    categories = args.category if args.category else None
    if categories:
        for cat in categories:
            if cat not in SCENE_CATEGORIES:
                parser.error(
                    f"Unknown category '{cat}'. "
                    f"Available: {', '.join(sorted(SCENE_CATEGORIES.keys()))}"
                )

    script_dir = Path(__file__).parent
    workspace_root = script_dir.parent

    try:
        gen = SceneTileGenerator(
            workspace_root=str(workspace_root),
            api_key=args.api_key,
            model=args.model,
            dry_run=args.dry_run,
            extract_only=args.extract_only,
            verbose=args.verbose,
        )
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)

    try:
        gen.process_all(categories=categories)
        if gen.stats["failed"] > 0:
            sys.exit(1)
    except KeyboardInterrupt:
        print("\n\nInterrupted by user.")
        gen._print_summary()
        sys.exit(1)
    except Exception as e:
        print(f"\nUnexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
