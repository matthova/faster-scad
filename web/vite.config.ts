import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { lezer } from "@lezer/generator/rollup";

export default defineConfig({
  plugins: [react(), lezer()],
  base: "./",
  worker: {
    format: "es",
  },
  server: {
    fs: {
      // allow importing the wasm-pack output living outside src/
      allow: [".."],
    },
  },
});
