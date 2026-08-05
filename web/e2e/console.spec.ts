import { test, expect } from "@playwright/test";
import { gotoApp, setEditor } from "./helpers";

// The console drawer has severity filter chips.
test("console severity chips filter the lines", async ({ page }) => {
  await gotoApp(page);
  await setEditor(page, 'foo();\ncube(10);\necho("hello");\n');
  await page.waitForTimeout(600);
  if ((await page.locator(".console").count()) === 0) {
    await page.locator(".console-toggle").click();
  }

  // Both a warning and an echo line are present under "All".
  await expect(page.locator(".console-line.warn")).toHaveCount(1);
  await expect(page.locator(".console-line.echo")).toHaveCount(1);

  // Filtering to Echo hides the warning.
  await page.locator(".console-chip.echo").click();
  await expect(page.locator(".console-line.warn")).toHaveCount(0);
  await expect(page.locator(".console-line.echo")).toHaveCount(1);

  // Filtering to Warnings hides the echo.
  await page.locator(".console-chip.warn").click();
  await expect(page.locator(".console-line.echo")).toHaveCount(0);
  await expect(page.locator(".console-line.warn")).toHaveCount(1);
});
