import { test, expect } from "@playwright/test";
import { gotoApp, waitForRender, waitForRerender } from "./helpers";

// Phase 1b (usePref) guards. The failure mode usePref exists to prevent is a
// toggle that flips its state + persists but leaves the shadow ref stale, so the
// button lights up while every render stays exact — invisible without checking
// the actual render output.
test("Fast toggle actually switches the render to preview", async ({
  page,
}) => {
  await gotoApp(page);
  await expect(page.locator(".status-integrity")).toHaveText("EXACT");
  await page.getByRole("button", { name: "Display" }).click();
  await waitForRerender(page, () =>
    page.getByRole("button", { name: "Fast preview" }).click(),
  );
  // The render honoured fastPreviewRef.current → the worker returned a preview.
  await expect(page.locator(".status-integrity")).toHaveText("FAST PREVIEW");
});

test("orthographic projection persists across reload", async ({ page }) => {
  // Not gotoApp: its addInitScript clears localStorage on every load (incl.
  // reload). A fresh Playwright context already starts with empty storage.
  await page.goto("/playground");
  await waitForRender(page);

  await page.getByRole("button", { name: /^Display/ }).click();
  const ortho = page.getByRole("checkbox", {
    name: "Orthographic projection",
  });
  await ortho.check();
  await expect(ortho).toBeChecked();

  await page.reload();
  await waitForRender(page);
  await page.getByRole("button", { name: /^Display/ }).click();
  await expect(
    page.getByRole("checkbox", { name: "Orthographic projection" }),
  ).toBeChecked();
});
