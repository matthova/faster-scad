//! Quito desktop backend (Tauri v2).
//!
//! The frontend is the same playground UI as the web build, but rendering runs
//! here in the *native* engine (C++ Manifold kernel) over Tauri IPC — much
//! faster than the browser's pure-Rust kernel — with `include`/`use` resolved
//! straight from disk and a geometry cache kept across renders.

use notify::{RecursiveMode, Watcher};
use quito_eval::{FileResolver, LoadedFile};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

/// Rendering runs on a worker thread with a large stack: recursive libraries
/// (e.g. BOSL2's attachment system) nest the evaluator deeply.
const RENDER_STACK: usize = 256 << 20;

#[derive(Default)]
struct AppState {
    cache: Arc<Mutex<quito_geom::GeomCache>>,
    /// Keeps the active file watchers alive (dropping one stops watching that
    /// file). One entry per watched project file with a disk path.
    watchers: Mutex<Vec<notify::RecommendedWatcher>>,
    /// Content last written by the app itself (`save_source`), keyed by
    /// canonicalized path. The watcher compares against this so a self-save is
    /// not echoed back as an external edit (reload-on-save would be jarring).
    last_write: Arc<Mutex<Option<(PathBuf, String)>>>,
    /// A `.scad` path passed at launch (double-click / open-with) before the
    /// webview is ready to listen; the frontend drains it via `take_pending_open`.
    pending_open: Mutex<Option<String>>,
}

/// A file opened from disk.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenedFile {
    path: String,
    name: String,
    dir: String,
    content: String,
}

/// Payload for the `file-changed` event (external edit detected).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileChanged {
    path: String,
    content: String,
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
    /// Whether the model is a 2D object (exportable to DXF/SVG).
    #[serde(rename = "is2D")]
    is_2d: bool,
    /// Customizer schema JSON for the current source.
    params: String,
    /// Structured diagnostics (JSON array) for inline editor squiggles.
    diagnostics: String,
    /// Preview color channel (only when the model uses `color`/`#`/`%`): a
    /// concatenated triangle soup plus a JSON array of per-group ranges/colors.
    preview_positions: Vec<f32>,
    preview_normals: Vec<f32>,
    groups: String,
    /// `$vp*` viewport variables as JSON (only when the source references `$vp`).
    viewport: String,
}

