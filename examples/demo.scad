// A parametric part: a rounded-ish plate with holes and a boss.
$fn = 48;

module hole(x, y, r) {
    translate([x, y, -1])
        cylinder(h = 12, r = r);
}

difference() {
    // base plate
    union() {
        cube([40, 30, 10], center = true);
        // central boss
        translate([0, 0, 5])
            cylinder(h = 6, r1 = 8, r2 = 6);
    }
    // bolt holes at the corners
    for (dx = [-15, 15], dy = [-10, 10])
        hole(dx, dy, 2.5);
    // central bore
    hole(0, 0, 3);
}

echo("plate corners:", 4, "boss height:", 6);
