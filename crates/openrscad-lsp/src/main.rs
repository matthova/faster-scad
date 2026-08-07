//! `openrscad-lsp` — a Language Server Protocol server for OpenSCAD, backed by the
//! OpenRSCAD engine.
//!
//! This is a fourth front-end (alongside the CLI, wasm, and Tauri app) over the
//! same `parse → eval_program_with_params` pipeline. It provides, in any
//! LSP-capable editor (VS Code, Neovim, Zed, Helix, Emacs, …):
//!
//!   * **Diagnostics** — parse/eval errors and warnings, mapped from the engine's
//!     byte spans to LSP ranges, on open/change/save.
//!   * **Hover** — signature + docs for built-ins and the document's own
//!     modules/functions/variables.
//!   * **Completion** — built-ins plus in-document symbols.
//!   * **Document symbols** — an outline of the file's defs.
//!   * **`openrscad.render` command** — render the document to a mesh/vector file
//!     (STL/OFF/OBJ/3MF/AMF/DXF/SVG) on disk, returning stats.
//!   * **Live preview streaming** — `openrscad.startPreview`/`openrscad.stopPreview`
//!     commands register a document for live rendering; the server then pushes a
//!     `openrscad/preview` notification (native-kernel geometry as base64 vertex
//!     buffers, or an error) on start, on save, and — debounced — as the buffer
//!     changes. Models using `color()`/`#`/`%` also carry a colored group
//!     channel. The editor supplies only the display surface; the geometry is
//!     computed here, on the fast native Manifold kernel.
//!
//! Evaluation and geometry run on a 256 MiB-stack worker thread (recursive
//! libraries like BOSL2 nest the evaluator deeply), mirroring the CLI.

mod analyze;
mod builtins;
mod line_index;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use line_index::LineIndex;
use openrscad_eval::{FileResolver, LoadedFile};
use serde_json::json;
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

/// The `include`/`use` resolver overlay: canonical path → live buffer source.
type Overlay = HashMap<String, String>;
/// Shared, mutable map of open documents (URI → current text).
type Docs = Arc<Mutex<HashMap<Url, String>>>;

/// How long the server waits for edits to settle before re-rendering a live
/// preview. Diagnostics still run on every keystroke; only geometry is debounced.
const PREVIEW_DEBOUNCE_MS: u64 = 200;

/// The custom `openrscad/preview` notification: server → client, carrying a freshly
/// rendered mesh (or an error) for a previewed document. See [`PreviewMsg`].
enum OpenRSCADPreview {}
impl Notification for OpenRSCADPreview {
    type Params = serde_json::Value;
    const METHOD: &'static str = "openrscad/preview";
}

/// A diagnostic as produced by the engine, before mapping to LSP coordinates.
struct RawDiag {
    severity: DiagnosticSeverity,
    message: String,
    /// Byte span into the source, or `None` (attach to the whole document).
    span: Option<std::ops::Range<usize>>,
}

/// Resolves `include`/`use` against open editor buffers first (so unsaved edits
/// are honored), then disk + `OPENSCADPATH`. Mirrors the CLI's `DiskResolver`
/// with an in-memory overlay bolted on.
struct OverlayResolver {
    /// Canonicalized absolute path → current buffer contents, for open files.
    overlay: HashMap<String, String>,
    libs: Vec<PathBuf>,
}

impl OverlayResolver {
    fn key_for(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }
}

impl FileResolver for OverlayResolver {
    fn load(&self, path: &str, from_dir: &str) -> Option<LoadedFile> {
        let candidates = std::iter::once(Path::new(from_dir).join(path))
            .chain(self.libs.iter().map(|l| l.join(path)));
        for c in candidates {
            let key = Self::key_for(&c);
            let dir = c
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Prefer an open buffer's live contents.
            if let Some(source) = self.overlay.get(&key) {
                return Some(LoadedFile {
                    key,
                    source: source.clone(),
                    dir,
                });
            }
            if let Ok(source) = std::fs::read_to_string(&c) {
                return Some(LoadedFile { key, source, dir });
            }
        }
        None
    }

