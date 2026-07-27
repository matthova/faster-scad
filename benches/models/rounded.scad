// Minkowski rounding + hull.
$fn = 32;
union() {
  minkowski() { cube([30,20,8], center=true); sphere(3); }
  hull() {
    translate([-18,0,10]) sphere(3);
    translate([ 18,0,10]) sphere(3);
  }
}
