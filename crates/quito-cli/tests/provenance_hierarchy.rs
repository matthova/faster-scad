//! End-to-end regression for hierarchical provenance (editor↔preview linking).
//!
//! Exercises the real pipeline (parse → eval → geometry) — the geom-crate unit
//! tests build `Node::Provenance` trees by hand and so can't catch how the
//! evaluator actually nests wrappers around user-module calls and a top-level
//! `difference()`. This mirrors the Parthenon's shape: a top-level
//! `difference(){ wrapper(); <disabled tool> }` where `wrapper()` calls a nested
//! `part()`. Before this feature the whole model collapsed to the `difference()`
//! block; now each leaf carries the full outer→inner span stack.

use quito_geom::{render_provenance_cached, GeomCache, RustManifoldKernel};

/// The source text of a `[start,end)` byte span.
fn text<'a>(src: &'a str, s: &std::ops::Range<usize>) -> &'a str {
    &src[s.start..s.end]
}

#[test]
fn difference_over_nested_modules_keeps_full_span_stack() {
    // `part()` makes the distinctive 2×2×2 cube (volume 8); `wrapper()` calls it
    // plus a unit-cube sibling; the top-level difference has a disabled (Empty)
    // tool, exactly like CUTAWAY=off.
    let src = "\
module part() {
    cube([2, 2, 2]);
}
module wrapper() {
    part();
    translate([10, 0, 0]) cube([1, 1, 1]);
}
difference() {
    wrapper();
    if (false) cube([100, 100, 100]);
}
";
    let prog = quito_syntax::parse(src).unwrap();
    let out = quito_eval::eval_program(&prog).unwrap();
    let mut cache = GeomCache::new();
    let groups =
        render_provenance_cached(&out.node, &RustManifoldKernel::new(), &mut cache).unwrap();

    // Two leaves survive the (no-op) difference: the 2×2×2 statue-equivalent and
    // the unit-cube sibling — neither collapsed into the difference block.
    assert_eq!(groups.len(), 2, "expected one group per leaf statement");

    // The statue-equivalent: the volume-8 cube.
    let statue = groups
        .iter()
        .find(|g| (g.mesh.volume() - 8.0).abs() < 1e-6)
        .expect("no volume-8 leaf");

    let stack: Vec<&str> = statue.spans.iter().map(|s| text(src, s)).collect();

    // Full outer→inner chain, no collapse to the difference block.
    assert!(stack[0].starts_with("difference()"), "outermost: {stack:?}");
    assert_eq!(stack[1], "wrapper();", "wrapper call level: {stack:?}");
    assert_eq!(stack[2], "part();", "part call level: {stack:?}");
    assert_eq!(stack[3], "cube([2, 2, 2]);", "deepest: {stack:?}");
    assert_eq!(stack.len(), 4, "unexpected stack depth: {stack:?}");

    // Preview→editor click: the deepest span is what a click selects.
    let deepest = statue.spans.last().unwrap();
    assert_eq!(text(src, deepest), "cube([2, 2, 2]);");

    // Editor→preview hierarchical highlight: a cursor on the `part()` call (byte
    // inside its span) must resolve — via narrowest-containing — to that span,
    // which the whole statue subtree carries. Emulate the frontend's Stage A.
    let part_call = src.find("part();").unwrap() + 1; // a byte inside `part()`
    let narrowest = groups
        .iter()
        .flat_map(|g| g.spans.iter())
        .filter(|s| part_call >= s.start && part_call < s.end)
        .min_by_key(|s| s.end - s.start)
        .expect("no span contains the part() call");
    assert_eq!(text(src, narrowest), "part();");
    // Every group whose stack contains that span highlights — here, just the statue.
    let lit: Vec<f64> = groups
        .iter()
        .filter(|g| g.spans.iter().any(|s| s == narrowest))
        .map(|g| g.mesh.volume())
        .collect();
    assert_eq!(lit.len(), 1);
    assert!((lit[0] - 8.0).abs() < 1e-6);
}
