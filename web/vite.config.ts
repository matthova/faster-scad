import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
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
