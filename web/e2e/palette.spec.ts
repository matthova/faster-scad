import { test, expect } from "@playwright/test";
import { gotoApp, waitForRerender } from "./helpers";

const mod = process.platform === "darwin" ? "Meta" : "Control";

test("command palette opens, filters, and runs a command", async ({ page }) => {
  await gotoApp(page);

  await page.keyboard.press(`${mod}+k`);
  await expect(page.locator(".palette")).toBeVisible();

  await page.locator(".palette-input").fill("console");
  await expect(page.locator(".palette-item")).toHaveCount(1);
  await page.keyboard.press("Enter"); // run "Toggle console"
  await expect(page.locator(".palette")).toHaveCount(0);
  await expect(page.locator(".console")).toBeVisible();
});

test("⌘K toggles, Escape closes", async ({ page }) => {
  await gotoApp(page);
  await page.keyboard.press(`${mod}+k`);
  await expect(page.locator(".palette")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.locator(".palette")).toHaveCount(0);
});

test("⌘↵ renders and does NOT insert a blank line in the editor", async ({
  page,
}) => {
  await gotoApp(page);
  const content = page.locator(".editor .cm-content");
  await content.click();
  // Cursor at doc start; ⌘↵ must render, not insert a blank line.
  await page.keyboard.press(`${mod}+Home`);
  const before = await content.innerText();
  await waitForRerender(page, () => page.keyboard.press(`${mod}+Enter`));
  const after = await content.innerText();
  expect(after).toBe(before); // no blank line inserted
});
