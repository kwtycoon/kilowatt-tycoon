import { defineConfig, devices } from "@playwright/test";

const BASE_URL = process.env.BASE_URL ?? "http://127.0.0.1:8080";

// How long to wait for the WASM module to initialise and render the splash page.
const WASM_LOAD_TIMEOUT_MS = 60_000;

export default defineConfig({
  testDir: "./specs",
  timeout: WASM_LOAD_TIMEOUT_MS * 2,
  expect: {
    timeout: 10_000,
    toMatchSnapshot: { maxDiffPixelRatio: 0.02 },
  },
  fullyParallel: true,
  workers: process.env.CI ? 1 : undefined,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: BASE_URL,
    actionTimeout: 15_000,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    // Enable WebGL rendering in headless Chromium via SwiftShader (software GPU).
    launchOptions: {
      args: [
        "--use-gl=angle",
        "--use-angle=swiftshader",
        "--enable-webgl",
        "--ignore-gpu-blocklist",
      ],
    },
  },

  projects: [
    // --- Phones (landscape) ---
    {
      name: "iphone-14-landscape",
      use: {
        ...devices["iPhone 14"],
        browserName: "chromium",
        viewport: { width: 844, height: 390 },
        // DPR 1 avoids physical/logical pixel mismatches in headless automation.
        deviceScaleFactor: 1,
      },
    },
    {
      name: "pixel-7-landscape",
      use: {
        ...devices["Pixel 7"],
        browserName: "chromium",
        viewport: { width: 915, height: 412 },
        // DPR 1 avoids physical/logical pixel mismatches in headless automation.
        deviceScaleFactor: 1,
      },
    },

    // --- Tablets (landscape) ---
    {
      name: "ipad-air-landscape",
      use: {
        ...devices["iPad (gen 7) landscape"],
        browserName: "chromium",
        viewport: { width: 1180, height: 820 },
        // DPR 1 avoids physical/logical pixel mismatches in headless automation.
        deviceScaleFactor: 1,
      },
    },

    // --- Portrait (should show rotation prompt) ---
    {
      name: "iphone-14-portrait",
      use: {
        ...devices["iPhone 14"],
        browserName: "chromium",
        // default iPhone 14 viewport is portrait (390x844)
      },
    },
  ],

  // Start trunk serve before the tests if no server is already running.
  webServer: {
    command: "trunk serve --port 8080",
    cwd: "../..",
    url: BASE_URL,
    reuseExistingServer: true,
    timeout: 120_000,
    stdout: "pipe",
    stderr: "pipe",
  },
});
