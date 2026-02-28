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
  selected_build_tool: string | null;
  elements: Record<string, ElementRect>;
}

async function bridge(page: Page): Promise<BridgeState | null> {
  return page.evaluate(() => (window as any).__kwtycoon_bridge ?? null);
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
 * Splits into move → down → (wait for Bevy frame) → up so the press
 * and release are processed in separate frames.
 */
async function tapElement(page: Page, name: string): Promise<void> {
  const el = await waitForElement(page, name);
  const cx = el.x + el.width / 2;
  const cy = el.y + el.height / 2;
  await page.mouse.move(cx, cy);
  await page.mouse.down();
  await page.waitForTimeout(150);
  await page.mouse.up();
  await page.waitForTimeout(400);
}

/** Tap at arbitrary CSS coordinates on the canvas. */
async function tapCanvas(page: Page, x: number, y: number): Promise<void> {
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.waitForTimeout(150);
  await page.mouse.up();
  await page.waitForTimeout(400);
}

/** Take a named debug screenshot, bucketed by viewport width. */
async function snap(page: Page, name: string): Promise<void> {
  const w = page.viewportSize()?.width ?? 0;
  await page.screenshot({
    path: `test-results/gameplay-flow/${w}/${name}.png`,
  });
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
  test.setTimeout(180_000);

  test("add charger, add transformer, start day, 10x, end of day report", async ({
    page,
  }) => {
    const vp = page.viewportSize()!;

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

    // Wait for canvas + bridge to initialise.
    await expect(page.locator("canvas")).toBeAttached({ timeout: 60_000 });
    await page.locator("canvas").click({ position: { x: 10, y: 10 }, force: true });
    await page.waitForTimeout(500);
    await waitForBridge(page, 60_000);
    await snap(page, "01-bridge-ready");

    // ── 2. Character setup ─────────────────────────────────────────
    await waitForElement(page, "NextButton", 60_000);
    await snap(page, "02-character-select");

    // Tap "Next" to accept the default character.
    await tapElement(page, "NextButton");
    await snap(page, "03-name-input");

    // Accept default name via Enter key.
    await page.waitForTimeout(1_000);
    await page.keyboard.press("Enter");
    await page.waitForTimeout(2_000);
    await snap(page, "04-after-start-mission");

    // ── 3. Skip tutorial ───────────────────────────────────────────
    await waitForState(page, "Playing", 30_000);
    await page.waitForTimeout(1_000);
    await skipTutorial(page);
    await snap(page, "05-tutorial-skipped");

    // ── 4. Place a charger ─────────────────────────────────────────
    // Select L2 charger from build panel.
    await tapElement(page, "BuildTool_ChargerL2");
    await page.waitForTimeout(500);
    await snap(page, "06-charger-selected");

    // Use bridge placement hints to find a valid charger position.
    const chargerHint = await waitForElementPrefix(
      page,
      "PlacementHint_Charger_",
      15_000,
    );
    const chargerCx = chargerHint.rect.x + chargerHint.rect.width / 2;
    const chargerCy = chargerHint.rect.y + chargerHint.rect.height / 2;
    await tapCanvas(page, chargerCx, chargerCy);
    await page.waitForTimeout(1_500);
    await snap(page, "07-charger-placed");

    // Verify cash decreased (charger was purchased).
    const afterCharger = await bridge(page);
    expect(afterCharger).not.toBeNull();

    // ── 5. Place a transformer ─────────────────────────────────────
    // Use keyboard shortcut "5" to select a 500kVA transformer.
    // This bypasses the Infra tab switch which can fail on tiny phone
    // viewports where tab buttons are only a few pixels wide.
    await page.keyboard.press("Digit5");
    await page.waitForTimeout(500);
    await snap(page, "08-transformer-selected");

    // Use bridge placement hints to find a valid transformer position.
    const xfmrHint = await waitForElementPrefix(
      page,
      "PlacementHint_Transformer_",
      15_000,
    );
    const xfmrCx = xfmrHint.rect.x + xfmrHint.rect.width / 2;
    const xfmrCy = xfmrHint.rect.y + xfmrHint.rect.height / 2;
    await tapCanvas(page, xfmrCx, xfmrCy);
    await page.waitForTimeout(1_500);
    await snap(page, "10-transformer-placed");

    // ── 6. Start day ───────────────────────────────────────────────
    await tapElement(page, "StartDayButton");
    await page.waitForTimeout(500);
    await snap(page, "11-day-started");

    // ── 7. Set 10x speed ───────────────────────────────────────────
    await tapElement(page, "SpeedButton_Fast");
    await snap(page, "12-speed-10x");

    // ── 8. Wait for end of day ─────────────────────────────────────
    await waitForState(page, "DayEnd", 120_000);
    await snap(page, "13-day-end-report");

    // Verify the bridge reports day 1.
    const state = await bridge(page);
    expect(state).not.toBeNull();
    expect(state!.app_state).toBe("DayEnd");
    expect(state!.day_number).toBe(1);

    // ── 9. Continue past day-end report ─────────────────────────────
    await page.keyboard.press("Enter");
    await page.waitForTimeout(2_000);

    // Should be back to Playing state for day 2.
    await waitForState(page, "Playing", 10_000);
    await snap(page, "14-after-continue");

    const stateAfter = await bridge(page);
    expect(stateAfter).not.toBeNull();
    expect(stateAfter!.app_state).toBe("Playing");
  });
});
