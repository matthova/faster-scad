// =============================================================================
//  DRAPED / FLUTED DOME TABLE LAMP  -  parametric OpenSCAD model
//  Modelled from reference photographs + published specification:
//      overall height 15"      shade length 12"     shade width 8"
//      base length    7"       base width   5"
//      cord 6' 7" (79")        max. 5.9 W integrated LED
//
//  All internal dimensions are INCHES.  Set export_units = "mm" to emit a
//  millimetre-scaled model (25.4x) for slicers / other CAD packages.
//
//  The shade is generated as a true constant-thickness shell:  an analytic
//  surface (elongated super-ellipsoid dome + arc-length-spaced flutes) is
//  tessellated, then offset inwards along its own vertex normals and closed
//  with a rim band.  One watertight polyhedron, no booleans.
// =============================================================================

/* [Output] */
// which piece to emit
part = "shade";      // [assembly, shade, shade_cut, base, stem, strip, exploded]
// unit system of the emitted geometry
export_units = "in";    // [in, mm]
// colourise the assembly (preview only)
tint = true;

/* [Master dimensions - inches] */
overall_height = 15;    // table surface -> top of shade
shade_length   = 12;    // long axis of shade
shade_width    = 8;     // short axis of shade
shade_height   = 6.0;   // rim -> apex
base_length    = 7;
base_width     = 5;

/* [Shade form] */
// glass wall thickness
shade_wall  = 1.6/25.4;
// plan outline: 2 = ellipse, higher = fuller / more stadium-like
sec_e       = 2.35;
// dome profile exponents  (t^prof_n + r^prof_m = 1)
prof_n      = 3.20;     // vertical  - larger = flatter top
prof_m      = 3.80;     // radial    - larger = fuller shoulder
// number of flutes around the whole perimeter (equally spaced by arc length)
flute_count = 12;
// radial depth of each flute
flute_depth = 0.22;
// flute cross-section shaping: <1 = broad ridges + narrow creases (draped
// fabric look), 1 = pure cosine, >1 = narrow ridges
flute_sharp = 0.60;
// how quickly flutes fade out towards the apex (larger = smoother top)
flute_ramp  = 3.20;
// rotate the flute pattern (0 = a ridge at each end of the long axis)
flute_phase = 0;
// how far the draped rim dips at each lobe
rim_wave    = 0.085;
rim_wave_ramp = 3.0;

/* [Brass top inlay] */
show_strip   = true;
strip_length = 6.40;
strip_width  = 0.85;
// how far the strip is let into the glass, and how far it stands proud
strip_depth  = 0.17;
strip_proud  = 0.02;
// offset of the inlay from the shade centreline, towards the back
strip_offset = 1.55;

/* [Stem / base / fittings] */
stem_dia      = 0.875;
stem_into_shade = 1.20; // how far the stem rises above the shade rim
base_thick    = 0.55;
base_fillet_top = 0.18;
base_fillet_bot = 0.05;
knob_dia      = 0.34;   // rotary dimmer on the base
knob_offset   = 1.45;   // from base centre, towards the front-left end
show_led      = true;   // 5.9 W integrated LED module inside the shade
show_cord     = true;   // short cord stub (real cord is 79" / 6'-7")
cord_dia      = 0.16;
cord_stub     = 2.2;

/* [Mesh resolution] */
// points around the shade / rings from apex to rim
res_u = $preview ? 128 : 128;
res_v = $preview ?  34 :  34;
$fn   = $preview ?  48 :  48;

// =============================================================================
//  derived
// =============================================================================
UNIT   = (export_units == "mm") ? 25.4 : 1;
A      = shade_length/2;            // semi-major of plan outline
B      = shade_width/2;             // semi-minor of plan outline
RIM_Z  = overall_height - shade_height;   // height of the shade rim
NU     = res_u;
NV     = res_v;
P_EXP  = 2/sec_e;                   // super-ellipse parametric exponent

// =============================================================================
//  small helpers
// =============================================================================
function sgnpow(v,e) = (v < 0 ? -1 : 1) * pow(abs(v), e);
function unitv(v)    = v / max(norm(v), 1e-12);
function rev(f)      = [for (k = [len(f)-1 : -1 : 0]) f[k]];

// =============================================================================
//  1.  DOME PROFILE   t^prof_n + r^prof_m = 1     (t = z/H, r = radius scale)
//      Sampled densely, then re-sampled at equal arc length so the mesh stays
//      even across the flat top and the near-vertical skirt.
// =============================================================================
KS   = 420;
RAVG = (shade_length + shade_width)/4;

