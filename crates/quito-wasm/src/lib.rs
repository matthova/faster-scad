//! wasm-bindgen engine surface for the browser playground.
//!
//! Exposes a single `render(source)` entry point that runs the full pipeline
//! (parse → eval → geometry) and returns mesh data as typed arrays plus
//! console output and diagnostics. Geometry uses the pure-Rust Manifold kernel
//! (the default on wasm).

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! {
    /// Persistent geometry cache across renders — makes warm edits incremental
    /// (only subtrees whose structure changed are re-rendered). The worker is
    /// single-threaded, so a thread-local is the whole story.
    static CACHE: RefCell<quito_geom::GeomCache> = RefCell::new(quito_geom::GeomCache::new());
}

/// Bound on cached subtrees; past this the cache is reset to cap memory.
const CACHE_CAP: usize = 8192;

/// Initialize panic hook for readable errors in the browser console.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Drop the persistent geometry cache (e.g. when loading a new document).
#[wasm_bindgen]
pub fn clear_cache() {
    CACHE.with(|c| c.borrow_mut().clear());
}

/// Engine version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// The result of rendering a `.scad` source string.
///
/// Mesh data is a non-indexed triangle soup with flat (per-face) normals:
/// `positions` and `normals` both hold 9 floats per triangle.
#[wasm_bindgen]
pub struct RenderResult {
    positions: Vec<f32>,
    normals: Vec<f32>,
    echo: String,
    warnings: String,
    error: Option<String>,
    triangle_count: u32,
    vertex_count: u32,
    volume: f64,
    area: f64,
    is_2d: bool,
}

#[wasm_bindgen]
impl RenderResult {
    /// Triangle-soup vertex positions (9 f32 per triangle) as a `Float32Array`.
    #[wasm_bindgen(getter)]
    pub fn positions(&self) -> Vec<f32> {
        self.positions.clone()
    }

    /// Per-face normals (9 f32 per triangle) as a `Float32Array`.
    #[wasm_bindgen(getter)]
    pub fn normals(&self) -> Vec<f32> {
        self.normals.clone()
    }

    /// Newline-joined `ECHO:` output.
    #[wasm_bindgen(getter)]
    pub fn echo(&self) -> String {
        self.echo.clone()
    }

    /// Newline-joined warnings.
    #[wasm_bindgen(getter)]
    pub fn warnings(&self) -> String {
        self.warnings.clone()
    }

    /// Error message, or empty string if the render succeeded.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> String {
        self.error.clone().unwrap_or_default()
    }

    /// Whether the render succeeded (no error).
    #[wasm_bindgen(getter)]
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }

    #[wasm_bindgen(getter)]
    pub fn triangle_count(&self) -> u32 {
        self.triangle_count
    }

    #[wasm_bindgen(getter)]
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    #[wasm_bindgen(getter)]
    pub fn volume(&self) -> f64 {
        self.volume
    }

    #[wasm_bindgen(getter)]
    pub fn area(&self) -> f64 {
        self.area
    }

    /// Whether the model is a 2D object (exportable to DXF/SVG) vs a 3D solid.
    #[wasm_bindgen(getter)]
    pub fn is_2d(&self) -> bool {
        self.is_2d
    }
}

impl RenderResult {
    fn from_error(msg: String, echo: String, warnings: String) -> Self {
        RenderResult {
            positions: Vec::new(),
            normals: Vec::new(),
            echo,
            warnings,
            error: Some(msg),
            triangle_count: 0,
            vertex_count: 0,
            volume: 0.0,
            area: 0.0,
            is_2d: false,
        }
    }
}

/// Render a 2D model and serialize it to DXF or SVG text. Returns an empty
/// string if the model isn't 2D or fails to evaluate (the caller checks
/// `RenderResult.is_2d` first). `format` is "dxf" or "svg".
#[wasm_bindgen]
pub fn export_2d(
    source: &str,
    names: Vec<String>,
    values: Vec<String>,
    file_names: Vec<String>,
    file_contents: Vec<String>,
    format: &str,
) -> String {
    let Ok(program) = quito_syntax::parse(source) else {
        return String::new();
    };
    let mut overrides = Vec::new();
    for (name, val) in names.iter().zip(values.iter()) {
        if let Some(pv) = quito_syntax::customizer::parse_value(val) {
            overrides.push((name.clone(), quito_eval::value_from_param(&pv)));
        }
    }
    let resolver = MapResolver {
        files: file_names.into_iter().zip(file_contents).collect(),
    };
    let Ok(eval) = quito_eval::eval_program_with_params(&program, &resolver, ".", &overrides)
    else {
        return String::new();
    };
    match quito_geom::render_contours(&eval.node) {
        Some(contours) if format == "dxf" => quito_geom::export_dxf(&contours),
        Some(contours) if format == "svg" => quito_geom::export_svg(&contours),
        _ => String::new(),
    }
}

