import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { gotoApp, waitForRender, setEditor } from "./helpers";

// axe-core across four app states × both appearances. This is the automated
// half of Track E's quality floor: color-contrast is enforced now (Phase 0 fixed
// every violation), and the remaining known gaps are enumerated in KNOWN_GAPS
// with the rule Track E §8.2 fixes them under. Phase 4 removes rules from that
// list as each is fixed, so the gate tightens monotonically and can never
// silently regress a rule that's already clean (e.g. color-contrast).
const KNOWN_GAPS = [
  "label-title-only", // §8.2 — section/scrub ranges labelled by title only
  "region", // §8.2 — console/banners not yet wrapped in a landmark
  "scrollable-region-focusable", // §8.2 — editor/console scroll regions
];

async function scan(page: import("@playwright/test").Page) {
  const results = await new AxeBuilder({ page })
    .disableRules(KNOWN_GAPS)
    .analyze();
  expect(results.violations).toEqual([]);
}

for (const scheme of ["dark", "light"] as const) {
  test.describe(`a11y: ${scheme}`, () => {
    test.use({ colorScheme: scheme });

    test("default project", async ({ page }) => {
      await gotoApp(page);
      await scan(page);
    });

    test("an example loaded", async ({ page }) => {
      await gotoApp(page);
      await page
        .locator(".examples-select")
        .selectOption({ label: "Rounded box" });
      await waitForRender(page);
      await scan(page);
    });

    test("error state", async ({ page }) => {
      await gotoApp(page);
      await setEditor(page, "cube([10, 10, 10\n");
      await expect(page.locator(".statusbar.err")).toBeVisible({
        timeout: 30_000,
      });
      await scan(page);
    });

    test("command palette open", async ({ page }) => {
      await gotoApp(page);
      await page.keyboard.press("Meta+k");
      await expect(page.locator(".palette-input")).toBeVisible();
      await scan(page);
    });
  });
}
