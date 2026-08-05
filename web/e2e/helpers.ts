import { Page, Locator, expect } from "@playwright/test";

/**
 * Load the playground with a clean slate and wait until the first render has
 * settled. `data-theme` follows the emulated `prefers-color-scheme`, so the
 * caller sets `colorScheme` on the context/test to pick light vs dark.
 */
export async function gotoApp(page: Page) {
  // Start from a clean localStorage so a saved project from a prior run can't
  // leak into the baseline. addInitScript runs before app code on every load.
  await page.addInitScript(() => {
    try {
      localStorage.clear();
    } catch {}
  });
  await page.goto("/");
  await waitForRender(page);
}

/** Wait for a successful render: the status bar shows its meta (dims · vol · ms). */
export async function waitForRender(page: Page) {
  await expect(page.locator(".status-meta")).toBeVisible({ timeout: 30_000 });
  // Let the one-shot auto-fit fly-to settle before we freeze the frame.
  await page.waitForTimeout(400);
}

/** Current render revision (bumps once per completed render). */
export async function renderRev(page: Page): Promise<number> {
  const v = await page.locator(".statusbar").getAttribute("data-render-rev");
  return Number(v ?? "0");
}

/** Run `action`, then wait for the next render to land (a new revision). Use
 *  when the meta is already visible, so its presence can't signal completion. */
export async function waitForRerender(page: Page, action: () => Promise<void>) {
  const before = await renderRev(page);
  await action();
  await expect
    .poll(() => renderRev(page), { timeout: 30_000 })
    .toBeGreaterThan(before);
}

/** The 3D canvas — masked in screenshots because WebGL output is not stable. */
export function viewerMask(page: Page): Locator {
  return page.locator(".viewer");
}

/** Replace the active editor's contents with `code` and wait for the result. */
export async function setEditor(page: Page, code: string) {
  const content = page.locator(".editor .cm-content");
  await content.click();
  const mod = process.platform === "darwin" ? "Meta" : "Control";
  await page.keyboard.press(`${mod}+a`);
  await page.keyboard.press("Delete");
  await content.pressSequentially(code);
}
