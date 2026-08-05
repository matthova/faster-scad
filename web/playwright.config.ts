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
    baseURL: "http://localhost:4173",
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,
    ...devices["Desktop Chrome"],
  },
  expect: {
    toHaveScreenshot: {
      // Chrome is deterministic on one machine; keep the tolerance tight enough
      // that adding/removing a single control is caught (a 1% ratio hid an
      // 80px-wide dropdown). A small absolute budget still absorbs text AA jitter.
      maxDiffPixels: 150,
      animations: "disabled",
    },
  },
  // Screenshot the built preview, not the dev server: the dev build is
  // unminified and injects the HMR client, so it isn't what ships. Requires a
  // prior `npm run build` (CI builds before the e2e step; locally, build first).
  webServer: {
    command: "npm run preview -- --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
