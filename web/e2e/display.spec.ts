import { test, expect } from "@playwright/test";
import { gotoApp } from "./helpers";

// The Display ▾ popover consolidates the set-and-forget viewport toggles.

test("Display popover toggles persist and it closes on outside click", async ({
  page,
}) => {
  await gotoApp(page);

  await page.getByRole("button", { name: "Display" }).click();
  const panel = page.locator(".popover-panel");
  await expect(panel).toBeVisible();

  // Turn the grid and edges off.
  await panel.getByText("Grid & axes").click();
  await panel.getByText("Edge overlay").click();
  await panel.getByText("Orthographic projection").click();

  const stored = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("quito.prefs.v1") || "{}"),
  );
  expect(stored.showGrid).toBe(false);
  expect(stored.showEdges).toBe(false);

  // The trigger flags a non-default state.
  await expect(page.getByRole("button", { name: "Display" })).toHaveClass(
    /active/,
  );

  // Clicking outside closes the panel.
  await page.locator(".editor").click();
  await expect(panel).toHaveCount(0);
});
