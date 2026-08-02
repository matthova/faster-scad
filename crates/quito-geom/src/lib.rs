//! Geometry: mesh types, the fragment formula + primitive tessellation, the
//! `Kernel` trait (CSG boolean backend), and the CSG-tree -> mesh renderer.

mod hull;
mod kernel;
mod mesh;
mod shape2d;
mod tessellate;
mod vector2d;

#[cfg(not(target_arch = "wasm32"))]
pub use kernel::ManifoldKernel;
pub use kernel::{BoolmeshKernel, Kernel, RustManifoldKernel};
pub use mesh::Mesh;
pub use shape2d::Contour;
pub use tessellate::{cube, cylinder, fragments, polyhedron, sphere};
pub use vector2d::{export_dxf, export_svg, import_dxf, import_svg};

/// Render a node's 2D profile as even-odd contours, or `None` if it isn't a 2D
/// object. Used by the CLI/engine to export 2D geometry to DXF/SVG.
pub fn render_contours(node: &Node) -> Option<Vec<Contour>> {
    is_2d(node).then(|| shape2d::render2d(node))
}

use quito_ir::{Node, Vec3};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, thiserror::Error)]
pub enum GeomError {
    #[error("kernel error: {0}")]
    Kernel(String),
    #[error("input geometry is not manifold: {0}")]
    NonManifold(String),
}

/// Non-fatal diagnostics from a render that still produced a mesh.
///
/// A render has three outcomes now: a hard failure (`Err(GeomError)`, no mesh);
/// a clean success; or a **degraded** success — the mesh is present but one or
/// more CSG ops failed (typically non-manifold operands) and were replaced by a
/// visible fallback so the whole model isn't blanked. The `errors` list carries
/// those degradations for a UI to flag; `warnings` carries softer notes (e.g. a
/// non-convex `minkowski` approximation). Both are deduped.
#[derive(Default, Debug, Clone)]
pub struct RenderDiagnostics {
    /// Soft warnings; the geometry is still exact.
    pub warnings: Vec<String>,
    /// Recoverable geometry errors: a CSG op failed and the result is an
    /// approximate fallback. Non-empty means the preview is geometrically wrong
    /// somewhere and the user should be alerted, even though a mesh exists.
    pub errors: Vec<String>,
}

impl RenderDiagnostics {
    /// Every message (warnings then errors) as one flat list, for callers that
    /// don't distinguish the two.
    fn flattened(mut self) -> Vec<String> {
        self.warnings.append(&mut self.errors);
        self.warnings
    }
}

/// A cached subtree render: its mesh plus the diagnostics its subtree produced.
/// Caching the diagnostics (not just the mesh) is what keeps a degradation
/// *sticky* across warm edits — a re-render that hits the cache for a degraded
/// subtree still re-reports the failure, so the UI's alert doesn't vanish when
/// the user edits an unrelated part of the model.
#[derive(Clone, Default)]
struct CachedNode {
    mesh: Mesh,
    warnings: Vec<String>,
    errors: Vec<String>,
}

/// A content-addressed geometry cache (M4): maps a structural hash of a CSG
/// subtree to its rendered mesh (and the diagnostics its subtree produced).
/// Reused across renders, it makes warm edits incremental — only subtrees whose
/// structure changed are re-rendered, the rest are cheap clones; within a single
/// render it also deduplicates identical subtrees (common-subexpression
/// elimination).
#[derive(Default)]
pub struct GeomCache {
    nodes: HashMap<u64, CachedNode>,
}

impl GeomCache {
    pub fn new() -> Self {
        GeomCache::default()
    }
    /// Number of cached subtrees.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    /// Drop all cached meshes.
    pub fn clear(&mut self) {
        self.nodes.clear();
    }
}

/// Shared state threaded through a single render traversal.
struct Ctx<'a> {
    kernel: &'a dyn Kernel,
    cache: &'a mut GeomCache,
    /// Precomputed structural hash of every node in the tree, by address.
    hashes: &'a HashMap<*const Node, u64>,
    /// Non-fatal geometry warnings collected during the render (e.g. non-convex
    /// `minkowski`). Deduped by the caller.
    warnings: &'a mut Vec<String>,
    /// Recoverable CSG failures collected during the render: a boolean/hull op
    /// failed (usually non-manifold operands) and was replaced by a fallback
    /// mesh so the render still yields geometry. Deduped by the caller.
    errors: &'a mut Vec<String>,
}

/// Render a CSG tree to a mesh using the default kernel for the target:
/// C++ Manifold on native, pure-Rust Manifold on wasm.
#[cfg(not(target_arch = "wasm32"))]
pub fn render(node: &Node) -> Result<Mesh, GeomError> {
    render_with(node, &ManifoldKernel::new())
}

/// Render a CSG tree to a mesh using the default kernel for the target:
/// C++ Manifold on native, pure-Rust Manifold on wasm.
#[cfg(target_arch = "wasm32")]
pub fn render(node: &Node) -> Result<Mesh, GeomError> {
    render_with(node, &RustManifoldKernel::new())
}

/// Render a CSG tree to a mesh using the given kernel (no persistent cache).
pub fn render_with(node: &Node, kernel: &dyn Kernel) -> Result<Mesh, GeomError> {
    let mut cache = GeomCache::new();
    render_cached(node, kernel, &mut cache)
}

/// Render colored preview groups using the default kernel (no persistent cache).
/// The fused [`render`] stays the source of truth for stats/export; this is the
/// preview/`color`-aware view (and the basis for colored 3MF export).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_groups(node: &Node) -> Result<Vec<ColoredMesh>, GeomError> {
    let mut cache = GeomCache::new();
    render_groups_cached(node, &ManifoldKernel::new(), &mut cache)
}

/// See [`render_groups`]. wasm target uses the pure-Rust kernel.
#[cfg(target_arch = "wasm32")]
pub fn render_groups(node: &Node) -> Result<Vec<ColoredMesh>, GeomError> {
    let mut cache = GeomCache::new();
    render_groups_cached(node, &RustManifoldKernel::new(), &mut cache)
}

/// Render using the given kernel and a caller-owned [`GeomCache`], enabling
/// incremental warm-edit re-renders (unchanged subtrees are not recomputed).
pub fn render_cached(
    node: &Node,
    kernel: &dyn Kernel,
    cache: &mut GeomCache,
) -> Result<Mesh, GeomError> {
    render_cached_warns(node, kernel, cache).map(|(m, _)| m)
}

/// Like [`render_cached`], but also returns non-fatal geometry warnings (e.g.
/// non-convex `minkowski`, which renders as the convex approximation), deduped.
/// Warnings and recoverable errors are flattened into one list; use
/// [`render_cached_diag`] to keep them separate (e.g. to alert a UI).
pub fn render_cached_warns(
    node: &Node,
    kernel: &dyn Kernel,
    cache: &mut GeomCache,
) -> Result<(Mesh, Vec<String>), GeomError> {
    let (mesh, diag) = render_cached_diag(node, kernel, cache)?;
    Ok((mesh, diag.flattened()))
}

/// Like [`render_cached_warns`], but keeps soft warnings and recoverable
/// geometry errors as separate lists (see [`RenderDiagnostics`]). A non-empty
/// `errors` list means the render degraded: a CSG op failed and the mesh is an
/// approximate fallback, so the caller should surface that to the user even
/// though geometry is present.
pub fn render_cached_diag(
    node: &Node,
    kernel: &dyn Kernel,
    cache: &mut GeomCache,
) -> Result<(Mesh, RenderDiagnostics), GeomError> {
    let mut hashes = HashMap::new();
    hash_all(node, &mut hashes);
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut ctx = Ctx {
        kernel,
        cache,
        hashes: &hashes,
        warnings: &mut warnings,
        errors: &mut errors,
    };
    let mesh = render_node(node, &mut ctx)?;
    warnings.sort();
    warnings.dedup();
    errors.sort();
    errors.dedup();
    Ok((mesh, RenderDiagnostics { warnings, errors }))
}

/// Memoized render of one node: cache hit → clone (and replay the subtree's
/// diagnostics); miss → render, capturing the diagnostics this subtree added, and
/// store them with the mesh so a later warm hit re-reports them.
fn render_node(node: &Node, ctx: &mut Ctx) -> Result<Mesh, GeomError> {
    let key = ctx.hashes[&(node as *const Node)];
    if let Some(entry) = ctx.cache.nodes.get(&key) {
        ctx.warnings.extend(entry.warnings.iter().cloned());
        ctx.errors.extend(entry.errors.iter().cloned());
        return Ok(entry.mesh.clone());
    }
    // Everything appended to ctx.warnings/errors while rendering this node is the
    // diagnostics of its subtree; snapshot the lengths to isolate that delta.
    let warn_start = ctx.warnings.len();
    let err_start = ctx.errors.len();
    let mesh = render_uncached(node, ctx)?;
    let warnings = ctx.warnings[warn_start..].to_vec();
    let errors = ctx.errors[err_start..].to_vec();
    ctx.cache.nodes.insert(
        key,
        CachedNode {
            mesh: mesh.clone(),
            warnings,
            errors,
        },
    );
    Ok(mesh)
}

/// How a preview group is displayed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DisplayMode {
    /// Normal solid (uses the group's color).
    Solid,
    /// `#` highlight — drawn translucent-red on top of the model.
    Highlight,
    /// `%` background — drawn translucent-gray and excluded from exports.
    Background,
}

/// A colored preview mesh: geometry plus its display color and mode.
pub struct ColoredMesh {
    pub mesh: Mesh,
    pub color: [f32; 4],
    pub mode: DisplayMode,
}

/// A geometry group tagged with the **stack** of enclosing source byte-spans that
/// produced it (outermost statement first, innermost last), for hierarchical
/// editor↔preview linking. Each entry is a `[start,end)` byte range into the main
/// source. The stack is **empty** for geometry with no attributable main-source
/// span (e.g. from an `include`d/`use`d file) — still emitted so the soup is
/// complete, but not pickable. The last (deepest) entry is what a click selects;
/// the whole stack is what the cursor→preview highlight matches against by
/// containment (so a cursor on any enclosing call lights that whole subtree).
pub struct TaggedMesh {
    pub mesh: Mesh,
    pub spans: Vec<std::ops::Range<usize>>,
}

/// Default preview color for uncolored geometry (the viewer's gold).
pub const DEFAULT_COLOR: [f32; 4] = [0.961, 0.647, 0.137, 1.0];

/// Whether the tree uses any display attribute (`color`/`#`/`%`), so callers can
/// skip the (additive) grouped-preview render for plain models.
pub fn has_display_attrs(node: &Node) -> bool {
    use Node::*;
    match node {
        Color { .. } | Highlight(_) | Background(_) => true,
        Group(cs) | Union(cs) | Difference(cs) | Intersection(cs) | Hull(cs) | Minkowski(cs) => {
            cs.iter().any(has_display_attrs)
        }
        Translate { child, .. }
        | Rotate { child, .. }
        | Scale { child, .. }
        | Mirror { child, .. }
        | MultMatrix { child, .. }
        | Resize { child, .. }
        | LinearExtrude { child, .. }
        | RotateExtrude { child, .. }
        | Offset { child, .. }
        | Projection { child, .. }
        | Provenance { child, .. } => has_display_attrs(child),
        _ => false,
    }
}

/// Render the tree into colored preview groups: each maximal subtree under a
/// single effective color/mode becomes one mesh. Booleans/hull/minkowski (and
/// resize/extrudes/projection/import) are opaque — rendered fused, taking the
/// enclosing color — matching OpenSCAD's "color applies to the *result* of the
/// subtree it wraps." Shares the cache with [`render_cached`], so opaque leaf
/// meshes are reused (never recomputed just because they are colored).
pub fn render_groups_cached(
    node: &Node,
    kernel: &dyn Kernel,
    cache: &mut GeomCache,
) -> Result<Vec<ColoredMesh>, GeomError> {
    let mut hashes = HashMap::new();
    hash_all(node, &mut hashes);
    let mut warnings = Vec::new();
    // The preview/color channel doesn't surface diagnostics (the fused render
    // above already did); degradations are recorded here but discarded.
    let mut errors = Vec::new();
    let mut ctx = Ctx {
        kernel,
        cache,
        hashes: &hashes,
        warnings: &mut warnings,
        errors: &mut errors,
    };
    let mut out = Vec::new();
    partition_groups(node, DEFAULT_COLOR, DisplayMode::Solid, &mut ctx, &mut out)?;
    coalesce_groups(&mut out);
    Ok(out)
}

