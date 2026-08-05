import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { lezer } from "@lezer/generator/rollup";

// Standalone from vite.config.ts (no PWA plugin needed). Two environments:
//   - `.test.ts`  → node: pure-logic and language modules (export writers,
//     customizer schema, share round-trip, the render-state reducer).
//   - `.test.tsx` → jsdom: component/DOM tests (the registry projections, the
//     Objects section) that need a DOM. The react plugin enables JSX there.
// The lezer plugin compiles the .grammar import so the completion tests can
// build a real OpenSCAD syntax tree.
export default defineConfig({
  plugins: [react(), lezer()],
  test: {
    include: ["src/**/*.test.{ts,tsx}"],
    environment: "node",
    environmentMatchGlobs: [["**/*.test.tsx", "jsdom"]],
    setupFiles: ["src/test-setup.ts"],
  },
});
