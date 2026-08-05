import { test, expect, Page } from "@playwright/test";
import { gotoApp } from "./helpers";

// Dragging a splitter must resize the neighbouring panel AND re-fit the canvas
// (the ResizeObserver), so the render never stretches — this is the regression
// most likely to ship unnoticed. deviceScaleFactor is 1 (see config), so the
// canvas backing store should track its CSS width 1:1.

async function drag(page: Page, selector: string, dx: number, dy: number) {
  const box = await page.locator(selector).boundingBox();
  if (!box) throw new Error(`no box for ${selector}`);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(
    box.x + box.width / 2 + dx,
    box.y + box.height / 2 + dy,
    {
      steps: 8,
    },
  );
  await page.mouse.up();
}

async function canvasFit(page: Page) {
  return page.evaluate(() => {
    const c = document.querySelector(".viewer canvas") as HTMLCanvasElement;
    return {
      backing: c.width,
      css: Math.round(c.getBoundingClientRect().width),
    };
  });
}

test("resizing the editor re-fits the canvas (no stretch)", async ({
  page,
}) => {
  await gotoApp(page);
  const editor = page.locator(".editor-col");
  const before = (await editor.boundingBox())!.width;

  await drag(page, ".resize-x >> nth=0", 120, 0);

  const after = (await editor.boundingBox())!.width;
  expect(after).toBeGreaterThan(before + 80); // editor grew

  // The canvas backing store tracks its CSS size → the render isn't stretched.
  const fit = await canvasFit(page);
  expect(Math.abs(fit.backing - fit.css)).toBeLessThanOrEqual(2);
});

test("console height is resizable and persisted", async ({ page }) => {
  await gotoApp(page);
  await page.locator(".console-toggle").click(); // open console
  const consoleEl = page.locator(".console");
  const before = (await consoleEl.boundingBox())!.height;

  await drag(page, ".resize-y", 0, -80); // drag up → taller
  const after = (await consoleEl.boundingBox())!.height;
  expect(after).toBeGreaterThan(before + 50);

  const stored = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("quito.prefs.v1") || "{}"),
  );
  expect(stored.consoleHeight).toBeGreaterThan(before + 50);
});
