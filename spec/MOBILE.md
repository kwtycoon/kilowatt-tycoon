# Mobile Interaction Spec

How touch input on iPad Safari flows through the engine to become button
presses, why we maintain a Bevy fork, and how Playwright E2E tests exercise
the full pipeline.

---

## 1. Touch Event Pipeline

A single finger tap on the iPad screen passes through five layers before a
Bevy UI button reads `Interaction::Pressed`.

```mermaid
flowchart TD
    A["Safari touchstart / touchmove / touchend"] --> B["winit web backend\n(canvas Pointer Events API)"]
    B --> C["winit WindowEvent::Touch"]
    C --> D["bevy_winit convert_touch_input()\n(logical pixels)"]
    D --> E["TouchInput message"]
    E --> F["bevy_input\ntouch_screen_input_system"]
    E --> G["bevy_picking\ntouch_pick_events"]
    F --> H["Res&lt;Touches&gt;"]
    G --> I["PointerInput\n(Press / Move / Release)"]
    H --> J["bevy_ui  ui_focus_system"]
    I --> K["bevy_ui  ui_picking\n(picking_backend.rs)"]
    J --> L["Interaction component\n(Pressed / Hovered / None)"]
    K --> M["PointerHits → HoverMap\n→ PickingInteraction"]
    L --> N["Game systems\nQuery Changed Interaction"]
```

### Layer by layer

#### 1a. Browser -> winit

Safari fires `touchstart`, `touchmove`, `touchend` on the `<canvas>`.
The winit web backend listens via the Pointer Events API, which unifies
mouse, touch, and pen. It translates each event into a
`winit::event::WindowEvent::Touch`.

#### 1b. winit -> Bevy (`bevy_winit`)

`bevy_winit/src/state.rs` (around line 343) receives `WindowEvent::Touch`,
converts the position to logical pixels via the window's scale factor, and
calls `convert_touch_input` (`bevy_winit/src/converters.rs`, line 47).
The result is a `TouchInput` struct with `phase` (`Started` / `Moved` /
`Ended` / `Canceled`), `position`, `window`, `force`, and `id`.

#### 1c. TouchInput fans out to two consumers

The `TouchInput` message is read by two independent systems:

| Consumer | Schedule | Output |
|----------|----------|--------|
| `bevy_input::touch::touch_screen_input_system` | `PreUpdate` | Updates `Res<Touches>` (pressed, just_pressed, just_released sets) |
| `bevy_picking::input::touch_pick_events` | `PreUpdate` | Spawns `PointerId::Touch(id)` entities, writes `PointerInput` (Press/Move/Release/Cancel) |

#### 1d. `ui_focus_system` (legacy path -- what buttons use)

`bevy_ui/src/focus.rs` runs every frame and determines the `Interaction`
component on each UI node:

1. **Cursor position**: `window.physical_cursor_position()` for mouse,
   falling back to `touches.first_pressed_position() * scale_factor` for
   touch.
2. **Press detection**: `mouse_button.just_pressed(Left) || touches.any_just_pressed()`.
3. **Release detection**: `mouse_button.just_released(Left) || touches.any_just_released()`.
4. **Hit test**: iterates the `UiStack` top-to-bottom; the first node whose
   rect contains the cursor (respecting clipping) gets
   `Interaction::Pressed` or `Interaction::Hovered`.

All game button handlers query `Changed<Interaction>`, so they respond
identically to mouse clicks and finger taps.

#### 1e. `ui_picking` (picking backend -- parallel path)

`bevy_picking::input::touch_pick_events` creates `PointerInput` events that
feed into `bevy_ui/src/picking_backend.rs` (`ui_picking`). This system
projects each pointer position into physical pixels (accounting for camera
scaling and `UiScale`), hit-tests the `UiStack`, and writes `PointerHits`.
Downstream, `generate_hovermap` and `update_interactions` turn hits into
`PickingInteraction`. The game currently relies on `Interaction` (from
`ui_focus_system`), not `PickingInteraction`, but both paths are active.

---

## 2. Code Considerations for Buttons and Interactables

### Unified pointer (`GamePointer`)

