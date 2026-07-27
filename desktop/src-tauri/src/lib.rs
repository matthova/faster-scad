//! Quito desktop backend (Tauri v2).
//!
//! The frontend is the same playground UI as the web build, but rendering runs
//! here in the *native* engine (C++ Manifold kernel) over Tauri IPC — much
//! faster than the browser's pure-Rust kernel — with `include`/`use` resolved
//! straight from disk and a geometry cache kept across renders.

use quito_eval::{FileResolver, LoadedFile};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Rendering runs on a worker thread with a large stack: recursive libraries
/// (e.g. BOSL2's attachment system) nest the evaluator deeply.
const RENDER_STACK: usize = 256 << 20;

#[derive(Default)]
struct AppState {
    cache: Arc<Mutex<quito_geom::GeomCache>>,
}

/// Result of a render, serialized to the frontend.
#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct RenderResult {
    ok: bool,
    error: String,
    echo: String,
    warnings: String,
    positions: Vec<f32>,
    normals: Vec<f32>,
    triangle_count: u32,
    vertex_count: u32,
    volume: f64,
    area: f64,
    /// Customizer schema JSON for the current source.
    params: String,
}

#[tauri::command]
fn parameters(source: String) -> String {
    quito_syntax::customizer::extract(&source).to_json()
}

/// Resolves `include`/`use`/`import` from disk: relative to the including file,
/// then each `OPENSCADPATH` entry.
struct DiskResolver {
    libs: Vec<PathBuf>,
}

impl DiskResolver {
    fn new() -> Self {
        let libs = std::env::var("OPENSCADPATH")
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
        DiskResolver { libs }
    }

    fn candidates(&self, path: &str, from_dir: &str) -> Vec<PathBuf> {
        std::iter::once(Path::new(from_dir).join(path))
            .chain(self.libs.iter().map(|l| l.join(path)))
            .collect()
    }
}

impl FileResolver for DiskResolver {
    fn load(&self, path: &str, from_dir: &str) -> Option<LoadedFile> {
        for c in self.candidates(path, from_dir) {
            if let Ok(source) = std::fs::read_to_string(&c) {
                let key = std::fs::canonicalize(&c)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| c.to_string_lossy().into_owned());
                let dir = c
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                return Some(LoadedFile { key, source, dir });
            }
        }
        None
    }

    fn load_bytes(&self, path: &str, from_dir: &str) -> Option<Vec<u8>> {
        self.candidates(path, from_dir).into_iter().find_map(|c| std::fs::read(&c).ok())
    }
}

/// The playground's in-memory files (open tabs) take precedence; anything not
/// found there falls back to disk (relative paths and `OPENSCADPATH` libraries).
struct CombinedResolver {
    files: HashMap<String, String>,
    disk: DiskResolver,
}

impl FileResolver for CombinedResolver {
    fn load(&self, path: &str, from_dir: &str) -> Option<LoadedFile> {
        let joined = if from_dir.is_empty() || from_dir == "." {
            path.to_string()
        } else {
            format!("{from_dir}/{path}")
        };
        for key in [path, joined.as_str()] {
            if let Some(source) = self.files.get(key) {
                let dir = key.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
                return Some(LoadedFile { key: key.to_string(), source: source.clone(), dir });
            }
        }
        self.disk.load(path, from_dir)
    }

    fn load_bytes(&self, path: &str, from_dir: &str) -> Option<Vec<u8>> {
        self.disk.load_bytes(path, from_dir)
    }
}

fn overrides(names: &[String], values: &[String]) -> Vec<(String, quito_eval::Value)> {
    names
        .iter()
        .zip(values)
        .filter_map(|(n, v)| {
            quito_syntax::customizer::parse_value(v).map(|pv| (n.clone(), quito_eval::value_from_param(&pv)))
        })
        .collect()
}

