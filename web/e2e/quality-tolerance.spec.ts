import { test, expect } from "@playwright/test";
import { gotoApp, setEditor, waitForRerender } from "./helpers";

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
  // waitForRerender guarantees the edit replaced the default render; the poll
  // then rides out intermediate renders (an incomplete, error-y "sphere(2" lands
  // first on a slow runner) until a valid sphere count settles.
  await waitForRerender(page, () => setEditor(page, "sphere(20);\n"));
  await expect
    .poll(() => triangles(page), { timeout: 30_000 })
    .toBeGreaterThan(0);
  const before = await triangles(page);

  await page.getByRole("button", { name: "Display" }).click();
  await waitForRerender(page, () =>
    page.getByRole("combobox", { name: "Quality" }).selectOption("custom"),
  );
  const fa = page.getByRole("spinbutton", { name: /\$fa/ });
  await waitForRerender(page, () => fa.fill("2")); // finer than the 12° default
  await expect
    .poll(() => triangles(page), { timeout: 30_000 })
    .toBeGreaterThan(before);
});
