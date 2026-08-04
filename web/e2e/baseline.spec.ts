import { test, expect } from "@playwright/test";
import { gotoApp, waitForRender, viewerMask, setEditor } from "./helpers";

// The M8 safety net. Each shot masks the WebGL canvas and captures the chrome
// (topbar, tabs, editor, dock, console, status bar) so token/layout changes are
// caught while non-deterministic 3D pixels are ignored. Run in both appearances
// because light mode is designed separately (Phase 4) and must not regress.

for (const scheme of ["dark", "light"] as const) {
  test.describe(`appearance: ${scheme}`, () => {
    test.use({ colorScheme: scheme });

    test("default project", async ({ page }) => {
      await gotoApp(page);
      await expect(page).toHaveScreenshot(`default-${scheme}.png`, {
        mask: [viewerMask(page)],
      });
    });

    test("an example loaded", async ({ page }) => {
      await gotoApp(page);
      await page
        .locator(".examples-select")
        .selectOption({ label: "Rounded box" });
      await waitForRender(page);
      await expect(page).toHaveScreenshot(`example-${scheme}.png`, {
        mask: [viewerMask(page)],
      });
    });

    test("error state", async ({ page }) => {
      await gotoApp(page);
      await setEditor(page, "cube([10, 10, 10\n"); // unterminated call
      // Wait for the error to surface in the status bar.
      await expect(page.locator(".statusbar.err")).toBeVisible({
        timeout: 30_000,
      });
      await page.waitForTimeout(300);
      await expect(page).toHaveScreenshot(`error-${scheme}.png`, {
        mask: [viewerMask(page)],
      });
    });
  });
}
