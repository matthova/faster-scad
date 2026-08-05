import { test, expect } from "@playwright/test";
import { gotoApp, waitForRender } from "./helpers";

// Phase 4: the topbar consolidates into group popovers and must not wrap at
// 1024px (the enforcing width). The transport moves below the canvas.
test("topbar is a single row at 1024px with ≤12 hit targets", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1024, height: 800 });
  await gotoApp(page);
  const box = await page.locator(".topbar").boundingBox();
  expect(box!.height).toBeLessThan(60); // ~48px one row, not the old 78–83px
  const targets = await page
    .locator(".actions button, .actions select")
    .count();
  expect(targets).toBeLessThanOrEqual(12);
});

test("Project ▾ groups the file actions", async ({ page }) => {
  await gotoApp(page);
  await page.getByRole("button", { name: /^Project/ }).click();
  await expect(
    page.getByRole("menuitem", { name: "New project" }),
  ).toBeVisible();
  await expect(
    page.getByRole("menuitem", { name: "Download .scad" }),
  ).toBeVisible();
});

test("Quality ▾ reveals $fn/$fa/$fs on Custom", async ({ page }) => {
  await gotoApp(page);
  await page.getByRole("button", { name: /^Quality/ }).click();
  await page.getByRole("menuitemradio", { name: "Custom" }).click();
  await expect(page.getByRole("spinbutton", { name: /\$fa/ })).toBeVisible();
  await expect(page.getByRole("spinbutton", { name: /\$fs/ })).toBeVisible();
});

test("Export split button shows the current format", async ({ page }) => {
  await gotoApp(page);
  await expect(page.getByRole("button", { name: /Export STL/ })).toBeVisible();
});

test("transport sits below the canvas and expands only for $t scripts", async ({
  page,
}) => {
  await gotoApp(page);
  // Default project has no $t → collapsed (no FPS/Steps fields).
  await expect(page.locator(".transport.collapsed")).toBeVisible();
  await expect(page.locator(".transport .anim-field")).toHaveCount(0);

  // A $t example expands it (turbine is self-contained — no CDN fetch).
  // loadExample confirms before replacing the project; accept it.
  page.on("dialog", (d) => d.accept());
  await page
    .locator(".examples-select")
    .selectOption({ label: "Animated turbine ($t)" });
  await waitForRender(page);
  await expect(page.locator(".transport.expanded")).toBeVisible();
  await expect(page.locator(".transport .anim-field").first()).toBeVisible();
});
