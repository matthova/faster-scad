// =============================================================================
//  MULTI-COLOR ROCKET  -  a full-color example for 3MF export
//
//  Every part is wrapped in its own color(), so each becomes a separate colored
//  object in the exported 3MF and the colors survive into a multi-material
//  slicer (Bambu Studio, PrusaSlicer, OrcaSlicer, ...).
//
//    quito examples/rocket.scad -o rocket.3mf
//
//  The rocket stands on a flat base (z = 0), so it prints upright without
//  supports.
// =============================================================================
$fn = 64;

/* [Rocket] */
body_d    = 24;   // [16:40] body diameter
body_h    = 50;   // [30:80] body height
nose_h    = 26;   // [12:40] nose cone height
fin_count = 4;    // [3:6]   number of fins

/* [Fins] */
fin_len = 16;     // [8:28]  how far the fins sweep out
fin_h   = 22;     // [10:36] fin height up the body
fin_t   = 3;      // [1:6]   fin thickness

body_r = body_d / 2;

// Body ------------------------------------------------------------------------
color("Gainsboro")
  cylinder(h = body_h, r = body_r);

// Nose cone -------------------------------------------------------------------
color("Crimson")
  translate([0, 0, body_h])
    cylinder(h = nose_h, r1 = body_r, r2 = 0);

// Porthole window -------------------------------------------------------------
color("DeepSkyBlue")
  translate([0, body_r - 1, body_h * 0.66])
    rotate([-90, 0, 0])
      cylinder(h = 3, r = body_r * 0.3);

// Fins ------------------------------------------------------------------------
color("RoyalBlue")
  for (i = [0 : fin_count - 1])
    rotate([0, 0, i * 360 / fin_count])
      translate([body_r - 1, 0, 0])
        rotate([90, 0, 0])
          linear_extrude(height = fin_t, center = true)
            polygon([[0, 0], [fin_len, 0], [0, fin_h]]);
