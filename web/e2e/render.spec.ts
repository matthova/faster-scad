import { test, expect } from "@playwright/test";
import { gotoApp, waitForRerender, renderRev, setEditor } from "./helpers";

// Behavioral coverage of the render loop — the contract renderState.ts's
// reduce/applyRenderEffects split must preserve (no screenshots, so it's stable
// across platforms and safe to gate in CI). This is the Phase 1a verification.
test.describe("render loop", () => {
  test("renders, reports an error, then recovers", async ({ page }) => {
    await gotoApp(page);

    // A clean default render: status bar green with a triangle count.
    await expect(page.locator(".statusbar")).not.toHaveClass(/err/);
    await expect(page.locator(".status-main")).toContainText("triangles");

    // Break the syntax and re-render: status goes red with the parse error, the
    // console auto-opens, and data-render-rev advances (a render landed).
    await waitForRerender(page, () => setEditor(page, "cube([10, 10, 10\n"));
    await expect(page.locator(".statusbar")).toHaveClass(/err/);
    await expect(page.locator(".console-filters")).toBeVisible();

    // Fix it: status returns to green and the count comes back.
    await waitForRerender(page, () => setEditor(page, "cube([10, 10, 10]);\n"));
    await expect(page.locator(".statusbar")).not.toHaveClass(/err/);
    await expect(page.locator(".status-main")).toContainText("triangles");
  });

  test("data-render-rev advances once per render", async ({ page }) => {
    await gotoApp(page);
    const before = await renderRev(page);
    await waitForRerender(page, () => setEditor(page, "sphere(6);\n"));
    expect(await renderRev(page)).toBeGreaterThan(before);
  });
});
