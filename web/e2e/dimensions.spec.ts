import { test, expect } from "@playwright/test";
import { gotoApp } from "./helpers";

// The signature dimension callouts toggle from Display ▾ (off by default) and
// persist. The callouts themselves are world-space three.js geometry, so their
// look is covered by the manual spike, not a pixel assertion.
test("dimension callouts toggle from Display and persist", async ({ page }) => {
  await gotoApp(page);

  // Off by default.
  let stored = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("openrscad.prefs.v1") || "{}"),
  );
  expect(stored.showDims ?? false).toBe(false);

  await page.getByRole("button", { name: "Display" }).click();
  await page.locator(".popover-panel").getByText("Dimensions").click();

  stored = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("openrscad.prefs.v1") || "{}"),
  );
  expect(stored.showDims).toBe(true);

  // The canvas is still rendering (no crash from the extra scene group).
  await expect(page.locator(".status-integrity")).toHaveText("EXACT");
});
