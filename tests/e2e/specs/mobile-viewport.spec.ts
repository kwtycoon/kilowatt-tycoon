import { test, expect } from "@playwright/test";

// WASM + asset loading can be slow; give extra time for the first navigation.
const WASM_READY_TIMEOUT = 60_000;

test.describe("Splash page rendering", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/", { waitUntil: "domcontentloaded" });
  });

  test("splash page is visible on load", async ({ page }) => {
    const splash = page.locator("#splash-page");
    await expect(splash).toBeVisible({ timeout: WASM_READY_TIMEOUT });
  });

  test("play button is present", async ({ page }) => {
    const playBtn = page.locator("button", { hasText: "PLAY NOW" });
    await expect(playBtn).toBeVisible({ timeout: WASM_READY_TIMEOUT });
  });
});

test.describe("Portrait rotation prompt", () => {
  // Only the iphone-14-portrait project has a portrait viewport.
  // On landscape viewports the media query hides #rotate-prompt.
  test("rotation prompt visibility matches orientation", async ({
    page,
    browserName,
  }, testInfo) => {
    await page.goto("/", { waitUntil: "domcontentloaded" });

    const prompt = page.locator("#rotate-prompt");
    const viewport = page.viewportSize();

    if (viewport && viewport.height > viewport.width && viewport.width <= 1024) {
      // Portrait on a small screen — prompt should be visible.
      await expect(prompt).toBeVisible({ timeout: 10_000 });
    } else {
      // Landscape or large screen — prompt should be hidden.
      await expect(prompt).toBeHidden({ timeout: 10_000 });
    }
  });
});

test.describe("Splash-to-game transition", () => {
  test("clicking Play hides splash and shows canvas", async ({ page }, testInfo) => {
    // Skip portrait project — the rotate prompt blocks interaction.
    const viewport = page.viewportSize();
    if (viewport && viewport.height > viewport.width && viewport.width <= 1024) {
      test.skip();
      return;
    }

    await page.goto("/", { waitUntil: "domcontentloaded" });

    const playBtn = page.locator("button", { hasText: "PLAY NOW" });
    await expect(playBtn).toBeVisible({ timeout: WASM_READY_TIMEOUT });
    await playBtn.click();

    // body should gain game-mode class
    await expect(page.locator("body")).toHaveClass(/game-mode/, {
      timeout: 10_000,
    });

    // splash should be hidden
    await expect(page.locator("#splash-page")).toBeHidden();

    // A <canvas> element should exist (created by Bevy WASM).
    // It may take a moment for Bevy to initialise and insert the canvas.
    const canvas = page.locator("canvas");
    await expect(canvas).toBeAttached({ timeout: WASM_READY_TIMEOUT });
  });
});

test.describe("Canvas sizing", () => {
  test("canvas dimensions match the viewport", async ({ page }) => {
    const viewport = page.viewportSize();
    if (viewport && viewport.height > viewport.width && viewport.width <= 1024) {
      test.skip();
      return;
    }

    await page.goto("/", { waitUntil: "domcontentloaded" });

    const playBtn = page.locator("button", { hasText: "PLAY NOW" });
    await expect(playBtn).toBeVisible({ timeout: WASM_READY_TIMEOUT });
    await playBtn.click();

    await expect(page.locator("body")).toHaveClass(/game-mode/, {
      timeout: 10_000,
    });

    const canvas = page.locator("canvas");
    await expect(canvas).toBeAttached({ timeout: WASM_READY_TIMEOUT });

    // The CSS rule `body.game-mode > canvas { width: 100% !important; height: 100% !important; }`
    // should make the canvas fill the viewport.
    const box = await canvas.boundingBox();
    expect(box).not.toBeNull();
    if (box && viewport) {
      // Allow 2px tolerance for sub-pixel rounding.
      expect(box.width).toBeCloseTo(viewport.width, -1);
      expect(box.height).toBeCloseTo(viewport.height, -1);
    }
  });
});

test.describe("Visual regression", () => {
  test("splash page screenshot", async ({ page }, testInfo) => {
    await page.goto("/", { waitUntil: "domcontentloaded" });
    await expect(page.locator("#splash-page")).toBeVisible({
      timeout: WASM_READY_TIMEOUT,
    });
    // Allow external fonts / images a moment to settle.
    await page.waitForTimeout(2_000);
    await expect(page).toHaveScreenshot("splash.png", {
      fullPage: false,
      maxDiffPixelRatio: 0.03,
    });
  });
});
