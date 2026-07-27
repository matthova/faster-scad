// Boolean-heavy: a grid of spheres differenced from a slab.
$fn = 32;
difference() {
  cube([60, 60, 12], center=true);
  for (x = [-2:2], y = [-2:2])
    translate([x*12, y*12, 0]) sphere(5);
  for (x = [-2:2], y = [-2:2])
    translate([x*12, y*12, 0]) cylinder(h=20, r=2, center=true);
}