`src/helpers/pointer.rs` provides a single `Res<GamePointer>` updated in
`PreUpdate`. It collapses mouse and touch into one struct with
`screen_position`, `just_pressed`, `pressed`, `just_released`, and
`is_touch`.

Priority rules:

1. **2+ fingers** -- clear all state so two-finger camera gestures never
   accidentally place tiles or press buttons.
2. **Mouse cursor + left button** -- desktop path.
3. **Single finger** -- mobile/tablet path; position comes from
   `Res<Touches>`.

Systems that need raw world interaction (build placement, radial menu
dismiss) read `GamePointer` instead of querying mouse/touch separately.

### Touch-to-scroll (`ui_touch_scroll_system`)

`src/states/mod.rs` converts single-finger drags over any container with
`Overflow::scroll_y()` into `ScrollPosition` updates.

- On `just_pressed`, hit-tests scrollable containers and records the
  target entity.
- Accumulates vertical drag; ignores movement below an 8 px dead zone.
- Once scrolling is active, suppresses `pointer.just_pressed` and
  `pointer.just_released` so buttons underneath do not fire mid-scroll.
- On release, clears state and (if scrolling was active) swallows the
  `just_released` so the button under the finger does not activate.

### Two-finger camera (`touch_camera_system`)

`src/helpers/camera_controller.rs` reads `Res<Touches>` directly and only
activates when `touches.iter().count() >= 2`. Centroid delta drives camera
translation; finger-distance ratio drives pinch-to-zoom. Because
`GamePointer` suppresses during two-finger gestures, there is no conflict
with UI.

### CSS `touch-action: none`

`index.html` sets `touch-action: none` on the game canvas. This prevents
Safari from intercepting gestures for its own scroll, zoom, or
swipe-back-to-navigate behaviour. Without this, single-finger drags would
cause the page to scroll instead of reaching winit.

### Button query pattern

All buttons follow the same pattern and work identically on mouse and touch:

```rust
fn handle_my_button(
    query: Query<(&Interaction, &MyButton), Changed<Interaction>>,
) {
    for (interaction, button) in &query {
        if *interaction == Interaction::Pressed {
            // ...
        }
    }
}
```

This works because `ui_focus_system` unifies mouse and touch before setting
`Interaction`.

---

## 3. Why We Need the Bevy Fork

The project patches `bevy` and `bevy_ui` via `[patch.crates-io]` in
`Cargo.toml`, pointing at `https://github.com/j-white/bevy` (branch
`v0.17.3-patched`, commit `bece35a17`).

### The bug

When `UiScale` is not `1.0`, the `ui_picking` function in
`bevy_ui/src/picking_backend.rs` computed pointer position as:

```rust
let mut pointer_pos =
    pointer_location.position * camera_data.target_scaling_factor().unwrap_or(1.);
```

This omits the `UiScale` multiplier. On tablets where we set `UiScale` to
adapt the UI to the viewport, the picking backend's coordinate space
diverges from where `bevy_ui` actually renders nodes. The result: taps land
in the wrong place and buttons become unresponsive or require tapping an
offset position.

### The fix

The fork multiplies by `ui_scale.0`:

```rust
let mut pointer_pos = pointer_location.position
    * camera_data.target_scaling_factor().unwrap_or(1.)
    * ui_scale.0;
```

The function also reads `Res<crate::UiScale>` as a new parameter.

### Scope

The fork contains exactly one commit on top of Bevy `release-0.17.3`:

| File | Change |
|------|--------|
| `crates/bevy_ui/src/picking_backend.rs` | Multiply pointer position by `ui_scale.0` |
| `.cursorrules` | Cursor rules file (development convenience, no runtime effect) |

### Upstream status

This fix should be submitted as an upstream Bevy PR. Once merged into a
Bevy release, the `[patch.crates-io]` entries in `Cargo.toml` can be
removed.

---

## 4. Playwright E2E Test Strategy

### Read-only test bridge

`src/systems/test_bridge.rs` runs every frame in `PostUpdate` and writes a
JSON snapshot to `window.__kwtycoon_bridge`. The bridge is strictly
read-only -- it never mutates game state or injects input events. All test
interactions use real browser input (`page.mouse.move`, `page.mouse.down`,
`page.mouse.up`, `page.keyboard.press`).

