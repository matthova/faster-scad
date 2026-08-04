import { describe, it, expect } from "vitest";
import { encodeProject, decodeSharedProject } from "./share";
import type { Project } from "./project";

const roundTrip = (p: Project, anim?: Parameters<typeof encodeProject>[1]) =>
  decodeSharedProject("#code/" + encodeProject(p, anim));

describe("encodeProject / decodeSharedProject", () => {
  it("round-trips files and overrides", () => {
    const p: Project = {
      files: [
        { name: "main.scad", content: "cube(10);" },
        { name: "lib.scad", content: "function f()=1;" },
      ],
      overrides: { width: 30, on: true, label: "hi", v: [1, 2, 3] },
      active: 1,
    };
    const out = roundTrip(p);
    expect(out).not.toBeNull();
    expect(out!.files).toEqual(p.files);
    expect(out!.overrides).toEqual(p.overrides);
    // `active` always decodes to 0 (the link doesn't carry the active tab).
    expect(out!.active).toBe(0);
    expect(out!.anim).toBeUndefined();
  });

  it("round-trips animation state when present", () => {
    const p: Project = {
      files: [{ name: "a.scad", content: "sphere(1);" }],
      overrides: {},
      active: 0,
    };
    const out = roundTrip(p, { t: 0.25, fps: 30, steps: 100, playing: true });
    expect(out!.anim).toEqual({ t: 0.25, fps: 30, steps: 100, playing: true });
  });

  it("drops a malformed anim payload", () => {
    const p: Project = {
      files: [{ name: "a.scad", content: "x" }],
      overrides: {},
      active: 0,
    };
    // fps as a string is invalid → the whole (now empty) anim is undefined.
    const out = roundTrip(p, { fps: "fast" } as unknown as { fps: number });
    expect(out!.anim).toBeUndefined();
  });

  it("preserves unicode content exactly", () => {
    const p: Project = {
      files: [{ name: "u.scad", content: "// 日本語 🎉\ncube(1);" }],
      overrides: {},
      active: 0,
    };
    expect(roundTrip(p)!.files[0].content).toBe(p.files[0].content);
  });

  it("returns null for non-share hashes and garbage payloads", () => {
    expect(decodeSharedProject("")).toBeNull();
    expect(decodeSharedProject("#other")).toBeNull();
    expect(decodeSharedProject("#code/!!!not-valid-lzstring!!!")).toBeNull();
  });

  it("returns null when the decoded project has no files", () => {
    const empty = encodeProject({ files: [], overrides: {}, active: 0 });
    expect(decodeSharedProject("#code/" + empty)).toBeNull();
  });
});
