import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearRenderPending, markRenderPending, wasRenderPending } from "./project";

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
});
