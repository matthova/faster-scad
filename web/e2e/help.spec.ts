import { test, expect } from "@playwright/test";
import { gotoApp } from "./helpers";

test("help sheet opens from ?, lists shortcuts, closes on Escape", async ({
  page,
}) => {
  await gotoApp(page);
  await page.getByRole("button", { name: "Help" }).click();

  const help = page.locator(".help");
  await expect(help).toBeVisible();
  await expect(help).toContainText("Command palette");
  await expect(help).toContainText("Navigation cube");
  await expect(help).toContainText("BOSL2");

  await page.keyboard.press("Escape");
  await expect(help).toHaveCount(0);
});