/// Merge preview groups sharing the same display mode and (8-bit quantized)
/// color into one mesh each, preserving first-appearance order. A model that
/// emits many same-color solids (e.g. a `for` loop under a single `color()`, or
/// the many colored regions a recursed boolean now yields) collapses to a
/// handful of groups — capping viewer materials/draw-calls and colored-3MF
/// `<object>`s at the number of distinct colors. `Background`/`Highlight` stay
/// in their own buckets (drawn differently, and `%` is excluded from export).
fn coalesce_groups(groups: &mut Vec<ColoredMesh>) {
    let key = |g: &ColoredMesh| -> (u8, [u8; 4]) {
        let m = match g.mode {
            DisplayMode::Solid => 0u8,
            DisplayMode::Highlight => 1,
            DisplayMode::Background => 2,
        };
        let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        (
            m,
            [q(g.color[0]), q(g.color[1]), q(g.color[2]), q(g.color[3])],
        )
    };
    let mut index: HashMap<(u8, [u8; 4]), usize> = HashMap::new();
    let mut merged: Vec<ColoredMesh> = Vec::new();
    for g in groups.drain(..) {
        match index.get(&key(&g)) {
            Some(&i) => append_mesh(&mut merged[i].mesh, &g.mesh),
            None => {
                index.insert(key(&g), merged.len());
                merged.push(g);
            }
        }
    }
    *groups = merged;
}

/// Append `src`'s geometry onto `dst`, offsetting `src`'s triangle indices by
/// `dst`'s existing vertex count.
fn append_mesh(dst: &mut Mesh, src: &Mesh) {
    let base = dst.verts.len() as u32;
    dst.verts.extend_from_slice(&src.verts);
    dst.tris.extend(
        src.tris
            .iter()
            .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
    );
}

/// Flatten colored groups into one triangle soup (for GPU upload) plus a JSON
/// array of per-group ranges — the preview channel shipped across the
/// wasm/Tauri boundary. `start`/`count` are **vertex** offsets into the soup
/// (three.js `addGroup` units); `mode` is "solid"/"highlight"/"background".
pub fn preview_channel(groups: &[ColoredMesh]) -> (Vec<f32>, Vec<f32>, String) {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut json = String::from("[");
    for (i, g) in groups.iter().enumerate() {
        let (p, n) = g.mesh.to_triangle_soup_f32();
        let start = positions.len() / 3;
        let count = g.mesh.tris.len() * 3;
        positions.extend_from_slice(&p);
        normals.extend_from_slice(&n);
        let mode = match g.mode {
            DisplayMode::Solid => "solid",
            DisplayMode::Highlight => "highlight",
            DisplayMode::Background => "background",
        };
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"start\":{start},\"count\":{count},\"color\":[{},{},{},{}],\"mode\":\"{mode}\"}}",
            g.color[0], g.color[1], g.color[2], g.color[3]
        ));
    }
    json.push(']');
    (positions, normals, json)
}

/// Render per-statement provenance groups using the default kernel (no persistent
/// cache). See [`render_provenance_cached`].
#[cfg(not(target_arch = "wasm32"))]
pub fn render_provenance(node: &Node) -> Result<Vec<TaggedMesh>, GeomError> {
    let mut cache = GeomCache::new();
    render_provenance_cached(node, &ManifoldKernel::new(), &mut cache)
}

/// See [`render_provenance`]. wasm target uses the pure-Rust kernel.
#[cfg(target_arch = "wasm32")]
pub fn render_provenance(node: &Node) -> Result<Vec<TaggedMesh>, GeomError> {
    let mut cache = GeomCache::new();
    render_provenance_cached(node, &RustManifoldKernel::new(), &mut cache)
}

/// Render the tree into fine-grained provenance groups for **hierarchical**
/// editor↔preview linking. Each leaf region becomes one [`TaggedMesh`] carrying
/// the full **stack** of enclosing [`Node::Provenance`] spans the evaluator
/// inserted (outermost statement first, innermost last) — e.g. a statue sub-shape
/// under `difference(){ parthenon(); … }` carries `[difference, parthenon(),
/// athena_parthenos(), …, cylinder]`. This lets the frontend select the deepest
/// span on a click and highlight any enclosing level by containment on a cursor
/// move. 3D difference/intersection recurse into the base operand so nested spans
/// survive (see [`partition_provenance`]); hull/minkowski/extrude/resize/import
/// and 2D booleans remain opaque leaves taking the current stack. Groups are
/// **not** coalesced (each leaf stays its own group). Shares the cache with
/// [`render_cached`], so leaf meshes are reused (never recomputed for a span).
pub fn render_provenance_cached(
    node: &Node,
    kernel: &dyn Kernel,
    cache: &mut GeomCache,
) -> Result<Vec<TaggedMesh>, GeomError> {
    let mut hashes = HashMap::new();
    hash_all(node, &mut hashes);
    let mut warnings = Vec::new();
    // The provenance channel doesn't surface diagnostics (the fused render does);
    // degradations are recorded here but discarded.
    let mut errors = Vec::new();
    let mut ctx = Ctx {
        kernel,
        cache,
        hashes: &hashes,
        warnings: &mut warnings,
        errors: &mut errors,
    };
    let mut out = Vec::new();
    let mut stack = Vec::new();
    partition_provenance(node, &mut stack, &mut ctx, &mut out)?;
    Ok(out)
}

/// Flatten provenance groups into one triangle soup plus a JSON array of
/// per-group ranges — the provenance channel shipped across the wasm/Tauri
/// boundary. `start`/`count` are **vertex** offsets into the soup (three.js
/// `addGroup` units, same as [`preview_channel`]); `spans` is the outermost→
/// innermost stack of `[start,end]` byte offsets into the main source (an empty
/// array when unattributable). The last entry is the deepest statement.
pub fn provenance_channel(groups: &[TaggedMesh]) -> (Vec<f32>, Vec<f32>, String) {
    let mut positions: Vec<f32> = Vec::new();
    let mut normals: Vec<f32> = Vec::new();
    let mut json = String::from("[");
    for (i, g) in groups.iter().enumerate() {
        let (p, n) = g.mesh.to_triangle_soup_f32();
        let start = positions.len() / 3;
        let count = g.mesh.tris.len() * 3;
        positions.extend_from_slice(&p);
        normals.extend_from_slice(&n);
        if i > 0 {
            json.push(',');
        }
        let spans = g
            .spans
            .iter()
            .map(|s| format!("[{},{}]", s.start, s.end))
            .collect::<Vec<_>>()
            .join(",");
        json.push_str(&format!(
            "{{\"start\":{start},\"count\":{count},\"spans\":[{spans}]}}"
        ));
    }
    json.push(']');
    (positions, normals, json)
}

/// Walk the tree, emitting one [`TaggedMesh`] per leaf region (see
/// [`render_provenance_cached`]). `stack` is the outermost→innermost list of
/// enclosing provenance spans seen so far; each [`Node::Provenance`] pushes its
/// span (all levels are captured — no outermost-wins collapse), and a leaf clones
/// the current stack. Transparent through color/group/union and affine transforms
/// (which mutate produced sub-meshes exactly as [`partition_groups`] does). 3D
/// difference/intersection recurse into the base operand so nested spans survive,
/// subtracting/clipping the fused tools per region — mirroring `partition_groups`.
/// Everything else (hull/minkowski/resize/extrudes/projection/primitives/import
/// and 2D booleans) is an opaque leaf fused into one mesh taking the current stack.
fn partition_provenance(
    node: &Node,
    stack: &mut Vec<std::ops::Range<usize>>,
    ctx: &mut Ctx,
    out: &mut Vec<TaggedMesh>,
) -> Result<(), GeomError> {
    match node {
        Node::Empty => {}
        // Every provenance level is captured: push this span, recurse, pop. The
        // deepest span (last) is what a click selects; the whole stack is what the
        // cursor→preview highlight matches against by containment.
        Node::Provenance { span: s, child } => {
            stack.push(s.clone());
            let r = partition_provenance(child, stack, ctx, out);
            stack.pop();
            r?;
        }
        // Display attributes are transparent to provenance (picking spans a
        // statement regardless of its color/`#`/`%`). `%` background geometry is
        // rendered here (it's shown, translucent, in the preview) so it stays
        // pickable — unlike the fused/exported mesh, which excludes it.
        Node::Color { child, .. } | Node::Highlight(child) | Node::Background(child) => {
            partition_provenance(child, stack, ctx, out)?
        }
        // Transparent containers: recurse so each child keeps its own group.
        Node::Group(children) | Node::Union(children) => {
            for c in children {
                partition_provenance(c, stack, ctx, out)?;
            }
        }
        // Affine transforms distribute over sub-meshes: recurse, then transform
        // each produced mesh (reusing the fused path's vertex mutators).
        Node::Translate { v, child } => {
            let start = out.len();
            partition_provenance(child, stack, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|t| translate(&mut t.mesh, *v));
        }
        Node::Rotate { deg, child } => {
            let start = out.len();
            partition_provenance(child, stack, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|t| rotate(&mut t.mesh, *deg));
        }
        Node::Scale { v, child } => {
            let start = out.len();
            partition_provenance(child, stack, ctx, out)?;
            out[start..].iter_mut().for_each(|t| scale(&mut t.mesh, *v));
        }
        Node::Mirror { v, child } => {
            let start = out.len();
            partition_provenance(child, stack, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|t| mirror(&mut t.mesh, *v));
        }
        Node::MultMatrix { m, child } => {
            let start = out.len();
            partition_provenance(child, stack, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|t| mult_matrix(&mut t.mesh, m));
        }
        // A 3D difference recurses into the base operand so each base region keeps
        // its own span stack, then subtracts the fused tools from each region.
        // `∪(regionᵢ − tool) == (∪regionᵢ) − tool`, so the per-region union equals
        // the true fused difference. Sawn faces come from the base region and
        // inherit the cut statement's stack (the tool has no surviving geometry to
        // pick). 2D differences fall through to the opaque arm (clipped in-plane by
        // `render_node`).
        Node::Difference(children) if !is_2d(node) => {
            if let Some((base, tools)) = children.split_first() {
                let mut regions = Vec::new();
                partition_provenance(base, stack, ctx, &mut regions)?;
                // Fuse the tools, dropping empties (e.g. a disabled cutaway whose
                // operand is `Empty`) so a no-op difference does zero extra kernel
                // work and passes the base regions straight through.
                let tools: Vec<Mesh> = render_all(tools, ctx)?
                    .into_iter()
                    .filter(|m| !m.is_empty())
                    .collect();
                if tools.is_empty() {
                    out.append(&mut regions);
                } else {
                    let tool = ctx.kernel.union(tools)?;
                    let tool_bb = tool.bbox();
                    for region in regions {
                        // Only regions overlapping the tool's AABB can be cut;
                        // pass the rest through untouched (bounds the per-region
                        // boolean cost for a localized cutaway).
                        if !bbox_overlaps(region.mesh.bbox(), tool_bb) {
                            out.push(region);
                            continue;
                        }
                        let mesh = ctx.kernel.difference(region.mesh, vec![tool.clone()])?;
                        if !mesh.is_empty() {
                            out.push(TaggedMesh {
                                mesh,
                                spans: region.spans,
                            });
                        }
                    }
                }
            }
        }
        // A 3D intersection recurses into the first operand so each region keeps
        // its stack, then clips each by the fused intersection of the rest.
        // `∪(regionᵢ ∩ rest) == operand[0] ∩ rest`, attributed by the base.
        Node::Intersection(children) if !is_2d(node) => {
            if let Some((base, rest)) = children.split_first() {
                let mut regions = Vec::new();
                partition_provenance(base, stack, ctx, &mut regions)?;
                if rest.is_empty() {
                    // `intersection()` of a single operand is that operand.
                    out.append(&mut regions);
                } else {
                    let clip = ctx.kernel.intersection(render_all(rest, ctx)?)?;
                    let clip_bb = clip.bbox();
                    for region in regions {
                        // A region outside the clip's AABB survives nothing.
                        if !bbox_overlaps(region.mesh.bbox(), clip_bb) {
                            continue;
                        }
                        let mesh = ctx.kernel.intersection(vec![region.mesh, clip.clone()])?;
                        if !mesh.is_empty() {
                            out.push(TaggedMesh {
                                mesh,
                                spans: region.spans,
                            });
                        }
                    }
                }
            }
        }
        // Everything else (hull, minkowski, resize, extrudes, projection,
        // primitives, import, and 2D booleans) is opaque: one fused mesh taking
        // the current stack.
        _ => {
            let mesh = render_node(node, ctx)?;
            if !mesh.tris.is_empty() {
                out.push(TaggedMesh {
                    mesh,
                    spans: stack.clone(),
                });
            }
        }
    }
    Ok(())
}