/// The customizer parameter schema for a source string, as a JSON string
/// (`{"params":[…]}`). The playground renders a control panel from this.
#[wasm_bindgen]
pub fn parameters(source: &str) -> String {
    quito_syntax::customizer::extract(source).to_json()
}

/// Run the full pipeline on a source string.
#[wasm_bindgen]
pub fn render(source: &str) -> RenderResult {
    render_with_params(source, Vec::new(), Vec::new())
}

/// Like [`render`], but with customizer overrides supplied as parallel arrays:
/// `names[i]` is a top-level parameter and `values[i]` its new value as a
/// literal string (`"30"`, `"true"`, `"\"hi\""`, `"[1,2,3]"`).
#[wasm_bindgen]
pub fn render_with_params(source: &str, names: Vec<String>, values: Vec<String>) -> RenderResult {
    render_with_files(source, names, values, Vec::new(), Vec::new())
}

/// A `FileResolver` over an in-memory map of `path -> source`, for resolving
/// `include`/`use` in the browser (extra playground files or a bundled library).
struct MapResolver {
    files: std::collections::HashMap<String, String>,
}

impl MapResolver {
    /// Resolve a path to a stored key: as written, then normalized against the
    /// including dir.
    fn resolve_key(&self, path: &str, from_dir: &str) -> Option<String> {
        if self.files.contains_key(path) {
            return Some(path.to_string());
        }
        let joined = if from_dir.is_empty() || from_dir == "." {
            path.to_string()
        } else {
            format!("{from_dir}/{path}")
        };
        self.files.contains_key(&joined).then_some(joined)
    }
}

impl quito_eval::FileResolver for MapResolver {
    fn load(&self, path: &str, from_dir: &str) -> Option<quito_eval::LoadedFile> {
        let key = self.resolve_key(path, from_dir)?;
        let source = self.files.get(&key)?.clone();
        let dir = key
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default();
        Some(quito_eval::LoadedFile {
            key: key.clone(),
            source,
            dir,
        })
    }

    /// Bytes for `import()` of a text-based profile (DXF/SVG) held in a tab.
    /// Binary meshes can't be carried as text tabs, so only text files resolve.
    fn load_bytes(&self, path: &str, from_dir: &str) -> Option<Vec<u8>> {
        let key = self.resolve_key(path, from_dir)?;
        self.files.get(&key).map(|s| s.clone().into_bytes())
    }
}

