import { test, expect } from "@playwright/test";
import { gotoApp } from "./helpers";

// Phase 2 (the signature): the Objects section lists the model's parts and
// clicking one isolates it in the viewport, with a retargeted readout.
test("Objects lists parts, isolates on click, and clears", async ({ page }) => {
  await gotoApp(page);
  const rows = page.locator(".objects-row");
  await expect(rows.first()).toBeVisible();
  expect(await rows.count()).toBeGreaterThanOrEqual(2);
  // Labels are sliced from the source (byte→char correct: not "unded_box").
  await expect(rows.first()).toContainText("rounded_box");

  // Isolate the first part: a readout appears and the row is marked active.
  await rows.first().click();
  await expect(page.locator(".objects-isolated")).toContainText("tris");
  await expect(page.locator(".objects-row.active")).toHaveCount(1);

  // "Show all" restores the whole model.
  await page.locator(".objects-clear").click();
  await expect(page.locator(".objects-isolated")).toHaveCount(0);
  await expect(page.locator(".objects-row.active")).toHaveCount(0);
});

test("Escape un-isolates", async ({ page }) => {
  await gotoApp(page);
  await page.locator(".objects-row").first().click();
  await expect(page.locator(".objects-isolated")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator(".objects-isolated")).toHaveCount(0);
});
