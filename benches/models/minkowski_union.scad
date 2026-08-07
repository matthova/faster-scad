// Minkowski union-distribution: a concave comb built from a union of convex
// bars, rounded by a sphere. Exercises the distribute-over-union() path (each
// bar ⊕ sphere is a convex hull; the results are unioned) rather than a single
// convex-hull approximation.
$fn = 24;
minkowski() {
  union() {
    cube([44, 4, 6], center=true);
    for (x = [-18:9:18]) translate([x, 10, 0]) cube([4, 24, 6], center=true);
  }
  sphere(1.5);
}
