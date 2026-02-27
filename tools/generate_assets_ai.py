#!/usr/bin/env python3
"""Generate game assets using Google Imagen 3 via Vertex AI or Google AI Studio.

This script generates tiles, vehicles, and VFX for Kilowatt Tycoon using
Google's Imagen 3 model. It loads asset definitions from asset_manifest.json,
constructs prompts with style configuration, and saves generated PNGs.

Supports two APIs:
  - Vertex AI: Requires GCP project and service account authentication
  - Google AI Studio (Gemini): Requires GOOGLE_API_KEY or GEMINI_API_KEY

Usage:
    python generate_assets_ai.py                        # Generate all assets
    python generate_assets_ai.py --dry-run              # Preview prompts only
    python generate_assets_ai.py --type tiles           # Tiles only
    python generate_assets_ai.py --type vehicles        # Vehicles only
    python generate_assets_ai.py --type vfx             # VFX only
    python generate_assets_ai.py --id 5 --type tiles    # Specific tile ID
    python generate_assets_ai.py --category parking --type tiles # Tile category
    python generate_assets_ai.py --project my-gcp-project # Specify GCP project
    python generate_assets_ai.py --api gemini           # Use Google AI Studio API
    python generate_assets_ai.py --api vertex           # Use Vertex AI API
"""

import argparse
import base64
import json
import os
import sys
import time
from pathlib import Path
from typing import Optional

# Import optional dependencies for actual generation
# For dry-run mode, these aren't needed
VERTEX_AI_AVAILABLE = False
GEMINI_AI_AVAILABLE = False
PILLOW_AVAILABLE = False

try:
    from google.cloud import aiplatform
    from vertexai.preview.vision_models import ImageGenerationModel
    VERTEX_AI_AVAILABLE = True
except ImportError:
    pass

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