fn partition_groups(
    node: &Node,
    color: [f32; 4],
    mode: DisplayMode,
    ctx: &mut Ctx,
    out: &mut Vec<ColoredMesh>,
) -> Result<(), GeomError> {
    match node {
        Node::Empty => {}
        // Display attributes set the effective color/mode for their subtree.
        Node::Color { rgba, child } => partition_groups(child, *rgba, mode, ctx, out)?,
        Node::Highlight(child) => partition_groups(child, color, DisplayMode::Highlight, ctx, out)?,
        Node::Background(child) => {
            partition_groups(child, color, DisplayMode::Background, ctx, out)?
        }
        // Provenance is transparent to color: recurse so the child keeps its own
        // colored regions. (The span is only read by the provenance partition.)
        Node::Provenance { child, .. } => partition_groups(child, color, mode, ctx, out)?,
        // Transparent to color: recurse so each child keeps its own regions.
        Node::Group(children) | Node::Union(children) => {
            for c in children {
                partition_groups(c, color, mode, ctx, out)?;
            }
        }
        // Affine transforms distribute over sub-meshes: recurse, then transform
        // each produced mesh (reusing the fused path's vertex mutators).
        Node::Translate { v, child } => {
            let start = out.len();
            partition_groups(child, color, mode, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|cm| translate(&mut cm.mesh, *v));
        }
        Node::Rotate { deg, child } => {
            let start = out.len();
            partition_groups(child, color, mode, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|cm| rotate(&mut cm.mesh, *deg));
        }
        Node::Scale { v, child } => {
            let start = out.len();
            partition_groups(child, color, mode, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|cm| scale(&mut cm.mesh, *v));
        }
        Node::Mirror { v, child } => {
            let start = out.len();
            partition_groups(child, color, mode, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|cm| mirror(&mut cm.mesh, *v));
        }
        Node::MultMatrix { m, child } => {
            let start = out.len();
            partition_groups(child, color, mode, ctx, out)?;
            out[start..]
                .iter_mut()
                .for_each(|cm| mult_matrix(&mut cm.mesh, m));
        }
        // A 3D difference keeps color: partition the base operand into its own
        // colored regions, then subtract the (colorless) fused tools from each
        // region. `∪(regionᵢ − tool) == (∪regionᵢ) − tool`, so the colored union
        // equals the true fused difference. Tool color is irrelevant to a
        // subtraction — sawn faces inherit the base solid's color. 2D differences
        // fall through to the opaque arm (clipped in-plane by `render_node`).
        Node::Difference(children) if !is_2d(node) => {
            if let Some((base, tools)) = children.split_first() {
                let mut regions = Vec::new();
                partition_groups(base, color, mode, ctx, &mut regions)?;
                // Fuse the tools into one mesh, dropping empties (e.g. a disabled
                // cutaway whose operand is `Empty`) so a no-op difference does
                // zero extra kernel work and passes the base regions through.
                let tools: Vec<Mesh> = render_all(tools, ctx)?
                    .into_iter()
                    .filter(|m| !m.is_empty())
                    .collect();
                if tools.is_empty() {
                    out.append(&mut regions);
                } else {
                    let tool = ctx.kernel.union(tools)?;
                    for ColoredMesh { mesh, color, mode } in regions {
                        let mesh = ctx.kernel.difference(mesh, vec![tool.clone()])?;
                        if !mesh.is_empty() {
                            out.push(ColoredMesh { mesh, color, mode });
                        }
                    }
                }
            }
        }
        // A 3D intersection keeps the first operand's color: partition it into
        // regions, then clip each by the fused intersection of the rest.
        // `∪(regionᵢ ∩ rest) == operand[0] ∩ rest`, colored by the base.
        Node::Intersection(children) if !is_2d(node) => {
            if let Some((base, rest)) = children.split_first() {
                let mut regions = Vec::new();
                partition_groups(base, color, mode, ctx, &mut regions)?;
                if rest.is_empty() {
                    // `intersection()` of a single operand is that operand.
                    out.append(&mut regions);
                } else {
                    let clip = ctx.kernel.intersection(render_all(rest, ctx)?)?;
                    for ColoredMesh { mesh, color, mode } in regions {
                        let mesh = ctx.kernel.intersection(vec![mesh, clip.clone()])?;
                        if !mesh.is_empty() {
                            out.push(ColoredMesh { mesh, color, mode });
                        }
                    }
                }
            }
        }
        // Everything else (hull, minkowski, resize, extrudes, projection,
        // primitives, import, and 2D booleans) is opaque: one fused mesh in the
        // current color. `Background` was already handled above, so the fused
        // render here still excludes any nested `%`.
        _ => {
            let mesh = render_node(node, ctx)?;
            if !mesh.tris.is_empty() {
                out.push(ColoredMesh { mesh, color, mode });
            }
        }
    }
    Ok(())
}

/// The actual per-variant renderer (children go back through [`render_node`]).
/// Is `node` a 2D subtree (result lies in the XY plane)?
/// Whether a node renders as a 2D object (its output is a flat profile, exportable
/// to DXF/SVG) rather than a 3D solid.
pub fn is_2d(node: &Node) -> bool {
    use Node::*;
    match node {
        Square { .. } | Circle { .. } | Polygon { .. } | Offset { .. } | Projection { .. } => true,
        Import { format, .. } => matches!(format.as_str(), "dxf" | "svg"),
        Cube { .. }
        | Sphere { .. }
        | Cylinder { .. }
        | Polyhedron { .. }
        | LinearExtrude { .. }
        | RotateExtrude { .. }
        | Empty => false,
        Translate { child, .. }
        | Rotate { child, .. }
        | Scale { child, .. }
        | Mirror { child, .. }
        | MultMatrix { child, .. }
        | Resize { child, .. } => is_2d(child),
        Group(cs) | Union(cs) | Difference(cs) | Intersection(cs) | Hull(cs) | Minkowski(cs) => {
            cs.iter().any(is_2d)
        }
        // Display attributes and provenance are transparent to geometry; `%`
        // background is excluded from the fused output, so it never contributes
        // 2D-ness.
        Color { child, .. } | Highlight(child) | Provenance { child, .. } => is_2d(child),
        Background(_) => false,
    }
}

/// Rewrite a 2D subtree, resolving every `Projection` to a `Polygon` leaf by
/// rendering its 3D child through the kernel. `shape2d::render2d` has no kernel,
/// so a projection reaching it (e.g. under `offset`/`hull`/`minkowski` or a bare
/// 2D boolean) would otherwise be dropped to empty geometry. The rest of the
/// tree is left structurally identical.
fn lower_projections(node: &Node, ctx: &mut Ctx) -> Result<Node, GeomError> {
    use Node::*;
    let lower = |c: &Node, ctx: &mut Ctx| lower_projections(c, ctx).map(Box::new);
    Ok(match node {
        Projection { cut, child } => {
            let mesh = render_node(child, ctx)?;
            let contours = if *cut {
                shape2d::slice_z0(&mesh)
            } else {
                shape2d::silhouette(&mesh)
            };
            contours_to_polygon(&contours)
        }
        Offset {
            r,
            delta,
            chamfer,
            frags,
            child,
        } => Offset {
            r: *r,
            delta: *delta,
            chamfer: *chamfer,
            frags: *frags,
            child: lower(child, ctx)?,
        },
        Translate { v, child } => Translate {
            v: *v,
            child: lower(child, ctx)?,
        },
        Rotate { deg, child } => Rotate {
            deg: *deg,
            child: lower(child, ctx)?,
        },
        Scale { v, child } => Scale {
            v: *v,
            child: lower(child, ctx)?,
        },
        Mirror { v, child } => Mirror {
            v: *v,
            child: lower(child, ctx)?,
        },
        MultMatrix { m, child } => MultMatrix {
            m: *m,
            child: lower(child, ctx)?,
        },
        Resize { new, auto, child } => Resize {
            new: *new,
            auto: *auto,
            child: lower(child, ctx)?,
        },
        // Transparent: keep the wrapper so provenance survives, lower the child.
        Provenance { span, child } => Provenance {
            span: span.clone(),
            child: lower(child, ctx)?,
        },
        Group(cs) => Group(lower_children(cs, ctx)?),
        Union(cs) => Union(lower_children(cs, ctx)?),
        Difference(cs) => Difference(lower_children(cs, ctx)?),
        Intersection(cs) => Intersection(lower_children(cs, ctx)?),
        Hull(cs) => Hull(lower_children(cs, ctx)?),
        Minkowski(cs) => Minkowski(lower_children(cs, ctx)?),
        other => other.clone(),
    })
}

fn lower_children(cs: &[Node], ctx: &mut Ctx) -> Result<Vec<Node>, GeomError> {
    cs.iter().map(|c| lower_projections(c, ctx)).collect()
}

/// Pack even-odd contours into a single `Polygon` node (one path per contour);
/// empty contour set → `Empty`. Round-trips through `render2d`'s polygon path.
fn contours_to_polygon(contours: &[Contour]) -> Node {
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut paths: Vec<Vec<u32>> = Vec::new();
    for c in contours {
        if c.len() < 3 {
            continue;
        }
        let start = points.len() as u32;
        paths.push((0..c.len() as u32).map(|i| start + i).collect());
        points.extend_from_slice(c);
    }
    if paths.is_empty() {
        Node::Empty
    } else {
        Node::Polygon {
            points,
            paths: Some(paths),
        }
    }
}

/// `render2d` with projections first lowered to polygons (see `lower_projections`).
fn render2d_lowered(node: &Node, ctx: &mut Ctx) -> Result<Vec<Contour>, GeomError> {
    Ok(shape2d::render2d(&lower_projections(node, ctx)?))
}

/// Run a CSG boolean and, on kernel failure, degrade instead of aborting: record
/// the error on `ctx` and return a raw concatenation of the operands so the
/// preview still shows *something*. This keeps a single non-manifold subtree from
/// blanking the entire model. `label` names the op for the error message.
///
/// The fallback is only built on the (rare) failure path. The kernel consumes
/// the operands, so on error we re-render the children — cheap, since they're
/// cache hits — and merge them. Difference is special: its fallback is the base
/// alone (the tools are meant to be *subtracted*, so drawing them as solids
/// would mislead), so it passes `base_only = true` and the base as the first
/// child.
fn boolean_or_fallback(
    children: &[Node],
    ctx: &mut Ctx,
    label: &str,
    base_only: bool,
    run: impl FnOnce(&dyn Kernel, Vec<Mesh>) -> Result<Mesh, GeomError>,
) -> Result<Mesh, GeomError> {
    let meshes = render_all(children, ctx)?;
    match run(ctx.kernel, meshes) {
        Ok(m) => Ok(m),
        Err(e) => {
            ctx.errors.push(format!(
                "{label}: {e} — showing un-combined geometry (the boolean was skipped)"
            ));
            // Re-render the operands (cache hits) for the fallback.
            let operands = render_all(children, ctx)?;
            let mut acc = Mesh::new();
            for (i, m) in operands.iter().enumerate() {
                if base_only && i > 0 {
                    break;
                }
                append_mesh(&mut acc, m);
            }
            Ok(acc)
        }
    }
}

