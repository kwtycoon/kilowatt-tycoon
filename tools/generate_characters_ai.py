#!/usr/bin/env python3
"""Generate hacker character sprites using Google Gemini AI.

This script uses Gemini's multimodal capabilities to generate hacker character
PNGs that match the existing robber art style (chunky bean characters with
transparent backgrounds). The robber images are sent as reference images so
Gemini replicates the art style.

Usage:
    python generate_characters_ai.py --all               # Generate all 4 hackers
    python generate_characters_ai.py --character 0 1     # Specific characters
    python generate_characters_ai.py --all --dry-run     # Preview prompts only
"""

import argparse
import base64
import os
import sys
import time
from pathlib import Path
from typing import Optional, Dict, List

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


# (filename, pose_description, hoodie_color, reference_robber_image)
CHARACTERS = [
    {
        "id": 0,
        "name": "HackerWalkingGreen",
        "filename": "character_hacker_walking_green.png",
        "pose": "walking",
        "color": "green",
        "color_hex": "#34D399",
        "reference": "character_robber_walking.png",
    },
    {
        "id": 1,
        "name": "HackerWalkingPurple",
        "filename": "character_hacker_walking_purple.png",
        "pose": "walking",
        "color": "purple",
        "color_hex": "#A78BFA",
        "reference": "character_robber_walking.png",
    },
    {
        "id": 2,
        "name": "HackerHackingGreen",
        "filename": "character_hacker_hacking_green.png",
        "pose": "hacking",
        "color": "green",
        "color_hex": "#34D399",
        "reference": "character_robber_stealing.png",
    },
    {
        "id": 3,
        "name": "HackerHackingPurple",
        "filename": "character_hacker_hacking_purple.png",
        "pose": "hacking",
        "color": "purple",
        "color_hex": "#A78BFA",
        "reference": "character_robber_stealing.png",
    },
]


