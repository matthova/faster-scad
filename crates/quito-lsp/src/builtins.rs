//! Curated table of OpenSCAD built-in modules and functions, with one-line
//! signatures and docs for hover and completion. Reconstructed from public
//! OpenSCAD documentation (no GPL source consulted), scoped to what the Quito
//! engine implements.

/// A built-in module or function.
pub struct Builtin {
    pub name: &'static str,
    /// True for modules (statement-level, `cube(...)`), false for functions
    /// (expression-level, `sin(...)`).
    pub is_module: bool,
    /// A one-line signature, e.g. `cube(size, center)`.
    pub signature: &'static str,
    /// A short human description.
    pub doc: &'static str,
}

/// Render a builtin as Markdown for a hover popup.
pub fn hover_markdown(b: &Builtin) -> String {
    let kind = if b.is_module { "module" } else { "function" };
    format!(
        "```openscad\n{sig}\n```\n\n**{kind}** — {doc}",
        sig = b.signature,
        kind = kind,
        doc = b.doc
    )
}

/// Look up a builtin by exact name.
pub fn lookup(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

macro_rules! b {
    ($name:literal, module, $sig:literal, $doc:literal) => {
        Builtin {
            name: $name,
            is_module: true,
            signature: $sig,
            doc: $doc,
        }
    };
    ($name:literal, function, $sig:literal, $doc:literal) => {
        Builtin {
            name: $name,
            is_module: false,
            signature: $sig,
            doc: $doc,
        }
    };
}

/// The built-in surface. Ordered roughly by category for readability; lookup and
/// completion iterate the whole slice so order is not significant.
pub static BUILTINS: &[Builtin] = &[
    // ---- 3D primitives ----
    b!("cube", module, "cube(size, center=false)", "Axis-aligned box. `size` is a scalar or `[x,y,z]`."),
    b!("sphere", module, "sphere(r | d, $fn, $fa, $fs)", "Sphere of radius `r` (or diameter `d`)."),
    b!("cylinder", module, "cylinder(h, r | r1,r2 | d, center=false)", "Cylinder or cone of height `h`."),
    b!("polyhedron", module, "polyhedron(points, faces, convexity)", "Arbitrary solid from vertices and faces."),
    // ---- 2D primitives ----
    b!("square", module, "square(size, center=false)", "Axis-aligned 2D rectangle."),
    b!("circle", module, "circle(r | d, $fn)", "2D circle of radius `r` (or diameter `d`)."),
    b!("polygon", module, "polygon(points, paths, convexity)", "2D polygon from a list of points."),
    b!("text", module, "text(t, size, font, halign, valign, spacing, direction, language, script)", "2D text outlines."),
    b!("import", module, "import(file, convexity, ...)", "Import geometry from STL/OFF/DXF/SVG."),
    // ---- Transforms ----
    b!("translate", module, "translate([x,y,z])", "Move children by a vector."),
    b!("rotate", module, "rotate(a | [x,y,z] | a, v)", "Rotate children (degrees)."),
    b!("scale", module, "scale([x,y,z])", "Scale children by a vector or scalar."),
    b!("resize", module, "resize([x,y,z], auto)", "Resize children to absolute dimensions."),
    b!("mirror", module, "mirror([x,y,z])", "Mirror children across a plane through the origin."),
    b!("multmatrix", module, "multmatrix(m)", "Apply a 4×3/4×4 affine matrix to children."),
    b!("color", module, "color(c | \"name\", alpha=1)", "Recolor children for preview."),
    b!("offset", module, "offset(r | delta, chamfer)", "Grow/shrink a 2D shape."),
    b!("hull", module, "hull()", "Convex hull of all children."),
    b!("minkowski", module, "minkowski()", "Minkowski sum of the children."),
    // ---- Booleans ----
    b!("union", module, "union()", "Combine all children into one solid."),
    b!("difference", module, "difference()", "Subtract later children from the first."),
    b!("intersection", module, "intersection()", "Keep only the volume shared by all children."),
    // ---- Extrusion / projection ----
    b!("linear_extrude", module, "linear_extrude(height, center, twist, slices, scale, $fn)", "Extrude a 2D shape along Z."),
    b!("rotate_extrude", module, "rotate_extrude(angle=360, $fn)", "Revolve a 2D shape around the Z axis."),
    b!("projection", module, "projection(cut=false)", "Project 3D geometry down to 2D."),
    // ---- Control-flow modules ----
    b!("for", module, "for (var = range) ...", "Iterate, instantiating children per value."),
    b!("intersection_for", module, "intersection_for (var = range) ...", "Intersect children across all iterations."),
    b!("if", module, "if (cond) ... else ...", "Conditionally instantiate children."),
    b!("let", module, "let (var = value) ...", "Bind variables for the children scope."),
    b!("children", module, "children(idx?)", "Instantiate the children passed to a module."),
    b!("echo", module, "echo(values...)", "Print values to the console."),
    b!("assert", module, "assert(cond, message?)", "Abort with a message if `cond` is false."),
    b!("render", module, "render(convexity)", "Force a full CSG render of children."),
    // ---- Math functions ----
    b!("sin", function, "sin(deg)", "Sine (degrees)."),
    b!("cos", function, "cos(deg)", "Cosine (degrees)."),
    b!("tan", function, "tan(deg)", "Tangent (degrees)."),
    b!("asin", function, "asin(x)", "Arcsine, in degrees."),
    b!("acos", function, "acos(x)", "Arccosine, in degrees."),
    b!("atan", function, "atan(x)", "Arctangent, in degrees."),
    b!("atan2", function, "atan2(y, x)", "Two-argument arctangent, in degrees."),
    b!("abs", function, "abs(x)", "Absolute value."),
    b!("sign", function, "sign(x)", "-1, 0, or 1 by sign of `x`."),
    b!("floor", function, "floor(x)", "Round down to an integer."),
    b!("ceil", function, "ceil(x)", "Round up to an integer."),
    b!("round", function, "round(x)", "Round to the nearest integer."),
    b!("sqrt", function, "sqrt(x)", "Square root."),
    b!("pow", function, "pow(base, exp)", "Exponentiation."),
    b!("exp", function, "exp(x)", "e raised to `x`."),
    b!("ln", function, "ln(x)", "Natural logarithm."),
    b!("log", function, "log(x)", "Base-10 logarithm."),
    b!("min", function, "min(a, b, ...) | min(vector)", "Smallest of the arguments."),
    b!("max", function, "max(a, b, ...) | max(vector)", "Largest of the arguments."),
    b!("norm", function, "norm(v)", "Euclidean length of a vector."),
    b!("cross", function, "cross(a, b)", "Cross product of two 3-vectors."),
    // ---- List / string functions ----
    b!("len", function, "len(value)", "Length of a vector or string."),
    b!("concat", function, "concat(a, b, ...)", "Concatenate vectors/values into one list."),
    b!("lookup", function, "lookup(key, table)", "Linear-interpolated table lookup."),
    b!("str", function, "str(values...)", "Concatenate values into a string."),
    b!("chr", function, "chr(codes...)", "Unicode code point(s) to a string."),
    b!("ord", function, "ord(char)", "First character to its Unicode code point."),
    b!("search", function, "search(match, table, num?)", "Find matches in a list/string."),
    b!("is_undef", function, "is_undef(x)", "True if `x` is undefined."),
    b!("is_bool", function, "is_bool(x)", "True if `x` is a boolean."),
    b!("is_num", function, "is_num(x)", "True if `x` is a number."),
    b!("is_string", function, "is_string(x)", "True if `x` is a string."),
    b!("is_list", function, "is_list(x)", "True if `x` is a list/vector."),
    b!("is_function", function, "is_function(x)", "True if `x` is a function value."),
    // ---- Special variables (offered as completions) ----
    b!("$fn", function, "$fn", "Fixed number of fragments in a circle."),
    b!("$fa", function, "$fa", "Minimum angle per fragment (degrees)."),
    b!("$fs", function, "$fs", "Minimum fragment size."),
    b!("$t", function, "$t", "Animation time, 0→1."),
    b!("$preview", function, "$preview", "True during F5 preview, false during F6 render."),
    b!("$vpr", function, "$vpr", "Viewport rotation `[x,y,z]`."),
    b!("$vpt", function, "$vpt", "Viewport translation `[x,y,z]`."),
    b!("$vpd", function, "$vpd", "Viewport camera distance."),
    b!("$vpf", function, "$vpf", "Viewport field of view."),
];
