import { describe, it, expect } from "vitest";
import { blankResponse } from "./renderResponse";
import type { RenderResponse } from "./engineWorker";
import {
  reduce,
  INITIAL_RENDER_STATE,
  keepOverrides,
  type ReduceCtx,
  type RenderState,
} from "./renderState";

// Runs in the node vitest project (no jsdom): if `reduce` reached for the DOM or
// React, this file would throw on import — that's the "pure, no React" assertion.

const CTX: ReduceCtx = {
  userPickedFmt: false,
  renderedSource: "cube(1);",
  paramsChanged: false,
};

/** A successful render response with `patch` on top. */
function ok(patch: Partial<RenderResponse> = {}): RenderResponse {
  return blankResponse(1, {
    ok: true,
    triangleCount: 12,
    vertexCount: 8,
    volume: 1,
    area: 6,
    version: "openrscad 0.7.1",
    ...patch,
  });
}

describe("renderState.reduce", () => {
  it("ok: fills stats, bumps renderRev, snapshots the source", () => {
    const next = reduce(INITIAL_RENDER_STATE, ok(), CTX);
    expect(next.status.ok).toBe(true);
    expect(next.status.message).toBe("12 triangles");
    expect(next.status.triangleCount).toBe(12);
    expect(next.status.volume).toBe(1);
    expect(next.version).toBe("openrscad 0.7.1");
    expect(next.renderRev).toBe(INITIAL_RENDER_STATE.renderRev + 1);
    expect(next.renderedSource).toBe("cube(1);");
    expect(next.status.preview).toBe(false);
  });

  it("error: keeps last-good stats + source, flips error fields", () => {
    const good = reduce(INITIAL_RENDER_STATE, ok({ triangleCount: 99 }), CTX);
    const bad = reduce(
      good,
      blankResponse(2, { ok: false, error: "parse error", ms: 5 }),
      { ...CTX, renderedSource: "broken(" },
    );
    expect(bad.status.ok).toBe(false);
    expect(bad.status.error).toBe("parse error");
    expect(bad.status.message).toBe("parse error");
    // Last good geometry stats survive so the status bar keeps its numbers.
    expect(bad.status.triangleCount).toBe(99);
    // The mesh didn't change, so the snapshot must not advance to the broken src.
    expect(bad.renderedSource).toBe("cube(1);");
    expect(bad.renderRev).toBe(good.renderRev + 1);
  });

  it("stopped: a watchdog/Stop result reduces like an error", () => {
    const stopped = reduce(
      INITIAL_RENDER_STATE,
      blankResponse(3, {
        ok: false,
        stopped: true,
        error: "Render stopped.",
        ms: 20000,
      }),
      CTX,
    );
    expect(stopped.status.ok).toBe(false);
    expect(stopped.status.error).toBe("Render stopped.");
  });

  it("geomErrors: degraded render flags the message", () => {
    const next = reduce(
      INITIAL_RENDER_STATE,
      ok({ geomErrors: "non-manifold" }),
      CTX,
    );
    expect(next.status.ok).toBe(true);
    expect(next.status.geomErrors).toBe("non-manifold");
    expect(next.status.message).toBe("12 triangles · geometry errors");
  });

  it("preview: fast-path result marks the status approximate", () => {
    const next = reduce(INITIAL_RENDER_STATE, ok({ preview: true }), CTX);
    expect(next.status.preview).toBe(true);
  });

  it("is2D: a 2D model switches the export format to a vector one", () => {
    const next = reduce(INITIAL_RENDER_STATE, ok({ is2D: true }), CTX);
    expect(next.is2D).toBe(true);
    expect(next.exportFmt).toBe("dxf");
  });

  it("schema-change: re-parses schema and drops stale overrides", () => {
    const prev: RenderState = {
      ...INITIAL_RENDER_STATE,
      overrides: { width: 40, gone: 1 },
    };
    const next = reduce(prev, ok({ params: `{"params":[{"name":"width"}]}` }), {
      ...CTX,
      paramsChanged: true,
    });
    expect(next.schema.map((p) => p.name)).toEqual(["width"]);
    expect(next.overrides).toEqual({ width: 40 }); // "gone" dropped
  });

  it("multi-color: >1 export color defaults an unpicked format to 3MF", () => {
    const groups = JSON.stringify([
      { start: 0, count: 3, color: [1, 0, 0], mode: "solid" },
      { start: 3, count: 3, color: [0, 1, 0], mode: "solid" },
    ]);
    const next = reduce(INITIAL_RENDER_STATE, ok({ groups }), CTX);
    expect(next.exportFmt).toBe("3mf");
    // But a user-picked format is respected.
    const picked = reduce(INITIAL_RENDER_STATE, ok({ groups }), {
      ...CTX,
      userPickedFmt: true,
    });
    expect(picked.exportFmt).toBe("stl");
  });
});

describe("keepOverrides", () => {
  it("returns the same reference when nothing is dropped (React bail-out)", () => {
    const ov = { a: 1, b: 2 };
    const schema = [
      { name: "a" },
      { name: "b" },
    ] as unknown as import("./customizer").Param[];
    expect(keepOverrides(ov, schema)).toBe(ov);
  });
});
