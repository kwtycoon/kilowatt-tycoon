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
 * Tap a named element, retrying until `check` returns true.
 * Throws if all attempts fail.
 */
async function tapElementUntil(
  page: Page,
  name: string,
  label: string,
  check: (b: BridgeState | null) => boolean,
  maxAttempts = 8,
): Promise<void> {
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    await tapElement(page, name);
    await page.waitForTimeout(500);
    const s = await bridge(page);
    const ok = check(s);
    log(`${label} attempt ${attempt}/${maxAttempts}: ok=${ok} ${bridgeSummary(s)}`);
    if (ok) return;
  }
  const final = await bridge(page);
  throw new Error(
    `${label}: all ${maxAttempts} tap attempts failed. Last: ${bridgeSummary(final)}`,
  );
}

/** Tap at arbitrary CSS coordinates on the canvas. */
async function tapCanvas(page: Page, x: number, y: number): Promise<void> {
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.waitForTimeout(150);
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
  }) => {
    const vp = page.viewportSize()!;
    log(`Viewport: ${vp.width}×${vp.height}  CI=${IS_CI}`);

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

    const chargerHint = await waitForElementPrefix(
      page,
      "PlacementHint_Charger_",
      15_000,
    );
    const chargerCx = chargerHint.rect.x + chargerHint.rect.width / 2;
    const chargerCy = chargerHint.rect.y + chargerHint.rect.height / 2;
    await tapCanvas(page, chargerCx, chargerCy);
    await page.waitForTimeout(1_000);
    await logBridge(page, "After charger placement");

    // ── 5. Place a transformer ─────────────────────────────────────
    await page.keyboard.press("Digit5");
    await page.waitForTimeout(500);

    const xfmrHint = await waitForElementPrefix(
      page,
      "PlacementHint_Transformer_",
      15_000,
    );
    const xfmrCx = xfmrHint.rect.x + xfmrHint.rect.width / 2;
    const xfmrCy = xfmrHint.rect.y + xfmrHint.rect.height / 2;
    await tapCanvas(page, xfmrCx, xfmrCy);
    await page.waitForTimeout(1_000);
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
    const state = await bridge(page);
    expect(state).not.toBeNull();
    expect(state!.app_state).toBe("DayEnd");
    expect(state!.day_number).toBe(1);
    log(`Day end verified: day=${state!.day_number} cash=${state!.cash.toFixed(0)}`);

    // ── 9. Continue past day-end report ─────────────────────────────
    await page.keyboard.press("Enter");
    await page.waitForTimeout(2_000);

    // Should be back to Playing state for day 2.
    await waitForState(page, "Playing", 10_000);

    const stateAfter = await bridge(page);
    expect(stateAfter).not.toBeNull();
    expect(stateAfter!.app_state).toBe("Playing");
    log("Test complete");
  });
});
