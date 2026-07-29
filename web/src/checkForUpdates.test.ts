import { describe, expect, it } from "vitest";
import { progressPct } from "./checkForUpdates";

describe("progressPct", () => {
  it("returns null when the total is unknown", () => {
    expect(progressPct(0, null)).toBeNull();
    expect(progressPct(500, 0)).toBeNull();
    expect(progressPct(500, -1)).toBeNull();
  });

  it("rounds to a whole percent", () => {
    expect(progressPct(0, 200)).toBe(0);
    expect(progressPct(50, 200)).toBe(25);
    expect(progressPct(1, 3)).toBe(33);
    expect(progressPct(2, 3)).toBe(67);
  });

  it("clamps overshoot to 100 (duplicate/replayed chunks)", () => {
    expect(progressPct(200, 200)).toBe(100);
    expect(progressPct(250, 200)).toBe(100);
  });
});
