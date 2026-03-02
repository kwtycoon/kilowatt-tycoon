import { test, expect, Page } from "@playwright/test";

// ── Bridge helpers ──────────────────────────────────────────────────
// The bridge is READ-ONLY. All interactions use real device input.

interface ElementRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface BridgeState {
  app_state: string;
  tutorial_step: string | null;
  day_number: number;
  cash: number;
  game_time: number;
  selected_build_tool: string | null;
  day_end_scroll_y: number | null;
  num_owned_sites: number;
  viewed_site_id: number | null;
  carousel_index: number;
  elements: Record<string, ElementRect>;
}

const IS_CI = !!process.env.CI;

function log(msg: string): void {
  const ts = new Date().toISOString().slice(11, 23);
  console.log(`[e2e ${ts}] ${msg}`);
}

async function bridge(page: Page): Promise<BridgeState | null> {
  return page.evaluate(() => (window as any).__kwtycoon_bridge ?? null);
}

function bridgeSummary(s: BridgeState | null): string {
  if (!s) return "bridge=null";
  const nEl = Object.keys(s.elements).length;
  return `state=${s.app_state} day=${s.day_number} gt=${s.game_time.toFixed(0)} cash=${s.cash.toFixed(0)} tool=${s.selected_build_tool ?? "-"} els=${nEl}`;
}

async function logBridge(page: Page, label: string): Promise<BridgeState | null> {
  const s = await bridge(page);
  log(`${label}: ${bridgeSummary(s)}`);
  return s;
}

async function waitForBridge(page: Page, timeout = 60_000): Promise<void> {
  await page.waitForFunction(
    () => (window as any).__kwtycoon_bridge != null,
    { timeout },
  );
}

async function waitForState(
  page: Page,
  state: string,
  timeout = 60_000,
): Promise<void> {
  await page.waitForFunction(
    (s) => (window as any).__kwtycoon_bridge?.app_state === s,
    state,
    { timeout },
  );
}

/**
 * Wait for a state, logging bridge status periodically so CI output
 * shows progress even when the wait is long.
 */
async function waitForStateWithLogs(
  page: Page,
  state: string,
  timeout: number,
  logIntervalMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const s = await bridge(page);
    if (s?.app_state === state) {
      log(`Reached state=${state} (gt=${s.game_time.toFixed(0)})`);
      return;
    }
    log(
      `Waiting for ${state}: ${bridgeSummary(s)}`,
    );
    const remaining = deadline - Date.now();
    if (remaining <= 0) break;
    await page.waitForTimeout(Math.min(logIntervalMs, remaining));
  }
  const final = await bridge(page);
  if (final?.app_state === state) return;
  throw new Error(
    `Timed out waiting for state=${state} after ${timeout}ms. Last: ${bridgeSummary(final)}`,
  );
}

async function waitForElement(
  page: Page,
  name: string,
  timeout = 30_000,
): Promise<ElementRect> {
  const el = await page.waitForFunction(
    (n) => {
      const e = (window as any).__kwtycoon_bridge?.elements?.[n];
      return e && e.width > 1 && e.height > 1 ? e : null;
    },
    name,
    { timeout },
  );
  return el.jsonValue() as Promise<ElementRect>;
}

/** Wait for any element matching a prefix (e.g. "PlacementHint_Charger_"). */
async function waitForElementPrefix(
  page: Page,
  prefix: string,
  timeout = 30_000,
): Promise<{ name: string; rect: ElementRect }> {
  const result = await page.waitForFunction(
    (pfx) => {
      const elems = (window as any).__kwtycoon_bridge?.elements;
      if (!elems) return null;
      for (const [key, val] of Object.entries(elems)) {
        const e = val as any;
        if (key.startsWith(pfx) && e.width > 1 && e.height > 1) {
          return { name: key, rect: e };
        }
      }
      return null;
    },
    prefix,
    { timeout },
  );
  return result.jsonValue() as Promise<{ name: string; rect: ElementRect }>;
}

