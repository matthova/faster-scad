import { test, expect } from "@playwright/test";
import { gotoApp, setEditor, waitForRerender } from "./helpers";

test("Model section shows inspector data and updates live", async ({
  page,
}) => {
  await gotoApp(page);

  // Model section is visible alongside Parameters (sections, not tabs).
  const model = page.locator(".model-panel");
  await expect(model).toContainText("Triangles");
  await expect(model).toContainText("Vertices");
  await expect(model).toContainText("helpers.scad"); // resolved library

  // Reading Model while changing a Parameter — no tab switch. Nudge the first
  // slider (size) and the Model numbers must change.
  const before = await model.innerText();
  await waitForRerender(page, async () => {
    await page.locator('.params-body input[type="range"]').first().focus();
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
  });
  await expect(model).not.toHaveText(before);
});

test("dock collapses to a spine when the script has no parameters", async ({
  page,
}) => {
  await gotoApp(page);
  await setEditor(page, "cube([10, 20, 30]);\n");
  await page.waitForTimeout(600);

  // No params → the dock is a 28px spine, not a 288px hole.
  await expect(page.locator(".dock.collapsed")).toBeVisible();
  await expect(page.locator(".dock-spine-label")).toContainText("Parameters");

  // Clicking the spine expands it; Parameters then says there are none.
  await page.locator(".dock-spine").click();
  await expect(page.locator(".dock.collapsed")).toHaveCount(0);
  await expect(page.locator(".dock-empty")).toContainText("no parameters");
});
