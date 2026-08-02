// A meshing gear train built with BOSL2's gears.scad.
//
// Four spur gears drive a linear rack. The angular speeds and phase offsets
// are chosen so the teeth actually interlock and stay meshed through the whole
// animation — press Play (the $t timeline) to watch it turn.
//
// The library is fetched on first render, so give it a few seconds.
include <BOSL2/std.scad>
include <BOSL2/gears.scad>

$fn = 24;

/* [Gear train] */
// Circular pitch — the tooth spacing. All meshing gears must share it.
circ_pitch = 9;   // [4:1:14]
// Face width (thickness) of every gear.
thickness = 6;    // [3:1:14]
// Bore diameter drilled through each gear hub.
bore = 3;         // [0:0.5:6]

/* [Teeth] */
red_teeth = 11;    // [8:1:40]
green_teeth = 20;  // [8:1:40]
blue_teeth = 6;    // [5:1:40]
orange_teeth = 16; // [8:1:40]
rack_teeth = 9;    // [4:1:20]

// Center distances that put each gear exactly in mesh with the red driver.
d_green  = gear_dist(circ_pitch = circ_pitch, teeth1 = red_teeth, teeth2 = green_teeth);
d_blue   = gear_dist(circ_pitch = circ_pitch, teeth1 = red_teeth, teeth2 = blue_teeth);
d_orange = gear_dist(circ_pitch = circ_pitch, teeth1 = red_teeth, teeth2 = orange_teeth);
d_rack   = gear_dist(circ_pitch = circ_pitch, teeth1 = red_teeth, teeth2 = 0);

// Spin angles. The driver turns at $t; each meshing gear turns in the opposite
// direction at a speed inversely proportional to its tooth count, with a phase
// offset so its teeth drop into the driver's gaps.
a_red    =  $t * 360 / red_teeth;
a_green  = -$t * 360 / green_teeth  + 180 / green_teeth;
a_blue   = -$t * 360 / blue_teeth   - 3 * 90 / blue_teeth;
a_orange = -$t * 360 / orange_teeth - 3.5 * 180 / orange_teeth;

color("#f77")             zrot(a_red)    spur_gear(circ_pitch, red_teeth,    thickness, bore);
color("#7f7") back(d_green)  zrot(a_green)  spur_gear(circ_pitch, green_teeth,  thickness, bore);
color("#77f") right(d_blue)  zrot(a_blue)   spur_gear(circ_pitch, blue_teeth,   thickness, bore);
color("#fc7") left(d_orange) zrot(a_orange) spur_gear(circ_pitch, orange_teeth, thickness, bore);

// The red gear also drives a rack: its pitch line rolls forward at the pitch
// circumference times $t, one full tooth-pitch per 1/red_teeth of a turn.
color("#ccc") fwd(d_rack) right(circ_pitch * $t)
    rack(pitch = circ_pitch, teeth = rack_teeth, thickness = thickness,
         width = 12, anchor = CENTER, orient = BACK);