/// Like [`render_with_params`], but `include`/`use` resolve against an in-memory
/// set of files (`file_names[i]` → `file_contents[i]`) — the playground's other
/// files and/or a bundled library.
#[wasm_bindgen]
pub fn render_with_files(
    source: &str,
    names: Vec<String>,
    values: Vec<String>,
    file_names: Vec<String>,
    file_contents: Vec<String>,
) -> RenderResult {
    // Parse.
    let program = match quito_syntax::parse(source) {
        Ok(p) => p,
        Err(e) => {
            return RenderResult::from_error(
                format!(
                    "parse error: {} (at {}..{})",
                    e.message, e.span.start, e.span.end
                ),
                String::new(),
                String::new(),
            )
        }
    };

    // Build overrides from the parallel arrays.
    let mut overrides = Vec::new();
    for (name, val) in names.iter().zip(values.iter()) {
        if let Some(pv) = quito_syntax::customizer::parse_value(val) {
            overrides.push((name.clone(), quito_eval::value_from_param(&pv)));
        }
    }

    // Build the in-memory file resolver from the parallel arrays.
    let resolver = MapResolver {
        files: file_names.into_iter().zip(file_contents).collect(),
    };

    // Evaluate.
    let eval = match quito_eval::eval_program_with_params(&program, &resolver, ".", &overrides) {
        Ok(o) => o,
        Err(e) => {
            return RenderResult::from_error(
                format!("evaluation error: {}", e.0),
                String::new(),
                String::new(),
            )
        }
    };
    let echo = eval.echoes.join("\n");
    let warnings = eval.warnings.join("\n");

    // Render geometry (pure-Rust Manifold on wasm), reusing the persistent cache
    // so unchanged subtrees survive across edits.
    let kernel = quito_geom::RustManifoldKernel::new();
    let mesh = CACHE.with(|c| {
        let mut cache = c.borrow_mut();
        if cache.len() > CACHE_CAP {
            cache.clear();
        }
        quito_geom::render_cached(&eval.node, &kernel, &mut cache)
    });
    let mesh = match mesh {
        Ok(m) => m,
        Err(e) => return RenderResult::from_error(format!("geometry error: {e}"), echo, warnings),
    };

    let (positions, normals) = mesh.to_triangle_soup_f32();
    RenderResult {
        triangle_count: mesh.tris.len() as u32,
        vertex_count: mesh.verts.len() as u32,
        volume: mesh.volume(),
        area: mesh.surface_area(),
        is_2d: quito_geom::is_2d(&eval.node),
        positions,
        normals,
        echo,
        warnings,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_shapes() {
        let src = "\
/* [Box] */
// the width
width = 10; // [1:100]
mode = 1;   // [0:Off, 1:On]
flag = true;
name = \"hi\"; // 8
v = [1, 2, 3];
";
        let json = quito_syntax::customizer::extract(src).to_json();
        // Spot-check the salient pieces (order preserved).
        assert!(json.contains(r#""name":"width""#));
        assert!(json.contains(r#""group":"Box""#));
        assert!(json.contains(r#""description":"the width""#));
        assert!(json.contains(r#""kind":"slider","min":1,"max":100,"step":null"#));
        assert!(json.contains(r#""kind":"dropdown","options":[{"value":0,"label":"Off"}"#));
        assert!(json.contains(
            r#""name":"flag","group":"Box","description":null,"type":"bool","value":true"#
        ));
        assert!(json.contains(r#""kind":"text","maxLength":8"#));
        assert!(json
            .contains(r#""type":"vector","value":[1,2,3],"control":{"kind":"vector","length":3}"#));
    }

    #[test]
    fn render_applies_overrides() {
        // width=10 default → 10*10*10; override width=4 → 4*10*10 = 400.
        let src = "width = 10;\ncube([width, 10, 10]);";
        let base = render_with_params(src, vec![], vec![]);
        assert!(base.ok());
        assert!(
            (base.volume() - 1000.0).abs() < 1e-6,
            "vol {}",
            base.volume()
        );

        let overridden = render_with_params(src, vec!["width".to_string()], vec!["4".to_string()]);
        assert!(overridden.ok());
        assert!(
            (overridden.volume() - 400.0).abs() < 1e-6,
            "vol {}",
            overridden.volume()
        );
    }

    #[test]
    fn render_resolves_files() {
        // `use` a helper file from the in-memory resolver.
        let main = "use <lib.scad>\ncube([side(), side(), side()]);";
        let lib = "function side() = 3;";
        let r = render_with_files(
            main,
            vec![],
            vec![],
            vec!["lib.scad".to_string()],
            vec![lib.to_string()],
        );
        assert!(r.ok(), "err: {}", r.error());
        assert!((r.volume() - 27.0).abs() < 1e-6, "vol {}", r.volume());
    }

    #[test]
    fn imports_dxf_from_a_tab() {
        // A DXF profile held in a tab is imported via load_bytes and extruded.
        let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 20.0], [0.0, 20.0]];
        let dxf = quito_geom::export_dxf(&[outer]);
        let r = render_with_files(
            "linear_extrude(3) import(\"p.dxf\");",
            vec![],
            vec![],
            vec!["p.dxf".to_string()],
            vec![dxf],
        );
        assert!(r.ok(), "err: {}", r.error());
        assert!((r.volume() - 600.0).abs() < 1e-3, "vol {}", r.volume()); // 10*20*3
    }

    #[test]
    fn export_2d_produces_dxf_and_svg() {
        let src = "square([10, 20]);";
        let dxf = export_2d(src, vec![], vec![], vec![], vec![], "dxf");
        assert!(dxf.contains("LWPOLYLINE"), "dxf: {dxf}");
        let svg = export_2d(src, vec![], vec![], vec![], vec![], "svg");
        assert!(svg.contains("<svg") && svg.contains("<path"), "svg: {svg}");
        // A 3D model yields no 2D export.
        assert!(export_2d("cube(1);", vec![], vec![], vec![], vec![], "dxf").is_empty());
    }
}