/**
 * Tap the center of a named element using real pointer input.
 *
 * The hold time must exceed the longest possible Bevy frame duration
 * so that at least one frame sees the mouse held down and sets
 * `Interaction::Pressed`.  In CI (debug WASM + SwiftShader on large
 * viewports) frames can exceed 1 second, so we hold for 2s.
 */
async function tapElement(
  page: Page,
  name: string,
  holdMs = 2_000,
): Promise<void> {
  const el = await waitForElement(page, name);
  const cx = el.x + el.width / 2;
  const cy = el.y + el.height / 2;
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  await page.waitForTimeout(holdMs);
  await page.mouse.up();
  await page.waitForTimeout(300);
}

/**
 * Click at CSS coordinates with retries until the bridge condition is met.
 * Each attempt performs a full click (down → hold → up) so placement systems
 * that fire on release are covered. The hold duration spans multiple Bevy
 * frames to ensure the press is registered even at very low frame rates.
 */
async function tapCanvasUntil(
  page: Page,
  x: number,
  y: number,
  label: string,
  check: (b: BridgeState | null) => boolean,
  timeout = 30_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  let attempt = 0;
  while (Date.now() < deadline) {
    attempt++;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.waitForTimeout(2_000);
    await page.mouse.up();
    // Wait for the game to process the click and update the bridge.
    await page.waitForTimeout(1_000);

    const s = await bridge(page);
    if (check(s)) {
      log(`${label} succeeded on attempt ${attempt}: ${bridgeSummary(s)}`);
      return;
    }
    log(`${label} attempt ${attempt} still pending, retrying...`);
  }
  const final_ = await bridge(page);
  throw new Error(
    `${label}: failed after ${attempt} attempts (${timeout}ms). Last: ${bridgeSummary(final_)}`,
  );
}

/**
 * Tap a named element, keeping the mouse held down while polling the
 * bridge until `check` returns true.  This adapts to arbitrarily slow
 * frame rates — the mouse stays pressed until Bevy processes the frame
 * and the expected state change is observed.
 *
 * Re-fetches the element position on each attempt so layout shifts
 * between retries don't cause stale coordinates.
 */
async function tapElementUntil(
  page: Page,
  name: string,
  label: string,
  check: (b: BridgeState | null) => boolean,
  timeout = 60_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  let attempt = 0;

  while (Date.now() < deadline) {
    attempt++;
    const el = await waitForElement(page, name, Math.min(10_000, deadline - Date.now()));
    const cx = el.x + el.width / 2;
    const cy = el.y + el.height / 2;
    await page.mouse.move(cx, cy);
    await page.mouse.down();

    // Poll while holding — mouse stays pressed so any Bevy frame will
    // see Interaction::Pressed regardless of frame rate.
    const holdEnd = Math.min(Date.now() + 20_000, deadline);
    while (Date.now() < holdEnd) {
      await page.waitForTimeout(500);
      const s = await bridge(page);
      if (check(s)) {
        await page.mouse.up();
        log(`${label} succeeded on attempt ${attempt}: ${bridgeSummary(s)}`);
        return;
      }
    }
    await page.mouse.up();
    await page.waitForTimeout(300);
    log(`${label} attempt ${attempt} still pending, retrying...`);
  }

  const final_ = await bridge(page);
  throw new Error(
    `${label}: failed after ${attempt} attempts (${timeout}ms). Last: ${bridgeSummary(final_)}`,
  );
}

/** Tap at arbitrary CSS coordinates on the canvas. */
async function tapCanvas(page: Page, x: number, y: number, holdMs = 2_000): Promise<void> {
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.waitForTimeout(holdMs);
  await page.mouse.up();
  await page.waitForTimeout(300);
}

/**
 * Take a named debug screenshot. Skipped in CI to save time
 * (SwiftShader screenshots are slow and the artifacts are rarely useful).
 */
