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
        content: `// Press ▶ (or drag the $t slider) in the toolbar to spin this.
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
    label: "Multi-color rocket (3MF)",
    files: [
      {
        name: "main.scad",
        content: `// Each part has its own color(), so exporting as 3MF keeps the colors
// as separate objects for multi-material printing (pick 3MF in Download).
$fn = 64;

/* [Rocket] */
body_d    = 24;   // [16:40]
body_h    = 50;   // [30:80]
nose_h    = 26;   // [12:40]
fin_count = 4;    // [3:6]

/* [Fins] */
fin_len = 16;     // [8:28]
fin_h   = 22;     // [10:36]
fin_t   = 3;      // [1:6]

body_r = body_d / 2;

// Body
color("Gainsboro")
  cylinder(h = body_h, r = body_r);

// Nose cone
color("Crimson")
  translate([0, 0, body_h])
    cylinder(h = nose_h, r1 = body_r, r2 = 0);

// Porthole window
color("DeepSkyBlue")
  translate([0, body_r - 1, body_h * 0.66])
    rotate([-90, 0, 0])
      cylinder(h = 3, r = body_r * 0.3);

// Fins
color("RoyalBlue")
  for (i = [0 : fin_count - 1])
    rotate([0, 0, i * 360 / fin_count])
      translate([body_r - 1, 0, 0])
        rotate([90, 0, 0])
          linear_extrude(height = fin_t, center = true)
            polygon([[0, 0], [fin_len, 0], [0, fin_h]]);
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
  {
    label: "Parthenon (polychrome)",
    files: [
      {
        name: "main.scad",
        content: `/* ============================================================================
 *   T H E   P A R T H E N O N
 *   Acropolis of Athens  ·  447 – 432 BCE
 *   Iktinos & Kallikrates, architects  ·  Pheidias, master sculptor
 *
 *   A parametric, polychrome OpenSCAD model.
 *
 *   UNITS ..... 1 OpenSCAD unit = 1 metre.  All principal dimensions are the
 *               published archaeological figures (stylobate 69.51 x 30.86 m,
 *               columns 10.43 m tall, 8 x 17 Doric columns, 92 metopes).
 *
 *   COLOUR .... The Parthenon was never white.  This model uses the
 *               reconstructed ancient polychromy: Pentelic-marble ground,
 *               Egyptian-blue triglyphs and mutules, red taeniae, regulae and
 *               metope/tympanum fields, gilded sculpture and bronze fittings.
 *
 *               Colour is shown in BOTH preview (F5) and render (F6), and is
 *               carried into 3MF export.  This needs the Manifold backend:
 *
 *                 GUI  ..  Edit > Preferences > Advanced > 3D rendering
 *                          backend = Manifold          (then F6)
 *                 CLI  ..  --backend=manifold --render
 *
 *               Manifold is the default in 2025+ builds.  On the older 2021.01
 *               release F6 collapses everything to a single yellow, because
 *               that version's CGAL backend simply cannot carry colour: if you
 *               see one flat colour after F6, that is the cause -- switch the
 *               backend or update OpenSCAD.  Colour is always correct in F5.
 * ========================================================================= */


/* ---------------------------------------------------------------------------
 * 1.  BUILD SWITCHES
 * ------------------------------------------------------------------------ */
SHOW_GROUND      = true;   // the limestone plateau of the Acropolis
SHOW_KREPIDOMA   = true;   // the three-stepped platform
SHOW_PERISTYLE   = true;   // the outer ring of 46 Doric columns
SHOW_ENTABLATURE = true;   // architrave + Doric frieze + cornice
SHOW_PEDIMENTS   = true;   // the two gables and their sculpture
SHOW_CELLA       = true;   // inner building, porches, Ionic frieze
SHOW_INTERIOR    = true;   // naos colonnade + the Athena Parthenos
SHOW_ROOF        = true;   // marble tile roof
CUTAWAY          = false;  // open the south-east quarter to reveal the naos
CUT_Y            = -4.20;  // the section plane passes through the near aisle,
CUT_X            = -2.00;  // leaving Athena and the far colonnade standing

DETAIL  = 2;   // 0 plain blocks · 1 mouldings · 2 guttae + reliefs · 3 all
FLUTED  = true;   // 20 flutes with entasis on every column
NFLUTE  = 20;     // Doric canon: 20 flutes, meeting at sharp arrises
FSEG    = 4;      // polygon samples per flute
NSLICE  = 7;      // vertical slices used to build the entasis curve

$fa = 6;
$fs = 0.10;
CFN = 40;         // segments for capitals / small revolutions


/* ---------------------------------------------------------------------------
 * 2.  PALETTE   (reconstructed ancient polychromy)
 * ------------------------------------------------------------------------ */
C_MARBLE   = [0.960, 0.940, 0.880];   // sunlit Pentelic marble
C_MARBLE_2 = [0.910, 0.880, 0.800];   // marble in shade / weathered
C_STEP     = [0.880, 0.850, 0.775];   // krepidoma
C_STEP_2   = [0.845, 0.812, 0.735];
C_BLUE     = [0.106, 0.302, 0.576];   // Egyptian blue  – triglyphs, mutules
C_BLUE_DK  = [0.062, 0.196, 0.400];   // shadowed glyph channels
C_RED      = [0.706, 0.192, 0.145];   // red ochre – metope & tympanum grounds
C_RED_DK   = [0.545, 0.145, 0.110];
C_GOLD     = [0.870, 0.680, 0.235];   // gilding on sculpture
C_GOLD_DK  = [0.700, 0.520, 0.160];
C_OCHRE    = [0.835, 0.650, 0.320];
C_BRONZE   = [0.470, 0.345, 0.160];   // bronze doors, shields, fittings
C_IVORY    = [0.968, 0.940, 0.862];   // chryselephantine flesh
C_GREEN    = [0.180, 0.420, 0.361];   // green accents in painted mouldings
C_TILE     = [0.935, 0.905, 0.840];   // marble roof tiles
C_TILE_RIB = [0.862, 0.812, 0.720];   // cover tiles
C_ROCK     = [0.615, 0.592, 0.540];   // the Acropolis rock
C_DARK     = [0.140, 0.128, 0.115];   // door voids, deep shade


/* ---------------------------------------------------------------------------
 * 3.  PRINCIPAL DIMENSIONS
 * ------------------------------------------------------------------------ */
STEP_H     = 0.51;                       // rise of each krepidoma step
STEP_T     = 0.70;                       // tread
N_STEP     = 3;
Z0         = N_STEP * STEP_H;            // top of the stylobate  = 1.53 m

COL_H      = 10.43;                      // column incl. capital
CAP_H      = 0.90;
D_LOW      = 1.905;                      // lower diameter (corner cols 1.944)
D_LOW_C    = 1.944;
D_TOP      = 1.480;

IA_FLANK   = 4.291;                      // normal interaxial, long sides
IA_FRONT   = 4.296;                      // normal interaxial, facades
IA_CORNER  = 3.680;                      // contracted corner interval

N_FRONT    = 8;                          // octastyle
N_FLANK    = 17;

// half-spans measured between corner column axes
AX_L       = (14 * IA_FLANK + 2 * IA_CORNER) / 2;   // 33.717
AX_W       = ( 5 * IA_FRONT + 2 * IA_CORNER) / 2;   // 14.420

OVER_L     = 1.04;                       // stylobate overhang past corner axis
OVER_W     = 1.01;
STYLO_L    = 2 * (AX_L + OVER_L);        // 69.51 m
STYLO_W    = 2 * (AX_W + OVER_W);        // 30.86 m

// ---- column axis coordinates (with corner contraction) --------------------
function axes(n, ia, iac) =
    let(half = ((n - 3) * ia + 2 * iac) / 2)
    concat([-half],
           [for (i = [0 : n - 3]) -half + iac + i * ia],
           [half]);

AX_X = axes(N_FLANK, IA_FLANK, IA_CORNER);   // 17 positions along the length
AX_Y = axes(N_FRONT, IA_FRONT, IA_CORNER);   //  8 positions across the front

// ---- entablature -----------------------------------------------------------
ARCH_H     = 1.35;                       // architrave (epistyle)
TAEN_H     = 0.16;                       // taenia, the fillet capping it
FRZ_H      = 1.35;                       // Doric frieze
CORN_H     = 0.62;                       // horizontal cornice (geison)
TRI_W      = 0.845;                      // triglyph width
TRI_D      = 0.14;                       // projection of triglyph over metope

ENT_L      = AX_L + 0.74;                // architrave face, long axis
ENT_W      = AX_W + 0.74;                // architrave face, short axis
CORN_OVER  = 0.85;                       // projection of the geison
CORN_L     = ENT_L + CORN_OVER;          // cornice face, long axis
CORN_W     = ENT_W + CORN_OVER;

Z_ARCH     = Z0 + COL_H;                 // 11.96
Z_FRZ      = Z_ARCH + ARCH_H;            // 13.31
Z_CORN     = Z_FRZ  + FRZ_H;             // 14.66
Z_TOP      = Z_CORN + CORN_H;            // 15.28  top of horizontal cornice

// ---- pediment & roof -------------------------------------------------------
PED_H      = 3.85;                       // gable apex above the cornice
PED_SLOPE  = atan(PED_H / CORN_W);       // 13.5 deg  – the Parthenon's pitch
RAKE_H     = 0.62;                       // depth of the raking cornice member
TILE_T     = 0.30;                       // thickness of the tile shell at eaves
Z_APEX     = Z_TOP + PED_H;              // top of the raking cornice, at ridge
Z_RIDGE    = Z_APEX + 0.10;              // ridge of the tiled roof
SLOPE_L    = sqrt(CORN_W*CORN_W + PED_H*PED_H);   // rafter length

// ---- cella (the inner building) --------------------------------------------
CEL_L      = 24.15;                      // half length of the cella block
CEL_W      = 10.86;                      // half width
CEL_T      = 1.15;                       // wall thickness
CEL_H      = 9.10;                       // wall height above the cella floor
Z_CEL      = Z0 + 0.30;                  // cella floor, one step up
ION_H      = 1.00;                       // the Ionic (Parthenon) frieze
CEL_CORN   = 0.42;
ANTA_X     = 19.00;                      // where the side walls stop (antae)
PORCH_X    = 21.90;                      // axis of the six porch columns
PORCH_D    = 1.70;
PORCH_H    = CEL_H;
NAOS_E     = 14.50;                      // east wall of the naos (with door)
NAOS_W     = -3.00;                      // cross wall closing the naos
WEST_W     = -19.00;                     // back wall of the west chamber


/* ---------------------------------------------------------------------------
 * 4.  SMALL HELPERS
 * ------------------------------------------------------------------------ */
module box(l, w, h)  { translate([-l/2, -w/2, 0]) cube([l, w, h]); }

// hollow rectangular ring: outer half-extents l,w · wall thickness t
module ring(l, w, h, t) {
    difference() {
        box(2*l, 2*w, h);
        translate([0, 0, -0.01]) box(2*(l-t), 2*(w-t), h + 0.02);
    }
}

// A prism running along X.  \`pts\` are literal [y, z] points, so every gable
// profile below can be read straight off an elevation drawing.
module prismX(len, pts) {
    translate([-len/2, 0, 0]) rotate([90, 0, 90])
        linear_extrude(height = len, convexity = 10) polygon(pts);
}

// outer gable triangle: half-span W, apex height H
function gable(W, H) = [[-W, 0], [W, 0], [0, H]];

// the same triangle with an eaves fascia of height e hanging below the base
function gable_eave(W, H, e) = [[-W, -e], [W, -e], [W, 0], [0, H], [-W, 0]];

// the triangle inset by a perpendicular thickness t, with base raised to tb:
// used both for the tympanum recess and to hollow the raking cornice
function gable_inset(W, H, t, tb) =
    let(dv = t / cos(PED_SLOPE), iw = W - (tb + dv) * W / H)
    [[-iw, tb], [iw, tb], [0, H - dv]];

// a single gutta (the little peg beneath regulae and mutules)
module gutta(r = 0.075, h = 0.13) { cylinder(r1 = r, r2 = r*0.78, h = h); }


/* ---------------------------------------------------------------------------
 * 5.  THE DORIC COLUMN
 *     20 shallow flutes meeting at sharp arrises, a linear taper corrected by
 *     a subtle convex entasis, then the necking, echinus and square abacus.
 * ------------------------------------------------------------------------ */
function col_r(t, r0, r1, bulge = 0.030) = r0 + (r1 - r0)*t + bulge*sin(180*t);

module flute_profile(r) {
    d = r * 0.038;                                   // flute depth
    polygon([for (i = [0 : NFLUTE*FSEG - 1])
        let(a  = 360 * i / (NFLUTE*FSEG),
            f  = (i % FSEG) / FSEG,
            rr = r - d * sin(180 * f))
        [rr*cos(a), rr*sin(a)]]);
}

module doric_shaft(h, r0, r1) {
    for (s = [0 : NSLICE-1]) {
        t0 = s / NSLICE;
        t1 = (s+1) / NSLICE;
        ra = col_r(t0, r0, r1);
        rb = col_r(t1, r0, r1);
        translate([0, 0, h*t0])
            linear_extrude(height = h/NSLICE + 0.002, scale = rb/ra, convexity = 8) {
                if (FLUTED) flute_profile(ra);
                else        circle(r = ra, $fn = 36);
            }
    }
}

module doric_capital(rt, h) {
    ab = rt * 1.20;             // half width of the abacus
    hy = h * 0.20;              // hypotrachelion / necking
    he = h * 0.50;              // echinus
    ha = h * 0.30;              // abacus

    color(C_MARBLE) cylinder(r = rt*0.985, h = hy, $fn = CFN);

    // three annulets ringing the neck
    if (DETAIL >= 1)
        color(C_MARBLE_2)
            for (i = [0:2])
                translate([0, 0, hy*0.30 + i*hy*0.22])
                    cylinder(r = rt*1.035, h = hy*0.11, $fn = CFN);

    color(C_MARBLE) translate([0, 0, hy])
        rotate_extrude($fn = CFN)
            polygon(concat([[0, 0]],
                    [for (i = [0:8]) let(t = i/8) [rt + (ab*0.90 - rt)*pow(t, 1.9), he*t]],
                    [[0, he]]));

    color(C_MARBLE) translate([0, 0, hy + he]) box(2*ab, 2*ab, ha);
}

module doric_column(h = COL_H, d0 = D_LOW, d1 = D_TOP) {
    color(C_MARBLE) doric_shaft(h - CAP_H, d0/2, d1/2);
    translate([0, 0, h - CAP_H]) doric_capital(d1/2, CAP_H);
}


/* ---------------------------------------------------------------------------
 * 6.  KREPIDOMA  —  the three-stepped platform
 * ------------------------------------------------------------------------ */
module krepidoma() {
    for (s = [0 : N_STEP-1]) {
        g = (N_STEP - 1 - s) * STEP_T;               // this step's overhang
        color(s % 2 == 0 ? C_STEP : C_STEP_2)
            translate([0, 0, s*STEP_H])
                box(STYLO_L + 2*g, STYLO_W + 2*g, STEP_H + 0.002);
    }
}


/* ---------------------------------------------------------------------------
 * 7.  PERISTYLE  —  8 x 17 = 46 columns, corners slightly thickened
 * ------------------------------------------------------------------------ */
module peristyle() {
    translate([0, 0, Z0]) {
        for (x = AX_X) for (y = [-AX_Y[N_FRONT-1], AX_Y[N_FRONT-1]])
            translate([x, y, 0])
                doric_column(d0 = (abs(x) > AX_L - 0.01) ? D_LOW_C : D_LOW);
        for (y = [for (i = [1 : N_FRONT-2]) AX_Y[i]])
            for (x = [-AX_L, AX_L])
                translate([x, y, 0]) doric_column(d0 = D_LOW);
    }
}


/* ---------------------------------------------------------------------------
 * 8.  ARCHITRAVE  (epistyle) + TAENIA
 * ------------------------------------------------------------------------ */
module architrave() {
    color(C_MARBLE) translate([0, 0, Z_ARCH]) ring(ENT_L, ENT_W, ARCH_H - TAEN_H, 1.30);
    // the taenia: a slender red-painted fillet capping the architrave
    color(C_RED) translate([0, 0, Z_ARCH + ARCH_H - TAEN_H])
        ring(ENT_L + 0.09, ENT_W + 0.09, TAEN_H, 1.40);
}


/* ---------------------------------------------------------------------------
 * 9.  DORIC FRIEZE  —  triglyphs and the 92 metopes
 *
 *     One triglyph over every column axis and every intercolumniation, and a
 *     triglyph flush with each corner: exactly the arrangement that forces the
 *     famous corner contraction.  14 metopes on each facade, 32 on each flank.
 * ------------------------------------------------------------------------ */
function trig_pos(ax, A) =
    let(n = len(ax))
    concat([-(A - TRI_W/2)],
           [for (i = [1 : n-2]) each [(ax[i-1] + ax[i])/2, ax[i]]],
           [(ax[n-2] + ax[n-1])/2],
           [A - TRI_W/2]);

module triglyph(h) {
    color(C_BLUE) box(TRI_W, TRI_D + 0.30, h);
    // two full glyphs and two half-glyphs at the arrises, cut as V-channels
    color(C_BLUE_DK) translate([0, TRI_D*0.62, 0]) {
        for (i = [-1, 1])
            translate([i * TRI_W/6, 0, 0]) rotate([0, 0, 45])
                box(0.115, 0.115, h * 0.94);
        for (i = [-1, 1])
            translate([i * TRI_W/2, 0, 0]) rotate([0, 0, 45])
                box(0.115, 0.115, h * 0.94);
    }
    color(C_MARBLE) translate([0, 0, h*0.94]) box(TRI_W + 0.02, TRI_D + 0.32, h*0.06);
}

// regula with its six guttae, tucked under the taenia below each triglyph
module regula() {
    color(C_RED) translate([0, 0, -0.30]) box(TRI_W, 0.20, 0.14);
    if (DETAIL >= 2)
        color(C_GOLD) for (i = [0:5])
            translate([-TRI_W/2 + TRI_W*(i + 0.5)/6, 0.02, -0.42]) gutta();
}

module metope_relief(w, h) {
    // an abstracted duel — Lapith and Centaur — in low gilded relief
    color(C_GOLD) {
        translate([-w*0.18, 0, h*0.16]) scale([1, 0.5, 1]) sphere(r = h*0.09);
        translate([-w*0.18, 0, h*0.42]) rotate([0, 12, 0])
            scale([1, 0.42, 1]) cylinder(r1 = h*0.10, r2 = h*0.055, h = h*0.38);
        translate([-w*0.18, 0, h*0.70]) scale([1, 0.45, 1]) sphere(r = h*0.075);
        translate([w*0.16, 0, h*0.20]) rotate([0, 90, 0])
            scale([1, 0.5, 1]) cylinder(r = h*0.085, h = w*0.34, center = true);
        translate([w*0.30, 0, h*0.36]) rotate([0, -20, 0])
            scale([1, 0.42, 1]) cylinder(r1 = h*0.09, r2 = h*0.05, h = h*0.34);
        translate([w*0.32, 0, h*0.62]) scale([1, 0.45, 1]) sphere(r = h*0.07);
    }
}

// one straight run of frieze, along Y, outer face at x = +R
module frieze_run(pos, R) {
    for (p = pos)
        translate([R - TRI_D, p, Z_FRZ]) rotate([0, 0, -90]) {
            triglyph(FRZ_H);
            regula();
        }
    for (i = [0 : len(pos)-2]) {
        c = (pos[i] + pos[i+1]) / 2;
        w = pos[i+1] - pos[i] - TRI_W;
        if (w > 0.2) {
            // metope: red-ground panel set just behind the triglyph faces
            color(C_RED) translate([R - 0.22, c, Z_FRZ])
                rotate([0, 0, -90]) box(w, 0.34, FRZ_H);
            if (DETAIL >= 2)
                translate([R - 0.07, c, Z_FRZ + FRZ_H*0.06])
                    rotate([0, 0, -90]) metope_relief(w, FRZ_H*0.88);
        }
    }
}

module doric_frieze() {
    // backing course so the frieze reads as solid wall from any angle
    color(C_MARBLE_2) translate([0, 0, Z_FRZ]) ring(ENT_L - 0.30, ENT_W - 0.30, FRZ_H, 1.10);
    frieze_run(trig_pos(AX_Y, ENT_W), ENT_L);
    rotate([0, 0, 180]) frieze_run(trig_pos(AX_Y, ENT_W), ENT_L);
    rotate([0, 0,  90]) frieze_run(trig_pos(AX_X, ENT_L), ENT_W);
    rotate([0, 0, 270]) frieze_run(trig_pos(AX_X, ENT_L), ENT_W);
}


/* ---------------------------------------------------------------------------
 * 10.  CORNICE (geison) with its blue mutules and gold guttae
 * ------------------------------------------------------------------------ */
module mutules(pos, R) {
    for (p = pos) for (o = [0, 1]) {
        c = (o == 0) ? p : (p + TRI_W/2 + 0.9);
        if (abs(c) < R + 4) {
            color(C_BLUE) translate([R - 0.58, c, Z_CORN - 0.16])
                rotate([0, 0, -90]) box(TRI_W + 0.15, 0.62, 0.17);
            if (DETAIL >= 3)
                color(C_GOLD)
                    for (a = [0:2]) for (b = [0:5])
                        translate([R - 0.90 + a*0.22,
                                   c - TRI_W/2 - 0.06 + (TRI_W + 0.12)*(b + 0.5)/6,
                                   Z_CORN - 0.26]) gutta(0.055, 0.10);
        }
    }
}

module cornice() {
    color(C_MARBLE) translate([0, 0, Z_CORN]) ring(CORN_L, CORN_W, CORN_H, 1.60);
    // painted sima: a cyma strip along the two flanks only, as in the original
    color(C_BLUE) for (sy = [-1, 1])
        translate([0, sy * (CORN_W - 0.16), Z_TOP - 0.005])
            box(2*CORN_L - 1.9, 0.36, 0.20);
    color(C_MARBLE) translate([0, 0, Z_CORN - 0.16]) ring(ENT_L + 0.10, ENT_W + 0.10, 0.16, 1.20);
    if (DETAIL >= 2) {
        mutules(trig_pos(AX_Y, ENT_W), CORN_L);
        rotate([0, 0, 180]) mutules(trig_pos(AX_Y, ENT_W), CORN_L);
        rotate([0, 0,  90]) mutules(trig_pos(AX_X, ENT_L), CORN_W);
        rotate([0, 0, 270]) mutules(trig_pos(AX_X, ENT_L), CORN_W);
    }
}


/* ---------------------------------------------------------------------------
 * 11.  PEDIMENTS
 *      East: the birth of Athena.   West: her contest with Poseidon.
 *      Tympanum ground painted red, figures fully in the round and gilded.
 * ------------------------------------------------------------------------ */
module figure(h, standing = true) {
    color(C_GOLD) {
        if (standing) {
            for (i = [-1, 1])
                translate([0, i*h*0.06, 0]) cylinder(r1 = h*0.055, r2 = h*0.045, h = h*0.45);
            translate([0, 0, h*0.42]) scale([0.85, 1, 1])
                cylinder(r1 = h*0.13, r2 = h*0.095, h = h*0.38);
            translate([0, 0, h*0.83]) sphere(r = h*0.085);
            for (i = [-1, 1])
                translate([0, i*h*0.11, h*0.74]) rotate([i*22, 0, 0])
                    cylinder(r1 = h*0.042, r2 = h*0.032, h = h*0.34, center = true);
        } else {
            rotate([0, 78, 0]) {
                translate([0, 0, -h*0.10]) cylinder(r1 = h*0.10, r2 = h*0.14, h = h*0.55);
                translate([0, 0, h*0.45]) sphere(r = h*0.13);
            }
            translate([h*0.36, 0, h*0.30]) sphere(r = h*0.115);
        }
    }
}

module pediment_sculpture(W, H) {
    // Figures graded to the falling height of the gable: gods standing at the
    // centre, heroes seated at the flanks, river-gods reclining in the angles.
    for (i = [-6 : 6]) {
        y     = i * W / 6.6;
        avail = (H - 0.62) * (1 - pow(abs(y)/(W*1.02), 1.5));
        if (avail > 0.85)
            translate([0, y, 0]) rotate([0, 0, y > 0 ? -14 : 14])
                figure(min(avail, H - 0.75), avail > H*0.52);
    }
}

module pediment(sx = 1) {
    d = RAKE_D;                           // depth of the raking cornice member
    rotate([0, 0, sx > 0 ? 0 : 180]) {

        // floor of the gable, closing the top of the cornice ring
        color(C_MARBLE_2) translate([CORN_L - d/2, 0, Z_TOP - 0.02])
            box(d + CORN_OVER + 0.7, 2*CORN_W - 0.1, 0.16);

        // raking cornice: outer triangle hollowed to leave the tympanum recess
        color(C_MARBLE) translate([0, 0, Z_TOP]) difference() {
            translate([CORN_L - d/2, 0, 0]) prismX(d, gable(CORN_W, PED_H));
            translate([CORN_L - d/2, 0, 0])
                prismX(d + 0.02, gable_inset(CORN_W, PED_H, RAKE_H, 0.34));
        }

        // tympanum: the red-ground back wall of the recess
        color(C_RED_DK) translate([ENT_L - 0.30, 0, Z_TOP])
            prismX(0.60, gable_inset(CORN_W, PED_H, RAKE_H + 0.10, 0.34));

        // and its sculpture, standing free in the recess
        translate([ENT_L + 0.42, 0, Z_TOP + 0.34])
            pediment_sculpture(CORN_W - 1.15, PED_H - 0.34);
    }
}

/* ---------------------------------------------------------------------------
 * 12.  ROOF  —  Pentelic marble tiles with cover-tile ribs and antefixes
 * ------------------------------------------------------------------------ */
RAKE_D  = CORN_OVER + 0.30;        // depth of the raking cornice member
ROOF_L  = 2*(CORN_L - RAKE_D);     // tiles stop behind the two gables
TILE_DP = RAKE_H * 0.62;           // how far the tile plane sits below the rake
HR      = PED_H - TILE_DP/cos(PED_SLOPE);       // ridge of the tile surface
HW      = CORN_W;                               // tiles reach the eaves
TILE_SL = atan(HR / HW);                        // true pitch of the tile plane

module roof() {
    // the tile shell: one prism on the roof planes, hollowed underneath
    color(C_TILE) translate([0, 0, Z_TOP]) difference() {
        prismX(ROOF_L, gable_eave(HW, HR, TILE_T));
        prismX(ROOF_L + 0.4, gable_inset(HW, HR, TILE_T, -3.0));
    }

    if (DETAIL >= 1) {
        n  = floor(ROOF_L / 1.05);
        rl = sqrt(HW*HW + HR*HR) - 0.04;              // rafter length
        for (i = [0 : n]) {
            x = -ROOF_L/2 + 0.16 + i * (ROOF_L - 0.32) / n;
            // cover tiles capping every joint, laid up each slope
            for (sy = [-1, 1])
                color(C_TILE_RIB)
                    translate([x, 0, Z_TOP + HR + 0.005])
                        rotate([-sy * TILE_SL, 0, 0])
                            translate([0, sy > 0 ? 0.02 : -rl, -0.01])
                                cube([0.26, rl, 0.15]);
            // antefixes: painted palmettes standing along both eaves
            if (DETAIL >= 2)
                for (sy = [-1, 1])
                    color(i % 2 == 0 ? C_RED : C_BLUE)
                        translate([x + 0.13, sy * (CORN_W - 0.16), Z_TOP + 0.16])
                            rotate([0, 0, 90]) scale([1, 0.30, 1])
                                cylinder(r1 = 0.21, r2 = 0.05, h = 0.40, $fn = 6);
        }
        // ridge beam
        color(C_TILE_RIB) translate([0, 0, Z_TOP + HR - 0.09])
            box(ROOF_L - 0.30, 0.46, 0.22);
    }
}

module acroteria() {
    for (sx = [-1, 1]) {
        translate([sx * (CORN_L - RAKE_D/2), 0, Z_TOP + PED_H - 0.12])
            color(C_GOLD) scale([0.42, 1, 1])
                cylinder(r1 = 0.90, r2 = 0.05, h = 2.5, $fn = 7);
        for (sy = [-1, 1])
            translate([sx * (CORN_L - 0.26), sy * (CORN_W - 0.30), Z_TOP + 0.02])
                color(C_GOLD_DK) rotate([0, 0, 90]) scale([0.42, 1, 1])
                    cylinder(r1 = 0.60, r2 = 0.05, h = 1.6, $fn = 7);
    }
}

/* ---------------------------------------------------------------------------
 * 13.  CELLA  —  naos, west chamber, two hexastyle porches,
 *      and the 160 m continuous Ionic frieze that ran around the whole block.
 * ------------------------------------------------------------------------ */
PORCH_Y = [for (i = [0:5]) -CEL_W + 1.9 + i * (2*CEL_W - 3.8)/5];

module ionic_frieze_band() {
    // deep blue ground with a gilded procession of riders in low relief
    color(C_BLUE) translate([0, 0, Z_CEL + CEL_H]) ring(CEL_L + 0.06, CEL_W + 0.06, ION_H, 0.55);
    if (DETAIL >= 2) {
        nx = 30;
        for (i = [0 : nx]) {
            x = -CEL_L + 0.8 + i * (2*CEL_L - 1.6)/nx;
            for (s = [-1, 1])
                translate([x, s*(CEL_W + 0.10), Z_CEL + CEL_H + ION_H*0.16])
                    color(C_GOLD) {
                        rotate([0, 90, 0]) scale([1, 0.55, 1])
                            cylinder(r = ION_H*0.16, h = 0.85, center = true);
                        for (l = [-1, 1])
                            translate([l*0.26, 0, -ION_H*0.16])
                                rotate([0, l*16, 0]) cylinder(r = ION_H*0.05, h = ION_H*0.28);
                        translate([0.30, 0, ION_H*0.30]) scale([1, 0.5, 1])
                            cylinder(r1 = ION_H*0.11, r2 = ION_H*0.07, h = ION_H*0.30);
                        translate([0.30, 0, ION_H*0.64]) scale([1, 0.5, 1]) sphere(r = ION_H*0.09);
                    }
        }
        // the same procession turning the corners of the two facades
        for (sx = [-1, 1]) for (i = [0:6]) {
            y = -CEL_W + 1.2 + i*(2*CEL_W - 2.4)/6;
            translate([sx*(CEL_L + 0.10), y, Z_CEL + CEL_H + ION_H*0.16])
                color(C_GOLD) rotate([0, 0, 90]) {
                    rotate([0, 90, 0]) scale([1, 0.55, 1])
                        cylinder(r = ION_H*0.16, h = 0.8, center = true);
                    translate([0.28, 0, ION_H*0.32]) scale([1, 0.5, 1])
                        cylinder(r1 = ION_H*0.11, r2 = ION_H*0.07, h = ION_H*0.30);
                    translate([0.28, 0, ION_H*0.66]) scale([1, 0.5, 1]) sphere(r = ION_H*0.09);
                }
        }
    }
    color(C_MARBLE) translate([0, 0, Z_CEL + CEL_H + ION_H])
        ring(CEL_L + 0.30, CEL_W + 0.30, CEL_CORN, 0.90);
}

module cella() {
    // floor of the cella block, one step above the stylobate
    color(C_MARBLE_2) translate([0, 0, Z0]) box(2*CEL_L + 0.6, 2*CEL_W + 0.6, 0.30);

    translate([0, 0, Z_CEL]) {
        // long side walls, stopping at the antae
        color(C_MARBLE) for (s = [-1, 1])
            translate([0, s*(CEL_W - CEL_T/2), 0]) box(2*ANTA_X, CEL_T, CEL_H);
        // antae: the thickened wall ends
        color(C_MARBLE_2) for (sx = [-1, 1]) for (sy = [-1, 1])
            translate([sx*(ANTA_X - 0.35), sy*(CEL_W - CEL_T/2), 0])
                box(0.80, CEL_T + 0.16, CEL_H);
        // east wall of the naos, with the great doorway
        color(C_MARBLE) difference() {
            translate([NAOS_E, 0, 0]) box(CEL_T, 2*CEL_W, CEL_H);
            translate([NAOS_E, 0, -0.01]) box(CEL_T + 0.2, 4.90, 7.60);
        }
        color(C_DARK)  translate([NAOS_E, 0, 0]) box(CEL_T*0.30, 4.90, 7.60);
        color(C_BRONZE) translate([NAOS_E + CEL_T*0.45, 0, 0]) {
            for (s = [-1, 1]) translate([0, s*1.20, 0]) box(0.16, 2.30, 7.45);
            if (DETAIL >= 2)
                for (r = [0:5]) for (c = [-1, 1])
                    translate([0.11, c*1.20, 0.9 + r*1.15]) rotate([0, 90, 0])
                        cylinder(r = 0.16, h = 0.09, $fn = 20);
        }
        // cross wall between naos and west chamber, and the west back wall
        color(C_MARBLE) translate([NAOS_W, 0, 0]) box(CEL_T, 2*CEL_W, CEL_H);
        color(C_MARBLE) difference() {
            translate([WEST_W, 0, 0]) box(CEL_T, 2*CEL_W, CEL_H);
            translate([WEST_W, 0, -0.01]) box(CEL_T + 0.2, 3.40, 6.20);
        }
        color(C_DARK) translate([WEST_W, 0, 0]) box(CEL_T*0.3, 3.40, 6.20);

        // the two hexastyle porches
        for (sx = [-1, 1]) for (y = PORCH_Y)
            translate([sx*PORCH_X, y, 0])
                doric_column(h = PORCH_H, d0 = PORCH_D, d1 = PORCH_D*0.79);
        // their architraves
        color(C_MARBLE) for (sx = [-1, 1])
            translate([sx*PORCH_X, 0, PORCH_H]) box(1.30, 2*CEL_W, ION_H*0.0 + 0.0 + 0.01);
    }

    // architrave course carrying the frieze over the porches
    color(C_MARBLE) translate([0, 0, Z_CEL + CEL_H - 0.02]) ring(CEL_L, CEL_W, 0.04, 1.20);
    ionic_frieze_band();

    // low inner roof over the cella, hidden beneath the great roof
    color(C_MARBLE_2) translate([0, 0, Z_CEL + CEL_H + ION_H + CEL_CORN])
        box(2*CEL_L + 0.5, 2*CEL_W + 0.5, 0.28);
}


/* ---------------------------------------------------------------------------
 * 14.  INTERIOR  —  the two-storey naos colonnade and the Athena Parthenos
 * ------------------------------------------------------------------------ */
module inner_colonnade() {
    d1 = 1.05;  h1 = 5.30;      // lower tier
    d2 = 0.80;  h2 = 3.40;      // upper tier
    ep = 0.55;                  // epistyle between the tiers
    zf = Z_CEL;
    xs = [for (i = [0:8]) -1.2 + i * (13.4 + 1.2)/8];

    color(C_MARBLE_2) translate([0, 0, zf]) {
        for (sy = [-1, 1]) for (x = xs) translate([x, sy*5.90, 0]) {
            doric_column(h = h1, d0 = d1, d1 = d1*0.80);
            translate([0, 0, h1 + ep])
                doric_column(h = h2, d0 = d2, d1 = d2*0.80);
        }
        for (y = [-2.95, 0, 2.95]) translate([-1.2, y, 0]) {
            doric_column(h = h1, d0 = d1, d1 = d1*0.80);
            translate([0, 0, h1 + ep]) doric_column(h = h2, d0 = d2, d1 = d2*0.80);
        }
        color(C_MARBLE) {
            for (sy = [-1, 1])
                translate([(xs[0] + xs[8])/2, sy*5.90, h1]) box(15.8, 1.0, ep);
            translate([-1.2, 0, h1]) box(1.0, 12.8, ep);
        }
    }
}

module athena_parthenos() {
    // Pheidias' chryselephantine statue: ~11.5 m of gold and ivory,
    // shield at her left, Nike alighting on her right hand.
    translate([5.60, 0, Z_CEL]) {
        color(C_MARBLE_2) box(7.0, 5.0, 0.55);            // pedestal
        color(C_BRONZE)  translate([0, 0, 0.55]) box(6.2, 4.3, 0.28);
        translate([0, 0, 0.83]) {
            H = 7.80;
            // peplos: a tall gilded cone with an ivory upper body
            color(C_GOLD) cylinder(r1 = 1.62, r2 = 1.05, h = H*0.55, $fn = 44);
            color(C_GOLD_DK) translate([0, 0, H*0.55 - 0.10])
                cylinder(r1 = 1.10, r2 = 1.00, h = 0.22, $fn = 44);
            color(C_IVORY) translate([0, 0, H*0.55])
                cylinder(r1 = 1.00, r2 = 0.62, h = H*0.24, $fn = 40);
            color(C_GOLD) translate([0, 0, H*0.55])          // aegis
                scale([0.55, 1, 0.30]) sphere(r = 1.05, $fn = 36);
            color(C_IVORY) translate([0, 0, H*0.81]) sphere(r = 0.52, $fn = 36);
            color(C_GOLD) translate([0, 0, H*0.85]) {        // triple-crested helmet
                cylinder(r1 = 0.56, r2 = 0.44, h = 0.55, $fn = 36);
                for (i = [-1, 0, 1])
                    translate([0, i*0.34, 0.5]) rotate([0, 0, 90])
                        scale([1, 0.22, 1]) cylinder(r1 = 0.42, r2 = 0.06, h = 0.9, $fn = 8);
            }
            // right arm, extended, carrying Nike
            color(C_IVORY) translate([1.15, 0, H*0.70]) rotate([0, 72, 0])
                cylinder(r1 = 0.24, r2 = 0.18, h = 1.5, $fn = 24);
            translate([2.35, 0, H*0.62]) {
                color(C_GOLD) cylinder(r = 0.34, h = 0.16, $fn = 24);
                color(C_GOLD) translate([0, 0, 0.16]) {
                    cylinder(r1 = 0.24, r2 = 0.15, h = 1.0, $fn = 20);
                    translate([0, 0, 1.05]) sphere(r = 0.17, $fn = 20);
                    for (s = [-1, 1]) translate([0, s*0.16, 0.80]) rotate([s*28, 0, 0])
                        scale([0.35, 1, 1]) cylinder(r1 = 0.30, r2 = 0.05, h = 1.1, $fn = 10);
                }
            }
            // shield leaning at her left, and the coiled serpent Erichthonios
            color(C_GOLD_DK) translate([-1.42, -0.15, 1.95]) rotate([0, 8, 0])
                cylinder(r = 2.05, h = 0.30, center = true, $fn = 52);
            color(C_BRONZE) translate([-1.58, -0.15, 1.95]) rotate([0, 8, 0])
                cylinder(r = 0.50, h = 0.34, center = true, $fn = 30);
            color(C_GREEN) translate([-1.15, 0.6, 0.4])
                for (i = [0:5]) translate([0, 0, i*0.55])
                    rotate([0, 0, i*55]) translate([0.35, 0, 0]) sphere(r = 0.24, $fn = 16);
            // spear
            color(C_BRONZE) translate([-1.05, 1.35, 0]) rotate([-6, 0, 0])
                cylinder(r = 0.10, h = 7.90, $fn = 12);
        }
    }
}


/* ---------------------------------------------------------------------------
 * 15.  GROUND
 * ------------------------------------------------------------------------ */
module ground() {
    color(C_ROCK) translate([0, 0, -1.4]) box(STYLO_L + 26, STYLO_W + 22, 1.4);
    color([0.665, 0.640, 0.585]) translate([0, 0, -0.02]) box(STYLO_L + 20, STYLO_W + 16, 0.06);
}


/* ---------------------------------------------------------------------------
 * 16.  ASSEMBLY
 * ------------------------------------------------------------------------ */
module parthenon() {
    if (SHOW_KREPIDOMA)   krepidoma();
    if (SHOW_PERISTYLE)   peristyle();
    if (SHOW_ENTABLATURE) { architrave(); doric_frieze(); cornice(); }
    if (SHOW_PEDIMENTS)   { pediment(1); pediment(-1); acroteria(); }
    if (SHOW_CELLA)       cella();
    if (SHOW_INTERIOR)    { inner_colonnade(); athena_parthenos(); }
    if (SHOW_ROOF)        roof();
}

if (SHOW_GROUND) ground();

// ---- sanity check: the canon says 46 columns and 92 metopes -------------
N_COLS   = 2*N_FRONT + 2*(N_FLANK - 2);
N_MET_F  = len(trig_pos(AX_Y, ENT_W)) - 1;
N_MET_S  = len(trig_pos(AX_X, ENT_L)) - 1;
echo(str("peristyle columns = ", N_COLS, "  (canon 46)"));
echo(str("metopes = ", 2*N_MET_F, " front/back + ", 2*N_MET_S, " flanks = ",
         2*N_MET_F + 2*N_MET_S, "  (canon 92)"));
echo(str("stylobate = ", STYLO_L, " x ", STYLO_W, " m   (canon 69.51 x 30.86)"));
echo(str("ridge height above stylobate = ", Z_RIDGE - Z0, " m"));

difference() {
    parthenon();
    // Colouring the cutting solid keeps the sawn faces stone-coloured in
    // preview rather than OpenSCAD's default highlight.  (In render the sawn
    // faces inherit the colour of whatever solid was cut, which is what we
    // want anyway -- marble walls stay marble.)
    if (CUTAWAY) color(C_MARBLE_2)
        translate([CUT_X, -160, Z0 - 0.02]) cube([200, 160 + CUT_Y, 60]);
}
`,
      },
    ],
  },
];
