// Curated table of OpenSCAD built-in modules and functions, with one-line
// signatures and docs for editor autocompletion.
//
// Mirror of crates/openrscad-lsp/src/builtins.rs — keep in sync. When you add,
// remove, or reword an entry there, make the matching change here (name,
// isModule, signature, doc). builtins.test.ts asserts name parity against the
// Rust file.

/** A built-in module or function. */
export interface Builtin {
  name: string;
  /** True for modules (statement-level, `cube(...)`), false for functions
   *  (expression-level, `sin(...)`). */
  isModule: boolean;
  /** A one-line signature, e.g. `cube(size, center)`. */
  signature: string;
  /** A short human description. */
  doc: string;
}

/** The built-in surface. Ordered roughly by category for readability;
 *  completion iterates the whole list so order is not significant. */
export const BUILTINS: Builtin[] = [
  // ---- 3D primitives ----
  {
    name: "cube",
    isModule: true,
    signature: "cube(size, center=false)",
    doc: "Axis-aligned box. `size` is a scalar or `[x,y,z]`.",
  },
  {
    name: "sphere",
    isModule: true,
    signature: "sphere(r | d, $fn, $fa, $fs)",
    doc: "Sphere of radius `r` (or diameter `d`).",
  },
  {
    name: "cylinder",
    isModule: true,
    signature: "cylinder(h, r | r1,r2 | d, center=false)",
    doc: "Cylinder or cone of height `h`.",
  },
  {
    name: "polyhedron",
    isModule: true,
    signature: "polyhedron(points, faces, convexity)",
    doc: "Arbitrary solid from vertices and faces.",
  },
  // ---- 2D primitives ----
  {
    name: "square",
    isModule: true,
    signature: "square(size, center=false)",
    doc: "Axis-aligned 2D rectangle.",
  },
  {
    name: "circle",
    isModule: true,
    signature: "circle(r | d, $fn)",
    doc: "2D circle of radius `r` (or diameter `d`).",
  },
  {
    name: "polygon",
    isModule: true,
    signature: "polygon(points, paths, convexity)",
    doc: "2D polygon from a list of points.",
  },
  {
    name: "text",
    isModule: true,
    signature:
      "text(t, size, font, halign, valign, spacing, direction, language, script)",
    doc: "2D text outlines.",
  },
  {
    name: "import",
    isModule: true,
    signature: "import(file, convexity, ...)",
    doc: "Import geometry from STL/OFF/DXF/SVG.",
  },
  // ---- Transforms ----
  {
    name: "translate",
    isModule: true,
    signature: "translate([x,y,z])",
    doc: "Move children by a vector.",
  },
  {
    name: "rotate",
    isModule: true,
    signature: "rotate(a | [x,y,z] | a, v)",
    doc: "Rotate children (degrees).",
  },
  {
    name: "scale",
    isModule: true,
    signature: "scale([x,y,z])",
    doc: "Scale children by a vector or scalar.",
  },
  {
    name: "resize",
    isModule: true,
    signature: "resize([x,y,z], auto)",
    doc: "Resize children to absolute dimensions.",
  },
  {
    name: "mirror",
    isModule: true,
    signature: "mirror([x,y,z])",
    doc: "Mirror children across a plane through the origin.",
  },
  {
    name: "multmatrix",
    isModule: true,
    signature: "multmatrix(m)",
    doc: "Apply a 4×3/4×4 affine matrix to children.",
  },
  {
    name: "color",
    isModule: true,
    signature: 'color(c | "name", alpha=1)',
    doc: "Recolor children for preview.",
  },
  {
    name: "offset",
    isModule: true,
    signature: "offset(r | delta, chamfer)",
    doc: "Grow/shrink a 2D shape.",
  },
  {
    name: "hull",
    isModule: true,
    signature: "hull()",
    doc: "Convex hull of all children.",
  },
  {
    name: "minkowski",
    isModule: true,
    signature: "minkowski()",
    doc: "Minkowski sum of the children.",
  },
  // ---- Booleans ----
  {
    name: "union",
    isModule: true,
    signature: "union()",
    doc: "Combine all children into one solid.",
  },
  {
    name: "difference",
    isModule: true,
    signature: "difference()",
    doc: "Subtract later children from the first.",
  },
  {
    name: "intersection",
    isModule: true,
    signature: "intersection()",
    doc: "Keep only the volume shared by all children.",
  },
  // ---- Extrusion / projection ----
  {
    name: "linear_extrude",
    isModule: true,
    signature: "linear_extrude(height, center, twist, slices, scale, $fn)",
    doc: "Extrude a 2D shape along Z.",
  },
  {
    name: "rotate_extrude",
    isModule: true,
    signature: "rotate_extrude(angle=360, $fn)",
    doc: "Revolve a 2D shape around the Z axis.",
  },
  {
    name: "projection",
    isModule: true,
    signature: "projection(cut=false)",
    doc: "Project 3D geometry down to 2D.",
  },
  // ---- Control-flow modules ----
  {
    name: "for",
    isModule: true,
    signature: "for (var = range) ...",
    doc: "Iterate, instantiating children per value.",
  },
  {
    name: "intersection_for",
    isModule: true,
    signature: "intersection_for (var = range) ...",
    doc: "Intersect children across all iterations.",
  },
  {
    name: "if",
    isModule: true,
    signature: "if (cond) ... else ...",
    doc: "Conditionally instantiate children.",
  },
  {
    name: "let",
    isModule: true,
    signature: "let (var = value) ...",
    doc: "Bind variables for the children scope.",
  },
  {
    name: "children",
    isModule: true,
    signature: "children(idx?)",
    doc: "Instantiate the children passed to a module.",
  },
  {
    name: "echo",
    isModule: true,
    signature: "echo(values...)",
    doc: "Print values to the console.",
  },
  {
    name: "assert",
    isModule: true,
    signature: "assert(cond, message?)",
    doc: "Abort with a message if `cond` is false.",
  },
  {
    name: "render",
    isModule: true,
    signature: "render(convexity)",
    doc: "Force a full CSG render of children.",
  },
  // ---- Math functions ----
  {
    name: "sin",
    isModule: false,
    signature: "sin(deg)",
    doc: "Sine (degrees).",
  },
  {
    name: "cos",
    isModule: false,
    signature: "cos(deg)",
    doc: "Cosine (degrees).",
  },
  {
    name: "tan",
    isModule: false,
    signature: "tan(deg)",
    doc: "Tangent (degrees).",
  },
  {
    name: "asin",
    isModule: false,
    signature: "asin(x)",
    doc: "Arcsine, in degrees.",
  },
  {
    name: "acos",
    isModule: false,
    signature: "acos(x)",
    doc: "Arccosine, in degrees.",
  },
  {
    name: "atan",
    isModule: false,
    signature: "atan(x)",
    doc: "Arctangent, in degrees.",
  },
  {
    name: "atan2",
    isModule: false,
    signature: "atan2(y, x)",
    doc: "Two-argument arctangent, in degrees.",
  },
  { name: "abs", isModule: false, signature: "abs(x)", doc: "Absolute value." },
  {
    name: "sign",
    isModule: false,
    signature: "sign(x)",
    doc: "-1, 0, or 1 by sign of `x`.",
  },
  {
    name: "floor",
    isModule: false,
    signature: "floor(x)",
    doc: "Round down to an integer.",
  },
  {
    name: "ceil",
    isModule: false,
    signature: "ceil(x)",
    doc: "Round up to an integer.",
  },
  {
    name: "round",
    isModule: false,
    signature: "round(x)",
    doc: "Round to the nearest integer.",
  },
  { name: "sqrt", isModule: false, signature: "sqrt(x)", doc: "Square root." },
  {
    name: "pow",
    isModule: false,
    signature: "pow(base, exp)",
    doc: "Exponentiation.",
  },
  {
    name: "exp",
    isModule: false,
    signature: "exp(x)",
    doc: "e raised to `x`.",
  },
  {
    name: "ln",
    isModule: false,
    signature: "ln(x)",
    doc: "Natural logarithm.",
  },
  {
    name: "log",
    isModule: false,
    signature: "log(x)",
    doc: "Base-10 logarithm.",
  },
  {
    name: "min",
    isModule: false,
    signature: "min(a, b, ...) | min(vector)",
    doc: "Smallest of the arguments.",
  },
  {
    name: "max",
    isModule: false,
    signature: "max(a, b, ...) | max(vector)",
    doc: "Largest of the arguments.",
  },
  {
    name: "norm",
    isModule: false,
    signature: "norm(v)",
    doc: "Euclidean length of a vector.",
  },
  {
    name: "cross",
    isModule: false,
    signature: "cross(a, b)",
    doc: "Cross product of two 3-vectors.",
  },
  // ---- List / string functions ----
  {
    name: "len",
    isModule: false,
    signature: "len(value)",
    doc: "Length of a vector or string.",
  },
  {
    name: "concat",
    isModule: false,
    signature: "concat(a, b, ...)",
    doc: "Concatenate vectors/values into one list.",
  },
  {
    name: "lookup",
    isModule: false,
    signature: "lookup(key, table)",
    doc: "Linear-interpolated table lookup.",
  },
  {
    name: "str",
    isModule: false,
    signature: "str(values...)",
    doc: "Concatenate values into a string.",
  },
  {
    name: "chr",
    isModule: false,
    signature: "chr(codes...)",
    doc: "Unicode code point(s) to a string.",
  },
  {
    name: "ord",
    isModule: false,
    signature: "ord(char)",
    doc: "First character to its Unicode code point.",
  },
  {
    name: "search",
    isModule: false,
    signature: "search(match, table, num?)",
    doc: "Find matches in a list/string.",
  },
  {
    name: "is_undef",
    isModule: false,
    signature: "is_undef(x)",
    doc: "True if `x` is undefined.",
  },
  {
    name: "is_bool",
    isModule: false,
    signature: "is_bool(x)",
    doc: "True if `x` is a boolean.",
  },
  {
    name: "is_num",
    isModule: false,
    signature: "is_num(x)",
    doc: "True if `x` is a number.",
  },
  {
    name: "is_string",
    isModule: false,
    signature: "is_string(x)",
    doc: "True if `x` is a string.",
  },
  {
    name: "is_list",
    isModule: false,
    signature: "is_list(x)",
    doc: "True if `x` is a list/vector.",
  },
  {
    name: "is_function",
    isModule: false,
    signature: "is_function(x)",
    doc: "True if `x` is a function value.",
  },
  // ---- Special variables (offered as completions) ----
  {
    name: "$fn",
    isModule: false,
    signature: "$fn",
    doc: "Fixed number of fragments in a circle.",
  },
  {
    name: "$fa",
    isModule: false,
    signature: "$fa",
    doc: "Minimum angle per fragment (degrees).",
  },
  {
    name: "$fs",
    isModule: false,
    signature: "$fs",
    doc: "Minimum fragment size.",
  },
  { name: "$t", isModule: false, signature: "$t", doc: "Animation time, 0→1." },
  {
    name: "$preview",
    isModule: false,
    signature: "$preview",
    doc: "True during F5 preview, false during F6 render.",
  },
  {
    name: "$vpr",
    isModule: false,
    signature: "$vpr",
    doc: "Viewport rotation `[x,y,z]`.",
  },
  {
    name: "$vpt",
    isModule: false,
    signature: "$vpt",
    doc: "Viewport translation `[x,y,z]`.",
  },
  {
    name: "$vpd",
    isModule: false,
    signature: "$vpd",
    doc: "Viewport camera distance.",
  },
  {
    name: "$vpf",
    isModule: false,
    signature: "$vpf",
    doc: "Viewport field of view.",
  },
];