    fn load_bytes(&self, path: &str, from_dir: &str) -> Option<Vec<u8>> {
        let candidates = std::iter::once(Path::new(from_dir).join(path))
            .chain(self.libs.iter().map(|l| l.join(path)));
        candidates.into_iter().find_map(|c| std::fs::read(&c).ok())
    }
}

/// Read `OPENSCADPATH` into a list of library directories.
fn openscad_libs() -> Vec<PathBuf> {
    std::env::var("OPENSCADPATH")
        .unwrap_or_default()
        .split(if cfg!(windows) { ';' } else { ':' })
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Run `f` on a worker thread with a 256 MiB stack (recursive libraries can nest
/// the evaluator deeply). Panics in `f` surface as an `Err` message.
fn on_big_stack<T, F>(f: F) -> std::thread::Result<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .expect("spawn worker thread")
        .join()
}

/// Parse + evaluate `source`, returning diagnostics. No geometry (fast enough to
/// run on every keystroke). `base_dir` roots relative `include`/`use`.
fn diagnose(source: &str, base_dir: &str, overlay: HashMap<String, String>) -> Vec<RawDiag> {
    let program = match openrscad_syntax::parse(source) {
        Ok(p) => p,
        Err(e) => {
            return vec![RawDiag {
                severity: DiagnosticSeverity::ERROR,
                message: e.message,
                span: Some(e.span),
            }];
        }
    };
    let resolver = OverlayResolver {
        overlay,
        libs: openscad_libs(),
    };
    match openrscad_eval::eval_program_with_params(&program, &resolver, base_dir, &[]) {
        Ok(out) => out
            .warnings
            .into_iter()
            .map(|w| RawDiag {
                severity: DiagnosticSeverity::WARNING,
                message: w.message,
                span: w.span,
            })
            .collect(),
        Err(e) => vec![RawDiag {
            severity: DiagnosticSeverity::ERROR,
            message: e.message,
            span: e.span,
        }],
    }
}

/// Outcome of a `openrscad.render` command.
enum RenderOutcome {
    Ok {
        path: String,
        triangles: usize,
        vertices: usize,
        volume: f64,
        area: f64,
    },
    Err(String),
}

/// Render `source` to `output` (format chosen by extension), returning stats.
fn render_to_file(
    source: &str,
    base_dir: &str,
    overlay: HashMap<String, String>,
    output: &Path,
) -> RenderOutcome {
    let program = match openrscad_syntax::parse(source) {
        Ok(p) => p,
        Err(e) => return RenderOutcome::Err(format!("parse error: {}", e.message)),
    };
    let resolver = OverlayResolver {
        overlay,
        libs: openscad_libs(),
    };
    let out = match openrscad_eval::eval_program_with_params(&program, &resolver, base_dir, &[]) {
        Ok(o) => o,
        Err(e) => return RenderOutcome::Err(format!("evaluation error: {}", e.message)),
    };

    let ext = output
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("stl")
        .to_ascii_lowercase();

    // 2D vector export needs only contours.
    if matches!(ext.as_str(), "dxf" | "svg") {
        return match openrscad_geom::render_contours(&out.node) {
            Some(contours) => {
                let text = if ext == "dxf" {
                    openrscad_geom::export_dxf(&contours)
                } else {
                    openrscad_geom::export_svg(&contours)
                };
                match std::fs::write(output, text) {
                    Ok(()) => RenderOutcome::Ok {
                        path: output.to_string_lossy().into_owned(),
                        triangles: 0,
                        vertices: 0,
                        volume: 0.0,
                        area: 0.0,
                    },
                    Err(e) => RenderOutcome::Err(format!("writing {}: {e}", output.display())),
                }
            }
            None => RenderOutcome::Err(format!(
                "{} export requires a 2D object",
                ext.to_uppercase()
            )),
        };
    }

    // 3D mesh.
    let mut cache = openrscad_geom::GeomCache::new();
    let (mesh, _warns) = match openrscad_geom::render_cached_warns(
        &out.node,
        &openrscad_geom::ManifoldKernel::new(),
        &mut cache,
    ) {
        Ok(v) => v,
        Err(e) => return RenderOutcome::Err(format!("geometry error: {e}")),
    };
    let name = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("openrscad");
    let bytes: Vec<u8> = match ext.as_str() {
        "off" => mesh.to_off().into_bytes(),
        "obj" => mesh.to_obj().into_bytes(),
        "amf" => mesh.to_amf().into_bytes(),
        "3mf" if openrscad_geom::has_display_attrs(&out.node) => {
            match openrscad_geom::render_groups(&out.node) {
                Ok(groups) => {
                    let colored: Vec<(&openrscad_geom::Mesh, [f32; 4])> = groups
                        .iter()
                        .filter(|g| g.mode != openrscad_geom::DisplayMode::Background)
                        .map(|g| (&g.mesh, g.color))
                        .collect();
                    openrscad_geom::Mesh::to_3mf_colored(&colored)
                }
                Err(e) => return RenderOutcome::Err(format!("geometry error: {e}")),
            }
        }
        "3mf" => mesh.to_3mf(),
        "stl_ascii" => mesh.to_ascii_stl(name).into_bytes(),
        _ => mesh.to_binary_stl(),
    };
    match std::fs::write(output, bytes) {
        Ok(()) => RenderOutcome::Ok {
            path: output.to_string_lossy().into_owned(),
            triangles: mesh.tris.len(),
            vertices: mesh.verts.len(),
            volume: mesh.volume(),
            area: mesh.surface_area(),
        },
        Err(e) => RenderOutcome::Err(format!("writing {}: {e}", output.display())),
    }
}

/// Standard base64 (RFC 4648) — a few lines, so no dependency. Used to pack raw
/// little-endian vertex buffers into the JSON `openrscad/preview` notification.
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Base64 of a `f32` slice as contiguous little-endian bytes, ready for the
/// client to reinterpret as a `Float32Array` (browsers are little-endian).
fn f32_slice_b64(v: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for f in v {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    base64_encode(&bytes)
}

/// The colored preview channel: a single triangle soup plus a JSON array of
/// per-group `{start, count, color, mode}` ranges (`color()`/`#`/`%`). Present
/// only for models that use display attributes; plain models omit it.
struct PreviewGroups {
    positions: Vec<f32>,
    normals: Vec<f32>,
    groups: serde_json::Value,
}

/// The provenance channel: a per-leaf triangle soup plus a JSON array of
/// `{start, count, spans}` ranges for hierarchical editor↔preview linking
/// (`spans` is the outermost→innermost stack of source byte-ranges). Present for
/// 3D models with geometry.
struct ProvenanceChannel {
    positions: Vec<f32>,
    normals: Vec<f32>,
    groups: serde_json::Value,
}

/// A `openrscad/preview` payload: either a rendered mesh or an error message.
enum PreviewMsg {
    Ok {
        /// Triangle-soup vertex positions, 9 f32 per triangle (fused mesh; the
        /// source of truth for stats and the plain, uncolored view).
        positions: Vec<f32>,
        /// Per-face (flat) normals, 9 f32 per triangle.
        normals: Vec<f32>,
        triangles: usize,
        vertices: usize,
        volume: f64,
        area: f64,
        /// Colored preview channel, when the model uses `color()`/`#`/`%`.
        /// Boxed to keep the `Ok` variant small (both channels carry full soups).
        groups: Option<Box<PreviewGroups>>,
        /// Provenance channel for editor↔preview linking (3D models). Boxed too.
        provenance: Option<Box<ProvenanceChannel>>,
    },
    Err(String),
}

impl PreviewMsg {
    /// Serialize for the `openrscad/preview` notification, tagged with its document.
    fn to_json(&self, uri: &Url) -> serde_json::Value {
        match self {
            PreviewMsg::Ok {
                positions,
                normals,
                triangles,
                vertices,
                volume,
                area,
                groups,
                provenance,
            } => {
                let mut v = json!({
                    "uri": uri.to_string(),
                    "ok": true,
                    "positions": f32_slice_b64(positions),
                    "normals": f32_slice_b64(normals),
                    "triangleCount": triangles,
                    "vertexCount": vertices,
                    "volume": volume,
                    "area": area,
                });
                let obj = v.as_object_mut().unwrap();
                // Attach the colored channel only when present; the client falls
                // back to the plain mesh otherwise.
                if let Some(g) = groups {
                    obj.insert(
                        "previewPositions".into(),
                        json!(f32_slice_b64(&g.positions)),
                    );
                    obj.insert("previewNormals".into(), json!(f32_slice_b64(&g.normals)));
                    obj.insert("groups".into(), g.groups.clone());
                }
                // Attach the provenance channel for editor↔preview linking.
                if let Some(p) = provenance {
                    obj.insert(
                        "provenancePositions".into(),
                        json!(f32_slice_b64(&p.positions)),
                    );
                    obj.insert("provenanceNormals".into(), json!(f32_slice_b64(&p.normals)));
                    obj.insert("provenance".into(), p.groups.clone());
                }
                v
            }
            PreviewMsg::Err(msg) => json!({
                "uri": uri.to_string(),
                "ok": false,
                "error": msg,
            }),
        }
    }
}

/// Parse + evaluate + render `source` to a triangle soup for live preview, on the
/// native Manifold kernel. Mirrors the wasm engine's plain (non-colored) path, so
/// the client renders identical geometry.
fn render_preview(source: &str, base_dir: &str, overlay: Overlay) -> PreviewMsg {
    let program = match openrscad_syntax::parse(source) {
        Ok(p) => p,
        Err(e) => return PreviewMsg::Err(format!("parse error: {}", e.message)),
    };
    let resolver = OverlayResolver {
        overlay,
        libs: openscad_libs(),
    };
    let out = match openrscad_eval::eval_program_with_params(&program, &resolver, base_dir, &[]) {
        Ok(o) => o,
        Err(e) => return PreviewMsg::Err(format!("evaluation error: {}", e.message)),
    };
    let kernel = openrscad_geom::ManifoldKernel::new();
    let mut cache = openrscad_geom::GeomCache::new();
    let (mesh, _warns) = match openrscad_geom::render_cached_warns(&out.node, &kernel, &mut cache) {
        Ok(v) => v,
        Err(e) => return PreviewMsg::Err(format!("geometry error: {e}")),
    };
    let (positions, normals) = mesh.to_triangle_soup_f32();

    // Colored preview channel — only for models that use `color()`/`#`/`%`, so
    // plain models keep the single-mesh path. Shares the cache with the fused
    // render above, so opaque leaf meshes aren't recomputed just to color them.
    let groups = if openrscad_geom::has_display_attrs(&out.node) {
        match openrscad_geom::render_groups_cached(&out.node, &kernel, &mut cache) {
            Ok(g) => {
                let (positions, normals, json) = openrscad_geom::preview_channel(&g);
                let groups = serde_json::from_str(&json).unwrap_or_else(|_| json!([]));
                Some(Box::new(PreviewGroups {
                    positions,
                    normals,
                    groups,
                }))
            }
            // A grouped-render failure just drops color; the plain mesh still shows.
            Err(_) => None,
        }
    } else {
        None
    };

    // Provenance channel for editor↔preview linking — any model with geometry
    // (2D flat meshes and 3D solids alike). Shares the cache with the fused
    // render, so opaque leaf meshes aren't recomputed just to tag them with a
    // span.
    let provenance = if !mesh.tris.is_empty() {
        match openrscad_geom::render_provenance_cached(&out.node, &kernel, &mut cache) {
            Ok(g) => {
                let (positions, normals, json) = openrscad_geom::provenance_channel(&g);
                let groups = serde_json::from_str(&json).unwrap_or_else(|_| json!([]));
                Some(Box::new(ProvenanceChannel {
                    positions,
                    normals,
                    groups,
                }))
            }
            Err(_) => None,
        }
    } else {
        None
    };

    PreviewMsg::Ok {
        positions,
        normals,
        triangles: mesh.tris.len(),
        vertices: mesh.verts.len(),
        volume: mesh.volume(),
        area: mesh.surface_area(),
        groups,
        provenance,
    }
}

/// Snapshot the open buffers as a `canonical-path → source` overlay for the
/// `include`/`use` resolver.
fn overlay_of(docs: &Docs) -> Overlay {
    docs.lock()
        .unwrap()
        .iter()
        .filter_map(|(uri, text)| {
            let path = uri.to_file_path().ok()?;
            Some((OverlayResolver::key_for(&path), text.clone()))
        })
        .collect()
}

/// Parse the `i`th command argument as a document `Url` (arguments arrive as
/// JSON strings).
fn arg_uri(params: &ExecuteCommandParams, i: usize) -> Option<Url> {
    params
        .arguments
        .get(i)
        .and_then(|v| v.as_str())
        .and_then(|s| Url::parse(s).ok())
}

/// Render `uri`'s current buffer and push the result to the client as a
/// `openrscad/preview` notification. Geometry runs on the big-stack worker thread.
async fn render_and_push(client: &Client, docs: &Docs, uri: Url) {
    let Some(text) = docs.lock().unwrap().get(&uri).cloned() else {
        return;
    };
    let base = Backend::base_dir(&uri);
    let overlay = overlay_of(docs);
    let msg = tokio::task::spawn_blocking(move || {
        on_big_stack(move || render_preview(&text, &base, overlay))
            .unwrap_or_else(|_| PreviewMsg::Err("render thread panicked".into()))
    })
    .await
    .unwrap_or_else(|e| PreviewMsg::Err(format!("render task failed: {e}")));
    client
        .send_notification::<OpenRSCADPreview>(msg.to_json(&uri))
        .await;
}

/// A registered live preview: a generation counter for debounce bookkeeping, and
/// whether to re-render as the buffer changes (vs. on save only).
struct PreviewState {
    generation: u64,
    live: bool,
}

/// The language server. Holds the set of open documents and active previews.
struct Backend {
    client: Client,
    /// Open documents: URI → current text.
    docs: Docs,
    /// Documents registered for live preview: URI → [`PreviewState`].
    previews: Arc<Mutex<HashMap<Url, PreviewState>>>,
}

impl Backend {
    /// Snapshot the open buffers as a `canonical-path → source` overlay for the
    /// `include`/`use` resolver.
    fn overlay(&self) -> Overlay {
        overlay_of(&self.docs)
    }

    /// Debounced re-render for a live preview. Called on every edit; only fires a
    /// render once edits go quiet for [`PREVIEW_DEBOUNCE_MS`], and skips entirely
    /// if the document isn't a live preview or a newer edit supersedes this one.
    fn schedule_preview(&self, uri: Url) {
        let generation = {
            let mut previews = self.previews.lock().unwrap();
            match previews.get_mut(&uri) {
                Some(st) if st.live => {
                    st.generation += 1;
                    st.generation
                }
                _ => return,
            }
        };
        let client = self.client.clone();
        let docs = self.docs.clone();
        let previews = self.previews.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(PREVIEW_DEBOUNCE_MS)).await;
            // A newer edit bumped the generation, or the preview was stopped.
            match previews.lock().unwrap().get(&uri) {
                Some(st) if st.generation == generation => {}
                _ => return,
            }
            render_and_push(&client, &docs, uri).await;
        });
    }

    /// The base directory for resolving a document's relative includes.
    fn base_dir(uri: &Url) -> String {
        uri.to_file_path()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ".".to_string())
    }

    /// Compute and publish diagnostics for one document.
    async fn publish(&self, uri: Url) {
        let Some(text) = self.docs.lock().unwrap().get(&uri).cloned() else {
            return;
        };
        let base = Self::base_dir(&uri);
        let overlay = self.overlay();
        // Eval on the big-stack worker, off the async runtime.
        let text_for_worker = text.clone();
        let raw = tokio::task::spawn_blocking(move || {
            on_big_stack(move || diagnose(&text_for_worker, &base, overlay)).unwrap_or_else(|_| {
                vec![RawDiag {
                    severity: DiagnosticSeverity::ERROR,
                    message: "internal error while analyzing document".into(),
                    span: None,
                }]
            })
        })
        .await
        .unwrap_or_default();

        let idx = LineIndex::new(&text);
        let whole = Range::new(idx.position(0), idx.position(text.len()));
        let diags: Vec<Diagnostic> = raw
            .into_iter()
            .map(|d| Diagnostic {
                range: d.span.map(|s| idx.range(s)).unwrap_or(whole),
                severity: Some(d.severity),
                source: Some("openrscad".into()),
                message: d.message,
                ..Default::default()
            })
            .collect();
        self.client.publish_diagnostics(uri, diags, None).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "openrscad-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["$".into()]),
                    ..Default::default()
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "openrscad.render".into(),
                        "openrscad.startPreview".into(),
                        "openrscad.stopPreview".into(),
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "openrscad-lsp ready")
            .await;
    }

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        self.docs
            .lock()
            .unwrap()
            .insert(uri.clone(), params.text_document.text);
        self.publish(uri).await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        // FULL sync: the last change holds the entire new text.
        if let Some(change) = params.content_changes.pop() {
            let uri = params.text_document.uri.clone();
            self.docs.lock().unwrap().insert(uri.clone(), change.text);
            self.publish(uri.clone()).await;
            // Live previews re-render (debounced) as the buffer changes.
            self.schedule_preview(uri);
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        // Re-analyze (a saved dependency may change a dependent's diagnostics).
        self.publish(uri.clone()).await;
        // A preview always refreshes on save, even when live rendering is off.
        if self.previews.lock().unwrap().contains_key(&uri) {
            render_and_push(&self.client, &self.docs, uri).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.docs.lock().unwrap().remove(&uri);
        self.previews.lock().unwrap().remove(&uri);
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let Some(text) = self.docs.lock().unwrap().get(&uri).cloned() else {
            return Ok(None);
        };
        let idx = LineIndex::new(&text);
        let byte = idx.offset(pos);
        let Some((word, span)) = idx.word_at(byte) else {
            return Ok(None);
        };

        // Built-in first, then a user-defined symbol.
        let markdown = if let Some(b) = builtins::lookup(&word) {
            Some(builtins::hover_markdown(b))
        } else {
            openrscad_syntax::parse(&text).ok().and_then(|prog| {
                analyze::collect(&prog)
                    .into_iter()
                    .find(|s| s.name == word)
                    .map(|s| format!("```openscad\n{}\n```", s.signature))
            })
        };

        Ok(markdown.map(|value| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(idx.range(span)),
        }))
    }

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let text = self.docs.lock().unwrap().get(&uri).cloned();

        let mut items: Vec<CompletionItem> = Vec::new();

        // Built-ins.
        for b in builtins::BUILTINS {
            items.push(CompletionItem {
                label: b.name.into(),
                // OpenSCAD modules and functions are both call-site completions;
                // the function icon reads best for both.
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(b.signature.into()),
                documentation: Some(Documentation::String(b.doc.into())),
                ..Default::default()
            });
        }

        // In-document symbols.
        if let Some(text) = text {
            if let Ok(prog) = openrscad_syntax::parse(&text) {
                for s in analyze::collect(&prog) {
                    let kind = match s.kind {
                        analyze::SymbolKind::Module => CompletionItemKind::MODULE,
                        analyze::SymbolKind::Function => CompletionItemKind::FUNCTION,
                        analyze::SymbolKind::Variable => CompletionItemKind::VARIABLE,
                    };
                    items.push(CompletionItem {
                        label: s.name,
                        kind: Some(kind),
                        detail: Some(s.signature),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(text) = self.docs.lock().unwrap().get(&uri).cloned() else {
            return Ok(None);
        };
        let Ok(prog) = openrscad_syntax::parse(&text) else {
            return Ok(None);
        };
        let idx = LineIndex::new(&text);
        #[allow(deprecated)] // `deprecated` field is required by the struct literal.
        let symbols: Vec<DocumentSymbol> = analyze::collect(&prog)
            .into_iter()
            .map(|s| {
                let range = idx.range(s.span);
                DocumentSymbol {
                    name: s.name,
                    detail: Some(s.signature),
                    kind: match s.kind {
                        analyze::SymbolKind::Module => SymbolKind::MODULE,
                        analyze::SymbolKind::Function => SymbolKind::FUNCTION,
                        analyze::SymbolKind::Variable => SymbolKind::VARIABLE,
                    },
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range: range,
                    children: None,
                }
            })
            .collect();
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> RpcResult<Option<serde_json::Value>> {
        // Preview lifecycle commands are handled up front; the remainder of this
        // function is the on-disk file export for `openrscad.render`.
        match params.command.as_str() {
            "openrscad.startPreview" => {
                let Some(uri) = arg_uri(&params, 0) else {
                    return Ok(Some(json!({"ok": false, "error": "invalid document URI"})));
                };
                // args[1] = optional `{ live: bool }` (default: render as you type).
                let live = params
                    .arguments
                    .get(1)
                    .and_then(|v| v.get("live"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                self.previews.lock().unwrap().insert(
                    uri.clone(),
                    PreviewState {
                        generation: 0,
                        live,
                    },
                );
                render_and_push(&self.client, &self.docs, uri).await;
                return Ok(Some(json!({"ok": true})));
            }
            "openrscad.stopPreview" => {
                if let Some(uri) = arg_uri(&params, 0) {
                    self.previews.lock().unwrap().remove(&uri);
                }
                return Ok(Some(json!({"ok": true})));
            }
            "openrscad.render" => {}
            _ => return Ok(None),
        }
        // args[0] = document URI (string); args[1] = optional output path (string).
        let uri_str = params
            .arguments
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let Ok(uri) = Url::parse(&uri_str) else {
            return Ok(Some(json!({"ok": false, "error": "invalid document URI"})));
        };
        let Some(text) = self.docs.lock().unwrap().get(&uri).cloned() else {
            return Ok(Some(json!({"ok": false, "error": "document not open"})));
        };
        let base = Self::base_dir(&uri);
        let overlay = self.overlay();

        // Output path: explicit arg, else the source with a `.stl` extension.
        let output: PathBuf = match params.arguments.get(1).and_then(|v| v.as_str()) {
            Some(p) => PathBuf::from(p),
            None => match uri.to_file_path() {
                Ok(p) => p.with_extension("stl"),
                Err(()) => {
                    return Ok(Some(
                        json!({"ok": false, "error": "cannot derive output path; pass one explicitly"}),
                    ))
                }
            },
        };

        let outcome = tokio::task::spawn_blocking(move || {
            on_big_stack(move || render_to_file(&text, &base, overlay, &output))
                .unwrap_or_else(|_| RenderOutcome::Err("render thread panicked".into()))
        })
        .await
        .unwrap_or_else(|e| RenderOutcome::Err(format!("render task failed: {e}")));

        let value = match outcome {
            RenderOutcome::Ok {
                path,
                triangles,
                vertices,
                volume,
                area,
            } => {
                self.client
                    .log_message(MessageType::INFO, format!("openrscad rendered {path}"))
                    .await;
                json!({
                    "ok": true,
                    "path": path,
                    "triangles": triangles,
                    "vertices": vertices,
                    "volume": volume,
                    "area": area,
                })
            }
            RenderOutcome::Err(msg) => {
                self.client
                    .show_message(
                        MessageType::ERROR,
                        format!("openrscad render failed: {msg}"),
                    )
                    .await;
                json!({ "ok": false, "error": msg })
            }
        };
        Ok(Some(value))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: Arc::new(Mutex::new(HashMap::new())),
        previews: Arc::new(Mutex::new(HashMap::new())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}