/// An engine error plus the structured diagnostic (with source span, if any) the
/// frontend needs to squiggle it.
#[derive(Debug)]
struct EngineError {
    message: String,
    diagnostic: quito_eval::Diagnostic,
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
        // In-memory tabs first (text profiles like DXF/SVG), then disk.
        let joined = if from_dir.is_empty() || from_dir == "." {
            path.to_string()
        } else {
            format!("{from_dir}/{path}")
        };
        for key in [path, joined.as_str()] {
            if let Some(source) = self.files.get(key) {
                return Some(source.clone().into_bytes());
            }
        }
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
) -> Result<(quito_geom::Mesh, quito_eval::EvalOutput, bool, Vec<String>), EngineError> {
    let program = quito_syntax::parse(source).map_err(|e| {
        let message = format!("parse error: {}", e.message);
        EngineError {
            diagnostic: quito_eval::parse_error_diagnostic(message.clone(), e.span),
            message,
        }
    })?;
    let resolver = CombinedResolver {
        files: file_names.iter().cloned().zip(file_contents.iter().cloned()).collect(),
        disk: DiskResolver::new(),
    };
    let out = quito_eval::eval_program_with_params(&program, &resolver, dir, &overrides(names, values))
        .map_err(|e| EngineError {
            message: format!("evaluation error: {}", e.message),
            diagnostic: quito_eval::eval_error_diagnostic(&e),
        })?;
    let is_2d = quito_geom::is_2d(&out.node);
    let kernel = quito_geom::ManifoldKernel::new();
    let (mesh, geom_warnings) = {
        let mut cache = cache.lock().unwrap();
        quito_geom::render_cached_warns(&out.node, &kernel, &mut cache).map_err(|e| {
            let message = format!("geometry error: {e}");
            EngineError {
                diagnostic: quito_eval::eval_error_diagnostic(&quito_eval::EvalError::new(
                    message.clone(),
                )),
                message,
            }
        })?
    };
    Ok((mesh, out, is_2d, geom_warnings))
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
            Ok((mesh, out, is_2d, geom_warnings)) => {
                let (positions, normals) = mesh.to_triangle_soup_f32();
                let diagnostics = quito_eval::diagnostics_json(None, &out.warnings);
                // Preview color channel — only when the model uses color/`#`/`%`.
                let (preview_positions, preview_normals, groups) =
                    if quito_geom::has_display_attrs(&out.node) {
                        let kernel = quito_geom::ManifoldKernel::new();
                        let mut cache = cache.lock().unwrap();
                        match quito_geom::render_groups_cached(&out.node, &kernel, &mut cache) {
                            Ok(g) => quito_geom::preview_channel(&g),
                            Err(_) => Default::default(),
                        }
                    } else {
                        Default::default()
                    };
                let viewport = if source.contains("$vp") {
                    quito_eval::viewport_json(&out.viewport)
                } else {
                    String::new()
                };
                RenderResult {
                    ok: true,
                    error: String::new(),
                    echo: out.echoes.join("\n"),
                    warnings: out
                        .warnings
                        .iter()
                        .map(|w| w.message.clone())
                        .chain(geom_warnings)
                        .collect::<Vec<_>>()
                        .join("\n"),
                    triangle_count: mesh.tris.len() as u32,
                    vertex_count: mesh.verts.len() as u32,
                    volume: mesh.volume(),
                    area: mesh.surface_area(),
                    is_2d,
                    positions,
                    normals,
                    params,
                    diagnostics,
                    preview_positions,
                    preview_normals,
                    groups,
                    viewport,
                }
            }
            Err(e) => RenderResult {
                ok: false,
                error: e.message,
                diagnostics: quito_eval::diagnostics_json(Some(&e.diagnostic), &[]),
                params,
                ..Default::default()
            },
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
        // 2D vector formats need the exact contours, not the flat mesh.
        if format == "dxf" || format == "svg" {
            let program = quito_syntax::parse(&source).map_err(|e| {
                format!("parse error: {} (at {}..{})", e.message, e.span.start, e.span.end)
            })?;
            let resolver = CombinedResolver {
                files: file_names.iter().cloned().zip(file_contents.iter().cloned()).collect(),
                disk: DiskResolver::new(),
            };
            let out = quito_eval::eval_program_with_params(
                &program,
                &resolver,
                &dir,
                &overrides(&param_names, &param_values),
            )
            .map_err(|e| format!("evaluation error: {}", e.message))?;
            let contours = quito_geom::render_contours(&out.node)
                .ok_or_else(|| "export requires a 2D model".to_string())?;
            let text = if format == "dxf" {
                quito_geom::export_dxf(&contours)
            } else {
                quito_geom::export_svg(&contours)
            };
            return std::fs::write(&path, text).map_err(|e| format!("write {path}: {e}"));
        }
        let (mesh, out, _, _) = eval_and_render(
            &cache,
            &source,
            &dir,
            &param_names,
            &param_values,
            &file_names,
            &file_contents,
        )
        .map_err(|e| e.message)?;
        // 3MF carries per-object color when the model uses color/`#`/`%`.
        if format == "3mf" && quito_geom::has_display_attrs(&out.node) {
            let groups =
                quito_geom::render_groups(&out.node).map_err(|e| format!("color groups: {e}"))?;
            let colored: Vec<(&quito_geom::Mesh, [f32; 4])> = groups
                .iter()
                .filter(|g| g.mode != quito_geom::DisplayMode::Background)
                .map(|g| (&g.mesh, g.color))
                .collect();
            return std::fs::write(&path, quito_geom::Mesh::to_3mf_colored(&colored))
                .map_err(|e| format!("write {path}: {e}"));
        }
        let bytes: Vec<u8> = match format.as_str() {
            "off" => mesh.to_off().into_bytes(),
            "obj" => mesh.to_obj().into_bytes(),
            "3mf" => mesh.to_3mf(),
            "amf" => mesh.to_amf().into_bytes(),
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

/// Watch `target`'s directory and call `on_change(content)` whenever the file is
/// modified/created externally (the "edit in your own editor" workflow). The
/// returned watcher must be kept alive.
///
/// `last_write` carries the content the app itself last wrote (see
/// `save_source`); a change whose on-disk content matches it is treated as a
/// self-save and swallowed, so saving from the app doesn't trigger a reload.
fn install_watcher<F>(
    target: &Path,
    last_write: Arc<Mutex<Option<(PathBuf, String)>>>,
    on_change: F,
) -> notify::Result<notify::RecommendedWatcher>
where
    F: Fn(String) + Send + 'static,
{
    let t = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    let parent = t.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let target = t.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        // React to any change touching our file (event kinds vary by platform —
        // macOS FSEvents in particular is coarse); ignore pure access events.
        let Ok(event) = res else { return };
        if matches!(event.kind, notify::EventKind::Access(_)) {
            return;
        }
        let hit = event.paths.iter().any(|p| {
            std::fs::canonicalize(p)
                .map(|pc| pc == target)
                .unwrap_or_else(|_| p.file_name() == target.file_name())
        });
        if hit {
            if let Ok(content) = std::fs::read_to_string(&target) {
                // Swallow our own writes. FSEvents is coarse and may replay a
                // write as several events, so we keep the marker (matching any
                // duplicate self-write) and only drop it once a genuinely
                // different edit to this file arrives — that's the real external
                // change we want to deliver.
                if let Ok(mut lw) = last_write.lock() {
                    if let Some((p, c)) = lw.as_ref() {
                        if *p == target {
                            if *c == content {
                                return; // self-write (or a duplicate) — ignore
                            }
                            *lw = None; // real external edit — forget the marker
                        }
                        // A marker for a different file: leave it untouched.
                    }
                }
                on_change(content);
            }
        }
    })?;
    watcher.watch(&parent, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

/// Best-effort canonical path that also works for a not-yet-created file
/// (canonicalize the parent, then rejoin the filename). Kept in sync with the
/// `target` computation in `install_watcher` so `last_write` markers match.
fn canonical(path: &str) -> PathBuf {
    let pb = PathBuf::from(path);
    std::fs::canonicalize(&pb).unwrap_or_else(|_| {
        match (pb.parent().and_then(|p| std::fs::canonicalize(p).ok()), pb.file_name()) {
            (Some(dir), Some(name)) => dir.join(name),
            _ => pb,
        }
    })
}

/// Install a watcher for `path` that emits `file-changed` on external edits.
fn spawn_watcher(
    app: &tauri::AppHandle,
    last_write: Arc<Mutex<Option<(PathBuf, String)>>>,
    path: &str,
) -> Option<notify::RecommendedWatcher> {
    let app = app.clone();
    let emit_path = path.to_string();
    match install_watcher(&PathBuf::from(path), last_write, move |content| {
        let _ = app.emit("file-changed", FileChanged { path: emit_path.clone(), content });
    }) {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!("file watch failed for {path}: {e}");
            None
        }
    }
}

/// Open a `.scad` file from disk and start watching it for external edits (which
/// fire a `file-changed` event). Returns the content plus its directory (used
/// for include/use resolution) and name.
#[tauri::command]
fn open_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<OpenedFile, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| format!("open {path}: {e}"))?;
    let pb = PathBuf::from(&path);
    let dir = pb.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
    let name =
        pb.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "untitled.scad".into());

    // Opening a file starts a fresh project, so replace any existing watchers.
    let mut ws = state.watchers.lock().unwrap();
    ws.clear();
    if let Some(w) = spawn_watcher(&app, state.last_write.clone(), &path) {
        ws.push(w);
    }
    Ok(OpenedFile { path, name, dir, content })
}

