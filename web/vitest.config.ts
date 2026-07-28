import { defineConfig } from "vitest/config";

// Standalone from vite.config.ts (no PWA/lezer/react plugins needed): these are
// unit tests of the pure-logic modules only. `node` env — no DOM required.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
