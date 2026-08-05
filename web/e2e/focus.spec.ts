import { test, expect } from "@playwright/test";
import { gotoApp } from "./helpers";

// Track E Phase 0 gate: keyboard focus must be visible. Before M9 there were
// zero focus styles in the app; this asserts the global :focus-visible ring
// actually renders on a real control after keyboard navigation, in both themes.
for (const scheme of ["light", "dark"] as const) {
  test(`keyboard focus shows a visible ring (${scheme})`, async ({ page }) => {
    await test.step("load", () => gotoApp(page));
    await page.emulateMedia({ colorScheme: scheme });

    // Tab from the document body onto the first interactive control. Keyboard
    // navigation is what triggers :focus-visible (a mouse click would not).
    await page.keyboard.press("Tab");

    const ring = await page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return null;
      const s = getComputedStyle(el);
      return {
        tag: el.tagName,
        width: parseFloat(s.outlineWidth),
        style: s.outlineStyle,
        color: s.outlineColor,
      };
    });

    expect(ring, "a control should be focused after Tab").not.toBeNull();
    expect(ring!.style).toBe("solid");
    expect(ring!.width).toBeGreaterThanOrEqual(2);
    // A real colour, not the transparent/`invert` default.
    expect(ring!.color).toMatch(/^rgb/);
  });
}