class AssetGenerator:
    """Generates game assets (tiles, vehicles, VFX) using Imagen 3."""

    def __init__(
        self,
        manifest_path: str,
        style_config_path: str,
        workspace_root: str,
        api: Optional[str] = None,
        project_id: Optional[str] = None,
        location: str = "us-central1",
        api_key: Optional[str] = None,
        dry_run: bool = False,
        verbose: bool = False,
    ):
        """Initialize the asset generator.

        Args:
            manifest_path: Path to asset_manifest.json
            style_config_path: Path to style_config.json
            workspace_root: Root directory of workspace (for output paths)
            api: API to use ("vertex" or "gemini"). Auto-detected if None.
            project_id: Google Cloud project ID (uses env var if None, Vertex only)
            location: GCP region for Vertex AI
            api_key: Google AI Studio API key (uses env var if None, Gemini only)
            dry_run: If True, only preview prompts without API calls
            verbose: If True, show full prompts (not truncated)
        """
        self.manifest_path = Path(manifest_path)
        self.style_config_path = Path(style_config_path)
        self.workspace_root = Path(workspace_root)
        self.dry_run = dry_run
        self.verbose = verbose

        # Load .env file from workspace root if present
        env_file = self.workspace_root / ".env"
        if env_file.exists():
            with open(env_file, 'r') as f:
                for line in f:
                    line = line.strip()
                    if not line or line.startswith('#') or '=' not in line:
                        continue
                    key, _, value = line.partition('=')
                    os.environ.setdefault(key.strip(), value.strip())

        # Determine which API to use
        self.api_key = api_key or os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY")
        self.project_id = project_id or os.environ.get("GOOGLE_CLOUD_PROJECT")
        self.location = location

        # Auto-detect API if not specified
        if api is None:
            if self.api_key:
                self.api = "gemini"
            elif self.project_id:
                self.api = "vertex"
            else:
                self.api = None
        else:
            self.api = api

        # Validate configuration
        if not dry_run:
            if self.api == "gemini":
                if not self.api_key:
                    raise ValueError(
                        "Google AI Studio API key required for Gemini API. "
                        "Set GOOGLE_API_KEY or GEMINI_API_KEY environment variable, "
                        "or use --api-key flag."
                    )
            elif self.api == "vertex":
                if not self.project_id:
                    raise ValueError(
                        "Google Cloud project ID required for Vertex AI. "
                        "Set GOOGLE_CLOUD_PROJECT environment variable or use --project flag."
                    )
            else:
                raise ValueError(
                    "No API credentials found. Either:\n"
                    "  - Set GOOGLE_API_KEY or GEMINI_API_KEY for Google AI Studio, or\n"
                    "  - Set GOOGLE_CLOUD_PROJECT for Vertex AI"
                )

        # Check dependencies before proceeding
        if not dry_run:
            if self.api == "gemini":
                if not GEMINI_AI_AVAILABLE:
                    print("Error: google-generativeai not installed.")
                    print("Install with: pip install google-generativeai")
                    sys.exit(1)
            elif self.api == "vertex":
                if not VERTEX_AI_AVAILABLE:
                    print("Error: google-cloud-aiplatform not installed.")
                    print("Install with: pip install google-cloud-aiplatform")
                    sys.exit(1)
            if not PILLOW_AVAILABLE:
                print("Error: Pillow not installed.")
                print("Install with: pip install Pillow")
                sys.exit(1)

        self.manifest = self._load_json(self.manifest_path)
        self.style_config = self._load_json(self.style_config_path)

        # Load asset sections from manifest
        self.tiles = self.manifest["tiles"]["entries"]
        self.vehicles = self.manifest["vehicles"]["entries"]
        self.vfx = self.manifest["vfx"]["entries"]
        
        # Get output directories from manifest
        self.tiles_dir = self.workspace_root / "assets" / self.manifest["tiles"]["asset_dir"]
        self.vehicles_dir = self.workspace_root / "assets" / self.manifest["vehicles"]["asset_dir"]
        self.vfx_dir = self.workspace_root / "assets" / self.manifest["vfx"]["asset_dir"]
        
        self.base_prompt = self.style_config["base_prompt"]
        self.negative_prompt = self.style_config["negative_prompt"]
        self.rate_limit_delay = self.style_config["rate_limit"][
            "delay_between_requests_seconds"
        ]

        self.generation_stats = {"success": 0, "failed": 0, "skipped": 0}

        if not dry_run:
            if self.api == "gemini":
                self._init_gemini_ai()
            else:
                self._init_vertex_ai()

    def _load_json(self, path: Path) -> dict:
        """Load JSON configuration file."""
        if not path.exists():
            raise FileNotFoundError(f"Configuration file not found: {path}")

        with open(path) as f:
            return json.load(f)

    def _init_gemini_ai(self):
        """Initialize Google AI Studio (Gemini) with API key."""
        print(f"Initializing Google AI Studio (Gemini)...")
        print(f"  API Key: {self.api_key[:8]}...{self.api_key[-4:]}")

        try:
            genai.configure(api_key=self.api_key)
            
            # Get the Gemini model name from config
            self.gemini_model_name = self.style_config.get(
                "gemini_model", "gemini-3-pro-image-preview"
            )
            self.gemini_client = genai.GenerativeModel(self.gemini_model_name)
            print(f"  Model: {self.gemini_model_name}")
            print()
        except Exception as e:
            print(f"Error initializing Google AI Studio: {e}")
            print("\nMake sure you have:")
            print("  1. A valid Google AI Studio API key")
            print("  2. Enabled the Generative AI API")
            print("  3. Access to the image generation model")
            sys.exit(1)

    def _init_vertex_ai(self):
        """Initialize Vertex AI with project and location."""
        print(f"Initializing Vertex AI...")
        print(f"  Project: {self.project_id}")
        print(f"  Location: {self.location}")

        try:
            aiplatform.init(project=self.project_id, location=self.location)
            self.model = ImageGenerationModel.from_pretrained(
                self.style_config["imagen_model"]
            )
            print(f"  Model: {self.style_config['imagen_model']}")
            print()
        except Exception as e:
            print(f"Error initializing Vertex AI: {e}")
            print("\nMake sure you have:")
            print("  1. Enabled Vertex AI API in your GCP project")
            print("  2. Authenticated: gcloud auth application-default login")
            print("  3. Imagen 3 access (may require allowlist approval)")
            sys.exit(1)

    def _build_prompt(self, asset: dict) -> str:
        """Build complete prompt for an asset.

        Args:
            asset: Asset definition from manifest (tile, vehicle, or VFX)

        Returns:
            Complete prompt string combining base, asset-specific, and color info
        """
        # Get color hex values for this asset (tiles have colors, others may not)
        color_hints = []
        for color_name in asset.get("colors", []):
            hex_value = self.style_config["color_palette"].get(color_name)
            if hex_value:
                color_hints.append(f"{color_name} {hex_value}")

        color_text = (
            f", primary colors: {', '.join(color_hints)}" if color_hints else ""
        )

        # Combine all parts - use ai_prompt field
        full_prompt = f"{self.base_prompt}, {asset['ai_prompt']}{color_text}"

        return full_prompt

    def _generate_image(self, asset: dict, aspect_ratio: str = "1:1") -> Optional[bytes]:
        """Generate image using Imagen 3.

        Args:
            asset: Asset definition from manifest
            aspect_ratio: Image aspect ratio (e.g., "1:1", "16:9")

        Returns:
            PNG image bytes or None if generation failed
        """
        prompt = self._build_prompt(asset)

        if self.dry_run:
            return None

        if self.api == "gemini":
            return self._generate_image_gemini(prompt, aspect_ratio)
        else:
            return self._generate_image_vertex(prompt, aspect_ratio)

    def _generate_image_gemini(self, prompt: str, aspect_ratio: str) -> Optional[bytes]:
        """Generate image using Google AI Studio (Gemini) API.

        Args:
            prompt: Complete prompt for image generation
            aspect_ratio: Image aspect ratio (e.g., "1:1", "16:9")

        Returns:
            PNG image bytes or None if generation failed
        """
        try:
            print(f"  Calling Google AI Studio (Gemini) API...")

            # Build full prompt with negative prompt guidance and explicit image request
            full_prompt = (
                f"Generate an image: {prompt}\n\n"
                f"Avoid: {self.negative_prompt}"
            )

            # Generate image with Gemini - no special config needed for image models
            response = self.gemini_client.generate_content(full_prompt)

            if not response or not response.candidates:
                print(f"  ⚠️  No response from API")
                return None

            # Extract image from response
            candidate = response.candidates[0]
            if not candidate.content or not candidate.content.parts:
                print(f"  ⚠️  No content in response")
                return None

            # Find image part in response
            for part in candidate.content.parts:
                # Check for inline_data (image data)
                if hasattr(part, 'inline_data') and part.inline_data:
                    inline_data = part.inline_data
                    mime_type = getattr(inline_data, 'mime_type', '')
                    if mime_type.startswith('image/'):
                        if hasattr(inline_data, 'data'):
                            data = inline_data.data
                            # Data may be base64 string or bytes
                            if isinstance(data, str):
                                return base64.b64decode(data)
                            elif isinstance(data, bytes):
                                return data
                # Check for file_data
                if hasattr(part, 'file_data') and part.file_data:
                    file_data = part.file_data
                    if hasattr(file_data, 'data'):
                        return file_data.data

            print(f"  ⚠️  No image data found in response")
            return None

        except Exception as e:
            print(f"  ❌ Error generating image: {e}")
            return None

    def _generate_image_vertex(self, prompt: str, aspect_ratio: str) -> Optional[bytes]:
        """Generate image using Vertex AI API.

        Args:
            prompt: Complete prompt for image generation
            aspect_ratio: Image aspect ratio (e.g., "1:1", "16:9")

        Returns:
            PNG image bytes or None if generation failed
        """
        try:
            print(f"  Calling Vertex AI API...")

            # Generate image with Imagen 3
            response = self.model.generate_images(
                prompt=prompt,
                negative_prompt=self.negative_prompt,
                number_of_images=1,
                aspect_ratio=aspect_ratio,
                safety_filter_level=self.style_config["generation_config"][
                    "safety_filter_level"
                ],
                person_generation=self.style_config["generation_config"][
                    "person_generation"
                ],
                add_watermark=self.style_config["generation_config"]["add_watermark"],
            )

            if not response or len(response.images) == 0:
                print(f"  ⚠️  No images returned from API")
                return None

            # Get the first generated image
            image = response.images[0]

            # Convert to PNG bytes
            image_bytes = image._as_base64_string()
            png_bytes = base64.b64decode(image_bytes)

            return png_bytes

        except Exception as e:
            print(f"  ❌ Error generating image: {e}")
            return None

    def _save_image(self, image_bytes: bytes, output_dir: Path, filename: str, target_size: tuple[int, int]) -> bool:
        """Save image bytes to PNG file.

        Args:
            image_bytes: PNG image data
            output_dir: Directory to save to
            filename: Output filename
            target_size: (width, height) tuple for resizing

        Returns:
            True if saved successfully, False otherwise
        """
        output_path = output_dir / filename

        try:
            # Create output directory if needed
            output_dir.mkdir(parents=True, exist_ok=True)

            # Verify it's a valid image and resize if needed
            img = Image.open(io.BytesIO(image_bytes))

            # Ensure correct size
            if img.size != target_size:
                print(f"  ⚠️  Resizing from {img.size} to {target_size}")
                img = img.resize(target_size, Image.Resampling.LANCZOS)

            # Save as PNG
            img.save(output_path, "PNG")
            print(f"  ✓ Saved to {output_path}")
            return True

        except Exception as e:
            print(f"  ❌ Error saving image: {e}")
            return False

    def generate_asset(self, asset: dict, asset_type: str, output_dir: Path) -> bool:
        """Generate a single asset.

        Args:
            asset: Asset definition from manifest
            asset_type: Type of asset ("tile", "vehicle", or "vfx")
            output_dir: Directory to save the asset to

        Returns:
            True if successful, False otherwise
        """
        # Display asset info
        if asset_type == "tile":
            print(f"\n[{asset['id']}] {asset['name']} ({asset.get('category', 'N/A')})")
        else:
            print(f"\n{asset['name']}")
        print(f"  Output: {asset['filename']}")

        # Get dimensions
        if asset_type == "tile":
            # Tiles use manifest-level dimensions
            dims = self.manifest["tiles"]["dimensions"]
            target_size = (dims["width"], dims["height"])
            aspect_ratio = "1:1"
        else:
            # Vehicles and VFX have per-entry dimensions
            dims = asset.get("dimensions", {"width": 64, "height": 64})
            target_size = (dims["width"], dims["height"])
            # Calculate aspect ratio
            width, height = target_size
            if width == height:
                aspect_ratio = "1:1"
            elif width > height:
                ratio = width / height
                if ratio > 3:
                    aspect_ratio = "16:9"
                else:
                    aspect_ratio = "3:2"
            else:
                ratio = height / width
                if ratio > 3:
                    aspect_ratio = "9:16"
                else:
                    aspect_ratio = "2:3"

        # Build and display prompt
        prompt = self._build_prompt(asset)
        if self.verbose:
            print(f"  Prompt: {prompt}")
        else:
            print(f"  Prompt: {prompt[:120]}..." if len(prompt) > 120 else f"  Prompt: {prompt}")

        if self.dry_run:
            print(f"  [DRY RUN] Skipped API call")
            self.generation_stats["skipped"] += 1
            return True

        # Generate image
        image_bytes = self._generate_image(asset, aspect_ratio)

        if not image_bytes:
            self.generation_stats["failed"] += 1
            return False

        # Save image
        if self._save_image(image_bytes, output_dir, asset["filename"], target_size):
            self.generation_stats["success"] += 1

            # Rate limiting
            time.sleep(self.rate_limit_delay)
            return True
        else:
            self.generation_stats["failed"] += 1
            return False

    def generate_assets(
        self,
        asset_types: list[str] = ["tiles", "vehicles", "vfx"],
        asset_ids: Optional[list[int]] = None,
        category: Optional[str] = None,
    ) -> dict:
        """Generate multiple assets.

        Args:
            asset_types: List of asset types to generate ("tiles", "vehicles", "vfx")
            asset_ids: List of specific IDs to generate (None = all, only for tiles)
            category: Filter by category (None = all, only for tiles)

        Returns:
            Generation statistics dictionary
        """
        print(f"=" * 70)
        print(f"ASSET GENERATION {'(DRY RUN)' if self.dry_run else ''}")
        print(f"=" * 70)

        # Generate tiles
        if "tiles" in asset_types:
            tiles_to_generate = self.tiles

            if asset_ids is not None:
                tiles_to_generate = [t for t in tiles_to_generate if t["id"] in asset_ids]

            if category is not None:
                tiles_to_generate = [
                    t for t in tiles_to_generate if t.get("category") == category
                ]

            if tiles_to_generate:
                print(f"\n[TILES] {len(tiles_to_generate)} to generate")
                print(f"  Output: {self.tiles_dir}")
                if asset_ids:
                    print(f"  Filter: IDs {asset_ids}")
                if category:
                    print(f"  Filter: Category '{category}'")

                for tile in tiles_to_generate:
                    self.generate_asset(tile, "tile", self.tiles_dir)

        # Generate vehicles
        if "vehicles" in asset_types:
            vehicles_to_generate = self.vehicles
            
            if vehicles_to_generate:
                print(f"\n[VEHICLES] {len(vehicles_to_generate)} to generate")
                print(f"  Output: {self.vehicles_dir}")

                for vehicle in vehicles_to_generate:
                    self.generate_asset(vehicle, "vehicle", self.vehicles_dir)

        # Generate VFX
        if "vfx" in asset_types:
            # Only generate VFX marked as "used"
            vfx_to_generate = [v for v in self.vfx if v.get("used", True)]
            
            if vfx_to_generate:
                print(f"\n[VFX] {len(vfx_to_generate)} to generate (used only)")
                print(f"  Output: {self.vfx_dir}")

                for vfx in vfx_to_generate:
                    self.generate_asset(vfx, "vfx", self.vfx_dir)

        # Print summary
        self._print_summary()

        return self.generation_stats

    def _print_summary(self):
        """Print generation summary."""
        print(f"\n{'=' * 70}")
        print(f"SUMMARY")
        print(f"{'=' * 70}")
        print(f"  ✓ Success: {self.generation_stats['success']}")
        print(f"  ❌ Failed:  {self.generation_stats['failed']}")
        print(f"  ⊘ Skipped: {self.generation_stats['skipped']}")
        print(f"{'=' * 70}\n")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Generate game assets using Google Imagen 3",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python generate_assets_ai.py                        # Generate all assets
  python generate_assets_ai.py --dry-run              # Preview prompts only
  python generate_assets_ai.py --type tiles           # Tiles only
  python generate_assets_ai.py --type vehicles        # Vehicles only
  python generate_assets_ai.py --type vfx             # VFX only
  python generate_assets_ai.py --id 0 5 6 --type tiles  # Specific tile IDs
  python generate_assets_ai.py --category parking --type tiles # Tile category
  python generate_assets_ai.py --api gemini           # Use Google AI Studio
  python generate_assets_ai.py --api vertex --project my-project  # Use Vertex AI

