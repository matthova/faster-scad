// Standalone entry for the marketing page (index.html, the site root). No React
// — it only
// (1) upgrades the hero's primary download button to the visitor's OS, and
// (2) keeps the light/dark theme in sync with the OS after first paint.
//
// Everything degrades: the four per-OS cards and the "other options" link are
// plain <a> tags that work without any of this, and the primary button ships
// pointing at the releases page, so a JS failure just means a slightly less
// tailored (but still functional) download.
import "./about.css";
import { ASSETS, DL, detectOs, isAppleSilicon } from "./downloads";

type Target = {
  /** Filename of the stable release alias, or null for "no desktop build". */
  asset: string | null;
  /** Button label, e.g. "Download for macOS". */
  label: string;
  /** Sub-line under the button. */
  note: string;
};

const MAC_ARM: Target = {
  asset: ASSETS.macArm,
  label: "Download for macOS",
  note: "Apple Silicon · .dmg — Intel & other options below",
};
const MAC_INTEL: Target = {
  asset: ASSETS.macIntel,
  label: "Download for macOS",
  note: "Intel · .dmg — Apple Silicon & other options below",
};
const WINDOWS: Target = {
  asset: ASSETS.windows,
  label: "Download for Windows",
  note: "x64 installer — other options below",
};
const LINUX: Target = {
  asset: ASSETS.linux,
  label: "Download for Linux",
  note: "x86_64 · AppImage — .deb / .rpm below",
};
// Phones/tablets and anything we can't place: there's no desktop build to push,
// so send them to the browser playground instead.
const OTHER: Target = {
  asset: null,
  label: "Open the playground",
  note: "The desktop app is available for macOS, Windows, and Linux.",
};

async function pickTarget(): Promise<Target> {
  switch (detectOs()) {
    case "mac":
      return (await isAppleSilicon()) ? MAC_ARM : MAC_INTEL;
    case "windows":
      return WINDOWS;
    case "linux":
      return LINUX;
    default:
      return OTHER;
  }
}

async function wirePrimaryDownload() {
  const btn = document.getElementById(
    "primary-download",
  ) as HTMLAnchorElement | null;
  const note = document.getElementById("primary-note");
  if (!btn) return;

  const target = await pickTarget();
  btn.textContent = target.label;
  if (target.asset) {
    btn.href = `${DL}/${target.asset}`;
  } else {
    // No desktop build for this device — the button opens the playground. Use a
    // relative link so it resolves to /openrscad/playground under the subpath.
    btn.href = "playground";
  }
  if (note) note.textContent = target.note;

  // Highlight the matching card in the OS grid so the autodetected choice and
  // the full list agree at a glance.
  const os = detectOs();
  const cardOs =
    os === "mac"
      ? target === MAC_INTEL
        ? "mac-intel"
        : "mac-arm"
      : os === "windows"
        ? "windows"
        : os === "linux"
          ? "linux"
          : null;
  if (cardOs) {
    document
      .querySelector(`.mk-os-card[data-os="${cardOs}"]`)
      ?.classList.add("mk-os-card--suggested");
  }
}

/** Follow the OS light/dark preference after the pre-paint script set the
 *  initial value (mirrors the app's behaviour in App.tsx). */
function syncTheme() {
  const mql = window.matchMedia("(prefers-color-scheme: dark)");
  const apply = () => {
    document.documentElement.dataset.theme = mql.matches ? "dark" : "light";
  };
  mql.addEventListener("change", apply);
}

// ── Render shootout ─────────────────────────────────────────────────────────
// Committed benchmark data: best of 3 runs, full-process wall-clock (ms), on an
// Apple M4 Max vs OpenSCAD 2024.12 (`cargo run -p xtask -- bench`). Ordered by
// CGAL speed-up, descending, so the chart reads big-win → small-win.
type Row = {
  model: string;
  note: string;
  openrscad: number;
  cgal: number;
  mfld: number;
};
const SHOOTOUT: Row[] = [
  {
    model: "Boolean grid",
    note: "spheres + cylinders diffed from a slab",
    openrscad: 53,
    cgal: 19338,
    mfld: 165,
  },
  {
    model: "Gears",
    note: "linear + rotate extrude",
    openrscad: 12,
    cgal: 1877,
    mfld: 59,
  },
  {
    model: "Rounded",
    note: "minkowski + hull",
    openrscad: 21,
    cgal: 334,
    mfld: 54,
  },
  {
    model: "Eval-bound",
    note: "heavy Collatz computation",
    openrscad: 102,
    cgal: 510,
    mfld: 501,
  },
  {
    model: "Lamp shade",
    note: "extrudes + booleans",
    openrscad: 37,
    cgal: 176,
    mfld: 173,
  },
];

