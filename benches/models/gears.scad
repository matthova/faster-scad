// Extrude + rotate_extrude heavy.
$fn = 48;
for (i = [0:11]) rotate([0,0,i*30]) translate([20,0,0]) linear_extrude(6, twist=0) circle(3);
rotate_extrude() translate([15,0]) circle(2);
