import { test, expect } from "@playwright/test";
import { waitForRender } from "./helpers";

// The browser-only "get the desktop app" callout: it shows by default, and
// dismissing it is sticky via prefs so it doesn't reappear on the next visit.
test("desktop-app callout shows, then stays dismissed across reload", async ({
  page,
}) => {
  // Not gotoApp: its addInitScript clears localStorage on every load (incl.
  // reload), which would wipe the dismissal we're asserting persists. A fresh
  // Playwright context already starts with empty storage.
  await page.goto("/");
  await waitForRender(page);

  const callout = page.getByRole("status").filter({ hasText: "desktop app" });
  await expect(callout).toBeVisible();

  await callout.getByRole("button", { name: "Dismiss" }).click();
  await expect(callout).toBeHidden();

  await page.reload();
  await waitForRender(page);
  await expect(
    page.getByRole("status").filter({ hasText: "desktop app" }),
  ).toBeHidden();
});
