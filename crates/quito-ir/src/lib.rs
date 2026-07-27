//! The CSG intermediate representation: a tree (later a DAG) of geometry
//! operations produced by evaluating a program, and consumed by the geometry
//! kernel.
//!
//! For M0 this is a plain tree. Structural hashing / DAG deduplication and
//! canonicalization (n-ary union flattening, transform folding) arrive in M3/M4.

pub type Vec3 = [f64; 3];

/// The `$fn` / `$fa` / `$fs` values in effect when a curved primitive was
/// instantiated. The concrete fragment count is derived from these plus the
/// radius by the geometry kernel (bit-exact fragment formula).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FragmentSpec {
    pub fn_: f64,
    pub fa: f64,
    pub fs: f64,
}

impl Default for FragmentSpec {
    fn default() -> Self {
        FragmentSpec {
            fn_: 0.0,
            fa: 12.0,
            fs: 2.0,
        }
    }
}

/// A node in the CSG tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// No geometry.
    Empty,
    /// An implicit group of children (unioned for rendering, kept as a list so
    /// transforms apply to the group as a whole).
    Group(Vec<Node>),

    // --- primitives ---
    Cube {
        size: Vec3,
        center: bool,
    },
    Sphere {
        r: f64,
        frags: FragmentSpec,
    },
    Cylinder {
        h: f64,
        r1: f64,
        r2: f64,
        center: bool,
        frags: FragmentSpec,
    },

    // --- transforms ---
    Translate {
        v: Vec3,
        child: Box<Node>,
    },
    /// Euler angles in degrees, applied X then Y then Z (OpenSCAD convention).
    Rotate {
        deg: Vec3,
        child: Box<Node>,
    },
    Scale {
        v: Vec3,
        child: Box<Node>,
    },

    // --- booleans ---
    Union(Vec<Node>),
    Difference(Vec<Node>),
    Intersection(Vec<Node>),
}

impl Node {
    /// Build a node from a list of children, wrapping in a group as needed.
    /// An empty list becomes [`Node::Empty`]; a single child is returned as-is.
    pub fn group(mut children: Vec<Node>) -> Node {
        children.retain(|c| !matches!(c, Node::Empty));
        match children.len() {
            0 => Node::Empty,
            1 => children.pop().unwrap(),
            _ => Node::Group(children),
        }
    }
}
