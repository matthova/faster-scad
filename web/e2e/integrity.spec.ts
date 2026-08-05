import { test, expect } from "@playwright/test";
import { gotoApp, waitForRender } from "./helpers";

// The render-integrity badge answers "is what I'm looking at the real thing?":
// EXACT for watertight geometry, FAST PREVIEW when unions are skipped.

test("integrity badge reflects the render path", async ({ page }) => {
  await gotoApp(page);
  const badge = page.locator(".status-integrity");
  await expect(badge).toHaveText("EXACT");

  // Fast preview: unions skipped, mesh not watertight.
  await page.getByRole("button", { name: "Fast" }).click();
  await waitForRender(page);
  await expect(badge).toHaveText("FAST PREVIEW");

  // Back to exact.
  await page.getByRole("button", { name: "Fast" }).click();
  await waitForRender(page);
  await expect(badge).toHaveText("EXACT");
});