fn render_uncached(node: &Node, ctx: &mut Ctx) -> Result<Mesh, GeomError> {
    // 2D CSG (boolean/hull/minkowski/group of 2D shapes) is clipped in the 2D
    // plane and returned as a flat mesh — the 3D kernel can't handle the
    // coplanar, zero-volume meshes these would otherwise produce.
    if matches!(
        node,
        Node::Group(_)
            | Node::Union(_)
            | Node::Difference(_)
            | Node::Intersection(_)
            | Node::Hull(_)
            | Node::Minkowski(_)
    ) && is_2d(node)
    {
        return Ok(shape2d::flat_mesh(&render2d_lowered(node, ctx)?));
    }
    match node {
        Node::Empty => Ok(Mesh::new()),
        Node::Cube { size, center } => Ok(cube(*size, *center)),
        Node::Sphere { r, frags } => Ok(sphere(*r, *frags)),
        Node::Cylinder {
            h,
            r1,
            r2,
            center,
            frags,
        } => Ok(cylinder(*h, *r1, *r2, *center, *frags)),
        Node::Polyhedron { points, faces } => Ok(polyhedron(points, faces)),
        Node::Import { data, format } => Ok(match format.as_str() {
            "off" => Mesh::from_off(&String::from_utf8_lossy(data)),
            "obj" => Mesh::from_obj(&String::from_utf8_lossy(data)),
            "3mf" => Mesh::from_3mf(data),
            "amf" => Mesh::from_amf(data),
            "dxf" | "svg" => shape2d::flat_mesh(&shape2d::render2d(node)),
            _ => Mesh::from_stl(data), // stl (binary or ascii)
        }),

        // 2D shapes rendered as a flat mesh at z=0.
        Node::Square { .. } | Node::Circle { .. } | Node::Polygon { .. } | Node::Offset { .. } => {
            Ok(shape2d::flat_mesh(&render2d_lowered(node, ctx)?))
        }
        Node::LinearExtrude {
            height,
            center,
            twist,
            scale,
            slices,
            child,
        } => extrude_csg(
            child,
            &|cs| shape2d::linear_extrude(cs, *height, *center, *twist, *scale, *slices),
            ctx,
        ),
        Node::RotateExtrude {
            angle,
            frags,
            child,
        } => extrude_csg(
            child,
            &|cs| shape2d::rotate_extrude(cs, *angle, *frags),
            ctx,
        ),
        Node::Projection { cut, child } => {
            let mesh = render_node(child, ctx)?;
            let contours = if *cut {
                shape2d::slice_z0(&mesh)
            } else {
                shape2d::silhouette(&mesh)
            };
            Ok(shape2d::flat_mesh(&contours))
        }

        Node::Group(children) => {
            boolean_or_fallback(children, ctx, "union", false, |k, m| k.union(m))
        }
        Node::Union(children) => {
            boolean_or_fallback(children, ctx, "union", false, |k, m| k.union(m))
        }
        Node::Intersection(children) => {
            boolean_or_fallback(children, ctx, "intersection", false, |k, m| {
                k.intersection(m)
            })
        }
        Node::Hull(children) => boolean_or_fallback(children, ctx, "hull", false, |k, m| k.hull(m)),
        Node::Minkowski(children) => {
            let meshes = render_all(children, ctx)?;
            // 3D minkowski is exact only for convex operands (hull of vertex
            // sums); a non-convex operand yields the convex approximation. Warn
            // rather than silently mislead. (2D minkowski is exact — see
            // shape2d::minkowski_2d.)
            if meshes.iter().any(|m| !m.is_empty() && !is_convex(m)) {
                ctx.warnings.push(
                    "minkowski: non-convex operand; result is the convex approximation \
                     (exact 3D minkowski is not yet implemented)"
                        .to_string(),
                );
            }
            Ok(minkowski_fold(meshes))
        }
        Node::Difference(children) => {
            if children.is_empty() {
                Ok(Mesh::new())
            } else {
                // On failure, fall back to the base alone: the tools are meant to
                // be subtracted, so drawing them as solids would mislead.
                boolean_or_fallback(children, ctx, "difference", true, |k, mut m| {
                    let base = m.remove(0);
                    k.difference(base, m)
                })
            }
        }

        Node::Translate { v, child } => {
            let mut m = render_node(child, ctx)?;
            translate(&mut m, *v);
            Ok(m)
        }
        Node::Rotate { deg, child } => {
            let mut m = render_node(child, ctx)?;
            rotate(&mut m, *deg);
            Ok(m)
        }
        Node::Scale { v, child } => {
            let mut m = render_node(child, ctx)?;
            scale(&mut m, *v);
            Ok(m)
        }
        Node::Mirror { v, child } => {
            let mut m = render_node(child, ctx)?;
            mirror(&mut m, *v);
            Ok(m)
        }
        Node::MultMatrix { m: mat, child } => {
            let mut mesh = render_node(child, ctx)?;
            mult_matrix(&mut mesh, mat);
            Ok(mesh)
        }
        Node::Resize { new, auto, child } => {
            let mut mesh = render_node(child, ctx)?;
            resize(&mut mesh, *new, *auto);
            Ok(mesh)
        }

        // Display attributes: `color`/`#` are transparent to the fused geometry;
        // `%` background is excluded from the rendered/exported mesh.
        Node::Color { child, .. } => render_node(child, ctx),
        Node::Highlight(child) => render_node(child, ctx),
        Node::Background(_) => Ok(Mesh::new()),
        // Provenance is transparent to the fused geometry: render the child.
        Node::Provenance { child, .. } => render_node(child, ctx),
    }
}

fn render_all(children: &[Node], ctx: &mut Ctx) -> Result<Vec<Mesh>, GeomError> {
    children.iter().map(|c| render_node(c, ctx)).collect()
}

/// Do two optional axis-aligned bounding boxes overlap? A `None` box (an empty
/// mesh) never overlaps. Used to skip a per-region boolean when a difference tool
/// / intersection clip cannot possibly touch a region (a conservative test: AABBs
/// may overlap when the meshes don't, which only costs an extra no-op boolean).
fn bbox_overlaps(a: Option<([f64; 3], [f64; 3])>, b: Option<([f64; 3], [f64; 3])>) -> bool {
    match (a, b) {
        (Some((alo, ahi)), Some((blo, bhi))) => {
            (0..3).all(|i| alo[i] <= bhi[i] && blo[i] <= ahi[i])
        }
        _ => false,
    }
}

