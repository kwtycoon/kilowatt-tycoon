import { test, expect, Page } from "@playwright/test";

// ── Bridge helpers (same read-only bridge as gameplay-flow) ─────────

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
  return `state=${s.app_state} day=${s.day_number} gt=${s.game_time.toFixed(0)} cash=${s.cash.toFixed(0)} scroll_y=${s.day_end_scroll_y?.toFixed(1) ?? "-"} els=${nEl}`;
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
    log(`Waiting for ${state}: ${bridgeSummary(s)}`);
    const remaining = deadline - Date.now();
    if (remaining <= 0) break;
    await page.waitForTimeout(Math.min(logIntervalMs, remaining));
  }
  const final_ = await bridge(page);
  if (final_?.app_state === state) return;
  throw new Error(
    `Timed out waiting for state=${state} after ${timeout}ms. Last: ${bridgeSummary(final_)}`,
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

async function tapCanvas(page: Page, x: number, y: number, holdMs = 2_000): Promise<void> {
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.waitForTimeout(holdMs);
  await page.mouse.up();
  await page.waitForTimeout(300);
}

async function skipTutorial(page: Page): Promise<void> {
  const state = await bridge(page);
  if (!state?.tutorial_step) return;
  await page.keyboard.press("Escape");
  await page.waitForTimeout(500);
}

// ── Drag gesture helper ─────────────────────────────────────────────

/**
 * Perform a pointer drag via Playwright's trusted mouse API.
 *
 * The scroll system reads from GamePointer (unified mouse/touch), so mouse
 * events work for both desktop and tablet testing.  Each step pauses long
 * enough for at least one Bevy frame to observe the position change.
 */
async function pointerDrag(
  page: Page,
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  steps = 10,
  stepDelayMs = 80,
): Promise<void> {
  await page.mouse.move(startX, startY);
  await page.mouse.down();
  await page.waitForTimeout(stepDelayMs);

  for (let i = 1; i <= steps; i++) {
    const t = i / steps;
    const cx = startX + (endX - startX) * t;
    const cy = startY + (endY - startY) * t;
    await page.mouse.move(cx, cy);
    await page.waitForTimeout(stepDelayMs);
  }

  await page.mouse.up();
}

// ── Test ─────────────────────────────────────────────────────────────

test.describe("Touch scroll on iPad", () => {
  test.setTimeout(480_000);

  test("day summary expanded view scrolls with single-finger touch drag", async ({
    page,
  }, testInfo) => {
    const vp = page.viewportSize()!;
    log(`Viewport: ${vp.width}×${vp.height}  project=${testInfo.project.name}  CI=${IS_CI}`);

    // This test exercises touch scrolling which needs a tablet-sized viewport.
    if (vp.width < 1000) {
      test.skip();
      return;
    }

    // ── 1. Splash → Play Now ──────────────────────────────────────
    await page.goto("/", { waitUntil: "domcontentloaded" });
    const playBtn = page.locator("button", { hasText: "PLAY NOW" });
    await expect(playBtn).toBeVisible({ timeout: 60_000 });
    await playBtn.click();
    log("Clicked PLAY NOW");

    await expect(page.locator("canvas")).toBeAttached({ timeout: 60_000 });
    await page.locator("canvas").click({ position: { x: 10, y: 10 }, force: true });
    await page.waitForTimeout(500);
    await waitForBridge(page, 60_000);
    log("Bridge ready");

    // ── 2. Character setup ────────────────────────────────────────
    await waitForElement(page, "NextButton", 60_000);
    await tapElement(page, "NextButton");
    await page.waitForTimeout(500);
    await page.keyboard.press("Enter");
    await page.waitForTimeout(1_000);

    // ── 3. Skip tutorial ──────────────────────────────────────────
    await waitForState(page, "Playing", 30_000);
    await page.waitForTimeout(500);
    await skipTutorial(page);
    log("Tutorial skipped");

    // ── 4. Place charger ──────────────────────────────────────────
    await tapElementUntil(
      page,
      "BuildTool_ChargerL2",
      "Select charger tool",
      (b) => b?.selected_build_tool === "ChargerL2",
    );

    const cashBefore = (await bridge(page))?.cash ?? 0;
    const chargerHint = await waitForElementPrefix(page, "PlacementHint_Charger_", 15_000);
    const chargerCx = chargerHint.rect.x + chargerHint.rect.width / 2;
    const chargerCy = chargerHint.rect.y + chargerHint.rect.height / 2;

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

    // ── 5. (Transformer skipped — L2 doesn't require one for validation)

    // ── 6. Start day, 10x speed, wait for DayEnd ─────────────────
    await tapElementUntil(
      page,
      "StartDayButton",
      "Start day",
      (b) => b != null && b.game_time > 0,
    );
    await tapElement(page, "SpeedButton_Fast");
    log("Day started at 10x speed");

    await waitForStateWithLogs(page, "DayEnd", 240_000, 10_000);
    log("Reached DayEnd state");

    // ── 7. Expand KPI section ────────────────────────────────────
    await tapElement(page, "KpiToggleButton");
    await page.waitForTimeout(1_000);
    log("Tapped KPI toggle to expand");

    // Shrink viewport height so the expanded content overflows the scroll area.
    // On large tablets the day-1 report fits without scrolling; a shorter
    // viewport reliably forces overflow, matching real-world scenarios with
    // more chargers and richer data.
    const originalVp = page.viewportSize()!;
    await page.setViewportSize({ width: originalVp.width, height: 500 });
    // Wait longer for Bevy to re-layout after viewport resize (CI frames can be >1s).
    await page.waitForTimeout(3_000);

    // Re-fetch scroll body rect after resize — layout coordinates have shifted.
    const scrollRect = await waitForElement(page, "DayEndScrollBody", 10_000);
    log(`Scroll body rect: x=${scrollRect.x.toFixed(0)} y=${scrollRect.y.toFixed(0)} w=${scrollRect.width.toFixed(0)} h=${scrollRect.height.toFixed(0)}`);

    // ── 8. Record initial scroll position ────────────────────────
    const before = await bridge(page);
    const scrollBefore = before?.day_end_scroll_y ?? 0;
    log(`Scroll position before touch drag: ${scrollBefore}`);

    // ── 9. Perform pointer drag (scroll down) ─────────────────────
    // Drag from lower region upward (finger/mouse up = content scrolls down).
    // Use longer step delays so slow CI frames (>1s) actually see movement
    // across multiple frames.
    const cx = scrollRect.x + scrollRect.width / 2;
    const dragStartY = scrollRect.y + scrollRect.height * 0.7;
    const dragEndY = scrollRect.y + scrollRect.height * 0.2;

    await pointerDrag(page, cx, dragStartY, cx, dragEndY, 10, IS_CI ? 300 : 80);
    log("Pointer drag complete");

    // Poll for scroll change — in CI the scroll update may be delayed.
    let scrollAfter = scrollBefore;
    const scrollDeadline = Date.now() + 10_000;
    while (Date.now() < scrollDeadline) {
      await page.waitForTimeout(500);
      const s = await bridge(page);
      scrollAfter = s?.day_end_scroll_y ?? 0;
      if (scrollAfter > scrollBefore) break;
    }
    log(`Scroll position after touch drag: ${scrollAfter}`);

    // ── 10. Verify scroll position changed ───────────────────────
    expect(scrollAfter).toBeGreaterThan(scrollBefore);
    log(`Touch scroll verified: ${scrollBefore.toFixed(1)} → ${scrollAfter.toFixed(1)}`);
  });
});