// [r, t] pairs, apex (phi=0) -> rim (phi=90)
PS = [for (k = [0:KS])
        let (p = 90*k/KS) [ pow(sin(p), 2/prof_m), pow(cos(p), 2/prof_n) ] ];

PSL = [for (k = [1:KS])
         norm([ (PS[k][0]-PS[k-1][0])*RAVG, (PS[k][1]-PS[k-1][1])*shade_height ]) ];

PSC = [for (k = 0, s = 0; k <= KS; k = k+1,
           s = s + ((k >= 1 && k <= KS) ? PSL[k-1] : 0)) s];
LTOT = PSC[KS];

function findk(s, lo, hi) =
    (hi - lo <= 1) ? lo
  : let (mid = floor((lo+hi)/2)) (PSC[mid] > s ? findk(s,lo,mid) : findk(s,mid,hi));

// fraction of arc length (0 = apex, 1 = rim)  ->  [r, t]
function prof_at(f) =
    let (s = LTOT*f,
         k = findk(s, 0, KS),
         d = PSC[k+1] - PSC[k],
         w = (d > 0) ? (s - PSC[k])/d : 0)
    [ PS[k][0] + (PS[k+1][0]-PS[k][0])*w,
      PS[k][1] + (PS[k+1][1]-PS[k][1])*w ];

// closed-form height for a given radius scale (used by the inlay)
function zprof(r) = shade_height * pow(max(0, 1 - pow(min(r,1), prof_m)), 1/prof_n);

// ring tables (index j = 0 .. NV-1, j = NV-1 is the rim)
RHOT = [for (j = [1:NV]) prof_at(j/NV) ];
RHO  = [for (j = [0:NV-1]) RHOT[j][0] ];
ZZ   = [for (j = [0:NV-1]) RHOT[j][1]*shade_height ];
FAMP = [for (j = [0:NV-1]) flute_depth * pow(RHO[j], flute_ramp) ];

// =============================================================================
//  2.  PLAN CROSS-SECTION  (super-ellipse) + equal-arc-length flute phase
// =============================================================================
CS = [for (i = [0:NU-1])
        let (th = 360*i/NU)
        [ sgnpow(cos(th), P_EXP)*A, sgnpow(sin(th), P_EXP)*B ] ];

CSL = [for (i = [0:NU-1]) norm(CS[(i+1)%NU] - CS[i]) ];
CSC = [for (i = 0, s = 0; i <= NU; i = i+1,
           s = s + ((i >= 1 && i <= NU) ? CSL[i-1] : 0)) s];

// outward 2-D normal of the plan outline (self-similar for every ring)
CSN = [for (i = [0:NU-1])
         let (d = CS[(i+1)%NU] - CS[(i+NU-1)%NU]) unitv([d[1], -d[0]]) ];

// normalised arc length -> flute wave.  WV in [-1,1]; FL in [-1,0] so the
// ridges sit exactly on the nominal envelope and the valleys cut inward,
// which keeps the shade exactly shade_length x shade_width in plan.
WV = [for (i = [0:NU-1]) cos(flute_count*360*CSC[i]/CSC[NU] + flute_phase) ];
// u = 1 on a ridge, 0 in a crease; the exponent broadens the ridges
FL = [for (i = [0:NU-1]) pow((WV[i] + 1)/2, flute_sharp) - 1 ];
// draped rim: the ridges hang lowest, the valleys lift
ZW = [for (i = [0:NU-1]) -rim_wave*(WV[i] + 1)/2 ];

// =============================================================================
//  3.  SURFACE, VERTEX NORMALS, INNER OFFSET
// =============================================================================
function gp(i,j) =
    let (r = RHO[j], a = FAMP[j]*FL[i])
    [ CS[i][0]*r + CSN[i][0]*a,
      CS[i][1]*r + CSN[i][1]*a,
      ZZ[j] + ZW[i]*pow(r, rim_wave_ramp) ];

G = [for (j = [0:NV-1]) [for (i = [0:NU-1]) gp(i,j) ] ];

function gnorm(i,j) =
    let (ip = (i+1)%NU, im = (i+NU-1)%NU,
         du = G[j][ip] - G[j][im],
         dv = (j == 0)      ? G[1][i]   - G[0][i]
            : (j == NV-1)   ? G[j][i]   - G[j-1][i]
            :                 G[j+1][i] - G[j-1][i])
    unitv(cross(dv, du));