class CharacterGenerator:
    """Generates hacker character sprites using Gemini AI."""

    DEFAULT_MODEL = "gemini-2.5-flash-image"

    def __init__(
        self,
        workspace_root: str,
        api_key: Optional[str] = None,
        model: Optional[str] = None,
        dry_run: bool = False,
        verbose: bool = False,
    ):
        self.workspace_root = Path(workspace_root)
        self.model_name = model or self.DEFAULT_MODEL
        self.dry_run = dry_run
        self.verbose = verbose

        env_file = self.workspace_root / ".env"
        if env_file.exists():
            with open(env_file, 'r') as f:
                for line in f:
                    line = line.strip()
                    if not line or line.startswith('#') or '=' not in line:
                        continue
                    key, _, value = line.partition('=')
                    os.environ.setdefault(key.strip(), value.strip())

        self.api_key = api_key or os.environ.get("GEMINI_API_KEY") or os.environ.get("GOOGLE_API_KEY")

        if not dry_run and not self.api_key:
            raise ValueError(
                "Google AI Studio API key required. "
                "Set GOOGLE_API_KEY or GEMINI_API_KEY environment variable, "
                "or use --api-key flag."
            )

        if not dry_run:
            if not GEMINI_AI_AVAILABLE:
                print("Error: google-generativeai not installed.")
                print("Install with: pip install google-generativeai")
                sys.exit(1)
            if not PILLOW_AVAILABLE:
                print("Error: Pillow not installed.")
                print("Install with: pip install Pillow")
                sys.exit(1)

        self.characters_dir = self.workspace_root / "assets" / "characters"
        self.stats = {"success": 0, "failed": 0, "skipped": 0}

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
            print(f"\nMake sure you have:")
            print(f"  1. A valid Google AI Studio API key")
            print(f"  2. Access to the '{self.model_name}' model")
            sys.exit(1)

    def _validate_model(self):
        """Check that the requested model exists."""
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

    def build_prompt(self, character: Dict) -> str:
        """Build prompt for a hacker character image."""
        pose = character["pose"]
        color = character["color"]
        color_hex = character["color_hex"]

        if pose == "walking":
            pose_desc = (
                "standing/walking pose, facing slightly to the right at a 3/4 angle, "
                "one arm slightly forward and one slightly back as if mid-stride"
            )
            prop_desc = "no props in hands, hands visible at sides"
        else:
            pose_desc = (
                "crouching/kneeling pose, hunched forward, "
                "looking down at a small laptop computer held in front"
            )
            prop_desc = "holding a small open laptop with a glowing screen"

        prompt = f"""Generate a character sprite for a game. Use the attached reference image as the 
EXACT art style to match — same chunky bean-shaped body proportions, same flat vector/illustration 
style, same viewing angle (3/4 top-down), same level of detail and shading.

THIS IS A HACKER CHARACTER, NOT A ROBBER. Key visual differences from the reference:
- Wears a {color} HOODIE ({color_hex}) with the hood pulled up (NOT a ski mask or beanie)
- The hood frames the face — you can see the character's eyes peeking out from under the hood
- Body color is {color} ({color_hex}) instead of dark gray/black
- {prop_desc}

POSE: {pose_desc}

CRITICAL REQUIREMENTS:
- Transparent/white background (NO ground, NO shadow on ground)
- Same chunky, rounded, bean-like body shape as the reference character
- Same simplified cartoon face style (simple dot/oval eyes)
- Character should be clearly recognizable as a DIFFERENT character from the robber
- The hoodie is the defining visual feature — it should be prominent and obvious
- High resolution, clean edges, suitable for use as a game sprite
- The character should fill most of the frame, centered

Generate a single character sprite matching these specifications."""

        return prompt

    def generate_character(self, character: Dict) -> bool:
        """Generate a single hacker character image."""
        try:
            print(f"\n{'='*70}")
            print(f"CHARACTER: {character['name']}")
            print(f"  Pose: {character['pose']}, Color: {character['color']}")
            print(f"{'='*70}")

            prompt = self.build_prompt(character)

            if self.verbose:
                print(f"\nPrompt:\n{prompt}\n")
            else:
                print(f"\nPrompt preview: {prompt[:200]}...\n")

            if self.dry_run:
                print("[DRY RUN] Skipping API call")
                self.stats["skipped"] += 1
                return True

            ref_path = self.characters_dir / character["reference"]
            if not ref_path.exists():
                print(f"  Error: reference image not found: {ref_path}")
                self.stats["failed"] += 1
                return False

            print(f"Loading reference: {character['reference']}")
            reference_image = Image.open(ref_path)

            print("Calling Gemini API...")
            content_parts = [
                prompt,
                "Reference image (match this EXACT art style, but make a hacker with a hoodie):",
                reference_image,
            ]

            response = self.model.generate_content(content_parts)

            if not response or not response.candidates:
                print("  No response from API")
                self.stats["failed"] += 1
                return False

            candidate = response.candidates[0]
            if not candidate.content or not candidate.content.parts:
                print("  No content in response")
                self.stats["failed"] += 1
                return False

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
                print("  No image data in response")
                if hasattr(response, 'text'):
                    print(f"  Response text: {response.text[:300]}")
                self.stats["failed"] += 1
                return False

            output_file = self.characters_dir / character["filename"]
            img = Image.open(io.BytesIO(image_data))
            img.save(output_file, "PNG")

            print(f"  Saved: {output_file} ({img.size[0]}x{img.size[1]})")
            self.stats["success"] += 1

            time.sleep(2)
            return True

        except Exception as e:
            print(f"  Error generating character: {e}")
            import traceback
            traceback.print_exc()
            self.stats["failed"] += 1
            return False

    def generate_all(self, character_ids: Optional[List[int]] = None) -> Dict:
        """Generate multiple characters."""
        if character_ids is None:
            characters = CHARACTERS
        else:
            characters = [c for c in CHARACTERS if c["id"] in character_ids]

        print(f"{'='*70}")
        print(f"HACKER CHARACTER GENERATION {'(DRY RUN)' if self.dry_run else ''}")
        print(f"{'='*70}")
        print(f"Characters to generate: {len(characters)}")
        print(f"Output directory: {self.characters_dir}")
        print()

        for character in characters:
            self.generate_character(character)

        self._print_summary()
        return self.stats

    def _print_summary(self):
        """Print generation summary."""
        print(f"\n{'='*70}")
        print("SUMMARY")
        print(f"{'='*70}")
        print(f"  Success: {self.stats['success']}")
        print(f"  Failed:  {self.stats['failed']}")
        print(f"  Skipped: {self.stats['skipped']}")
        print(f"{'='*70}\n")


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Generate hacker character sprites using Gemini AI",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python generate_characters_ai.py --all               # Generate all 4 hackers
  python generate_characters_ai.py --character 0 1     # Specific characters
  python generate_characters_ai.py --all --dry-run     # Preview prompts only

Characters:
  0: HackerWalkingGreen   (green hoodie, walking)
  1: HackerWalkingPurple  (purple hoodie, walking)
  2: HackerHackingGreen   (green hoodie, crouched with laptop)
  3: HackerHackingPurple  (purple hoodie, crouched with laptop)

API Authentication:
  Set GOOGLE_API_KEY or GEMINI_API_KEY environment variable
        """,
    )

    parser.add_argument(
        "--character",
        type=int,
        nargs="+",
        metavar="ID",
        help="Generate specific character(s) by ID (0-3)",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Generate all 4 hacker characters",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Preview prompts without calling Gemini API",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Show full prompts (not truncated)",
    )
    parser.add_argument(
        "--model",
        type=str,
        metavar="MODEL",
        default=None,
        help=f"Gemini model name (default: {CharacterGenerator.DEFAULT_MODEL})",
    )
    parser.add_argument(
        "--api-key",
        type=str,
        metavar="API_KEY",
        help="Google AI Studio API key (defaults to env var)",
    )

    args = parser.parse_args()

    if not args.character and not args.all:
        parser.error("Must specify --character or --all")
    if args.character and args.all:
        parser.error("Cannot specify both --character and --all")

    character_ids = args.character if args.character else None
    if character_ids:
        for cid in character_ids:
            if cid < 0 or cid > 3:
                parser.error(f"Character ID {cid} out of range. Must be 0-3.")

    workspace_root = Path(__file__).parent.parent

    try:
        generator = CharacterGenerator(
            workspace_root=str(workspace_root),
            api_key=args.api_key,
            model=args.model,
            dry_run=args.dry_run,
            verbose=args.verbose,
        )
    except Exception as e:
        print(f"Error initializing generator: {e}")
        sys.exit(1)

    try:
        stats = generator.generate_all(character_ids=character_ids)
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