/** ms → a compact human string: sub-second in ms, else seconds. */
function fmtTime(ms: number): string {
  return ms < 1000
    ? `${Math.round(ms)} ms`
    : `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)} s`;
}
/** A speed-up multiplier, e.g. 4.8 → "4.8×", 365 → "365×". */
function fmtX(v: number): string {
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)}×`;
}

// Log TIME axis, 10 ms → 30 s. Render times span ~12 ms to ~19 s, so a linear
// axis would bury every fast bar under the slow one; log keeps all three
// engines readable. Bar length + gridlines share this one scale.
const AXIS_MIN_LOG = 1; // log10(10 ms)
const AXIS_MAX_LOG = Math.log10(30000); // 30 s, headroom past the 19 s worst case
const frac = (ms: number) =>
  Math.max(
    0,
    Math.min(
      1,
      (Math.log10(ms) - AXIS_MIN_LOG) / (AXIS_MAX_LOG - AXIS_MIN_LOG),
    ),
  );

const el = (tag: string, cls?: string, text?: string) => {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
};

// The three series, in legend / stacking order (OpenRSCAD on top). Each row draws a
// bar per engine; colour identity is backed up by the direct time label.
const SERIES = [
  {
    key: "openrscad",
    cls: "mk-bar--openrscad",
    swatch: "mk-swatch--openrscad",
    label: "OpenRSCAD (native)",
  },
  {
    key: "mfld",
    cls: "mk-bar--mfld",
    swatch: "mk-swatch--mfld",
    label: "OpenSCAD · Manifold",
  },
  {
    key: "cgal",
    cls: "mk-bar--cgal",
    swatch: "mk-swatch--cgal",
    label: "OpenSCAD · CGAL (CSG, default)",
  },
] as const;

/** Grouped horizontal bars per model — one bar per engine, log time axis. */
function renderChart(host: HTMLElement) {
  const chart = el("div", "mk-chart-inner");

  // Legend (three series → always present; also direct-labeled on each bar).
  const legend = el("div", "mk-legend");
  for (const s of SERIES) {
    const item = el("span", "mk-legend-item");
    item.append(
      el("span", `mk-swatch ${s.swatch}`),
      el("span", undefined, s.label),
    );
    legend.append(item);
  }
  chart.append(legend);

  // Gridlines at 10 ms, 100 ms, 1 s, 10 s.
  const plot = el("div", "mk-plot");
  const grid = el("div", "mk-grid");
  for (const [ms, tick] of [
    [10, "10 ms"],
    [100, "100 ms"],
    [1000, "1 s"],
    [10000, "10 s"],
  ] as const) {
    const line = el("div", "mk-gridline");
    line.style.left = `${frac(ms) * 100}%`;
    line.append(el("span", "mk-gridtick", tick));
    grid.append(line);
  }
  plot.append(grid);

  for (const r of SHOOTOUT) {
    const row = el("div", "mk-row");
    const label = el("div", "mk-row-label");
    label.append(
      el("span", "mk-row-name", r.model),
      el("span", "mk-row-note", r.note),
    );
    row.append(label);

    const bars = el("div", "mk-bars");
    for (const s of SERIES) {
      const ms = r[s.key];
      const track = el("div", "mk-track");
      const bar = el("div", `mk-bar ${s.cls}`);
      bar.style.width = `${frac(ms) * 100}%`;
      bar.append(el("span", "mk-bar-value", fmtTime(ms)));
      track.append(bar);
      bars.append(track);
    }
    row.append(bars);
    plot.append(row);
  }
  chart.append(plot);
  host.replaceChildren(chart);
}

/** The exact numbers — credibility, and the accessible view of the chart. */
function renderTable(host: HTMLElement) {
  const table = el("table", "mk-table");
  const thead = el("thead");
  const htr = el("tr");
  for (const [h, cls] of [
    ["Model", ""],
    ["OpenRSCAD", "num"],
    ["OpenSCAD CGAL", "num"],
    ["OpenSCAD Manifold", "num"],
    ["vs CGAL", "num"],
    ["vs Manifold", "num"],
  ] as const) {
    const th = el("th", cls || undefined, h);
    th.setAttribute("scope", "col");
    htr.append(th);
  }
  thead.append(htr);
  table.append(thead);

  const tbody = el("tbody");
  for (const r of SHOOTOUT) {
    const tr = el("tr");
    tr.append(el("th", undefined, r.model));
    (tr.lastChild as HTMLElement).setAttribute("scope", "row");
    tr.append(
      el("td", "num", fmtTime(r.openrscad)),
      el("td", "num", fmtTime(r.cgal)),
      el("td", "num", fmtTime(r.mfld)),
      el("td", "num strong", fmtX(r.cgal / r.openrscad)),
      el("td", "num strong", fmtX(r.mfld / r.openrscad)),
    );
    tbody.append(tr);
  }
  table.append(tbody);

  const cap = el(
    "p",
    "mk-table-cap",
    "Full-process render time, best of 3 runs (lower is better).",
  );
  host.replaceChildren(table, cap);
}

function renderShootout() {
  const chart = document.getElementById("shootout-chart");
  const table = document.getElementById("shootout-table");
  if (chart) renderChart(chart);
  if (table) renderTable(table);
}

syncTheme();
renderShootout();
void wirePrimaryDownload();