GI = [for (j = [0:NV-1]) [for (i = [0:NU-1]) G[j][i] - shade_wall*gnorm(i,j) ] ];

APEX_O = [0, 0, shade_height];
APEX_I = [0, 0, shade_height - shade_wall];

NPO = 1 + NV*NU;                    // outer apex + outer rings
function oi(i,j) = 1 + j*NU + i;
function ii(i,j) = NPO + 1 + j*NU + i;

SHADE_PTS = concat(
    [APEX_O],
    [for (j = [0:NV-1]) for (i = [0:NU-1]) G[j][i] ],
    [APEX_I],
    [for (j = [0:NV-1]) for (i = [0:NU-1]) GI[j][i] ]);

// faces written CCW seen from outside, then reversed (OpenSCAD wants CW)
F_CCW = concat(
    // outer apex fan
    [for (i = [0:NU-1]) [0, oi(i,0), oi((i+1)%NU,0)] ],
    // outer quads
    [for (j = [0:NV-2]) for (i = [0:NU-1])
        [oi(i,j), oi(i,j+1), oi((i+1)%NU,j+1), oi((i+1)%NU,j)] ],
    // inner apex fan (reversed)
    [for (i = [0:NU-1]) rev([NPO, ii(i,0), ii((i+1)%NU,0)]) ],
    // inner quads (reversed)
    [for (j = [0:NV-2]) for (i = [0:NU-1])
        rev([ii(i,j), ii(i,j+1), ii((i+1)%NU,j+1), ii((i+1)%NU,j)]) ],
    // rim band
    [for (i = [0:NU-1])
        [oi(i,NV-1), ii(i,NV-1), ii((i+1)%NU,NV-1), oi((i+1)%NU,NV-1)] ]);

SHADE_FACES = [for (f = F_CCW) rev(f)];

module shade_shell() {
    polyhedron(points = SHADE_PTS, faces = SHADE_FACES, convexity = 10);
}

// -----------------------------------------------------------------------------
//  smooth (un-fluted) dome solid, offset by `off` - only used to cut the inlay
// -----------------------------------------------------------------------------
module dome_smooth(off = 0, nu = 72, nv = 26) {
    pts = concat(
        [[0,0,shade_height + off]],
        [for (j = [1:nv]) for (i = [0:nu-1])
            let (pr = prof_at(j/nv), r = pr[0], z = pr[1]*shade_height,
                 th = 360*i/nu,
                 xy = [sgnpow(cos(th),P_EXP)*A, sgnpow(sin(th),P_EXP)*B],
                 d  = [sgnpow(cos(th+1),P_EXP)*A, sgnpow(sin(th+1),P_EXP)*B]
                      - [sgnpow(cos(th-1),P_EXP)*A, sgnpow(sin(th-1),P_EXP)*B],
                 n2 = unitv([d[1],-d[0]]),
                 // approximate outward normal of the smooth dome
                 dz = (j < nv) ? (prof_at((j+1)/nv)[1] - pr[1])*shade_height : 0,
                 dr = (j < nv) ? (prof_at((j+1)/nv)[0] - r)*RAVG : 1,
                 nn = unitv([n2[0]*(-dz), n2[1]*(-dz), dr]))
            [xy[0]*r + nn[0]*off, xy[1]*r + nn[1]*off, z + nn[2]*off] ],
        [[0,0,-1]]);                       // bottom hub for the flat cap
    last = len(pts)-1;
    fo = concat(
        [for (i = [0:nu-1]) [0, 1+i, 1+(i+1)%nu] ],
        [for (j = [0:nv-2]) for (i = [0:nu-1])
            [1+j*nu+i, 1+(j+1)*nu+i, 1+(j+1)*nu+(i+1)%nu, 1+j*nu+(i+1)%nu] ],
        [for (i = [0:nu-1]) [last, 1+(nv-1)*nu+(i+1)%nu, 1+(nv-1)*nu+i] ]);
    polyhedron(points = pts, faces = [for (f = fo) rev(f)], convexity = 6);
}

// =============================================================================
//  4.  BRASS INLAY ON TOP OF THE SHADE
// =============================================================================
module stadium(l, w) {                       // 2-D pill
    c = max(0, l/2 - w/2);
    hull() { translate([-c,0]) circle(d=w); translate([c,0]) circle(d=w); }
}

