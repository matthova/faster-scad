import { test, expect } from "@playwright/test";
import { waitForRender } from "./helpers";

const mod = process.platform === "darwin" ? "Meta" : "Control";

// Theme toggle (Auto/Light/Dark) — a forced choice must override the OS and
// survive a reload. Not gotoApp (its init script clears localStorage on reload).
test("forced theme overrides the OS and persists across reload", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/playground");
  await waitForRender(page);
  // Auto (default) follows the OS → light.
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

  // Force dark via the command palette.
  await page.keyboard.press(`${mod}+k`);
  await page.locator(".palette-input").fill("Theme: Dark");
  await page.keyboard.press("Enter");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

  // Persists despite the OS still being light.
  await page.reload();
  await waitForRender(page);
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});
