#!/usr/bin/env python3
"""Generate reimagined visual concepts for game levels using Google Gemini AI.

This script uses Gemini's multimodal capabilities to generate new visual concepts
for Kilowatt Tycoon levels. It takes level specifications and existing screenshots
as input, then produces concept images showing reimagined designs.

Usage:
    python reimagine_levels_ai.py --level 1           # Generate concept for level 1
    python reimagine_levels_ai.py --level 1 2 3      # Multiple levels
    python reimagine_levels_ai.py --all              # All levels 1-6
    python reimagine_levels_ai.py --level 1 --dry-run # Preview prompts only
"""

import argparse
import base64
import json
import os
import re
import sys
import time
from pathlib import Path
from typing import Optional, Dict, List
from datetime import datetime

# Import optional dependencies
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


class LevelReimagineer:
    """Generates reimagined visual concepts for game levels using Gemini AI."""
    
    # Map level numbers to (file_base, display_name, archetype)
    LEVEL_FILES = {
        1: ("01_first_street", "First Street Station", "parking_lot"),
        2: ("02_quick_charge_express", "Quick Charge Express", "gas_station"),
        3: ("03_central_fleet_plaza", "Central Fleet Plaza", "fleet_depot"),
    }
    
    DEFAULT_MODEL = "nano-banana-pro-preview"

    def __init__(
        self,
        workspace_root: str,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        dry_run: bool = False,
        verbose: bool = False,
    ):
        """Initialize the level reimagineer.
        
        Args:
            workspace_root: Root directory of workspace
            api_key: Google AI Studio API key (uses env var if None)
            model: Gemini model name (defaults to DEFAULT_MODEL)
            dry_run: If True, only preview prompts without API calls
            verbose: If True, show full prompts (not truncated)
        """
        self.workspace_root = Path(workspace_root)
        self.model_name = model or self.DEFAULT_MODEL
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
        
        # Get API key from parameter or environment
        self.api_key = api_key or os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY")
        
        # Validate configuration
        if not dry_run and not self.api_key:
            raise ValueError(
                "Google AI Studio API key required. "
                "Set GOOGLE_API_KEY or GEMINI_API_KEY environment variable, "
                "or use --api-key flag."
            )
        
        # Check dependencies
        if not dry_run:
            if not GEMINI_AI_AVAILABLE:
                print("Error: google-generativeai not installed.")
                print("Install with: pip install google-generativeai")
                sys.exit(1)
            if not PILLOW_AVAILABLE:
                print("Error: Pillow not installed.")
                print("Install with: pip install Pillow")
                sys.exit(1)
        
        # Set up paths
        self.spec_dir = self.workspace_root / "spec" / "levels"
        self.output_dir = self.spec_dir / "reimagined"
        
        # Create output directory
        if not dry_run:
            self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # Initialize statistics
        self.stats = {"success": 0, "failed": 0, "skipped": 0}
        
        # Initialize Gemini if not dry run
        if not dry_run:
            self._init_gemini()
    
    def _init_gemini(self):
        """Initialize Gemini AI client."""
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
            print("\nMake sure you have:")
            print(f"  1. A valid Google AI Studio API key")
            print(f"  2. Access to the '{self.model_name}' model")
            sys.exit(1)

    def _validate_model(self):
        """Check that the requested model exists, listing available ones if not."""
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
    
    def load_level_data(self, level_num: int) -> Dict:
        """Load level specification and screenshot.
        
        Args:
            level_num: Level number (1-6)
            
        Returns:
            Dictionary with level data including spec text and screenshot path
        """
        if level_num not in self.LEVEL_FILES:
            raise ValueError(f"Invalid level number: {level_num}. Must be 1-6.")
        
        file_base, display_name, archetype = self.LEVEL_FILES[level_num]
        
        # Load markdown specification
        spec_file = self.spec_dir / f"{file_base}.md"
        if not spec_file.exists():
            raise FileNotFoundError(f"Level spec not found: {spec_file}")
        
        with open(spec_file, 'r') as f:
            spec_text = f.read()
        
        # Extract visual identity section
        visual_identity = ""
        grid_size = ""
        
        # Parse the markdown
        lines = spec_text.split('\n')
        in_visual_section = False
        in_site_info = False
        
        for i, line in enumerate(lines):
            if '## Visual Identity' in line:
                in_visual_section = True
                continue
            elif line.startswith('## ') and in_visual_section:
                in_visual_section = False
            elif in_visual_section and line.strip():
                visual_identity += line + '\n'
            
            if '| Grid Size |' in line:
                parts = line.split('|')
                if len(parts) >= 3:
                    grid_size = parts[2].strip()
        
        # Load screenshot
        screenshot_file = self.spec_dir / f"level_{level_num:02d}_{file_base.split('_', 1)[1]}.png"
        if not screenshot_file.exists():
            print(f"  Warning: Screenshot not found: {screenshot_file}")
            screenshot_file = None
        
        return {
            "level_num": level_num,
            "file_base": file_base,
            "display_name": display_name,
            "archetype": archetype,
            "spec_text": spec_text,
            "visual_identity": visual_identity.strip(),
            "grid_size": grid_size,
            "screenshot_file": screenshot_file,
        }
    
    def build_prompt(self, level_data: Dict) -> str:
        """Build comprehensive prompt for AI image generation.
        
        Args:
            level_data: Level data dictionary from load_level_data
            
        Returns:
            Complete prompt string
        """
        prompt = f"""You are an expert pixel artist creating a top-down game map for "Kilowatt Tycoon", an EV charging station tycoon game.

ART STYLE (CRITICAL - MUST FOLLOW):
- GTA 2 aesthetic: top-down pixel art with a gritty, urban feel
- PIXELATED look - visible pixels, chunky details, retro game art
- TRUE TOP-DOWN camera angle (90 degrees straight down), just like GTA 2
- Rich, saturated colors with strong contrast between elements
- Dark asphalt roads with painted lane markings
- Buildings shown as solid rooftop shapes (you're looking down at them)
- Small environmental details: dumpsters, bollards, curbs, manhole covers, puddles, tire marks
- Lighting cues from overhead lamps casting small pools of light on the ground

COLOR PALETTE:
- Asphalt/roads: dark grays and charcoals with subtle variation and wear
- Parking line markings: bright white or yellow paint
- Sidewalks/concrete: lighter warm gray with pixel-level cracks
- Grass/landscaping: rich greens, varying shades to suggest texture
- Buildings: muted brick reds, concrete grays, industrial browns - flat rooftops seen from above
- Accent colors: neon greens and electric blues for EV charging equipment
- Streetlights cast warm yellow circles on the ground
- Overall vibe: urban, slightly gritty, colorful but grounded - like a GTA 2 map zoomed in

WHAT TO AVOID:
- NO vector art, NO clean/flat UI style
- NO isometric or 3D perspective
- NO smooth gradients - use dithering or flat pixel shading instead
- NO photorealistic rendering
- NO high-resolution smooth edges - everything should have visible pixel stepping

LEVEL TO REIMAGINE:
Level: {level_data['display_name']}
Archetype: {level_data['archetype']}
Grid Size: {level_data['grid_size']} tiles

VISUAL IDENTITY:
{level_data['visual_identity']}

FUNCTIONAL REQUIREMENTS (MUST MAINTAIN):
1. Entry point for vehicles (road opening at map edge)
2. Exit point for vehicles (road opening at opposite edge)
3. Area for TRANSFORMER placement (utility box, at least 2x2 tiles, on open ground)
4. Space for EV CHARGERS adjacent to parking bays
5. Parking bays where vehicles park and charge
6. Drive lanes connecting entry -> parking -> exit
7. Clear vehicle circulation flow
8. Open/buildable space for the player to place charging infrastructure

DESIGN TASK:
Generate a top-down pixel-art map of this level in the style of GTA 2. The map should:

1. Look like a zoomed-in neighborhood block from GTA 2, but pixelated
2. Show the ENTIRE level from directly above with all functional areas visible
3. Have gritty urban character - cracked pavement, painted curbs, scattered props
4. Use pixel art techniques: visible pixels, dithering for shading, limited palette per surface
5. Maintain clear visual distinction between roads, parking, sidewalks, and buildings
6. Include small props and details that bring the scene to life (but don't clutter functional areas)

Generate a single pixel-art top-down game map in the style of GTA 2."""
        
        return prompt
    
    def generate_concept(self, level_num: int) -> bool:
        """Generate reimagined concept for a single level.
        
        Args:
            level_num: Level number (1-6)
            
        Returns:
            True if successful, False otherwise
        """
        try:
            # Load level data
            print(f"\n{'='*70}")
            print(f"LEVEL {level_num}: {self.LEVEL_FILES[level_num][1]}")
            print(f"{'='*70}")
            
            level_data = self.load_level_data(level_num)
            
            # Build prompt
            prompt = self.build_prompt(level_data)
            
            if self.verbose:
                print(f"\nPrompt:\n{prompt}\n")
            else:
                print(f"\nPrompt preview: {prompt[:200]}...\n")
            
            if self.dry_run:
                print("[DRY RUN] Skipping API call")
                self.stats["skipped"] += 1
                return True
            
            # Load reference image if available
            reference_image = None
            if level_data["screenshot_file"]:
                print(f"Loading reference screenshot: {level_data['screenshot_file'].name}")
                reference_image = Image.open(level_data["screenshot_file"])
            
            # Generate concept
            print("Calling Gemini API to generate concept...")
            
            # Prepare content for API
            content_parts = [prompt]
            if reference_image:
                content_parts.append("Reference image (current design):")
                content_parts.append(reference_image)
            
            # Call Gemini
            response = self.model.generate_content(content_parts)
            
            if not response or not response.candidates:
                print("  ⚠️  No response from API")
                self.stats["failed"] += 1
                return False
            
            # Extract image from response
            candidate = response.candidates[0]
            if not candidate.content or not candidate.content.parts:
                print("  ⚠️  No content in response")
                self.stats["failed"] += 1
                return False
            
            # Find image part in response
            image_data = None
            for part in candidate.content.parts:
                if hasattr(part, 'inline_data') and part.inline_data:
                    inline_data = part.inline_data
                    mime_type = getattr(inline_data, 'mime_type', '')
                    if mime_type.startswith('image/'):
                        if hasattr(inline_data, 'data'):
                            data = inline_data.data
                            if isinstance(data, str):
                                image_data = base64.b64decode(data)
                            elif isinstance(data, bytes):
                                image_data = data
                            break
            
            if not image_data:
                print("  ⚠️  No image data in response")
                print("  Response text:", response.text if hasattr(response, 'text') else "No text")
                self.stats["failed"] += 1
                return False
            
            # Save image
            output_file = self.output_dir / f"level_{level_num:02d}_{level_data['file_base'].split('_', 1)[1]}_concept.png"
            
            img = Image.open(io.BytesIO(image_data))
            img.save(output_file, "PNG")
            
            print(f"  ✓ Saved concept to: {output_file}")
            self.stats["success"] += 1
            
            # Rate limiting
            time.sleep(2)
            
            return True
            
        except Exception as e:
            print(f"  ❌ Error generating concept: {e}")
            import traceback
            traceback.print_exc()
            self.stats["failed"] += 1
            return False
    
    def generate_all_concepts(self, levels: Optional[List[int]] = None) -> Dict:
        """Generate concepts for multiple levels.
        
        Args:
            levels: List of level numbers, or None for all levels 1-6
            
        Returns:
            Statistics dictionary
        """
        if levels is None:
            levels = list(range(1, 7))
        
        print(f"{'='*70}")
        print(f"AI LEVEL REIMAGINE {'(DRY RUN)' if self.dry_run else ''}")
        print(f"{'='*70}")
        print(f"Levels to process: {levels}")
        print(f"Output directory: {self.output_dir}")
        print()
        
        for level_num in levels:
            self.generate_concept(level_num)
        
        # Print summary
        self._print_summary()
        
        return self.stats
    
    def _print_summary(self):
        """Print generation summary."""
        print(f"\n{'='*70}")
        print("SUMMARY")
        print(f"{'='*70}")
        print(f"  ✓ Success: {self.stats['success']}")
        print(f"  ❌ Failed:  {self.stats['failed']}")
        print(f"  ⊘ Skipped: {self.stats['skipped']}")
        print(f"{'='*70}\n")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Generate reimagined visual concepts for game levels using Gemini AI",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python reimagine_levels_ai.py --level 1           # Generate concept for level 1
  python reimagine_levels_ai.py --level 1 2 3      # Multiple levels
  python reimagine_levels_ai.py --all              # All levels 1-6
  python reimagine_levels_ai.py --level 1 --dry-run # Preview prompts only

