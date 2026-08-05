// Single source for the 3D viewer's theme colors.
//
// three.js can't cheaply read the CSS custom properties in index.css, so the
// viewer's palette lived as ~9 hex literals scattered through viewer.ts. This
// module gathers them in one place so the Phase-4 palette work changes the
// viewport in the same edit as the chrome. The values here mirror the CSS
// palette in index.css — keep the two in sync until a build-time single source
// is worth it.
import type { ThemeMode } from "./viewer";

/** Theme-independent viewer colors (hex ints for three.js). */
export const viewerConst = {
  /** World axes: X red / Y green / Z blue — identical in both appearances. */
  axisX: 0xd9584f,
  axisY: 0x59a14f,
  axisZ: 0x4f83d9,
  /** Thin dark wireframe drawn over meshes and the nav cube. */
  edge: 0x000000,
  /** Default single-color mesh = the app accent (amber). */
  mesh: 0xf5a623,
  /** Pick highlight + nav-cube hover wash. Cyan means "the thing you pointed
   *  at" everywhere it appears; do not split this hue (see track-D §3). */
  selection: 0x4fc3f7,
  /** `!` modifier (debug highlight, red) and `%` modifier (background, gray). */
  modifierHighlight: 0xff3b30,
  modifierBackground: 0x888888,
} as const;

/** Colors that flip with the OS appearance. */
export interface ViewerTheme {
  /** Canvas clear color — matches the chrome's panel surface. */
  background: number;
  /** Adaptive floor grid line color, and its opacity. */
  gridLine: number;
  gridLineOpacity: number;
  /** Numeric ruler tick labels on the grid. */
  gridLabel: string;
  /** Highlighted axis tick labels along X/Y/Z. */
  axisTick: string;
  /** Nav-cube face fill / border / label. */
  cubeFace: string;
  cubeStroke: string;
  cubeText: string;
}

const DARK: ViewerTheme = {
  background: 0x1a1d23,
  gridLine: 0x3a3f4b,
  gridLineOpacity: 0.6,
  gridLabel: "#ced3dc",
  axisTick: "#9cc0ec",
  cubeFace: "#2c313b",
  cubeStroke: "#495060",
  cubeText: "#c6ccd6",
};

const LIGHT: ViewerTheme = {
  background: 0xf3f3f3,
  gridLine: 0xcccccc,
  gridLineOpacity: 0.9,
  gridLabel: "#3a3a3a",
  axisTick: "#3f73c9",
  cubeFace: "#eceef1",
  cubeStroke: "#b6bcc6",
  cubeText: "#484d55",
};

export function viewerTheme(mode: ThemeMode): ViewerTheme {
  return mode === "dark" ? DARK : LIGHT;
}

/** Format a hex int as a CSS "#rrggbb" string. */
export function hex(n: number): string {
  return "#" + n.toString(16).padStart(6, "0");
}
