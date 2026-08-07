import { test, expect } from "@playwright/test";
import { gotoApp } from "./helpers";

const stored = (page: import("@playwright/test").Page) =>
  page.evaluate(() =>
    JSON.parse(localStorage.getItem("openrscad.prefs.v1") || "{}"),
  );

test("section plane toggles, exposes axis controls, and persists", async ({
  page,
}) => {
  await gotoApp(page);
  await page.getByRole("button", { name: "Display" }).click();
  const panel = page.locator(".popover-panel");

  // Off by default: no axis controls.
  await expect(panel.locator(".section-controls")).toHaveCount(0);

  await panel.getByText("Section plane").click();
  await expect(panel.locator(".section-controls")).toBeVisible();
  expect((await stored(page)).sectionOn).toBe(true);

  // Switch the cut axis to X.
  await panel.locator(".section-axes button", { hasText: "X" }).click();
  expect((await stored(page)).sectionAxis).toBe("x");

  // Still rendering (clipping didn't break the pipeline).
  await expect(page.locator(".status-integrity")).toHaveText("EXACT");
});
