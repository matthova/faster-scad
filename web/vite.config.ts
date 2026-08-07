import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { lezer } from "@lezer/generator/rollup";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    react(),
    lezer(),
    VitePWA({
      registerType: "autoUpdate",
      includeAssets: ["favicon.svg"],
      // Precache the app shell + the wasm engine so the playground works
      // offline (libraries fetched from a CDN still need the network).
      workbox: {
        globPatterns: ["**/*.{js,css,html,wasm,svg}"],
        // The OpenSCAD engine is opt-in and its wasm is ~9.6 MB; keep it out of
        // the precache so it's only fetched when a user selects that engine.
        globIgnores: ["**/openscad/**"],
        maximumFileSizeToCacheInBytes: 8 * 1024 * 1024,
      },
      manifest: {
        name: "Quito playground",
        short_name: "Quito",
        description: "A fast OpenSCAD-compatible modeling playground.",
        theme_color: "#141414",
        background_color: "#141414",
        display: "standalone",
        icons: [
          { src: "icon-192.png", sizes: "192x192", type: "image/png" },
          { src: "icon-512.png", sizes: "512x512", type: "image/png" },
          {
            src: "icon-512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "maskable",
          },
        ],
      },
    }),
  ],
  base: "./",
  build: {
    rollupOptions: {
      // Multi-page: the playground (index.html) plus the standalone marketing
      // page. about.html emits to dist/about.html, which GitHub Pages serves at
      // /faster-scad/about (extensionless). Kept at the root — not nested — so
      // its relative asset URLs resolve against /faster-scad/ with no trailing-
      // slash ambiguity.
      input: {
        main: "index.html",
        about: "about.html",
      },
    },
  },
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