module top_inlay() {
    intersection() {
        translate([0, strip_offset, 0])
            linear_extrude(shade_height + 2) stadium(strip_length, strip_width);
        difference() {
            dome_smooth(strip_proud);          // top face (flush by default)
            dome_smooth(-strip_depth);         // let into the glass
        }
    }
}

// =============================================================================
//  5.  BASE, STEM, FITTINGS
// =============================================================================
module torus(r_out, tube) {
    rotate_extrude(convexity = 4) translate([r_out - tube, 0]) circle(r = tube);
}

module rdisc(r, h, ft, fb) {                 // disc, filleted top & bottom edges
    hull() {
        translate([0,0,fb])   torus(r, fb);
        translate([0,0,h-ft]) torus(r, ft);
    }
}

module base_plate() {
    c = base_length/2 - base_width/2;
    difference() {
        hull() {
            translate([-c,0,0]) rdisc(base_width/2, base_thick, base_fillet_top, base_fillet_bot);
            translate([ c,0,0]) rdisc(base_width/2, base_thick, base_fillet_top, base_fillet_bot);
        }
        // recessed felt pad underneath
        translate([0,0,-0.01]) linear_extrude(0.06)
            stadium(base_length - 0.9, base_width - 0.9);
        if (show_cord)                       // cord exit
            translate([-base_length/2 + 0.2, 0, base_thick/2])
                rotate([0,-90,0]) cylinder(d = cord_dia + 0.06, h = 0.5, center = true);
    }
    // rotary dimmer knob
    translate([-knob_offset, 0, base_thick - 0.01]) {
        cylinder(d = knob_dia, h = 0.11);
        translate([0,0,0.11]) scale([1,1,0.45]) sphere(d = knob_dia);
    }
}

module stem() {
    h = RIM_Z + stem_into_shade - base_thick;
    translate([0,0,base_thick]) {
        cylinder(d1 = stem_dia + 0.30, d2 = stem_dia, h = 0.24);   // collar
        cylinder(d = stem_dia, h = h);
        // socket / LED housing at the top
        translate([0,0,h - 0.02]) cylinder(d1 = stem_dia, d2 = 1.05, h = 0.26);
        translate([0,0,h + 0.22]) cylinder(d = 1.05, h = 0.30);
    }
}

module led_module() {                        // 5.9 W integrated LED board
    translate([0,0,RIM_Z + stem_into_shade + 0.52])
        cylinder(d = 0.92, h = 0.09);
}

module cord() {
    // exits the back of the base, curves down and runs along the table top
    z0 = base_thick/2;
    r  = z0 - cord_dia/2;                 // stays on / above the table surface
    x0 = -base_length/2 + 0.30;
    translate([x0, 0, z0]) rotate([90,0,0]) rotate([0,0,180])
        rotate_extrude(angle = 90, $fn = 64) translate([r,0]) circle(d = cord_dia);
    translate([x0 - r, 0, cord_dia/2])
        rotate([0,-90,0]) cylinder(d = cord_dia, h = cord_stub);
    translate([x0 - r - cord_stub, 0, cord_dia/2]) sphere(d = cord_dia);
}

// =============================================================================
//  6.  ASSEMBLY
// =============================================================================
module _brass()  { if (tint) color([0.85,0.70,0.33]) children(); else children(); }
module _glass()  { if (tint) color([0.97,0.94,0.87]) children(); else children(); }
module _dark()   { if (tint) color([0.15,0.15,0.15]) children(); else children(); }

module shade_assembly() {
    _glass() translate([0,0,RIM_Z]) shade_shell();
    if (show_strip) _brass() translate([0,0,RIM_Z]) top_inlay();
}

module lamp() {
    _brass() base_plate();
    _brass() stem();
    if (show_led)  _glass() led_module();
    if (show_cord) _dark()  cord();
    shade_assembly();
}

// =============================================================================
//  7.  OUTPUT
// =============================================================================
scale(UNIT) {
    if (part == "assembly")  lamp();
    else if (part == "shade")     shade_shell();
    else if (part == "strip")     top_inlay();
    else if (part == "base")      base_plate();
    else if (part == "stem")      stem();
    else if (part == "shade_cut")
        difference() {
            shade_assembly();
            translate([-shade_length, 0, RIM_Z - 1])
                cube([2*shade_length, shade_length, shade_height + 2]);
        }
    else if (part == "exploded") {
        _brass() base_plate();
        translate([0,0,1.5]) _brass() stem();
        translate([0,0,4.0]) shade_assembly();
    }
}
