// A concave shape built as a union of two convex boxes, rounded by a cube.
// Minkowski distributes over the union, so this is exact (not the convex hull).
minkowski() { union() { cube([10,4,4]); cube([4,10,4]); } cube(2, center=true); }