/// Write UTF-8 source text to `path` (⌘S / Save As). Records a self-write marker
/// first so the file watcher swallows the resulting change instead of reloading.
#[tauri::command]
fn save_source(
    state: tauri::State<'_, AppState>,
    path: String,
    content: String,
) -> Result<(), String> {
    *state.last_write.lock().unwrap() = Some((canonical(&path), content.clone()));
    std::fs::write(&path, content).map_err(|e| {
        *state.last_write.lock().unwrap() = None;
        format!("write {path}: {e}")
    })
}

/// Write arbitrary bytes to `path` (e.g. a captured PNG). No watcher marker —
/// image files aren't tracked as editable source.
#[tauri::command]
fn save_bytes(path: String, bytes: Vec<u8>) -> Result<(), String> {
    std::fs::write(&path, bytes).map_err(|e| format!("write {path}: {e}"))
}

/// Watch a set of project files (every tab with a disk path) for external edits.
/// Replaces the current watcher set.
#[tauri::command]
fn watch_files(app: tauri::AppHandle, state: tauri::State<'_, AppState>, paths: Vec<String>) {
    let mut ws = state.watchers.lock().unwrap();
    ws.clear();
    for p in &paths {
        if let Some(w) = spawn_watcher(&app, state.last_write.clone(), p) {
            ws.push(w);
        }
    }
}

