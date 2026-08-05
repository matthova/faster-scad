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

test("mobile toolbar dropdown escapes the scroll container", async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 800 });
  await gotoApp(page);

  await page.getByRole("button", { name: /Project/ }).click();
  const itemOverViewer = page.getByRole("menuitem", {
    name: "Download .scad",
  });
  await expect(itemOverViewer).toBeVisible();

  // Visibility alone does not catch overflow clipping. Verify an item that
  // overlaps the WebGL viewer wins hit-testing over the canvas beneath it.
  expect(
    await itemOverViewer.evaluate((item) => {
      const rect = item.getBoundingClientRect();
      const hit = document.elementFromPoint(
        rect.left + rect.width / 2,
        rect.top + rect.height / 2,
      );
      return hit === item || item.contains(hit);
    }),
  ).toBe(true);
});

test("Display controls remain reachable in mobile landscape", async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 375 });
  await gotoApp(page);

  await page.getByRole("button", { name: "Display" }).click();
  await page.getByRole("combobox", { name: "Quality" }).selectOption("custom");

  const panel = page.locator(".popover-panel");
  expect(
    await panel.evaluate((menu) => menu.getBoundingClientRect().bottom),
  ).toBeLessThanOrEqual(375);
  await page.getByRole("spinbutton", { name: /\$fs/ }).scrollIntoViewIfNeeded();
  await expect(page.getByRole("spinbutton", { name: /\$fs/ })).toBeVisible();
});
