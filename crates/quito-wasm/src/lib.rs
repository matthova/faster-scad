//! wasm-bindgen engine surface for the browser playground.
//!
//! Exposes a single `render(source)` entry point that runs the full pipeline
//! (parse → eval → geometry) and returns mesh data as typed arrays plus
//! console output and diagnostics. Geometry uses the pure-Rust boolmesh kernel
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
        }
    }
}

/// Run the full pipeline on a source string.
#[wasm_bindgen]
pub fn render(source: &str) -> RenderResult {
    // Parse.
    let program = match quito_syntax::parse(source) {
        Ok(p) => p,
        Err(e) => {
            return RenderResult::from_error(
                format!("parse error: {} (at {}..{})", e.message, e.span.start, e.span.end),
                String::new(),
                String::new(),
            )
        }
    };

    // Evaluate.
    let eval = match quito_eval::eval_program(&program) {
        Ok(o) => o,
        Err(e) => return RenderResult::from_error(format!("evaluation error: {}", e.0), String::new(), String::new()),
    };
    let echo = eval.echoes.join("\n");
    let warnings = eval.warnings.join("\n");

    // Render geometry (boolmesh kernel on wasm), reusing the persistent cache
    // so unchanged subtrees survive across edits.
    let kernel = quito_geom::BoolmeshKernel::new();
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
        positions,
        normals,
        echo,
        warnings,
        error: None,
    }
}