/// Return (and clear) a `.scad` path passed at launch via double-click/open-with,
/// so the frontend can open it once the webview is ready.
#[tauri::command]
fn take_pending_open(state: tauri::State<'_, AppState>) -> Option<String> {
    state.pending_open.lock().unwrap().take()
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

/// Native menu bar. Custom File/View items carry ids that `on_menu_event` relays
/// to the frontend as `menu-action`; Edit uses the OS's predefined edit items,
/// and the leading app submenu supplies the standard macOS application menu.
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    let check_updates =
        MenuItemBuilder::with_id("check-updates", "Check for Updates…").build(app)?;
    let app_menu = SubmenuBuilder::new(app, "Quito")
        .about(None)
        .separator()
        .item(&check_updates)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let new_item = MenuItemBuilder::with_id("new", "New").accelerator("CmdOrCtrl+N").build(app)?;
    let open = MenuItemBuilder::with_id("open", "Open…").accelerator("CmdOrCtrl+O").build(app)?;
    let save = MenuItemBuilder::with_id("save", "Save").accelerator("CmdOrCtrl+S").build(app)?;
    let save_as =
        MenuItemBuilder::with_id("save-as", "Save As…").accelerator("CmdOrCtrl+Shift+S").build(app)?;
    let export = MenuItemBuilder::with_id("export", "Export…").accelerator("CmdOrCtrl+E").build(app)?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&new_item)
        .item(&open)
        .separator()
        .item(&save)
        .item(&save_as)
        .separator()
        .item(&export)
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let reset_view =
        MenuItemBuilder::with_id("reset-view", "Reset View").accelerator("CmdOrCtrl+0").build(app)?;
    let view = SubmenuBuilder::new(app, "View").item(&reset_view).build()?;

    MenuBuilder::new(app).items(&[&app_menu, &file, &edit, &view]).build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init());

    // Auto-update is desktop-only: the updater fetches the signed release
    // manifest, and `process::relaunch` restarts into the installed version.
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    let app = builder
        .manage(AppState::default())
        .menu(build_menu)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if matches!(
                id,
                "new" | "open" | "save" | "save-as" | "export" | "reset-view" | "check-updates"
            ) {
                let _ = app.emit("menu-action", id.to_string());
            }
        })
        .invoke_handler(tauri::generate_handler![
            render,
            save_model,
            save_source,
            save_bytes,
            watch_files,
            take_pending_open,
            parameters,
            open_file,
            engine_version
        ])
        .build(tauri::generate_context!())
        .expect("error while building Quito desktop");

    app.run(|_app_handle, _event| {
        // macOS "open-with" / double-click delivers file URLs via Opened. Buffer
        // the path (for cold start, before the webview listens) and also emit it
        // (for a warm app already running).
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let tauri::RunEvent::Opened { urls } = _event {
            use tauri::Manager;
            for url in urls {
                let Ok(path) = url.to_file_path() else { continue };
                if path.extension().and_then(|e| e.to_str()) == Some("scad") {
                    let p = path.to_string_lossy().into_owned();
                    *_app_handle.state::<AppState>().pending_open.lock().unwrap() = Some(p.clone());
                    let _ = _app_handle.emit("open-path", p);
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_error_carries_span_for_parse_only() {
        let cache = Arc::new(Mutex::new(quito_geom::GeomCache::new()));
        // Parse error → the diagnostic carries a byte span.
        let e = eval_and_render(&cache, "cube(", ".", &[], &[], &[], &[]).unwrap_err();
        assert!(e.message.starts_with("parse error"));
        assert!(e.diagnostic.start >= 0 && e.diagnostic.end >= e.diagnostic.start);
        // Eval error (assert) → still surfaced, with the offending statement span.
        let e = eval_and_render(&cache, "assert(false);", ".", &[], &[], &[], &[]).unwrap_err();
        assert!(e.message.starts_with("evaluation error"));
        assert!(e.diagnostic.start >= 0, "eval error should carry a statement span");
    }

    #[test]
    fn native_render_command_logic() {
        let cache = Arc::new(Mutex::new(quito_geom::GeomCache::new()));
        let (mesh, _, _, _) = eval_and_render(&cache, "cube([2,3,4]);", ".", &[], &[], &[], &[]).unwrap();
        assert!((mesh.volume() - 24.0).abs() < 1e-6);

        // Overrides apply, like the customizer.
        let (mesh, out, _, _) = eval_and_render(
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
        assert_eq!(out.echoes, vec!["ECHO: 5"]);

        // In-memory library file resolves via the combined resolver.
        let (mesh, _, _, _) = eval_and_render(
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

    #[test]
    fn file_watch_detects_external_change() {
        use std::sync::mpsc::channel;
        use std::time::Duration;
        let dir = std::env::temp_dir().join(format!("quito_watch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("w.scad");
        std::fs::write(&f, "cube(1);").unwrap();

        let (tx, rx) = channel();
        let last_write = Arc::new(Mutex::new(None));
        let _w = install_watcher(&f, last_write, move |c| {
            let _ = tx.send(c);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300)); // let the watcher arm
        std::fs::write(&f, "cube(2);").unwrap();

        // FSEvents may replay the initial write, so drain until the new content
        // shows up (or we hit the deadline).
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut saw_new = false;
        while std::time::Instant::now() < deadline {
            if let Ok(s) = rx.recv_timeout(Duration::from_millis(500)) {
                if s.contains("cube(2)") {
                    saw_new = true;
                    break;
                }
            }
        }
        std::fs::remove_dir_all(&dir).ok();
        assert!(saw_new, "watcher did not report the external change");
    }

    #[test]
    fn watcher_swallows_self_write_but_reports_external() {
        use std::sync::mpsc::channel;
        use std::time::Duration;
        let dir = std::env::temp_dir().join(format!("quito_selfwrite_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("s.scad");
        std::fs::write(&f, "cube(1);").unwrap();

        let (tx, rx) = channel();
        let last_write = Arc::new(Mutex::new(None));
        let _w = install_watcher(&f, last_write.clone(), move |c| {
            let _ = tx.send(c);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300)); // let the watcher arm

        // Simulate a `save_source`: record the self-write marker, then write it.
        let canon = std::fs::canonicalize(&f).unwrap();
        *last_write.lock().unwrap() = Some((canon, "cube(9);".to_string()));
        std::fs::write(&f, "cube(9);").unwrap();

        // The self-write must NOT be delivered (marker matches on-disk content).
        let self_seen = {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let mut seen = false;
            while std::time::Instant::now() < deadline {
                if let Ok(s) = rx.recv_timeout(Duration::from_millis(300)) {
                    if s.contains("cube(9)") {
                        seen = true;
                        break;
                    }
                }
            }
            seen
        };

        // A subsequent genuine external edit IS delivered.
        std::fs::write(&f, "cube(7);").unwrap();
        let ext_seen = {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut seen = false;
            while std::time::Instant::now() < deadline {
                if let Ok(s) = rx.recv_timeout(Duration::from_millis(500)) {
                    if s.contains("cube(7)") {
                        seen = true;
                        break;
                    }
                }
            }
            seen
        };

        std::fs::remove_dir_all(&dir).ok();
        assert!(!self_seen, "watcher wrongly reported the self-write as an external edit");
        assert!(ext_seen, "watcher did not report the later external change");
    }
}
