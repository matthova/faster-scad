import { defineConfig, devices } from "@playwright/test";

// Screenshot baselines are the M8 UI safety net: before any control moves, a
// handful of masked-chrome shots (default / example / error, in light + dark)
// must stay green. The 3D canvas is masked in the specs — WebGL output is not
// byte-stable across machines, and the point of these baselines is the chrome.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: process.env.CI ? "line" : "list",
  snapshotPathTemplate: "{testDir}/__screenshots__/{arg}{ext}",
  use: {
    baseURL: "http://localhost:5173",
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,
    ...devices["Desktop Chrome"],
  },
  expect: {
    toHaveScreenshot: {
      // Chrome is deterministic; allow a tiny slack for sub-pixel text AA so a
      // font-metrics hiccup on a different box doesn't fail the whole suite.
      maxDiffPixelRatio: 0.01,
      animations: "disabled",
    },
  },
  webServer: {
    command: "npm run dev -- --port 5173 --strictPort",
    url: "http://localhost:5173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
