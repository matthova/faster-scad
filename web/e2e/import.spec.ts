import { test, expect } from "@playwright/test";
import { gotoApp, waitForRerender } from "./helpers";

// Drag-and-drop import of a local text file. A dropped .scad on the pristine
// default project becomes main and renders; a binary file surfaces a message.

async function drop(
  page: import("@playwright/test").Page,
  name: string,
  body: string,
) {
  const dt = await page.evaluateHandle(
    ({ name, body }) => {
      const dt = new DataTransfer();
      dt.items.add(new File([body], name, { type: "text/plain" }));
      return dt;
    },
    { name, body },
  );
  await page.locator(".app").dispatchEvent("dragover", { dataTransfer: dt });
  await page.locator(".app").dispatchEvent("drop", { dataTransfer: dt });
}

test("dropping a .scad loads and renders it", async ({ page }) => {
  await gotoApp(page);
  await waitForRerender(page, () => drop(page, "widget.scad", "sphere(12);\n"));
  await expect(page.locator(".tab", { hasText: "widget.scad" })).toBeVisible();
  await expect(page.locator(".editor")).toContainText("sphere(12)");
  await expect(page.locator(".status-integrity")).toHaveText("EXACT");
});

test("dropping a binary file surfaces a message", async ({ page }) => {
  await gotoApp(page);
  await drop(page, "part.3mf", "binary-ish");
  await expect(page.locator(".update-banner.error")).toContainText(
    "binary STL/3MF/PNG",
  );
});
