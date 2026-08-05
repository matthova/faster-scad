import { test, expect } from "@playwright/test";
import { gotoApp, waitForRender, waitForRerender, renderRev } from "./helpers";

// The customizer override path (overrides + schema) is the state most touched by
// the renderState consolidation, and had no e2e. Loading a parametric example
// builds a schema; changing a slider must flow an override through to a render.
test("changing a customizer parameter re-renders", async ({ page }) => {
  await gotoApp(page);
  await page.locator(".examples-select").selectOption({ label: "Rounded box" });
  await waitForRender(page);

  // Schema parsed from the render → the customizer shows parameter rows.
  const slider = page.locator('.param-row input[type="range"]').first();
  await expect(slider).toBeVisible();

  const before = await renderRev(page);
  await waitForRerender(page, async () => {
    // A real user gesture so React's controlled-input onChange fires (setting
    // .value directly is swallowed by React's value tracker).
    await slider.focus();
    await slider.press("End"); // jump to the slider's max
  });
  // A render landed for the new override value, and the model is still valid.
  expect(await renderRev(page)).toBeGreaterThan(before);
  await expect(page.locator(".statusbar")).not.toHaveClass(/err/);
});