/// Parse → eval → render, returning the mesh plus console output.
#[allow(clippy::too_many_arguments)]
fn eval_and_render(
    cache: &Arc<Mutex<quito_geom::GeomCache>>,
    source: &str,
    dir: &str,
    names: &[String],
    values: &[String],
    file_names: &[String],
    file_contents: &[String],
) -> Result<(quito_geom::Mesh, Vec<String>, Vec<String>), String> {
    let program = quito_syntax::parse(source).map_err(|e| {
        format!("parse error: {} (at {}..{})", e.message, e.span.start, e.span.end)
    })?;
    let resolver = CombinedResolver {
        files: file_names.iter().cloned().zip(file_contents.iter().cloned()).collect(),
        disk: DiskResolver::new(),
    };
    let out = quito_eval::eval_program_with_params(&program, &resolver, dir, &overrides(names, values))
        .map_err(|e| format!("evaluation error: {}", e.0))?;
    let kernel = quito_geom::ManifoldKernel::new();
    let mesh = {
        let mut cache = cache.lock().unwrap();
        quito_geom::render_cached(&out.node, &kernel, &mut cache).map_err(|e| format!("geometry error: {e}"))?
    };
    Ok((mesh, out.echoes, out.warnings))
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn render(
    state: tauri::State<'_, AppState>,
    source: String,
    dir: Option<String>,
    param_names: Vec<String>,
    param_values: Vec<String>,
    file_names: Vec<String>,
    file_contents: Vec<String>,
) -> RenderResult {
    let cache = state.cache.clone();
    let dir = dir.unwrap_or_else(|| ".".to_string());
    let work = move || {
        let params = quito_syntax::customizer::extract(&source).to_json();
        match eval_and_render(&cache, &source, &dir, &param_names, &param_values, &file_names, &file_contents) {
            Ok((mesh, echoes, warnings)) => {
                let (positions, normals) = mesh.to_triangle_soup_f32();
                RenderResult {
                    ok: true,
                    error: String::new(),
                    echo: echoes.join("\n"),
                    warnings: warnings.join("\n"),
                    triangle_count: mesh.tris.len() as u32,
                    vertex_count: mesh.verts.len() as u32,
                    volume: mesh.volume(),
                    area: mesh.surface_area(),
                    positions,
                    normals,
                    params,
                }
            }
            Err(e) => RenderResult { ok: false, error: e, params, ..Default::default() },
        }
    };
    run_big_stack(work)
}

/// Render and write the model to `path` as STL (binary), OFF, or OBJ.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn save_model(
    state: tauri::State<'_, AppState>,
    path: String,
    format: String,
    source: String,
    dir: Option<String>,
    param_names: Vec<String>,
    param_values: Vec<String>,
    file_names: Vec<String>,
    file_contents: Vec<String>,
) -> Result<(), String> {
    let cache = state.cache.clone();
    let dir = dir.unwrap_or_else(|| ".".to_string());
    let work = move || -> Result<(), String> {
        let (mesh, _, _) = eval_and_render(
            &cache,
            &source,
            &dir,
            &param_names,
            &param_values,
            &file_names,
            &file_contents,
        )?;
        let bytes: Vec<u8> = match format.as_str() {
            "off" => mesh.to_off().into_bytes(),
            "obj" => mesh.to_obj().into_bytes(),
            _ => mesh.to_binary_stl(),
        };
        std::fs::write(&path, bytes).map_err(|e| format!("write {path}: {e}"))
    };
    run_big_stack(work)
}

#[tauri::command]
fn engine_version() -> String {
    format!("quito-desktop {}", env!("CARGO_PKG_VERSION"))
}

/// Run `f` on a worker thread with a large stack and return its result.
fn run_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(RENDER_STACK)
        .spawn(f)
        .expect("spawn render thread")
        .join()
        .expect("render thread panicked")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![render, save_model, parameters, engine_version])
        .run(tauri::generate_context!())
        .expect("error while running Quito desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_render_command_logic() {
        let cache = Arc::new(Mutex::new(quito_geom::GeomCache::new()));
        let (mesh, _, _) = eval_and_render(&cache, "cube([2,3,4]);", ".", &[], &[], &[], &[]).unwrap();
        assert!((mesh.volume() - 24.0).abs() < 1e-6);

        // Overrides apply, like the customizer.
        let (mesh, echoes, _) = eval_and_render(
            &cache,
            "w = 2;\necho(w);\ncube([w, 3, 4]);",
            ".",
            &["w".into()],
            &["5".into()],
            &[],
            &[],
        )
        .unwrap();
        assert!((mesh.volume() - 60.0).abs() < 1e-6);
        assert_eq!(echoes, vec!["ECHO: 5"]);

        // In-memory library file resolves via the combined resolver.
        let (mesh, _, _) = eval_and_render(
            &cache,
            "use <lib.scad>\ncube([side(), side(), side()]);",
            ".",
            &[],
            &[],
            &["lib.scad".into()],
            &["function side() = 3;".into()],
        )
        .unwrap();
        assert!((mesh.volume() - 27.0).abs() < 1e-6);
    }
}
