//! `quito` — command-line renderer for the Quito OpenSCAD reimplementation.

use anyhow::{Context, Result};
use clap::Parser;
use quito_eval::{FileResolver, LoadedFile};
use std::path::{Path, PathBuf};
use std::time::Instant;

mod raster;

/// Resolves `include`/`use` paths from disk: relative to the including file,
/// then each `OPENSCADPATH` library directory.
struct DiskResolver {
    libs: Vec<PathBuf>,
}

impl FileResolver for DiskResolver {
    fn load(&self, path: &str, from_dir: &str) -> Option<LoadedFile> {
        let candidates = std::iter::once(Path::new(from_dir).join(path))
            .chain(self.libs.iter().map(|l| l.join(path)));
        for c in candidates {
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
        let candidates = std::iter::once(Path::new(from_dir).join(path))
            .chain(self.libs.iter().map(|l| l.join(path)));
        candidates.into_iter().find_map(|c| std::fs::read(&c).ok())
    }
}

/// A fast OpenSCAD-compatible renderer (M0 subset).
#[derive(Parser, Debug)]
#[command(name = "quito", version, about)]
struct Cli {
    /// Input `.scad` file.
    input: PathBuf,

    /// Output file. Format by extension: 3D `.stl`/`.off`/`.obj`/`.3mf`/`.amf`,
    /// 2D `.dxf`/`.svg`. If omitted, only prints model statistics.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// STL output format.
    #[arg(long, value_enum, default_value_t = StlFormat::Binary)]
    format: StlFormat,

    /// Print echo/warning output only; do not render geometry.
    #[arg(long)]
    check: bool,

    /// Override a top-level parameter, e.g. `-D width=20` or `-D label="hi"`.
    /// Repeatable. Values are literals (number/bool/string/vector), matching
    /// the customizer.
    #[arg(short = 'D', long = "param", value_name = "NAME=VALUE")]
    params: Vec<String>,

    /// PNG image size, e.g. `--imgsize 800,600` (or `800x600`). Default 512,512.
    #[arg(long, value_name = "W,H")]
    imgsize: Option<String>,

    /// PNG camera: eye/center `ex,ey,ez,cx,cy,cz` (6 values) or OpenSCAD gimbal
    /// `tx,ty,tz,rx,ry,rz,dist` (7). Omit to auto-frame the model.
    #[arg(long, value_name = "…")]
    camera: Option<String>,

    /// PNG projection.
    #[arg(long, value_enum, default_value_t = Proj::Perspective)]
    projection: Proj,

    /// PNG: frame the whole model, ignoring the camera distance.
    #[arg(long)]
    viewall: bool,

