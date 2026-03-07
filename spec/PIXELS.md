# Pixel Iteration Notes

## Problem

Small visual tuning changes are currently expensive to iterate on.

Typical examples:

- Moving an overlay by a few pixels
- Nudging rotation to better match a sprite roof angle
- Testing whether text should sit above, on, or beside a prop
- Comparing one-digit vs `kW` label treatments

Right now, even tiny presentation tweaks usually require:

1. Editing Rust constants or transform code
2. Waiting for Rust compilation
3. Re-launching or re-checking the scene
4. Taking another screenshot and repeating

That loop is too slow for pixel work. Pixel polish needs a much tighter feedback cycle than normal gameplay/system changes.

## Goal

Make visual/layout iteration fast enough that we can tune overlays, labels, bars, offsets, and rotations in a few seconds instead of full code-edit cycles.

## Ways To Improve Future Cycles

### 1. Move Pixel-Tuning Values Into Data

Put overlay-specific values into external data instead of hard-coding them in Rust.

Examples:

- per-prop text offsets
- per-prop bar offsets
- bar widths/heights
- rotation angles
- font sizes
- anchor presets

Good formats:

- `ron`
- `toml`
- `json`

If those files are asset-loaded or dev-reloaded, we can tweak numbers without recompiling gameplay code.

## 2. Add A Dev Overlay Tuning Mode

Create a debug mode for selected props that shows:

- local origin
- anchor point
- overlay bounds
- current X/Y offset
- current rotation
- current font size / bar size

Useful controls:

- arrow keys to nudge X/Y
- `[` and `]` to rotate
- `-` and `=` to change scale/font
- copy current values to the clipboard or log them in a paste-ready format

That turns screenshot-based guesswork into direct manipulation.

## 3. Separate Presentation Constants From Simulation Logic

The more layout code is mixed into scene spawning logic, the slower it is to reason about and tweak.

We should keep:

- simulation math in gameplay systems
- visual placement constants in a dedicated layout/config area
- canopy/sprite-specific overlay rules in one place

That makes “move the bar left 4 px” a single obvious edit instead of a hunt through spawn code.

## 4. Support Hot Reload For Visual Specs

For assets and overlay layout rules, hot reload is ideal.

A good target workflow:

1. Edit a spec/config file
2. Save
3. Scene updates automatically
4. Re-check immediately

Even if true hot reload is not available everywhere, a cheap “reload visual layout config” dev shortcut would help a lot.

## 5. Add A Screenshot/Reference Workflow

Pixel work gets easier when comparisons are consistent.

Useful improvements:

- fixed camera screenshot command
- known test scene for charger/canopy variants
- side-by-side before/after captures
- named reference shots for “good” alignment

That would reduce subjective iteration and make it easier to spot whether a tweak actually improved alignment.

## 6. Prefer Narrow Validation For Visual-Only Changes

When changing only presentation constants, the validation path should be as narrow as possible.

Examples:

- fast compile target for scene/UI code paths
- targeted tests for layout helpers
- lightweight dev launch path for one site / one prop setup

This does not replace full validation, but it makes the first few feedback loops much faster.

## Recommended Direction

The highest-leverage improvement is:

1. move prop overlay constants into reloadable data
2. add a small dev tuning mode for offsets/rotation
3. keep a fixed screenshot scene for visual regression checks

That combination would make roof/bar/text alignment work dramatically faster than recompiling Rust for every tiny pixel adjustment.