API Authentication:
  Set GOOGLE_API_KEY or GEMINI_API_KEY environment variable
        """,
    )
    
    parser.add_argument(
        "--level",
        type=int,
        nargs="+",
        metavar="LEVEL",
        help="Generate concept for specific level(s) (1-6)",
    )
    
    parser.add_argument(
        "--all",
        action="store_true",
        help="Generate concepts for all levels 1-6",
    )
    
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Preview prompts without calling Gemini API",
    )
    
    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show full prompts (not truncated)",
    )
    
    parser.add_argument(
        "--model",
        type=str,
        metavar="MODEL",
        default=None,
        help=f"Gemini model name (default: {LevelReimagineer.DEFAULT_MODEL})",
    )
    
    parser.add_argument(
        "--api-key",
        type=str,
        metavar="API_KEY",
        help="Google AI Studio API key (defaults to GOOGLE_API_KEY or GEMINI_API_KEY env var)",
    )
    
    args = parser.parse_args()
    
    # Validate arguments
    if not args.level and not args.all:
        parser.error("Must specify --level or --all")
    
    if args.level and args.all:
        parser.error("Cannot specify both --level and --all")
    
    # Determine levels to process
    levels = args.level if args.level else None
    
    if levels:
        for level in levels:
            if level < 1 or level > 6:
                parser.error(f"Level {level} out of range. Must be 1-6.")
    
    # Determine workspace root
    script_dir = Path(__file__).parent
    workspace_root = script_dir.parent
    
    # Create reimagineer
    try:
        reimagineer = LevelReimagineer(
            workspace_root=str(workspace_root),
            api_key=args.api_key,
            model=args.model,
            dry_run=args.dry_run,
            verbose=args.verbose,
        )
    except Exception as e:
        print(f"Error initializing reimagineer: {e}")
        sys.exit(1)
    
    # Generate concepts
    try:
        stats = reimagineer.generate_all_concepts(levels=levels)
        
        # Exit with error code if any failures
        if stats["failed"] > 0:
            sys.exit(1)
    
    except KeyboardInterrupt:
        print("\n\nInterrupted by user.")
        reimagineer._print_summary()
        sys.exit(1)
    except Exception as e:
        print(f"\nUnexpected error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    main()