API Authentication:
  Gemini (Google AI Studio): Set GOOGLE_API_KEY or GEMINI_API_KEY env var
  Vertex AI: Set GOOGLE_CLOUD_PROJECT env var + gcloud auth
        """,
    )

    parser.add_argument(
        "--type",
        type=str,
        nargs="+",
        choices=["tiles", "vehicles", "vfx", "all"],
        default=["all"],
        metavar="TYPE",
        help="Asset type(s) to generate: tiles, vehicles, vfx, or all (default: all)",
    )

    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Preview prompts without calling Imagen 3 API",
    )

    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show full prompts (not truncated)",
    )

    parser.add_argument(
        "--id",
        type=int,
        nargs="+",
        metavar="ID",
        help="Generate specific asset ID(s) only (tiles only)",
    )

    parser.add_argument(
        "--category",
        type=str,
        metavar="CATEGORY",
        help="Generate assets in specific category only (tiles only, e.g., 'parking', 'gas_station')",
    )

    parser.add_argument(
        "--api",
        type=str,
        choices=["gemini", "vertex"],
        metavar="API",
        help="API to use: 'gemini' (Google AI Studio) or 'vertex' (Vertex AI). "
             "Auto-detected from environment if not specified.",
    )

    parser.add_argument(
        "--api-key",
        type=str,
        metavar="API_KEY",
        help="Google AI Studio API key (defaults to GOOGLE_API_KEY or GEMINI_API_KEY env var)",
    )

    parser.add_argument(
        "--project",
        type=str,
        metavar="PROJECT_ID",
        help="Google Cloud project ID for Vertex AI (defaults to GOOGLE_CLOUD_PROJECT env var)",
    )

    parser.add_argument(
        "--location",
        type=str,
        default="us-central1",
        metavar="LOCATION",
        help="GCP region for Vertex AI (default: us-central1)",
    )

    args = parser.parse_args()

    # Determine paths relative to script location
    script_dir = Path(__file__).parent
    workspace_root = script_dir.parent
    manifest_path = script_dir / "asset_manifest.json"
    style_config_path = script_dir / "style_config.json"

    # Handle "all" type
    asset_types = args.type
    if "all" in asset_types:
        asset_types = ["tiles", "vehicles", "vfx"]

    # Create generator
    try:
        generator = AssetGenerator(
            manifest_path=str(manifest_path),
            style_config_path=str(style_config_path),
            workspace_root=str(workspace_root),
            api=args.api,
            project_id=args.project,
            location=args.location,
            api_key=args.api_key,
            dry_run=args.dry_run,
            verbose=args.verbose,
        )
    except Exception as e:
        print(f"Error initializing generator: {e}")
        sys.exit(1)

    # Generate assets
    try:
        stats = generator.generate_assets(
            asset_types=asset_types,
            asset_ids=args.id,
            category=args.category,
        )

        # Exit with error code if any failures
        if stats["failed"] > 0:
            sys.exit(1)

    except KeyboardInterrupt:
        print("\n\nInterrupted by user.")
        generator._print_summary()
        sys.exit(1)
    except Exception as e:
        print(f"\nUnexpected error: {e}")
        import traceback

        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
