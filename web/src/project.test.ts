import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  clearRenderPending,
  markRenderPending,
  settleRenderPending,
  wasRenderPending,
} from "./project";

// A minimal in-memory localStorage so these run under the `node` test env.
class MemStorage {
  private m = new Map<string, string>();
  getItem(k: string) {
    return this.m.has(k) ? this.m.get(k)! : null;
  }
  setItem(k: string, v: string) {
    this.m.set(k, String(v));
  }
  removeItem(k: string) {
    this.m.delete(k);
  }
}

describe("render-pending crash sentinel", () => {
  beforeEach(() => {
    (globalThis as unknown as { localStorage: Storage }).localStorage =
      new MemStorage() as unknown as Storage;
  });
  afterEach(() => {
    delete (globalThis as unknown as { localStorage?: Storage }).localStorage;
  });

  it("is clear by default", () => {
    expect(wasRenderPending()).toBe(false);
  });

  it("reports pending once armed, and clears", () => {
    markRenderPending();
    expect(wasRenderPending()).toBe(true);
    clearRenderPending();
    expect(wasRenderPending()).toBe(false);
  });

  it("survives across a simulated reload (persisted state)", () => {
    markRenderPending();
    // A reload loses in-memory app state but not localStorage — a fresh read
    // still sees the armed sentinel, which is what triggers recovery mode.
    expect(wasRenderPending()).toBe(true);
  });

  it("never throws when localStorage is unavailable", () => {
    delete (globalThis as unknown as { localStorage?: Storage }).localStorage;
    expect(() => markRenderPending()).not.toThrow();
    expect(() => clearRenderPending()).not.toThrow();
    expect(wasRenderPending()).toBe(false);
  });

  // Regression: the 20s watchdog / user Stop delivers a synthetic `stopped`
  // result. Settling on it must NOT clear the armed sentinel — otherwise the
  // watchdog wipes its own recovery net and a too-heavy model re-freezes on every
  // launch (the exact bug this fixes). Only a genuine result clears it.
  it("keeps an armed sentinel armed for a stopped render", () => {
    markRenderPending(); // slow-timer armed it while the render was in flight
    settleRenderPending(true); // watchdog timeout / user Stop
    expect(wasRenderPending()).toBe(true);
  });

  it("clears the sentinel for a genuine render result", () => {
    markRenderPending();
    settleRenderPending(false); // engine returned a real result (ok or error)
    expect(wasRenderPending()).toBe(false);
  });

  it("leaves a disarmed sentinel disarmed for a stopped render", () => {
    // A quick Stop before the slow-timer armed anything: settling must not
    // spuriously arm recovery for the next launch.
    settleRenderPending(true);
    expect(wasRenderPending()).toBe(false);
  });
});