### Element rect projection (`to_rect`)

For Bevy UI nodes (buttons, scroll bodies), the `to_rect` helper converts
Bevy's internal coordinate spaces to CSS pixels that Playwright can target.
Both `UiGlobalTransform.translation` and `ComputedNode::size()` are in
physical pixels; the bridge divides both by DPR
(`camera.target_scaling_factor()`) to produce true CSS-pixel rects:

```rust
fn to_rect(ugt: &UiGlobalTransform, cn: &ComputedNode, dpr: f32) -> ElementRect {
    let cx = ugt.translation.x / dpr;
    let cy = ugt.translation.y / dpr;
    let w = cn.size().x / dpr;
    let h = cn.size().y / dpr;
    ElementRect { x: cx - w / 2.0, y: cy - h / 2.0, width: w, height: h }
}
```

It is important that **both** position and size divide by the same DPR
value. An earlier version used `ComputedNode::inverse_scale_factor()`
(which is `1 / (DPR * UiScale)`) for size. This produced `Val::Px` units
instead of CSS pixels. The mismatch was invisible when `UiScale == 1.0`
(desktop) but broke offset-based clicking on viewports where UiScale
differs (phones, tablets, retina displays). Switching to `size() / dpr`
produces a true CSS bounding box and makes proportional click offsets work
identically across all viewports.

For world-space placement hints (charger pads, transformer slots), the
bridge uses `camera.world_to_viewport` which returns logical (CSS-pixel)
coordinates via `logical_viewport_rect`. No additional scaling is needed.

### SystemParam bundling

Bevy limits system functions to 16 parameters. The bridge system uses a
`#[derive(SystemParam)]` struct `UiElementQueries` to bundle 14 query
parameters into one, keeping the total system parameter count at 10.

### Interaction helpers

Tests cannot assume fast frame rates. In CI, debug WASM running on
SwiftShader can produce frames over 1 second long. Two helpers handle this:

| Helper | Behaviour |
|--------|-----------|
| `tapElement(name, holdMs=2000)` | Moves mouse to element center, holds down for 2 s, releases. Guarantees at least one Bevy frame sees `Interaction::Pressed`. |
| `tapElementUntil(name, check, timeout)` | Holds mouse down while polling `__kwtycoon_bridge` every 500 ms. Releases only when the `check` predicate returns true (e.g. `b.selected_build_tool === "ChargerL2"`). Retries with fresh press if the hold window expires. |

Both use `page.mouse` (pointer events), not touch events. On the web,
Chromium's pointer events flow into winit's canvas listener the same way
real touch events do, so this exercises the full pipeline.

### Scrolling

`page.mouse.wheel` does **not** work for Bevy UI scroll containers. Bevy's
`ui_touch_scroll_system` is driven by pointer/touch drag, not wheel events.
Tests scroll via multi-step pointer drags:

```typescript
await page.mouse.move(cx, startY);
await page.mouse.down();
for (let i = 1; i <= 10; i++) {
    await page.mouse.move(cx, startY + (endY - startY) * (i / 10));
    await page.waitForTimeout(80);
}
await page.mouse.up();
```

### Device profiles

`tests/e2e/playwright.config.ts` defines six projects:

| Project | Viewport | DPR | Notes |
|---------|----------|-----|-------|
| `desktop-chromium` | 1440 x 900 | 1 | Fast iteration baseline |
| `iphone-14-landscape` | 844 x 390 | 1 | Phone landscape |
| `pixel-7-landscape` | 915 x 412 | 1 | Phone landscape |
| `ipad-air-landscape` | 1180 x 820 | 1 | Tablet landscape |
| `ipad-air-landscape-retina` | 1180 x 820 | 2 | Real iPad DPR; catches coordinate scaling bugs |
| `iphone-14-portrait` | 390 x 844 | default | Portrait; verifies rotation prompt |

Most landscape projects use `deviceScaleFactor: 1` to keep CI fast --
SwiftShader (software GPU) frames already exceed 1 s at DPR 1.

