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
