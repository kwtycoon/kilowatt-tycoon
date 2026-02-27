Kilowatt Tycoon — Art Style Guide
1. Art Direction

Style: Vector-ish flat
Goal: Maximum readability, calm “management sim” feel, works at small scales
Inspiration: Two Point, Mini Metro, Infographic UI, modern SaaS dashboards

Key principles

Simple shapes > detail

Color communicates state

Everything readable at zoomed-out view

2. Camera & Perspective

True top-down (90°) — no isometric tilt

No visible sides of objects

Depth is implied only via soft shadow

3. Scale & Grid

Base tile size: 64 × 64 px

Road width: 1 tile

Parking spot: 1 × 2 tiles

Charger footprint:

L2: 1 × 1 tile

DCFC: 1 × 2 tiles

Cars/Vans: ~1.25 × 2 tiles

People: ~0.5 × 0.5 tile

Rule: Objects must visually “snap” to the grid.

4. Shapes & Lines

No outlines or ultra-thin only (≤1px at 64px)

Rounded corners (4–8px radius at 64px scale)

Avoid sharp angles unless functional (lightning bolt, warning icon)

5. Color Palette

Base palette

Asphalt: dark blue-gray

Concrete: warm light gray

Grass: muted green (no neon)

Chargers: neutral body + colored status light

Accent colors (states)

🟢 Available

🔵 Charging

🟡 Degraded / Warning

🔴 Offline / Fault

🟣 Upgrade / Subsidy

Rules

Flat fills only

Max 2 shades per object

No gradients (except shadows)

6. Lighting & Shadows

Single soft shadow under every object

Shadow color: black at ~10–15% opacity

Shadow offset: directly below (no directional lighting)

No highlights or reflections

7. Texture & Detail

No photoreal textures

Very subtle noise allowed only on large surfaces

Use decals (cracks, stains, snow patches) sparingly for variation

8. UI & Icons

Icons match world style (flat, filled)

No text baked into sprites

Icons designed at 48 × 48 px

UI panels: flat color + 1px soft shadow

9. Animation Guidelines

Minimal, functional animations

Examples:

Pulsing charger light

Cable sway (2–3 frames)

Snow accumulation over time

Avoid bouncy/cartoon motion

10. AI Generation Rules (IMPORTANT)

Every prompt must include:

“2D top-down game sprite”

“vector-ish flat style”

“no outlines”

“soft shadow underneath”

“transparent background”

“consistent scale with existing assets”

Never accept assets that:

Change perspective

Add gradients or realism

Break scale rules

11. Asset Export

Format: PNG

Background: transparent

Naming: category_object_state.png

Example: charger_dcfc_offline.png

Design mantra:

If it’s not readable at a glance, it doesn’t ship.