The `ipad-air-landscape-retina` project uses the real iPad Air DPR of 2.
It runs the full test suite (gameplay flow, touch scroll, viewport checks)
at 4x the pixel count. This is slower but exercises the physical-to-logical
coordinate conversion in the picking pipeline -- the exact class of bug the
Bevy fork fixes. At DPR 1, `target_scaling_factor()` returns `1.0` and
incorrect scaling code is invisible. At DPR 2 the full multiplication chain
is exercised and a regression would cause button taps to miss their targets,
failing the gameplay flow test.

The portrait project uses the device default DPR to test the CSS media query
that shows the "Rotate Your Device" prompt.

### Test specs

| Spec file | What it tests |
|-----------|---------------|
| `mobile-viewport.spec.ts` | Splash page visible, Play button present, rotation prompt in portrait, canvas fills viewport after Play |
| `gameplay-flow.spec.ts` | Full gameplay loop: character setup, tutorial skip, place charger + transformer, start day, 10x speed, wait for DayEnd, day-end summary/expand/scroll/clock, continue to day 2, navigate to Locations panel, carousel browse, buy location 2, verify site switch |
| `touch-scroll.spec.ts` | Reaches DayEnd on iPad viewport, expands KPI section, performs pointer drag on scroll body, asserts `ScrollPosition.y` increased |

### CI integration

The `e2e-mobile` job in `.github/workflows/ci.yml`:

1. Builds the WASM bundle via `trunk build`.
2. Serves the static `dist/` directory on port 8080 with `npx serve`.
3. Runs all Playwright specs against headless Chromium with SwiftShader
   WebGL (`--use-gl=angle --use-angle=swiftshader`).
4. Uploads `test-results/` as an artifact on failure for debugging.

The job runs on every push and PR, using a single worker in CI (`workers: 1`)
with one retry.

---

## 5. Bridge Snapshot Reference

### Scalar fields

| Field | Type | Source |
|-------|------|--------|
| `app_state` | string | `AppState` enum variant |
| `tutorial_step` | string or null | Current tutorial step |
| `day_number` | u32 | `GameClock.day` |
| `cash` | f32 | `GameState.cash` |
| `game_time` | f32 | `GameClock.game_time` |
| `selected_build_tool` | string or null | Active `BuildTool` (null when Select) |
| `day_end_scroll_y` | f32 or null | `ScrollPosition.y` of day-end body |
| `num_owned_sites` | usize | `MultiSiteManager.owned_sites.len()` |
| `viewed_site_id` | u32 or null | `MultiSiteManager.viewed_site_id` |
| `carousel_index` | usize | `RentCarouselState.current_index` |

### Named UI elements

Each entry is a CSS-pixel rect `{ x, y, width, height }`.

| Element name | Component | When visible |
|--------------|-----------|--------------|
| `NextButton` | `NextButton` | Character setup |
| `StartButton` | `StartButton` | Character setup |
| `TutorialNextButton` | `TutorialNextButton` | Tutorial active |
| `TutorialSkipButton` | `TutorialSkipButton` | Tutorial active |
| `StartDayButton` | `StartDayButton` | Playing (build phase) |
| `SpeedButton_Normal` | `SpeedButton` | Playing (day running) |
| `SpeedButton_Fast` | `SpeedButton` | Playing (day running) |
| `SpeedButton_Paused` | `SpeedButton` | Playing (day running) |
| `DayEndContinueButton` | `DayEndContinueButton` | DayEnd |
| `KpiToggleButton` | `KpiToggleButton` | DayEnd |
| `DayEndScrollBody` | `DayEndScrollBody` | DayEnd |
| `BuildTool_{:?}` | `BuildToolButton` | Playing (always in sidebar) |
| `NavButton_Rent` | `PrimaryNavButton` | Playing (top nav bar) |
| `NavButton_Build` | `PrimaryNavButton` | Playing (top nav bar) |
| `NavButton_Strategy` | `PrimaryNavButton` | Playing (top nav bar) |
| `NavButton_Stats` | `PrimaryNavButton` | Playing (top nav bar) |
| `CarouselButton_Previous` | `CarouselButton` | Rent panel active (zero-size, see section 6) |
| `CarouselButton_Next` | `CarouselButton` | Rent panel active (zero-size, see section 6) |
| `RentSiteButton` | `RentSiteButton` | Rent panel active (zero-size, see section 6) |
| `RentPanel` | `RentPanel` | Rent panel active (container has valid size) |
| `SiteTab_{n}` | `SiteTab` | Playing, sorted by SiteId (zero-size after rebuild, see section 6) |
| `PlacementHint_Charger_{n}` | (world-space) | Valid charger positions |
| `PlacementHint_Transformer_{n}` | (world-space) | Valid transformer positions |

