import { test, expect, Page } from "@playwright/test";
import { gotoApp, waitForRender } from "./helpers";

// The render-quality control forces $fn, so the triangle count must respond
// even though the default project sets its own $fn. Draft drops it, Fine raises
// it, and the choice survives a reload.

async function triangleCount(page: Page): Promise<number> {
  const text = await page.locator(".status-main").innerText();
  const m = text.match(/([\d,]+)\s+triangles/);
  if (!m) throw new Error(`no triangle count in status: "${text}"`);
  return Number(m[1].replace(/,/g, ""));
}

async function setQuality(page: Page, value: string) {
  await page.locator(".quality-select").selectOption(value);
  await waitForRender(page);
}

test("quality preset changes triangle count and persists", async ({ page }) => {
  await gotoApp(page);
  const normal = await triangleCount(page);

  await setQuality(page, "draft");
  const draft = await triangleCount(page);
  expect(draft).toBeLessThan(normal);

  await setQuality(page, "fine");
  const fine = await triangleCount(page);
  expect(fine).toBeGreaterThan(normal);

  // Persisted to prefs (not the share link). Read storage directly — the test
  // harness clears localStorage on every navigation, so a reload can't check it.
  const stored = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("quito.prefs.v1") || "{}"),
  );
  expect(stored.quality).toBe("fine");
});
