// Curated example projects for the playground, showcasing the language and
// engine: CSG, 2D + vector export, text, extrusion, animation, and BOSL2.
import type { File } from "./project";

export interface Example {
  label: string;
  files: File[];
}

export const EXAMPLES: Example[] = [
  {
    label: "Rounded box",
    files: [
      {
        name: "main.scad",
        content: `// A rounded box built with minkowski() + a helper module.
use <helpers.scad>
$fn = 48;

/* [Box] */
size = 30;    // [10:60]
radius = 4;   // [1:12]

/* [Lid] */
lid = true;
lid_gap = 1;  // [0:0.5:4]

rounded_box([size, size, size], radius);
if (lid)
  translate([0, 0, size/2 + lid_gap + radius])
    rounded_box([size, size, 4], radius);
`,
      },
      {
        name: "helpers.scad",
        content: `module rounded_box(sz, r) {
  minkowski() {
    cube([sz[0] - 2*r, sz[1] - 2*r, sz[2] - 2*r], center = true);
    sphere(r);
  }
}
`,
      },
    ],
  },
  {
    label: "Twisted vase",
    files: [
      {
        name: "main.scad",
        content: `// linear_extrude with twist + scale sweeps a polygon into a vase.
$fn = 6;

/* [Vase] */
height = 60;   // [20:120]
twist = 90;    // [0:360]
taper = 0.4;   // [0.1:0.05:1]

linear_extrude(height = height, twist = twist, scale = taper)
  translate([12, 0]) circle(6);
`,
      },
    ],
  },
  {
    label: "Text keychain",
    files: [
      {
        name: "main.scad",
        content: `// Extruded text() using the bundled Liberation Sans font.
$fn = 32;

/* [Text] */
label = "Quito";
size = 12;      // [6:40]
thickness = 3;  // [1:10]

linear_extrude(thickness)
  text(label, size = size, font = "Liberation Sans",
       halign = "center", valign = "center");
`,
      },
    ],
  },
  {
    label: "2D gasket (DXF/SVG)",
    files: [
      {
        name: "main.scad",
        content: `// A flat 2D profile — the export dropdown offers DXF and SVG for it.
$fn = 64;

/* [Gasket] */
outer = 30;   // [15:60]
bore = 12;    // [4:25]
holes = 6;    // [3:12]

difference() {
  circle(outer);
  for (a = [0 : 360/holes : 359])
    rotate(a) translate([outer - 8, 0]) circle(4);
  circle(bore);
}
`,
      },
    ],
  },
  {
    label: "Animated turbine ($t)",
    files: [
      {
        name: "main.scad",
        content: `// Drag the $t slider in the toolbar to spin this.
/* [Turbine] */
blades = 6;   // [3:12]
radius = 16;  // [8:30]

rotate([0, 0, 360 * $t])
  for (i = [0 : blades - 1])
    rotate([0, 0, i * 360 / blades])
      translate([radius, 0, 0])
        rotate([0, 30, 0])
          cube([10, 3, 3], center = true);

cylinder(h = 6, r = 4, center = true, $fn = 24);
`,
      },
    ],
  },
  {
    label: "Surface (heightmap)",
    files: [
      {
        name: "main.scad",
        content: `// surface() drapes a solid over a heightmap read from wave.dat (the tab).
surface("wave.dat", center = true);
`,
      },
      {
        name: "wave.dat",
        content: `7.70 8.64 9.52 10.27 10.85 11.22 11.41 11.44 11.36 11.23 11.08 10.96 10.89 10.89 10.96 11.08 11.23 11.36 11.44 11.41 11.22 10.85 10.27 9.52 8.64
8.64 9.60 10.41 11.00 11.35 11.45 11.34 11.08 10.74 10.38 10.06 9.83 9.71 9.71 9.83 10.06 10.38 10.74 11.08 11.34 11.45 11.35 11.00 10.41 9.60
9.52 10.41 11.05 11.39 11.42 11.18 10.74 10.17 9.58 9.02 8.56 8.24 8.07 8.07 8.24 8.56 9.02 9.58 10.17 10.74 11.18 11.42 11.39 11.05 10.41
10.27 11.00 11.39 11.41 11.08 10.48 9.71 8.87 8.07 7.38 6.83 6.47 6.28 6.28 6.47 6.83 7.38 8.07 8.87 9.71 10.48 11.08 11.41 11.39 11.00
10.85 11.35 11.42 11.08 10.38 9.44 8.40 7.38 6.47 5.74 5.21 4.87 4.71 4.71 4.87 5.21 5.74 6.47 7.38 8.40 9.44 10.38 11.08 11.42 11.35
11.22 11.45 11.18 10.48 9.44 8.24 7.02 5.92 5.04 4.40 4.00 3.78 3.69 3.69 3.78 4.00 4.40 5.04 5.92 7.02 8.24 9.44 10.48 11.18 11.45
11.41 11.34 10.74 9.71 8.40 7.02 5.74 4.71 4.00 3.61 3.46 3.45 3.48 3.48 3.45 3.46 3.61 4.00 4.71 5.74 7.02 8.40 9.71 10.74 11.34
11.44 11.08 10.17 8.87 7.38 5.92 4.71 3.88 3.49 3.48 3.70 3.99 4.19 4.19 3.99 3.70 3.48 3.49 3.88 4.71 5.92 7.38 8.87 10.17 11.08
11.36 10.74 9.58 8.07 6.47 5.04 4.00 3.49 3.53 3.99 4.69 5.35 5.75 5.75 5.35 4.69 3.99 3.53 3.49 4.00 5.04 6.47 8.07 9.58 10.74
11.23 10.38 9.02 7.38 5.74 4.40 3.61 3.48 3.99 5.00 6.20 7.27 7.89 7.89 7.27 6.20 5.00 3.99 3.48 3.61 4.40 5.74 7.38 9.02 10.38
11.08 10.06 8.56 6.83 5.21 4.00 3.46 3.70 4.69 6.20 7.89 9.37 10.23 10.23 9.37 7.89 6.20 4.69 3.70 3.46 4.00 5.21 6.83 8.56 10.06
10.96 9.83 8.24 6.47 4.87 3.78 3.45 3.99 5.35 7.27 9.37 11.19 12.27 12.27 11.19 9.37 7.27 5.35 3.99 3.45 3.78 4.87 6.47 8.24 9.83
10.89 9.71 8.07 6.28 4.71 3.69 3.48 4.19 5.75 7.89 10.23 12.27 13.54 13.54 12.27 10.23 7.89 5.75 4.19 3.48 3.69 4.71 6.28 8.07 9.71
10.89 9.71 8.07 6.28 4.71 3.69 3.48 4.19 5.75 7.89 10.23 12.27 13.54 13.54 12.27 10.23 7.89 5.75 4.19 3.48 3.69 4.71 6.28 8.07 9.71
10.96 9.83 8.24 6.47 4.87 3.78 3.45 3.99 5.35 7.27 9.37 11.19 12.27 12.27 11.19 9.37 7.27 5.35 3.99 3.45 3.78 4.87 6.47 8.24 9.83
11.08 10.06 8.56 6.83 5.21 4.00 3.46 3.70 4.69 6.20 7.89 9.37 10.23 10.23 9.37 7.89 6.20 4.69 3.70 3.46 4.00 5.21 6.83 8.56 10.06
11.23 10.38 9.02 7.38 5.74 4.40 3.61 3.48 3.99 5.00 6.20 7.27 7.89 7.89 7.27 6.20 5.00 3.99 3.48 3.61 4.40 5.74 7.38 9.02 10.38
11.36 10.74 9.58 8.07 6.47 5.04 4.00 3.49 3.53 3.99 4.69 5.35 5.75 5.75 5.35 4.69 3.99 3.53 3.49 4.00 5.04 6.47 8.07 9.58 10.74
11.44 11.08 10.17 8.87 7.38 5.92 4.71 3.88 3.49 3.48 3.70 3.99 4.19 4.19 3.99 3.70 3.48 3.49 3.88 4.71 5.92 7.38 8.87 10.17 11.08
11.41 11.34 10.74 9.71 8.40 7.02 5.74 4.71 4.00 3.61 3.46 3.45 3.48 3.48 3.45 3.46 3.61 4.00 4.71 5.74 7.02 8.40 9.71 10.74 11.34
11.22 11.45 11.18 10.48 9.44 8.24 7.02 5.92 5.04 4.40 4.00 3.78 3.69 3.69 3.78 4.00 4.40 5.04 5.92 7.02 8.24 9.44 10.48 11.18 11.45
10.85 11.35 11.42 11.08 10.38 9.44 8.40 7.38 6.47 5.74 5.21 4.87 4.71 4.71 4.87 5.21 5.74 6.47 7.38 8.40 9.44 10.38 11.08 11.42 11.35
10.27 11.00 11.39 11.41 11.08 10.48 9.71 8.87 8.07 7.38 6.83 6.47 6.28 6.28 6.47 6.83 7.38 8.07 8.87 9.71 10.48 11.08 11.41 11.39 11.00
9.52 10.41 11.05 11.39 11.42 11.18 10.74 10.17 9.58 9.02 8.56 8.24 8.07 8.07 8.24 8.56 9.02 9.58 10.17 10.74 11.18 11.42 11.39 11.05 10.41
8.64 9.60 10.41 11.00 11.35 11.45 11.34 11.08 10.74 10.38 10.06 9.83 9.71 9.71 9.83 10.06 10.38 10.74 11.08 11.34 11.45 11.35 11.00 10.41 9.60
`,
      },
    ],
  },
  {
    label: "BOSL2 rounded cuboid",
    files: [
      {
        name: "main.scad",
        content: `// Uses the BOSL2 library (fetched on first render — give it a few seconds).
include <BOSL2/std.scad>

/* [Cuboid] */
s = 30;         // [15:60]
rounding = 5;   // [0:15]

cuboid([s, s, s], rounding = rounding, $fn = 32);
`,
      },
    ],
  },
];
