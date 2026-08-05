import { test, expect } from "@playwright/test";
import { gotoApp, waitForRender, waitForRerender, setEditor } from "./helpers";

async function triangles(
  page: import("@playwright/test").Page,
): Promise<number> {
  const txt = await page.locator(".statusbar").innerText();
  const m = txt.match(/([\d,]+)\s+triangles/);
  return m ? Number(m[1].replace(/,/g, "")) : 0;
}

// $fa/$fs custom-quality tolerances now have a UI. A finer $fa must tessellate a
// curve into more triangles. Use a sphere with no script-set $fn so the injected
// tolerance actually governs.
test("custom $fa tightens tessellation (more triangles)", async ({ page }) => {
  await gotoApp(page);
  await waitForRerender(page, () => setEditor(page, "sphere(20);\n"));
  const before = await triangles(page);
  expect(before).toBeGreaterThan(0);

  await page.getByRole("button", { name: /^Quality/ }).click();
  await page.getByRole("menuitemradio", { name: "Custom" }).click();
  const fa = page.getByRole("spinbutton", { name: /\$fa/ });
  await waitForRerender(page, async () => {
    await fa.fill("2"); // finer than the 12° default
  });
  expect(await triangles(page)).toBeGreaterThan(before);
});
