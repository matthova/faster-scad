import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

// The standalone marketing page (index.html → served at the site root
// /faster-scad/). It must render without the app bundle, expose a download CTA
// per OS, and route "other options" to the GitHub releases page.

test("about page renders hero, features, and per-OS downloads", async ({
  page,
}) => {
  await page.goto("/");

  await expect(
    page.getByRole("heading", { level: 1, name: /OpenSCAD/i }),
  ).toBeVisible();

  // All four stable per-OS download aliases are present and point at the
  // version-less latest-release URLs.
  const dl = "https://github.com/matthova/faster-scad/releases/latest/download";
  await expect(page.locator('.mk-os-card[data-os="mac-arm"]')).toHaveAttribute(
    "href",
    `${dl}/Quito-macos-aarch64.dmg`,
  );
  await expect(
    page.locator('.mk-os-card[data-os="mac-intel"]'),
  ).toHaveAttribute("href", `${dl}/Quito-macos-x64.dmg`);
  await expect(page.locator('.mk-os-card[data-os="windows"]')).toHaveAttribute(
    "href",
    `${dl}/Quito-windows-x64-setup.exe`,
  );
  await expect(page.locator('.mk-os-card[data-os="linux"]')).toHaveAttribute(
    "href",
    `${dl}/Quito-linux-x86_64.AppImage`,
  );

  // "Other download options" routes to the releases page.
  await expect(
    page.getByRole("link", { name: /other download options/i }),
  ).toHaveAttribute(
    "href",
    "https://github.com/matthova/faster-scad/releases/latest",
  );
});

test("shootout renders a per-model chart and data table", async ({ page }) => {
  await page.goto("/");

  // Headline stat tiles (static).
  await expect(page.locator(".mk-stat-num").first()).toHaveText("29×");

  // Chart: three engines (Quito, CGAL, Manifold) × five models = 15 bars, each
  // labelled with its render time. First bar is the boolean-grid Quito time.
  await expect(page.locator("#shootout-chart .mk-bar")).toHaveCount(15);
  await expect(
    page.locator("#shootout-chart .mk-bar-value").first(),
  ).toHaveText("53 ms");
  await expect(page.locator("#shootout-chart .mk-legend-item")).toHaveCount(3);

  // Table: header row + five model rows, with exact times.
  const rows = page.locator("#shootout-table tbody tr");
  await expect(rows).toHaveCount(5);
  await expect(rows.first()).toContainText("19 s"); // booleans CGAL time
  await expect(rows.first()).toContainText("53 ms"); // booleans quito time
});

for (const scheme of ["dark", "light"] as const) {
  test(`about page has no axe violations (${scheme})`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    const results = await new AxeBuilder({ page }).analyze();
    expect(results.violations).toEqual([]);
  });
}

test("primary CTA autodetects the OS and links a concrete installer", async ({
  page,
}) => {
  await page.goto("/");
  const primary = page.locator("#primary-download");

  // JS upgrades the button from the releases-page fallback to a concrete asset
  // (or the playground on a non-desktop device). Either way it must move off the
  // bare releases URL — assert it resolves to a real target.
  await expect
    .poll(async () => primary.getAttribute("href"))
    .not.toBe("https://github.com/matthova/faster-scad/releases/latest");

  const href = await primary.getAttribute("href");
  const ok =
    href === "playground" || // non-desktop → playground
    /releases\/latest\/download\/Quito-/.test(href ?? ""); // desktop installer
  expect(ok).toBe(true);
});