---

## 6. Bevy Deferred-Command Zero-Layout Bug

### Symptom

UI entities created via deferred commands (`commands.entity(parent).with_children(...)`)
report `ComputedNode::size() == Vec2::ZERO` and
`UiGlobalTransform::translation == Vec3::ZERO` permanently, even though
they render visually at the correct position. The zero layout persists
indefinitely across many frames.

This affects any UI element that is dynamically rebuilt at runtime via
deferred commands, regardless of whether the parent has `Display::Flex` or
`Display::None` at the time of creation.

### Root cause

Bevy's taffy-based layout system (`ui_layout_system`) runs in `PostUpdate`.
Entities created via deferred commands within `Update` are flushed before
`PostUpdate`, so they exist when layout runs. However, the layout system
appears not to incorporate newly created entities into its taffy layout tree
on the frame they appear. The `ComputedNode` is initialized to zero and
never recomputed on subsequent frames because the layout system only
recomputes nodes whose `Node` component is marked as changed.

Continuously destroying and recreating entities (e.g. `update_site_tabs`
rebuilds all tabs whenever `multi_site` changes) means the layout system
always sees just-created entities with zero `ComputedNode`, never the
ones that were laid out on the previous frame.

### Affected elements

| System | Component | Behaviour |
|--------|-----------|-----------|
| `update_rent_panel` | `CarouselButton`, `RentSiteButton` | Rebuilt when `multi_site`, `carousel`, or `game_state` changes |
| `update_site_tabs` | `SiteTab` | Rebuilt when `multi_site`, `game_state`, or `build_state` changes |

### Impact on E2E tests

1. `waitForElement` checks `width > 1 && height > 1`, so zero-size
   elements time out.
2. `ui_focus_system` hit-tests against `ComputedNode` rects. A zero-size
   node never matches any click position, so `Interaction::Pressed` is
   never set. Clicking at the correct screen coordinates has no effect.

### Workarounds in place

**Rent panel: dirty-flag deferred rebuild.** `update_rent_panel` uses a
`RentPanelDirty` resource to track when content needs refreshing. It only
spawns children when the panel has `Display::Flex` (checked via the
panel's `Node` component). This avoids creating entities under an invisible
parent, though the zero-layout bug still affects the created entities.

**Proportional offset clicking.** Since the `RentPanel` container entity
(created at startup, not via deferred commands) has valid `ComputedNode`
layout, the test uses it as an anchor. Carousel and rent button clicks are
placed at proportional CSS-pixel offsets from the panel rect:

- Carousel ">" button: 90% of panel width, 17% of panel height
- Rent button: 50% of panel width, 90% of panel height

These proportional offsets work across all viewports because `to_rect`
produces true CSS-pixel rects (both position and size divided by DPR).

**Site tab verification via bridge.** After buying a second location,
the game auto-switches to the new site. The test verifies this via the
bridge's `viewed_site_id` field rather than trying to click site tab
buttons (which also have zero-size `ComputedNode`).

### Potential upstream fixes

- File a Bevy bug: newly created UI entities should be added to the taffy
  layout tree and computed on the same frame they appear, or at minimum
  on the following frame.
- Alternative: use `Visibility::Hidden` instead of `Display::None` for
  panel toggling. `Visibility::Hidden` still computes layout. Downside:
  all panels participate in layout simultaneously.
- Alternative: stop destroying/recreating entities on every data change.
  Update existing entities in place so their `ComputedNode` is never lost.
