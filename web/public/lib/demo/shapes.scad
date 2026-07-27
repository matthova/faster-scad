// A fetched library that itself includes a sibling (transitive resolution).
include <prims.scad>
module demo_shape(s) {
  union() {
    prim_cube(s);
    translate([0, 0, s * 0.6]) sphere(s * 0.4, $fn = 24);
  }
}
