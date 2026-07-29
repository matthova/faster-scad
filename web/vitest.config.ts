import { defineConfig } from "vitest/config";
import { lezer } from "@lezer/generator/rollup";

// Standalone from vite.config.ts (no PWA/react plugins needed): these are unit
// tests of pure-logic and language modules only. `node` env — no DOM required.
// The lezer plugin compiles the .grammar import so the completion tests can
// build a real OpenSCAD syntax tree.
export default defineConfig({
  plugins: [lezer()],
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