async function snap(page: Page, name: string): Promise<void> {
  if (IS_CI) return;
  try {
    const w = page.viewportSize()?.width ?? 0;
    await page.screenshot({
      path: `test-results/gameplay-flow/${w}/${name}.png`,
      timeout: 30_000,
    });
  } catch {
    // Debug screenshots are non-essential; swallow timeout/render errors.
  }
}

/**
 * Skip the tutorial regardless of what step it's on.
 * Escape now skips both modal and pointer steps.
 */
async function skipTutorial(page: Page): Promise<void> {
  const state = await bridge(page);
  if (!state?.tutorial_step) return;
  await page.keyboard.press("Escape");
  await page.waitForTimeout(500);
}

// ── Test ─────────────────────────────────────────────────────────────

test.describe("Full gameplay flow", () => {
  test.setTimeout(480_000);

  test("add charger, add transformer, start day, 10x, end of day report", async ({
    page,
  }, testInfo) => {
    const vp = page.viewportSize()!;
    log(`Viewport: ${vp.width}×${vp.height}  project=${testInfo.project.name}  CI=${IS_CI}`);

    // Skip portrait viewports — the rotate-device prompt blocks interaction.
    if (vp.height > vp.width && vp.width <= 1024) {
      test.skip();
      return;
    }

    // ── 1. Splash page → Play Now ──────────────────────────────────
    await page.goto("/", { waitUntil: "domcontentloaded" });
    const playBtn = page.locator("button", { hasText: "PLAY NOW" });
    await expect(playBtn).toBeVisible({ timeout: 60_000 });
    await playBtn.click();
    log("Clicked PLAY NOW");

    // Wait for canvas + bridge to initialise.
    await expect(page.locator("canvas")).toBeAttached({ timeout: 60_000 });
    await page.locator("canvas").click({ position: { x: 10, y: 10 }, force: true });
    await page.waitForTimeout(500);
    await waitForBridge(page, 60_000);
    await logBridge(page, "Bridge ready");
    await snap(page, "01-bridge-ready");

    // ── 2. Character setup ─────────────────────────────────────────
    await waitForElement(page, "NextButton", 60_000);

    // Tap "Next" to accept the default character.
    await tapElement(page, "NextButton");

    // Accept default name via Enter key.
    await page.waitForTimeout(500);
    await page.keyboard.press("Enter");
    await page.waitForTimeout(1_000);

    // ── 3. Skip tutorial ───────────────────────────────────────────
    await waitForState(page, "Playing", 30_000);
    await page.waitForTimeout(500);
    await skipTutorial(page);
    await logBridge(page, "Tutorial skipped");

    // ── 4. Place a charger ─────────────────────────────────────────
    await tapElementUntil(
      page,
      "BuildTool_ChargerL2",
      "Select charger tool",
      (b) => b?.selected_build_tool === "ChargerL2",
    );

    const cashBefore = (await bridge(page))?.cash ?? 0;
    const chargerHint = await waitForElementPrefix(
      page,
      "PlacementHint_Charger_",
      15_000,
    );
    const chargerCx = chargerHint.rect.x + chargerHint.rect.width / 2;
    const chargerCy = chargerHint.rect.y + chargerHint.rect.height / 2;

    // Retry click until bridge confirms the charger was placed (cash decreased).
    for (let placeAttempt = 0; placeAttempt < 10; placeAttempt++) {
      await tapCanvas(page, chargerCx, chargerCy);
      await page.waitForTimeout(1_000);
      const s = await bridge(page);
      if (s && s.cash < cashBefore) {
        log(`Charger placed on attempt ${placeAttempt + 1}: ${bridgeSummary(s)}`);
        break;
      }
      log(`Charger placement attempt ${placeAttempt + 1} pending...`);
    }
    await logBridge(page, "After charger placement");

    // ── 5. Place a transformer ─────────────────────────────────────
    await page.keyboard.press("Digit5");
    await page.waitForTimeout(500);

    const cashBeforeXfmr = (await bridge(page))?.cash ?? 0;
    const xfmrHint = await waitForElementPrefix(
      page,
      "PlacementHint_Transformer_",
      15_000,
    );
    const xfmrCx = xfmrHint.rect.x + xfmrHint.rect.width / 2;
    const xfmrCy = xfmrHint.rect.y + xfmrHint.rect.height / 2;

    for (let placeAttempt = 0; placeAttempt < 10; placeAttempt++) {
      await tapCanvas(page, xfmrCx, xfmrCy);
      await page.waitForTimeout(1_000);
      const s = await bridge(page);
      if (s && s.cash < cashBeforeXfmr) {
        log(`Transformer placed on attempt ${placeAttempt + 1}: ${bridgeSummary(s)}`);
        break;
      }
      log(`Transformer placement attempt ${placeAttempt + 1} pending...`);
    }
    await logBridge(page, "After transformer placement");

    // ── 6. Start day (retry until bridge confirms clock is running) ─
    await tapElementUntil(
      page,
      "StartDayButton",
      "Start day",
      (b) => b != null && b.game_time > 0,
    );

    // ── 7. Set 10x speed ───────────────────────────────────────────
    await tapElement(page, "SpeedButton_Fast");
    await logBridge(page, "After speed button");

    // ── 8. Wait for end of day (with periodic logging) ──────────────
    await waitForStateWithLogs(page, "DayEnd", 240_000, 10_000);

    // Verify the bridge reports day 1.
    const dayEndState = await bridge(page);
    expect(dayEndState).not.toBeNull();
    expect(dayEndState!.app_state).toBe("DayEnd");
    expect(dayEndState!.day_number).toBe(1);
    log(`Day end verified: day=${dayEndState!.day_number} cash=${dayEndState!.cash.toFixed(0)}`);
    await snap(page, "08-day-end-summary");

    // ── 9. See summary — verify key day-end elements are present ────
    await waitForElement(page, "DayEndContinueButton", 10_000);
    await waitForElement(page, "KpiToggleButton", 10_000);
    log("Day-end summary elements visible");

    // ── 10. Expand KPI view ─────────────────────────────────────────
    await tapElement(page, "KpiToggleButton");
    await page.waitForTimeout(1_000);
    log("Tapped KPI toggle to expand");
    await snap(page, "10-kpi-expanded");

    // ── 11. Scroll the day-end modal via pointer drag ─────────────
    const scrollEl = await waitForElement(page, "DayEndScrollBody", 10_000);
    const scrollCx = scrollEl.x + scrollEl.width / 2;
    const dragStartY = scrollEl.y + scrollEl.height * 0.7;
    const dragEndY = scrollEl.y + scrollEl.height * 0.2;

    const scrollBefore = (await bridge(page))?.day_end_scroll_y ?? 0;

    // Pointer drag: finger up = content scrolls down.
    // Longer step delays so slow CI frames actually see movement.
    const stepDelay = IS_CI ? 300 : 80;
    await page.mouse.move(scrollCx, dragStartY);
    await page.mouse.down();
    await page.waitForTimeout(stepDelay);
    const dragSteps = 10;
    for (let i = 1; i <= dragSteps; i++) {
      const t = i / dragSteps;
      await page.mouse.move(scrollCx, dragStartY + (dragEndY - dragStartY) * t);
      await page.waitForTimeout(stepDelay);
    }
    await page.mouse.up();
    await page.waitForTimeout(1_000);

    const scrollAfter = (await bridge(page))?.day_end_scroll_y ?? 0;
    log(`Scroll: ${scrollBefore.toFixed(1)} → ${scrollAfter.toFixed(1)}`);
    expect(scrollAfter).toBeGreaterThanOrEqual(scrollBefore);
    await snap(page, "11-scrolled");

    // ── 12. Verify clock / day info ─────────────────────────────────
    const clockState = await bridge(page);
    expect(clockState!.day_number).toBe(1);
    expect(clockState!.game_time).toBeGreaterThan(80_000);
    log(`Clock check: day=${clockState!.day_number} game_time=${clockState!.game_time.toFixed(0)}`);

    // ── 13. Continue past day-end report ────────────────────────────
    await tapElement(page, "DayEndContinueButton");
    await waitForState(page, "Playing", 30_000);
    const day2State = await bridge(page);
    expect(day2State!.app_state).toBe("Playing");
    expect(day2State!.num_owned_sites).toBe(1);
    log(`Continued to day 2: cash=${day2State!.cash.toFixed(0)} sites=${day2State!.num_owned_sites}`);
    await snap(page, "13-day2-playing");

    // ── 14. Go to Locations panel ───────────────────────────────────
    // Rent panel children (CarouselButton, RentSiteButton) spawned via
    // deferred commands report zero ComputedNode size due to a Bevy layout
    // quirk. We use the RentPanel container rect (which IS laid out) as
    // an anchor and click at relative offsets within it.
    await tapElement(page, "NavButton_Rent");
    await page.waitForTimeout(2_000);
    const panel = await waitForElement(page, "RentPanel", 15_000);
    log(`Locations panel visible: x=${panel.x.toFixed(0)} y=${panel.y.toFixed(0)} w=${panel.width.toFixed(0)} h=${panel.height.toFixed(0)}`);
    await snap(page, "14-locations-panel");

    // ── 15. Click Next carousel arrow ──────────────────────────────
    // Rent panel children have zero-size ComputedNode (Bevy deferred-
    // command layout bug), so we click at proportional offsets from the
    // RentPanel CSS rect. The ">" button sits at ~17% from the top and
    // near the right edge.
    const carouselNextX = panel.x + panel.width * 0.9;
    const carouselNextY = panel.y + panel.height * 0.17;

    await page.mouse.move(carouselNextX, carouselNextY);
    await page.mouse.down();
    await page.waitForTimeout(2_000);
    await page.mouse.up();
    await page.waitForTimeout(1_000);

    const carouselState = await bridge(page);
    expect(carouselState!.carousel_index).toBeGreaterThan(0);
    log(`Carousel advanced to index ${carouselState!.carousel_index}`);
    await snap(page, "15-location-2");

    // ── 16. Buy location 2 (Rent button in the card) ────────────────
    // The Rent button sits at ~90% from the panel top, centered.
    // We try a few proportional offsets to handle layout variation.
    const firstSiteId = day2State!.viewed_site_id;
    const rentBtnX = panel.x + panel.width * 0.5;
    const rentPcts = [0.90, 0.85, 0.95, 0.80];

    for (const pct of rentPcts) {
      const rentBtnY = panel.y + panel.height * pct;
      log(`Buy location: trying pct=${pct} y=${rentBtnY.toFixed(0)}`);
      await page.mouse.move(rentBtnX, rentBtnY);
      await page.mouse.down();
      await page.waitForTimeout(2_000);
      await page.mouse.up();
      await page.waitForTimeout(500);
      const s = await bridge(page);
      if (s && s.num_owned_sites === 2) {
        log(`Buy succeeded at pct=${pct}`);
        break;
      }
    }

    const afterBuy = await bridge(page);
    expect(afterBuy!.num_owned_sites).toBe(2);
    log(`Bought location 2: sites=${afterBuy!.num_owned_sites} cash=${afterBuy!.cash.toFixed(0)} viewed=${afterBuy!.viewed_site_id}`);
    await snap(page, "16-bought-location-2");

    // ── 17. Verify purchase and new site view ───────────────────────
    // Site tab entities also have the zero-layout bug, so we verify
    // ownership and view state via the bridge instead of clicking tabs.
    expect(afterBuy!.viewed_site_id).not.toBe(firstSiteId);
    log(`Verified: 2 sites owned, viewing new site (id=${afterBuy!.viewed_site_id})`);
    await snap(page, "17-verified");

    log("Test complete");
  });
});
