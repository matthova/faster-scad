import { test, expect } from "@playwright/test";
import { gotoApp, setEditor } from "./helpers";

// A diagnostic that resolves to a source span is clickable and jumps the cursor
// there; echo output carries no span and must not even look clickable.

test("diagnostics with a span are clickable; echo is not", async ({ page }) => {
  await gotoApp(page);
  await setEditor(page, 'foo();\ncube(10);\necho("hello");\n');
  await page.waitForTimeout(600);

  // Open the console if it isn't already.
  if ((await page.locator(".console").count()) === 0) {
    await page.locator(".console-toggle").click();
  }

  const warning = page.locator("button.console-line.clickable", {
    hasText: "unknown module 'foo'",
  });
  await expect(warning).toBeVisible();

  // The echo line renders as a plain div, never a clickable button.
  await expect(
    page.locator("div.console-line.echo", { hasText: "hello" }),
  ).toBeVisible();
  await expect(
    page.locator("button.console-line", { hasText: "hello" }),
  ).toHaveCount(0);

  // Clicking the warning focuses the editor and selects its source span, so
  // CodeMirror renders a non-empty selection layer.
  await warning.click();
  await expect(page.locator(".cm-editor")).toHaveClass(/cm-focused/);
  await expect(page.locator(".cm-selectionBackground").first()).toBeVisible();
});