    /// PNG: shift the model so its center is the view target.
    #[arg(long)]
    autocenter: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum StlFormat {
    Binary,
    Ascii,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum Proj {
    Perspective,
    Ortho,
}

/// Parse `--imgsize` (`W,H` or `WxH`).
fn parse_imgsize(s: &str) -> Result<(u32, u32)> {
    let parts: Vec<&str> = s.split([',', 'x', 'X']).collect();
    if parts.len() != 2 {
        anyhow::bail!("--imgsize expects W,H (e.g. 800,600)");
    }
    let w = parts[0].trim().parse().context("--imgsize width")?;
    let h = parts[1].trim().parse().context("--imgsize height")?;
    Ok((w, h))
}

/// Parse `--camera`: 6 numbers → eye/center, 7 → OpenSCAD gimbal.
fn parse_camera(s: &str) -> Result<raster::Camera> {
    let nums: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .context("--camera expects comma-separated numbers")?;
    match nums.len() {
        6 => Ok(raster::Camera::Eye {
            eye: [nums[0], nums[1], nums[2]],
            center: [nums[3], nums[4], nums[5]],
        }),
        7 => Ok(raster::Camera::Gimbal {
            target: [nums[0], nums[1], nums[2]],
            rot: [nums[3], nums[4], nums[5]],
            dist: nums[6],
        }),
        n => anyhow::bail!("--camera expects 6 (eye,center) or 7 (gimbal) numbers, got {n}"),
    }
}

fn main() -> Result<()> {
    // Run on a worker thread with a large stack: recursive libraries (e.g.
    // BOSL2's attachment system) can nest the evaluator deeply, and OpenSCAD
    // itself runs with a large stack for the same reason.
    std::thread::Builder::new()
        .stack_size(256 << 20) // 256 MiB
        .spawn(run)
        .context("spawning worker thread")?
        .join()
        .map_err(|_| anyhow::anyhow!("worker thread panicked"))?
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let src = std::fs::read_to_string(&cli.input)
        .with_context(|| format!("reading {}", cli.input.display()))?;

    // Parse.
    let program = quito_syntax::parse(&src)
        .map_err(|e| anyhow::anyhow!("parse error at {:?}: {}", e.span, e.message))?;

    // Evaluate, resolving include/use relative to the input file + OPENSCADPATH.
    let base_dir = cli
        .input
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ".".to_string());
    let libs = std::env::var("OPENSCADPATH")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    let resolver = DiskResolver { libs };

    // Parameter overrides (`-D name=value`).
    let mut overrides = Vec::new();
    for p in &cli.params {
        let (name, val) = p
            .split_once('=')
            .with_context(|| format!("--param must be NAME=VALUE, got '{p}'"))?;
        let pv = quito_syntax::customizer::parse_value(val.trim())
            .with_context(|| format!("invalid parameter value: '{val}'"))?;
        overrides.push((name.trim().to_string(), quito_eval::value_from_param(&pv)));
    }

    let out = quito_eval::eval_program_with_params(&program, &resolver, &base_dir, &overrides)
        .map_err(|e| anyhow::anyhow!("evaluation error: {}", e.message))?;

    for line in &out.echoes {
        println!("{line}");
    }
    for w in &out.warnings {
        eprintln!("WARNING: {}", w.message);
    }

    if cli.check {
        return Ok(());
    }

    // 2D vector export (DXF/SVG): write contours directly, no 3D mesh needed.
    if let Some(path) = &cli.output {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(ext.as_str(), "dxf" | "svg") {
            match quito_geom::render_contours(&out.node) {
                Some(contours) => {
                    let text = if ext == "dxf" {
                        quito_geom::export_dxf(&contours)
                    } else {
                        quito_geom::export_svg(&contours)
                    };
                    std::fs::write(path, text)?;
                    eprintln!("wrote {} ({} contours)", path.display(), contours.len());
                }
                None => anyhow::bail!("{} export requires a 2D object", ext.to_uppercase()),
            }
            return Ok(());
        }
    }

    // Render.
    let t0 = Instant::now();
    let mesh = quito_geom::render(&out.node).context("rendering geometry")?;
    let elapsed = t0.elapsed();

    let manifold_ok = mesh.signed_volume() > 0.0 || mesh.is_empty();
    eprintln!(
        "rendered {} triangles, {} vertices in {:.1?} (volume {:.4}, area {:.4}{})",
        mesh.tris.len(),
        mesh.verts.len(),
        elapsed,
        mesh.volume(),
        mesh.surface_area(),
        if manifold_ok {
            ""
        } else {
            ", WARNING: inward-facing"
        },
    );

    if let Some(path) = &cli.output {
        let name = cli
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("quito");
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("stl")
            .to_ascii_lowercase();
        match ext.as_str() {
            "off" => std::fs::write(path, mesh.to_off())?,
            "obj" => std::fs::write(path, mesh.to_obj())?,
            // 3MF carries per-object color: partition into color groups (dropping
            // `%` background) and write one object per color. Falls back to the
            // fused single-object 3MF when the model uses no color.
            "3mf" if quito_geom::has_display_attrs(&out.node) => {
                let groups =
                    quito_geom::render_groups(&out.node).context("rendering color groups")?;
                let colored: Vec<(&quito_geom::Mesh, [f32; 4])> = groups
                    .iter()
                    .filter(|g| g.mode != quito_geom::DisplayMode::Background)
                    .map(|g| (&g.mesh, g.color))
                    .collect();
                std::fs::write(path, quito_geom::Mesh::to_3mf_colored(&colored))?
            }
            "3mf" => std::fs::write(path, mesh.to_3mf())?,
            "amf" => std::fs::write(path, mesh.to_amf())?,
            // PNG: headless software rasterizer over the colored groups (dropping
            // `%` background), honoring --imgsize/--camera/--projection.
            "png" => {
                let (w, h) = match &cli.imgsize {
                    Some(s) => parse_imgsize(s)?,
                    None => (512, 512),
                };
                let camera = match &cli.camera {
                    Some(s) => parse_camera(s)?,
                    None => raster::Camera::Auto,
                };
                let projection = match cli.projection {
                    Proj::Perspective => raster::Projection::Perspective { fov_deg: 45.0 },
                    Proj::Ortho => raster::Projection::Ortho,
                };
                let groups =
                    quito_geom::render_groups(&out.node).context("rendering color groups")?;
                let colored: Vec<(&quito_geom::Mesh, [f32; 4])> = groups
                    .iter()
                    .filter(|g| g.mode != quito_geom::DisplayMode::Background)
                    .map(|g| (&g.mesh, g.color))
                    .collect();
                let opts = raster::RenderOpts {
                    width: w,
                    height: h,
                    camera,
                    projection,
                    viewall: cli.viewall,
                    autocenter: cli.autocenter,
                    ..Default::default()
                };
                let bytes = raster::render_png(&colored, &opts)
                    .map_err(|e| anyhow::anyhow!("png render: {e}"))?;
                std::fs::write(path, bytes)?;
            }
            "stl" if matches!(cli.format, StlFormat::Ascii) => {
                std::fs::write(path, mesh.to_ascii_stl(name))?
            }
            _ => std::fs::write(path, mesh.to_binary_stl())?,
        }
        eprintln!("wrote {}", path.display());
    }

    Ok(())
}
