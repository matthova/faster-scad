import { test, expect } from "@playwright/test";
import { gotoApp } from "./helpers";

async function noOverflow(page: import("@playwright/test").Page) {
  return page.evaluate(
    () =>
      document.documentElement.scrollWidth <=
      document.documentElement.clientWidth,
  );
}

test("desktop (1280) keeps both panes, no pane switch", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await gotoApp(page);
  await expect(page.locator(".editor-col")).toBeVisible();
  await expect(page.locator(".viewer")).toBeVisible();
  await expect(page.locator(".pane-switch")).toBeHidden();
  expect(await noOverflow(page)).toBe(true);
});

for (const width of [820, 480]) {
  test(`narrow (${width}) uses the Code⎪Model switch, no overflow`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 800 });
    await gotoApp(page);

    // Core loop still renders (status meta present).
    await expect(page.locator(".status-meta")).toBeVisible();
    expect(await noOverflow(page)).toBe(true);

    // Default is the Model pane: viewer + customizer, editor hidden.
    await expect(page.locator(".viewer")).toBeVisible();
    await expect(page.locator(".editor-col")).toBeHidden();

    // Switch to Code: editor shows, viewer hides.
    await page.getByRole("tab", { name: "Code" }).click();
    await expect(page.locator(".editor-col")).toBeVisible();
    await expect(page.locator(".viewer")).toBeHidden();
    expect(await noOverflow(page)).toBe(true);
  });
}