/// Structural hash of every node in the tree, keyed by node address. Computed
/// once per render in a single O(n) post-order pass (child hashes combine into
/// the parent's), so [`render_node`] can look up any subtree's hash in O(1)
/// without re-traversing it.
fn hash_all(node: &Node, out: &mut HashMap<*const Node, u64>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::mem::discriminant(node).hash(&mut h);
    let bits = |x: &f64, h: &mut std::collections::hash_map::DefaultHasher| x.to_bits().hash(h);
    let frags = |f: &quito_ir::FragmentSpec, h: &mut std::collections::hash_map::DefaultHasher| {
        f.fn_.to_bits().hash(h);
        f.fa.to_bits().hash(h);
        f.fs.to_bits().hash(h);
    };
    match node {
        Node::Empty => {}
        Node::Group(cs)
        | Node::Union(cs)
        | Node::Difference(cs)
        | Node::Intersection(cs)
        | Node::Hull(cs)
        | Node::Minkowski(cs) => {
            for c in cs {
                hash_all(c, out).hash(&mut h);
            }
        }
        Node::Cube { size, center } => {
            for x in size {
                bits(x, &mut h);
            }
            center.hash(&mut h);
        }
        Node::Sphere { r, frags: f } => {
            bits(r, &mut h);
            frags(f, &mut h);
        }
        Node::Cylinder {
            h: hh,
            r1,
            r2,
            center,
            frags: f,
        } => {
            bits(hh, &mut h);
            bits(r1, &mut h);
            bits(r2, &mut h);
            center.hash(&mut h);
            frags(f, &mut h);
        }
        Node::Polyhedron { points, faces } => {
            for p in points {
                for x in p {
                    bits(x, &mut h);
                }
            }
            faces.hash(&mut h);
        }
        Node::Square { size, center } => {
            for x in size {
                bits(x, &mut h);
            }
            center.hash(&mut h);
        }
        Node::Circle { r, frags: f } => {
            bits(r, &mut h);
            frags(f, &mut h);
        }
        Node::Polygon { points, paths } => {
            for p in points {
                for x in p {
                    bits(x, &mut h);
                }
            }
            paths.hash(&mut h);
        }
        Node::LinearExtrude {
            height,
            center,
            twist,
            scale,
            slices,
            child,
        } => {
            bits(height, &mut h);
            center.hash(&mut h);
            bits(twist, &mut h);
            for x in scale {
                bits(x, &mut h);
            }
            slices.hash(&mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::RotateExtrude {
            angle,
            frags: f,
            child,
        } => {
            bits(angle, &mut h);
            frags(f, &mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::Offset {
            r,
            delta,
            chamfer,
            frags: f,
            child,
        } => {
            bits(r, &mut h);
            bits(delta, &mut h);
            chamfer.hash(&mut h);
            frags(f, &mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::Translate { v, child } | Node::Scale { v, child } | Node::Mirror { v, child } => {
            for x in v {
                bits(x, &mut h);
            }
            hash_all(child, out).hash(&mut h);
        }
        Node::Rotate { deg, child } => {
            for x in deg {
                bits(x, &mut h);
            }
            hash_all(child, out).hash(&mut h);
        }
        Node::MultMatrix { m, child } => {
            for row in m {
                for x in row {
                    bits(x, &mut h);
                }
            }
            hash_all(child, out).hash(&mut h);
        }
        Node::Resize { new, auto, child } => {
            for x in new {
                bits(x, &mut h);
            }
            auto.hash(&mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::Import { data, format } => {
            data.hash(&mut h);
            format.hash(&mut h);
        }
        Node::Projection { cut, child } => {
            cut.hash(&mut h);
            hash_all(child, out).hash(&mut h);
        }
        Node::Color { rgba, child } => {
            for c in rgba {
                c.to_bits().hash(&mut h);
            }
            hash_all(child, out).hash(&mut h);
        }
        Node::Highlight(child) | Node::Background(child) => {
            hash_all(child, out).hash(&mut h);
        }
        // Hash only the child (skip the span) so identical geometry at different
        // source lines dedupes in `GeomCache` — provenance never pollutes it.
        Node::Provenance { child, .. } => {
            hash_all(child, out).hash(&mut h);
        }
    }
    let val = h.finish();
    out.insert(node as *const Node, val);
    val
}

/// Whether a closed mesh is (approximately) convex: its volume matches its
/// convex hull's. A non-convex solid has strictly less volume than its hull; a
/// convex faceted mesh (e.g. a tessellated sphere) equals its own hull. The
/// relative tolerance absorbs tessellation/float noise.
fn is_convex(m: &Mesh) -> bool {
    let hull_vol = hull::convex_hull(&m.verts).volume();
    hull_vol <= 1e-9 || m.volume() >= hull_vol * (1.0 - 1e-3)
}

/// Minkowski sum of a chain of meshes. Exact for convex operands (the common
/// rounding case, e.g. `minkowski(){ cube; sphere; }`); for non-convex operands
/// it is the convex Minkowski approximation. After the first sum the accumulator
/// is convex, so the rest are exact.
fn minkowski_fold(meshes: Vec<Mesh>) -> Mesh {
    let mut it = meshes.into_iter().filter(|m| !m.is_empty());
    let Some(mut acc) = it.next() else {
        return Mesh::new();
    };
    for m in it {
        acc = minkowski_pair(&acc, &m);
    }
    acc
}

fn minkowski_pair(a: &Mesh, b: &Mesh) -> Mesh {
    let mut pts = Vec::with_capacity(a.verts.len() * b.verts.len());
    for va in &a.verts {
        for vb in &b.verts {
            pts.push([va[0] + vb[0], va[1] + vb[1], va[2] + vb[2]]);
        }
    }
    hull::convex_hull(&pts)
}

/// Extrude a 2D subtree, distributing over 2D booleans: because the extrusion
/// transform is applied identically to every operand (per height slice / per
/// revolution step), `extrude(A op B) == extrude(A) op extrude(B)`, so 2D CSG
/// is realized with the existing 3D kernel (no separate 2D kernel needed).
fn extrude_csg(
    node: &Node,
    extrude: &dyn Fn(&[shape2d::Contour]) -> Mesh,
    ctx: &mut Ctx,
) -> Result<Mesh, GeomError> {
    match node {
        Node::Empty => Ok(Mesh::new()),
        // Provenance is transparent to geometry: unwrap so the 2D-CSG
        // distribution arms below still see the underlying boolean/leaf.
        Node::Provenance { child, .. } => extrude_csg(child, extrude, ctx),
        Node::Union(children) | Node::Group(children) => {
            let meshes = children
                .iter()
                .map(|c| extrude_csg(c, extrude, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            ctx.kernel.union(meshes)
        }
        Node::Intersection(children) => {
            let meshes = children
                .iter()
                .map(|c| extrude_csg(c, extrude, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            ctx.kernel.intersection(meshes)
        }
        Node::Difference(children) => {
            let mut meshes = children
                .iter()
                .map(|c| extrude_csg(c, extrude, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            if meshes.is_empty() {
                Ok(Mesh::new())
            } else {
                let base = meshes.remove(0);
                ctx.kernel.difference(base, meshes)
            }
        }
        // projection: render the 3D child, flatten, then extrude.
        Node::Projection { cut, child } => {
            let mesh = render_node(child, ctx)?;
            let contours = if *cut {
                shape2d::slice_z0(&mesh)
            } else {
                shape2d::silhouette(&mesh)
            };
            Ok(extrude(&contours))
        }
        // A leaf 2D shape (primitive or transform chain, possibly wrapping a
        // projection): lower projections, then render to contours.
        leaf => Ok(extrude(&render2d_lowered(leaf, ctx)?)),
    }
}

fn translate(m: &mut Mesh, v: Vec3) {
    for p in &mut m.verts {
        p[0] += v[0];
        p[1] += v[1];
        p[2] += v[2];
    }
}

fn scale(m: &mut Mesh, v: Vec3) {
    for p in &mut m.verts {
        p[0] *= v[0];
        p[1] *= v[1];
        p[2] *= v[2];
    }
    // A negative determinant mirrors the mesh, inverting winding.
    if v[0] * v[1] * v[2] < 0.0 {
        m.flip_winding();
    }
}

/// Reflect across the plane through the origin with normal `v`.
fn mirror(m: &mut Mesh, v: Vec3) {
    let d = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if d == 0.0 {
        return;
    }
    // Householder reflection I - 2 v vᵀ / (v·v).
    let h = [
        [
            1.0 - 2.0 * v[0] * v[0] / d,
            -2.0 * v[0] * v[1] / d,
            -2.0 * v[0] * v[2] / d,
        ],
        [
            -2.0 * v[1] * v[0] / d,
            1.0 - 2.0 * v[1] * v[1] / d,
            -2.0 * v[1] * v[2] / d,
        ],
        [
            -2.0 * v[2] * v[0] / d,
            -2.0 * v[2] * v[1] / d,
            1.0 - 2.0 * v[2] * v[2] / d,
        ],
    ];
    for p in &mut m.verts {
        let [x, y, z] = *p;
        *p = [
            h[0][0] * x + h[0][1] * y + h[0][2] * z,
            h[1][0] * x + h[1][1] * y + h[1][2] * z,
            h[2][0] * x + h[2][1] * y + h[2][2] * z,
        ];
    }
    m.flip_winding(); // reflection inverts orientation
}

/// Apply a 4x4 affine matrix (row-major).
fn mult_matrix(m: &mut Mesh, mat: &[[f64; 4]; 4]) {
    for p in &mut m.verts {
        let [x, y, z] = *p;
        *p = [
            mat[0][0] * x + mat[0][1] * y + mat[0][2] * z + mat[0][3],
            mat[1][0] * x + mat[1][1] * y + mat[1][2] * z + mat[1][3],
            mat[2][0] * x + mat[2][1] * y + mat[2][2] * z + mat[2][3],
        ];
    }
    // Flip winding if the linear part has negative determinant.
    let det = mat[0][0] * (mat[1][1] * mat[2][2] - mat[1][2] * mat[2][1])
        - mat[0][1] * (mat[1][0] * mat[2][2] - mat[1][2] * mat[2][0])
        + mat[0][2] * (mat[1][0] * mat[2][1] - mat[1][1] * mat[2][0]);
    if det < 0.0 {
        m.flip_winding();
    }
}

/// Scale so the bounding box matches `new` (0 = keep; `auto` scales that axis
/// by another axis's factor).
fn resize(m: &mut Mesh, new: Vec3, auto: [bool; 3]) {
    let Some((lo, hi)) = m.bbox() else { return };
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let mut factor = [1.0; 3];
    let mut explicit = None;
    for i in 0..3 {
        if new[i] > 0.0 && size[i] > 0.0 {
            factor[i] = new[i] / size[i];
            if explicit.is_none() {
                explicit = Some(factor[i]);
            }
        }
    }
    // auto axes with no explicit target adopt the first explicit factor.
    if let Some(f) = explicit {
        for i in 0..3 {
            if new[i] == 0.0 && auto[i] {
                factor[i] = f;
            }
        }
    }
    for p in &mut m.verts {
        for i in 0..3 {
            p[i] *= factor[i];
        }
    }
    if factor[0] * factor[1] * factor[2] < 0.0 {
        m.flip_winding();
    }
}

/// Rotate by Euler angles (degrees), applied X then Y then Z (OpenSCAD order:
/// the combined matrix is Rz * Ry * Rx).
fn rotate(m: &mut Mesh, deg: Vec3) {
    let (a, b, c) = (
        deg[0].to_radians(),
        deg[1].to_radians(),
        deg[2].to_radians(),
    );
    let (sa, ca) = (a.sin(), a.cos());
    let (sb, cb) = (b.sin(), b.cos());
    let (sc, cc) = (c.sin(), c.cos());
    for p in &mut m.verts {
        let [x, y, z] = *p;
        // Rx
        let (y1, z1) = (y * ca - z * sa, y * sa + z * ca);
        let x1 = x;
        // Ry
        let (x2, z2) = (x1 * cb + z1 * sb, -x1 * sb + z1 * cb);
        let y2 = y1;
        // Rz
        let (x3, y3) = (x2 * cc - y2 * sc, x2 * sc + y2 * cc);
        *p = [x3, y3, z2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quito_ir::FragmentSpec;

    #[test]
    fn union_of_two_cubes_volume() {
        // two unit cubes overlapping in half -> volume 1.5
        let node = Node::Union(vec![
            Node::Cube {
                size: [1.0, 1.0, 1.0],
                center: false,
            },
            Node::Translate {
                v: [0.5, 0.0, 0.0],
                child: Box::new(Node::Cube {
                    size: [1.0, 1.0, 1.0],
                    center: false,
                }),
            },
        ]);
        let m = render(&node).unwrap();
        assert!((m.volume() - 1.5).abs() < 1e-6, "vol = {}", m.volume());
    }

    #[test]
    fn difference_hole() {
        // 20mm cube minus a through cylinder
        let node = Node::Difference(vec![
            Node::Cube {
                size: [20.0, 20.0, 20.0],
                center: true,
            },
            Node::Cylinder {
                h: 40.0,
                r1: 5.0,
                r2: 5.0,
                center: true,
                frags: FragmentSpec {
                    fn_: 64.0,
                    fa: 12.0,
                    fs: 2.0,
                },
            },
        ]);
        let m = render(&node).unwrap();
        let expected = 8000.0 - std::f64::consts::PI * 25.0 * 20.0;
        let rel = (m.volume() - expected).abs() / expected;
        assert!(
            rel < 0.01,
            "difference volume off by {rel}, vol={}",
            m.volume()
        );
    }

    #[test]
    fn intersection_box_sphere() {
        let node = Node::Intersection(vec![
            Node::Cube {
                size: [10.0, 10.0, 10.0],
                center: true,
            },
            Node::Sphere {
                r: 6.0,
                frags: FragmentSpec {
                    fn_: 64.0,
                    fa: 12.0,
                    fs: 2.0,
                },
            },
        ]);
        let m = render(&node).unwrap();
        assert!(m.volume() > 0.0);
        // intersection is smaller than the cube
        assert!(m.volume() < 1000.0);
    }

    #[test]
    fn linear_extrude_square() {
        let node = Node::LinearExtrude {
            height: 10.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Square {
                size: [4.0, 6.0],
                center: false,
            }),
        };
        let m = render(&node).unwrap();
        assert!((m.volume() - 240.0).abs() < 1e-6, "vol {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn projection_cut_section() {
        // Section a cube at z=0 (translated so z=0 is inside), extrude → prism.
        let node = Node::LinearExtrude {
            height: 3.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Projection {
                cut: true,
                child: Box::new(Node::Translate {
                    v: [0.0, 0.0, 1.0],
                    child: Box::new(Node::Cube {
                        size: [8.0, 6.0, 10.0],
                        center: true,
                    }),
                }),
            }),
        };
        let m = render(&node).unwrap();
        assert!(
            (m.volume() - 144.0).abs() < 1e-6,
            "projection vol {}",
            m.volume()
        );
    }

    #[test]
    fn offset_shapes() {
        let frags = FragmentSpec {
            fn_: 64.0,
            fa: 12.0,
            fs: 2.0,
        };
        let ext = |child| Node::LinearExtrude {
            height: 1.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(child),
        };
        // mitred grow: 10x10 -> 14x14 = 196
        let m = render(&ext(Node::Offset {
            r: 0.0,
            delta: 2.0,
            chamfer: false,
            frags,
            child: Box::new(Node::Square {
                size: [10.0, 10.0],
                center: false,
            }),
        }))
        .unwrap();
        assert!((m.volume() - 196.0).abs() < 1e-6, "miter {}", m.volume());
        // inset: 10x10 -> 6x6 = 36
        let m = render(&ext(Node::Offset {
            r: -2.0,
            delta: 0.0,
            chamfer: false,
            frags,
            child: Box::new(Node::Square {
                size: [10.0, 10.0],
                center: false,
            }),
        }))
        .unwrap();
        assert!((m.volume() - 36.0).abs() < 1e-6, "inset {}", m.volume());
    }

    /// Total (unsigned) area of a flat 2D mesh at z=0.
    fn flat_area(m: &Mesh) -> f64 {
        m.tris
            .iter()
            .map(|t| {
                let p = |i: u32| m.verts[i as usize];
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn bare_2d_difference() {
        // square(10) minus a 4×4 square hole → flat area 100 − 16 = 84 (the 3D
        // kernel would produce garbage on these coplanar flat meshes).
        let node = Node::Difference(vec![
            Node::Square {
                size: [10.0, 10.0],
                center: false,
            },
            Node::Translate {
                v: [3.0, 3.0, 0.0],
                child: Box::new(Node::Square {
                    size: [4.0, 4.0],
                    center: false,
                }),
            },
        ]);
        let m = render(&node).unwrap();
        assert!(
            (flat_area(&m) - 84.0).abs() < 1e-6,
            "area {}",
            flat_area(&m)
        );
    }

    #[test]
    fn bare_2d_intersection_and_union() {
        let sq = |x: f64, y: f64| Node::Translate {
            v: [x, y, 0.0],
            child: Box::new(Node::Square {
                size: [10.0, 10.0],
                center: false,
            }),
        };
        // Overlap of two squares offset by 5 → 5×5 = 25.
        let inter = Node::Intersection(vec![sq(0.0, 0.0), sq(5.0, 5.0)]);
        assert!((flat_area(&render(&inter).unwrap()) - 25.0).abs() < 1e-6);
        // Union of the same two → 200 − 25 overlap = 175.
        let uni = Node::Union(vec![sq(0.0, 0.0), sq(5.0, 5.0)]);
        assert!((flat_area(&render(&uni).unwrap()) - 175.0).abs() < 1e-6);
    }

    #[test]
    fn render2d_mirror_in_boolean() {
        // A mirror inside a 2D union must keep the mirrored child (regression:
        // render2d had no Mirror arm, so `mirror() halftooth` vanished — halving
        // every gear tooth). square [0,1]×[0,2] ∪ its y-mirror [0,1]×[-2,0] = 4.
        let sq = Node::Square {
            size: [1.0, 2.0],
            center: false,
        };
        let node = Node::Union(vec![
            sq.clone(),
            Node::Mirror {
                v: [0.0, 1.0, 0.0],
                child: Box::new(sq),
            },
        ]);
        let m = render(&node).unwrap();
        assert!((flat_area(&m) - 4.0).abs() < 1e-6, "area {}", flat_area(&m));
    }

    #[test]
    fn projection_silhouette() {
        // projection(cut=false) of a 10×20×30 box → its 10×20 footprint (200).
        let node = Node::Projection {
            cut: false,
            child: Box::new(Node::Cube {
                size: [10.0, 20.0, 30.0],
                center: false,
            }),
        };
        let m = render(&node).unwrap();
        assert!(
            (flat_area(&m) - 200.0).abs() < 1e-3,
            "area {}",
            flat_area(&m)
        );
    }

    /// A projection reached through the `render2d` path (here under `hull`) must
    /// not vanish. `render2d` has no kernel, so before A4 it dropped the
    /// projection to empty geometry; `lower_projections` now resolves it first.
    #[test]
    fn projection_inside_2d_op_not_dropped() {
        let node = Node::Hull(vec![Node::Projection {
            cut: false,
            child: Box::new(Node::Cube {
                size: [10.0, 20.0, 30.0],
                center: false,
            }),
        }]);
        let m = render(&node).unwrap();
        // hull of a 10×20 rectangle is the rectangle itself (area 200), not empty.
        assert!(
            (flat_area(&m) - 200.0).abs() < 1e-3,
            "hull-of-projection area {}",
            flat_area(&m)
        );
    }

    #[test]
    fn extrude_polygon_with_hole() {
        // A 10×10 square with a centered 4×4 hole (even-odd), extruded 1 mm →
        // volume 100 − 16 = 84. Exercises the earcut hole triangulation.
        let points = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0], // outer
            [3.0, 3.0],
            [3.0, 7.0],
            [7.0, 7.0],
            [7.0, 3.0], // hole
        ];
        let paths = Some(vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]]);
        let node = Node::LinearExtrude {
            height: 1.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Polygon { points, paths }),
        };
        let m = render(&node).unwrap();
        assert!((m.volume() - 84.0).abs() < 1e-6, "vol {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn minkowski_rounded_cube() {
        let frags = FragmentSpec {
            fn_: 24.0,
            fa: 12.0,
            fs: 2.0,
        };
        let node = Node::Minkowski(vec![
            Node::Cube {
                size: [10.0, 10.0, 10.0],
                center: true,
            },
            Node::Sphere { r: 2.0, frags },
        ]);
        let m = render(&node).unwrap();
        // matches OpenSCAD (~2592.88); allow small tessellation tolerance
        assert!(
            (m.volume() - 2592.88).abs() < 5.0,
            "minkowski vol {}",
            m.volume()
        );
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn minkowski_convex_3d_does_not_warn() {
        let frags = FragmentSpec {
            fn_: 16.0,
            fa: 12.0,
            fs: 2.0,
        };
        let node = Node::Minkowski(vec![
            Node::Cube {
                size: [10.0, 10.0, 10.0],
                center: true,
            },
            Node::Sphere { r: 2.0, frags },
        ]);
        let (_, warns) =
            render_cached_warns(&node, &ManifoldKernel::new(), &mut GeomCache::new()).unwrap();
        assert!(
            warns.is_empty(),
            "convex minkowski should not warn: {warns:?}"
        );
    }

    #[test]
    fn minkowski_nonconvex_3d_warns() {
        // Two cubes forming an L — a non-convex operand — summed with a cube.
        let lbar = Node::Union(vec![
            Node::Cube {
                size: [20.0, 6.0, 6.0],
                center: false,
            },
            Node::Cube {
                size: [6.0, 20.0, 6.0],
                center: false,
            },
        ]);
        let node = Node::Minkowski(vec![
            lbar,
            Node::Cube {
                size: [2.0, 2.0, 2.0],
                center: false,
            },
        ]);
        let (_, warns) =
            render_cached_warns(&node, &ManifoldKernel::new(), &mut GeomCache::new()).unwrap();
        assert!(
            warns.iter().any(|w| w.contains("non-convex")),
            "expected non-convex warning, got {warns:?}"
        );
    }

    #[test]
    fn extrude_2d_difference() {
        // linear_extrude of (square - circle) = a plate with a hole.
        let frags = FragmentSpec {
            fn_: 64.0,
            fa: 12.0,
            fs: 2.0,
        };
        let node = Node::LinearExtrude {
            height: 5.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Difference(vec![
                Node::Square {
                    size: [20.0, 20.0],
                    center: true,
                },
                Node::Circle { r: 5.0, frags },
            ])),
        };
        let m = render(&node).unwrap();
        let expected = (400.0 - std::f64::consts::PI * 25.0) * 5.0;
        let rel = (m.volume() - expected).abs() / expected;
        assert!(rel < 0.01, "plate vol off by {rel}: {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn rotate_extrude_torus() {
        // circle r=2 at radius 10 revolved -> torus, volume 2*pi^2*R*r^2.
        let frags = FragmentSpec {
            fn_: 64.0,
            fa: 12.0,
            fs: 2.0,
        };
        let node = Node::RotateExtrude {
            angle: 360.0,
            frags,
            child: Box::new(Node::Translate {
                v: [10.0, 0.0, 0.0],
                child: Box::new(Node::Circle { r: 2.0, frags }),
            }),
        };
        let m = render(&node).unwrap();
        let expected = 2.0 * std::f64::consts::PI.powi(2) * 10.0 * 4.0;
        let rel = (m.volume() - expected).abs() / expected;
        assert!(rel < 0.01, "torus vol off by {rel}: {}", m.volume());
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn cache_matches_cold_and_reuses() {
        let frags = FragmentSpec {
            fn_: 32.0,
            fa: 12.0,
            fs: 2.0,
        };
        let model = |r: f64| {
            Node::Difference(vec![
                Node::Cube {
                    size: [20.0, 20.0, 20.0],
                    center: true,
                },
                Node::Sphere { r, frags },
            ])
        };
        let kernel = RustManifoldKernel::new();
        let mut cache = GeomCache::new();

        // Warm render matches a cold render.
        let cold = render_with(&model(8.0), &kernel).unwrap();
        let warm = render_cached(&model(8.0), &kernel, &mut cache).unwrap();
        assert!((cold.volume() - warm.volume()).abs() < 1e-6);
        let after_first = cache.len();
        assert!(after_first > 0);

        // Re-rendering the identical tree adds nothing (pure cache hits).
        let again = render_cached(&model(8.0), &kernel, &mut cache).unwrap();
        assert!((again.volume() - warm.volume()).abs() < 1e-6);
        assert_eq!(
            cache.len(),
            after_first,
            "identical re-render should not grow cache"
        );

        // A warm edit (changed radius) reuses the unchanged cube leaf: only the
        // sphere and the difference are new, so the cache grows by exactly 2.
        let edited = render_cached(&model(7.0), &kernel, &mut cache).unwrap();
        let cold_edit = render_with(&model(7.0), &kernel).unwrap();
        assert!((edited.volume() - cold_edit.volume()).abs() < 1e-6);
        assert_eq!(
            cache.len(),
            after_first + 2,
            "warm edit should reuse the cube leaf"
        );
    }

    #[test]
    fn cache_cse_dedups_identical_subtrees() {
        // Two identical spheres in a union hash the same → rendered once.
        let frags = FragmentSpec {
            fn_: 32.0,
            fa: 12.0,
            fs: 2.0,
        };
        let node = Node::Union(vec![
            Node::Translate {
                v: [0.0, 0.0, 0.0],
                child: Box::new(Node::Sphere { r: 5.0, frags }),
            },
            Node::Translate {
                v: [20.0, 0.0, 0.0],
                child: Box::new(Node::Sphere { r: 5.0, frags }),
            },
        ]);
        let kernel = RustManifoldKernel::new();
        let mut cache = GeomCache::new();
        render_cached(&node, &kernel, &mut cache).unwrap();
        // Entries: 1 sphere (shared), 2 translates (distinct v), 1 union = 4.
        assert_eq!(
            cache.len(),
            4,
            "identical spheres should share one cache entry"
        );
    }

    #[test]
    fn non_manifold_boolean_degrades_instead_of_aborting() {
        // A lone open surface (a single triangle) is non-manifold: every edge is
        // a boundary. Unioning it with a valid cube must not blank the whole
        // render — the boolean is skipped, a fallback (the operands concatenated)
        // is returned, and the failure is reported as a recoverable error.
        let open_tri = Node::Polyhedron {
            points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            faces: vec![vec![0, 1, 2]],
        };
        let node = Node::Union(vec![
            Node::Cube {
                size: [10.0, 10.0, 10.0],
                center: false,
            },
            open_tri.clone(),
        ]);
        let kernel = RustManifoldKernel::new();
        let mut cache = GeomCache::new();
        let (mesh, diag) = render_cached_diag(&node, &kernel, &mut cache).unwrap();
        assert!(
            !mesh.tris.is_empty(),
            "degraded render should still yield geometry"
        );
        assert!(!diag.errors.is_empty(), "degradation must be reported");
        assert!(diag.errors[0].contains("union"), "{:?}", diag.errors);
        // render_cached_warns flattens the error into its single list.
        let (_, warns) = render_cached_warns(&node, &kernel, &mut cache).unwrap();
        assert!(warns.iter().any(|w| w.contains("union")), "{warns:?}");

        // Difference degrades to the base alone (tools are dropped, not drawn).
        // Two non-manifold tools force the tool-union step, which fails.
        let diff = Node::Difference(vec![
            Node::Cube {
                size: [10.0, 10.0, 10.0],
                center: false,
            },
            open_tri.clone(),
            open_tri,
        ]);
        let (dmesh, ddiag) = render_cached_diag(&diff, &kernel, &mut cache).unwrap();
        assert!(!dmesh.tris.is_empty());
        assert!(
            ddiag.errors.iter().any(|e| e.contains("difference")),
            "{:?}",
            ddiag.errors
        );
    }

    #[test]
    fn extrude_polygon_with_duplicate_vertices_is_manifold() {
        // A profile with consecutive duplicate points (a zero-length edge) must
        // still extrude to a clean manifold solid — `prepare` drops the repeat
        // so no degenerate side wall is emitted. Generated profiles (e.g.
        // BOSL2's rack2d) routinely contain such duplicates; before the fix the
        // extrusion was non-manifold and any union over it degraded.
        let points = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 0.0], // duplicate of the previous vertex
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0], // closing repeat of the first vertex
        ];
        let plate = Node::LinearExtrude {
            height: 5.0,
            center: false,
            twist: 0.0,
            scale: [1.0, 1.0],
            slices: 1,
            child: Box::new(Node::Polygon {
                points,
                paths: None,
            }),
        };
        // Volume is exactly the 10×10×5 box — the duplicates change nothing.
        let m = render(&plate).unwrap();
        assert!((m.volume() - 500.0).abs() < 1e-6, "vol {}", m.volume());

        // Unioning it with a disjoint cube must run the boolean cleanly (a
        // non-manifold operand would degrade and report a "union" error).
        let node = Node::Union(vec![
            plate,
            Node::Cube {
                size: [1.0, 1.0, 1.0],
                center: false,
            },
        ]);
        let kernel = RustManifoldKernel::new();
        let mut cache = GeomCache::new();
        let (mesh, diag) = render_cached_diag(&node, &kernel, &mut cache).unwrap();
        assert!(
            diag.errors.is_empty(),
            "unexpected degradation: {:?}",
            diag.errors
        );
        assert!(!mesh.tris.is_empty());
    }

    #[test]
    fn rust_kernel_handles_coincident_union_surfaces() {
        // Honeycomb borders commonly union two solids that share the same
        // cylindrical outer skin. The former browser kernel panicked while
        // rebuilding the resulting half-edge topology.
        let frags = FragmentSpec {
            fn_: 64.0,
            fa: 12.0,
            fs: 2.0,
        };
        let cylinder = |r| Node::Cylinder {
            h: 10.0,
            r1: r,
            r2: r,
            center: true,
            frags,
        };
        let node = Node::Union(vec![
            Node::Difference(vec![
                cylinder(20.0),
                Node::Translate {
                    v: [18.0, 0.0, 0.0],
                    child: Box::new(cylinder(4.0)),
                },
            ]),
            Node::Difference(vec![cylinder(20.0), cylinder(18.0)]),
        ]);

        let mesh = render_with(&node, &RustManifoldKernel::new()).unwrap();
        assert!(mesh.volume() > 0.0);
        assert!(mesh.signed_volume() > 0.0);
    }

    /// Bake-off: the pure-Rust Manifold kernel must agree with the C++ Manifold
    /// kernel to within tolerance on a mixed union/difference/intersection model.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn kernels_agree() {
        let frags = FragmentSpec {
            fn_: 48.0,
            fa: 12.0,
            fs: 2.0,
        };
        let cases = [
            Node::Union(vec![
                Node::Cube {
                    size: [10.0, 10.0, 10.0],
                    center: true,
                },
                Node::Sphere { r: 6.5, frags },
            ]),
            Node::Difference(vec![
                Node::Cube {
                    size: [20.0, 20.0, 20.0],
                    center: true,
                },
                Node::Cylinder {
                    h: 40.0,
                    r1: 5.0,
                    r2: 5.0,
                    center: true,
                    frags,
                },
            ]),
            Node::Intersection(vec![
                Node::Cube {
                    size: [10.0, 10.0, 10.0],
                    center: true,
                },
                Node::Sphere { r: 6.0, frags },
            ]),
        ];
        let cpp = ManifoldKernel::new();
        let rs = RustManifoldKernel::new();
        for (i, node) in cases.iter().enumerate() {
            let a = render_with(node, &cpp).unwrap();
            let b = render_with(node, &rs).unwrap();
            let rel = (a.volume() - b.volume()).abs() / a.volume().max(1e-9);
            assert!(
                rel < 0.005,
                "case {i}: kernels disagree: cpp={} rust-manifold={} (Δ={rel})",
                a.volume(),
                b.volume()
            );
            assert!(
                b.signed_volume() > 0.0,
                "case {i}: rust-manifold output inward-facing"
            );
        }
    }

    // ---- B3: color / highlight / background ---------------------------------

    fn unit_cube() -> Node {
        Node::Cube {
            size: [1.0, 1.0, 1.0],
            center: false,
        }
    }

    #[test]
    fn background_excluded_and_highlight_kept_in_fused() {
        // `%` background is dropped from the fused (exported) mesh; `#` is kept.
        let with_bg = Node::Union(vec![
            unit_cube(),
            Node::Background(Box::new(Node::Translate {
                v: [5.0, 0.0, 0.0],
                child: Box::new(unit_cube()),
            })),
        ]);
        assert!((render(&with_bg).unwrap().volume() - 1.0).abs() < 1e-6);

        let with_hl = Node::Union(vec![
            unit_cube(),
            Node::Highlight(Box::new(Node::Translate {
                v: [5.0, 0.0, 0.0],
                child: Box::new(unit_cube()),
            })),
        ]);
        assert!((render(&with_hl).unwrap().volume() - 2.0).abs() < 1e-6);

        // color() never changes fused geometry.
        let colored = Node::Color {
            rgba: [1.0, 0.0, 0.0, 1.0],
            child: Box::new(unit_cube()),
        };
        assert!((render(&colored).unwrap().volume() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn render_groups_partitions_by_color_and_mode() {
        let node = Node::Union(vec![
            Node::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
                child: Box::new(unit_cube()),
            },
            Node::Highlight(Box::new(Node::Translate {
                v: [2.0, 0.0, 0.0],
                child: Box::new(unit_cube()),
            })),
            Node::Background(Box::new(Node::Translate {
                v: [4.0, 0.0, 0.0],
                child: Box::new(unit_cube()),
            })),
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(groups[0].mode, DisplayMode::Solid);
        assert_eq!(groups[1].mode, DisplayMode::Highlight);
        assert_eq!(groups[2].mode, DisplayMode::Background);
        // The highlight group was translated by +2 in x.
        let min_x = groups[1]
            .mesh
            .verts
            .iter()
            .map(|v| v[0])
            .fold(f64::MAX, f64::min);
        assert!(
            min_x >= 2.0 - 1e-6,
            "highlight group not translated: min_x={min_x}"
        );
    }

    #[test]
    fn difference_keeps_base_color() {
        // `difference(red 4-cube, blue 1-cube)` → ONE group colored RED (the
        // base operand's color survives the cut), with the true fused-difference
        // geometry (volume < the base cube's 64).
        let node = Node::Difference(vec![
            Node::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
                child: Box::new(Node::Cube {
                    size: [4.0, 4.0, 4.0],
                    center: false,
                }),
            },
            Node::Color {
                rgba: [0.0, 0.0, 1.0, 1.0],
                child: Box::new(unit_cube()),
            },
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].color, [1.0, 0.0, 0.0, 1.0]);
        let vol = groups[0].mesh.volume();
        assert!((vol - 63.0).abs() < 1e-6, "difference volume {vol}");
    }

    #[test]
    fn difference_with_empty_tool_preserves_nested_colors() {
        // The Parthenon's top-level shape: `difference(){ union(red a; blue b); }`
        // with no (or a disabled) tool operand must keep both nested colors —
        // this is exactly the CUTAWAY=false case that used to collapse to gold.
        let base = Node::Union(vec![
            Node::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
                child: Box::new(unit_cube()),
            },
            Node::Color {
                rgba: [0.0, 0.0, 1.0, 1.0],
                child: Box::new(Node::Translate {
                    v: [2.0, 0.0, 0.0],
                    child: Box::new(unit_cube()),
                }),
            },
        ]);
        // A trailing `Empty` tool mirrors `if (CUTAWAY) …` evaluating to nothing.
        let node = Node::Difference(vec![base, Node::Empty]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(groups[1].color, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn intersection_keeps_first_operand_color() {
        // `intersection(red 4-cube, blue 1-cube)` → ONE group colored RED (the
        // first operand wins), geometry equal to the 1-cube overlap (volume 1).
        let node = Node::Intersection(vec![
            Node::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
                child: Box::new(Node::Cube {
                    size: [4.0, 4.0, 4.0],
                    center: false,
                }),
            },
            Node::Color {
                rgba: [0.0, 0.0, 1.0, 1.0],
                child: Box::new(unit_cube()),
            },
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].color, [1.0, 0.0, 0.0, 1.0]);
        let vol = groups[0].mesh.volume();
        assert!((vol - 1.0).abs() < 1e-6, "intersection volume {vol}");
    }

    #[test]
    fn coalesce_merges_same_color_and_keeps_distinct_separate() {
        // Three same-color solids under one color() coalesce to one group whose
        // geometry is the sum of all three; a distinct color/mode stays separate.
        let same = |x: f64| Node::Translate {
            v: [x, 0.0, 0.0],
            child: Box::new(unit_cube()),
        };
        let node = Node::Union(vec![
            Node::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
                child: Box::new(Node::Union(vec![same(0.0), same(2.0), same(4.0)])),
            },
            Node::Color {
                rgba: [0.0, 0.0, 1.0, 1.0],
                child: Box::new(same(6.0)),
            },
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert!((groups[0].mesh.volume() - 3.0).abs() < 1e-6);
        assert_eq!(groups[1].color, [0.0, 0.0, 1.0, 1.0]);
        assert!((groups[1].mesh.volume() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn difference_subtracts_each_region_and_keeps_all_colors() {
        // The CUTAWAY=true path: a *multi-color* base minus a real tool. Each
        // colored region is subtracted independently and every color survives.
        // red 4-cube at x∈[0,4], blue 4-cube at x∈[5,9]; the tool (unit cube at
        // the origin) bites only the red one → red 63, blue 64, both present.
        let big = |x: f64, rgba: [f32; 4]| Node::Color {
            rgba,
            child: Box::new(Node::Translate {
                v: [x, 0.0, 0.0],
                child: Box::new(Node::Cube {
                    size: [4.0, 4.0, 4.0],
                    center: false,
                }),
            }),
        };
        let node = Node::Difference(vec![
            Node::Union(vec![
                big(0.0, [1.0, 0.0, 0.0, 1.0]),
                big(5.0, [0.0, 0.0, 1.0, 1.0]),
            ]),
            unit_cube(), // tool at the origin, inside the red cube only
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert!((groups[0].mesh.volume() - 63.0).abs() < 1e-6);
        assert_eq!(groups[1].color, [0.0, 0.0, 1.0, 1.0]);
        assert!((groups[1].mesh.volume() - 64.0).abs() < 1e-6);
    }

    #[test]
    fn difference_drops_fully_consumed_region() {
        // A base region entirely inside the tool disappears (empty meshes are not
        // emitted); an untouched region survives. red 4-cube at x∈[0,4], blue
        // unit cube at x∈[5,6]; the tool covers x∈[5,10], swallowing the blue.
        let node = Node::Difference(vec![
            Node::Union(vec![
                Node::Color {
                    rgba: [1.0, 0.0, 0.0, 1.0],
                    child: Box::new(Node::Cube {
                        size: [4.0, 4.0, 4.0],
                        center: false,
                    }),
                },
                Node::Color {
                    rgba: [0.0, 0.0, 1.0, 1.0],
                    child: Box::new(Node::Translate {
                        v: [5.0, 0.0, 0.0],
                        child: Box::new(unit_cube()),
                    }),
                },
            ]),
            Node::Translate {
                v: [5.0, -1.0, -1.0],
                child: Box::new(Node::Cube {
                    size: [5.0, 5.0, 5.0],
                    center: false,
                }),
            },
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert!((groups[0].mesh.volume() - 64.0).abs() < 1e-6);
    }

    #[test]
    fn intersection_with_empty_operand_is_empty() {
        // Unlike difference (which skips empty tools), an intersection with an
        // empty operand yields nothing — matching the fused `render` semantics.
        let node = Node::Intersection(vec![
            Node::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
                child: Box::new(unit_cube()),
            },
            Node::Empty,
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert!(
            groups.is_empty(),
            "expected no groups, got {}",
            groups.len()
        );
    }

    #[test]
    fn two_d_difference_stays_opaque_in_enclosing_color() {
        // 2D booleans must NOT take the color-recursion path (their flat, coplanar
        // meshes would break the 3D kernel). A `difference(square, circle)` is
        // clipped in-plane and emitted as ONE group in the *enclosing* color
        // (DEFAULT here — the inner `color(red)` is swallowed, as before).
        let frags = FragmentSpec {
            fn_: 32.0,
            fa: 12.0,
            fs: 2.0,
        };
        let node = Node::Difference(vec![
            Node::Color {
                rgba: [1.0, 0.0, 0.0, 1.0],
                child: Box::new(Node::Square {
                    size: [4.0, 4.0],
                    center: true,
                }),
            },
            Node::Circle { r: 1.0, frags },
        ]);
        let mut cache = GeomCache::new();
        let groups = render_groups_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].color, DEFAULT_COLOR);
        // A real flat profile came through the 2D path (square − hole ≈ 16 − π).
        let area = flat_area(&groups[0].mesh);
        assert!(
            (area - (16.0 - std::f64::consts::PI)).abs() < 0.05,
            "area {area}"
        );
    }

    // ---- provenance partition (editor↔preview linking) ---------------------

    /// Wrap a node in a provenance span, mirroring what the evaluator emits for
    /// each `ModuleCall`.
    fn prov(span: std::ops::Range<usize>, child: Node) -> Node {
        Node::Provenance {
            span,
            child: Box::new(child),
        }
    }

    fn sphere8() -> Node {
        Node::Sphere {
            r: 8.0,
            frags: FragmentSpec {
                fn_: 24.0,
                fa: 12.0,
                fs: 2.0,
            },
        }
    }

    #[test]
    fn provenance_difference_recurses_base_keeping_span_stack() {
        // `difference(){ cube(20); translate([5,5,5]) sphere(8); }` — the 3D
        // difference recurses into the base (the cube) so its span survives; the
        // result is ONE group whose stack is [difference, cube] (outer→inner), with
        // the true fused-difference geometry (the sphere took a bite).
        let diff_span = 0..60;
        let cube_span = 12..20;
        let node = prov(
            diff_span.clone(),
            Node::Difference(vec![
                prov(
                    cube_span.clone(),
                    Node::Cube {
                        size: [20.0, 20.0, 20.0],
                        center: false,
                    },
                ),
                prov(
                    22..48,
                    Node::Translate {
                        v: [5.0, 5.0, 5.0],
                        child: Box::new(prov(35..43, sphere8())),
                    },
                ),
            ]),
        );
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].spans, vec![diff_span, cube_span]);
        let vol = groups[0].mesh.volume();
        assert!(vol > 0.0 && vol < 8000.0, "difference volume {vol}");
    }

    #[test]
    fn provenance_two_objects_are_two_groups_with_their_span_stacks() {
        // Two top-level statements → two groups, each with its own stack. The
        // second nests an inner call, so its stack is [outer, inner] and its mesh
        // is translated.
        let node = Node::Group(vec![
            prov(0..8, unit_cube()),
            prov(
                10..40,
                Node::Translate {
                    v: [5.0, 0.0, 0.0],
                    child: Box::new(prov(30..38, unit_cube())),
                },
            ),
        ]);
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].spans, vec![0..8]);
        assert_eq!(groups[1].spans, vec![10..40, 30..38]);
        // The second group was translated +5 in x.
        let min_x = groups[1]
            .mesh
            .verts
            .iter()
            .map(|v| v[0])
            .fold(f64::MAX, f64::min);
        assert!(min_x >= 5.0 - 1e-6, "second group not translated: {min_x}");
    }

    #[test]
    fn provenance_difference_with_empty_tool_preserves_nested_span_stacks() {
        // The CUTAWAY=off case: `difference(){ union(a, b); <empty> }`. With no
        // surviving tool, the base regions pass straight through, each keeping its
        // full stack [difference, region] — no collapse to the difference span.
        let node = prov(
            100..200,
            Node::Difference(vec![
                Node::Union(vec![
                    prov(0..5, unit_cube()),
                    prov(
                        10..15,
                        Node::Translate {
                            v: [3.0, 0.0, 0.0],
                            child: Box::new(unit_cube()),
                        },
                    ),
                ]),
                Node::Empty,
            ]),
        );
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].spans, vec![100..200, 0..5]);
        assert_eq!(groups[1].spans, vec![100..200, 10..15]);
    }

    #[test]
    fn provenance_difference_subtracts_each_region_and_bbox_passes_through() {
        // A multi-region base minus a real tool: each region keeps its stack, the
        // tool is subtracted only from the region it overlaps, and a region outside
        // the tool's AABB is passed through untouched (the bbox optimization).
        // red 4-cube at x∈[0,4] (bitten by a unit tool at the origin) → vol 63;
        // blue 4-cube far away at x∈[20,24] (bbox miss) → vol 64.
        let big = |x: f64, span: std::ops::Range<usize>| {
            prov(
                span,
                Node::Translate {
                    v: [x, 0.0, 0.0],
                    child: Box::new(Node::Cube {
                        size: [4.0, 4.0, 4.0],
                        center: false,
                    }),
                },
            )
        };
        let node = prov(
            100..200,
            Node::Difference(vec![
                Node::Union(vec![big(0.0, 0..5), big(20.0, 10..15)]),
                unit_cube(), // tool at the origin, inside the first cube only
            ]),
        );
        let mut cache = GeomCache::new();
        let mut groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        // Order by innermost span so the assertions are position-independent.
        groups.sort_by_key(|g| g.spans.last().unwrap().start);
        assert_eq!(groups[0].spans, vec![100..200, 0..5]);
        assert!((groups[0].mesh.volume() - 63.0).abs() < 1e-6);
        assert_eq!(groups[1].spans, vec![100..200, 10..15]);
        assert!((groups[1].mesh.volume() - 64.0).abs() < 1e-6);
    }

    #[test]
    fn provenance_intersection_keeps_base_span_stacks() {
        // `intersection(){ union(a, b); clip }` — the first operand is partitioned
        // into regions, each clipped by the fused rest, keeping its stack. A region
        // outside the clip survives nothing.
        let clip = Node::Cube {
            size: [3.0, 3.0, 3.0],
            center: false,
        };
        let node = prov(
            100..200,
            Node::Intersection(vec![
                Node::Union(vec![
                    prov(0..5, unit_cube()), // at origin, overlaps the 3-cube clip
                    prov(
                        10..15,
                        Node::Translate {
                            v: [20.0, 0.0, 0.0], // far away, clipped to nothing
                            child: Box::new(unit_cube()),
                        },
                    ),
                ]),
                clip,
            ]),
        );
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].spans, vec![100..200, 0..5]);
        assert!((groups[0].mesh.volume() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn provenance_is_transparent_to_the_fused_render() {
        // A provenance-wrapped tree renders a byte-identical fused mesh to the
        // bare tree (provenance never touches the fused geometry or its normals).
        let bare = Node::Difference(vec![
            Node::Cube {
                size: [20.0, 20.0, 20.0],
                center: true,
            },
            sphere8(),
        ]);
        let wrapped = prov(
            0..40,
            Node::Difference(vec![
                prov(5..13, {
                    Node::Cube {
                        size: [20.0, 20.0, 20.0],
                        center: true,
                    }
                }),
                prov(15..24, sphere8()),
            ]),
        );
        let kernel = RustManifoldKernel::new();
        let a = render_with(&bare, &kernel).unwrap().to_triangle_soup_f32();
        let b = render_with(&wrapped, &kernel)
            .unwrap()
            .to_triangle_soup_f32();
        assert_eq!(a.0, b.0, "positions differ");
        assert_eq!(a.1, b.1, "normals differ");
    }

    #[test]
    fn provenance_does_not_pollute_the_geometry_cache() {
        // Two identical cubes on different source lines dedupe in the cache
        // (the span is skipped when hashing) yet stay two independent groups.
        let node = Node::Group(vec![prov(0..8, unit_cube()), prov(10..18, unit_cube())]);
        let kernel = RustManifoldKernel::new();
        let mut cache = GeomCache::new();
        // Fused render: cache has the cube (shared), the provenance node (shared,
        // same hash), and the group — 3 entries, cube rendered once.
        render_cached(&node, &kernel, &mut cache).unwrap();
        assert_eq!(cache.len(), 3, "identical cubes should share cache entries");
        // Provenance partition still yields two distinct groups.
        let groups = render_provenance_cached(&node, &kernel, &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].spans, vec![0..8]);
        assert_eq!(groups[1].spans, vec![10..18]);
    }

    #[test]
    fn provenance_channel_serializes_ranges_and_span_stacks() {
        let node = Node::Group(vec![
            prov(0..8, unit_cube()),
            prov(
                10..18,
                Node::Translate {
                    v: [3.0, 0.0, 0.0],
                    child: Box::new(prov(11..17, unit_cube())),
                },
            ),
        ]);
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        let (positions, normals, json) = provenance_channel(&groups);
        // Each unit cube is 12 triangles = 36 vertices; two groups = 72 vertices.
        assert_eq!(positions.len(), 72 * 3);
        assert_eq!(normals.len(), 72 * 3);
        // First group: single-level stack. Second: nested [outer, inner].
        assert!(json.contains("\"spans\":[[0,8]]"), "{json}");
        assert!(json.contains("\"spans\":[[10,18],[11,17]]"), "{json}");
        assert!(json.contains("\"start\":36,\"count\":36"), "{json}");
    }

    #[test]
    fn provenance_group_from_include_has_empty_span_stack() {
        // Geometry with no enclosing provenance (e.g. spliced from an `include`d
        // file) is still emitted, but with an empty span stack (not pickable);
        // serialized as `"spans":[]`.
        let node = Node::Group(vec![unit_cube(), prov(5..13, unit_cube())]);
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 2);
        assert!(groups[0].spans.is_empty());
        assert_eq!(groups[1].spans, vec![5..13]);
        let (_, _, json) = provenance_channel(&groups);
        assert!(json.contains("\"spans\":[]"), "{json}");
    }

    // ---- 2D provenance (picking/highlighting parity with 3D) ----------------

    #[test]
    fn provenance_2d_transform_is_one_group_translated_at_z0() {
        // `translate([2,0,0]) circle(3)` — a 2D leaf under an affine transform is
        // one group with the outer span, its flat mesh translated +x and pinned to
        // z=0 (the plane the flat mesh renders in).
        let frags = FragmentSpec {
            fn_: 32.0,
            fa: 12.0,
            fs: 2.0,
        };
        let outer = 0..24;
        let node = prov(
            outer.clone(),
            Node::Translate {
                v: [2.0, 0.0, 0.0],
                child: Box::new(prov(13..22, Node::Circle { r: 3.0, frags })),
            },
        );
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].spans, vec![outer, 13..22]);
        // Flat 2D mesh: every vertex sits on the z=0 plane.
        assert!(
            groups[0].mesh.verts.iter().all(|v| v[2].abs() < 1e-9),
            "2D provenance mesh not flat at z=0"
        );
        // Translated +2 in x: a radius-3 circle now spans x ∈ [-1, 5].
        let min_x = groups[0]
            .mesh
            .verts
            .iter()
            .map(|v| v[0])
            .fold(f64::MAX, f64::min);
        assert!(min_x >= -1.0 - 1e-6, "circle not translated: {min_x}");
    }

    #[test]
    fn provenance_2d_difference_is_one_fused_group() {
        // `difference(){ square(10); circle(4); }` — a 2D boolean stays opaque (the
        // 3D per-region recursion is guarded by `!is_2d`), so the whole result is
        // ONE group whose stack is just the outer difference span; the inner operand
        // spans are not visited (guards against a future contour-native path
        // fragmenting it per-operand).
        let frags = FragmentSpec {
            fn_: 32.0,
            fa: 12.0,
            fs: 2.0,
        };
        let diff_span = 0..40;
        let node = prov(
            diff_span.clone(),
            Node::Difference(vec![
                prov(
                    13..22,
                    Node::Square {
                        size: [10.0, 10.0],
                        center: true,
                    },
                ),
                prov(24..33, Node::Circle { r: 4.0, frags }),
            ]),
        );
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].spans, vec![diff_span]);
        // A real flat profile came through the 2D path: the 100-unit square with a
        // ~16π bite taken out (the hole is a 32-gon, so a hair under π·16).
        let area = flat_area(&groups[0].mesh);
        assert!(
            (area - (100.0 - 16.0 * std::f64::consts::PI)).abs() < 0.5,
            "difference area {area}"
        );
    }

    #[test]
    fn provenance_2d_for_loop_instances_share_one_span() {
        // A `for` loop unrolls into a Group of instances under ONE provenance span,
        // so every instance is its own group but all carry the loop's span — they
        // light up together.
        let frags = FragmentSpec {
            fn_: 16.0,
            fa: 12.0,
            fs: 2.0,
        };
        let loop_span = 0..30;
        let instance = |x: f64| Node::Translate {
            v: [x, 0.0, 0.0],
            child: Box::new(Node::Circle { r: 1.0, frags }),
        };
        let node = prov(
            loop_span.clone(),
            Node::Group(vec![instance(0.0), instance(5.0), instance(10.0)]),
        );
        let mut cache = GeomCache::new();
        let groups =
            render_provenance_cached(&node, &RustManifoldKernel::new(), &mut cache).unwrap();
        assert_eq!(groups.len(), 3);
        assert!(
            groups.iter().all(|g| g.spans == vec![loop_span.clone()]),
            "all loop instances should share the loop span"
        );
    }

    #[test]
    fn to_3mf_colored_model_has_materials_and_objects() {
        let a = cube([1.0, 1.0, 1.0], false);
        let b = cube([2.0, 2.0, 2.0], false);
        let xml =
            Mesh::to_3mf_colored_model(&[(&a, [1.0, 0.0, 0.0, 1.0]), (&b, [0.0, 0.0, 1.0, 0.5])]);
        assert_eq!(xml.matches("<object ").count(), 2);
        assert_eq!(xml.matches("<item ").count(), 2);
        assert!(xml.contains("displaycolor=\"#FF0000FF\""), "{xml}");
        assert!(xml.contains("displaycolor=\"#0000FF80\""), "{xml}");
    }
}
