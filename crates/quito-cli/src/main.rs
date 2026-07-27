//! `quito` — command-line renderer for the Quito OpenSCAD reimplementation.

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::time::Instant;

/// A fast OpenSCAD-compatible renderer (M0 subset).
#[derive(Parser, Debug)]
#[command(name = "quito", version, about)]
struct Cli {
    /// Input `.scad` file.
    input: PathBuf,

    /// Output file (`.stl`). If omitted, only prints model statistics.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// STL output format.
    #[arg(long, value_enum, default_value_t = StlFormat::Binary)]
    format: StlFormat,

    /// Print echo/warning output only; do not render geometry.
    #[arg(long)]
    check: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum StlFormat {
    Binary,
    Ascii,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let src = std::fs::read_to_string(&cli.input)
        .with_context(|| format!("reading {}", cli.input.display()))?;

    // Parse.
    let program = quito_syntax::parse(&src).map_err(|e| {
        anyhow::anyhow!("parse error at {:?}: {}", e.span, e.message)
    })?;

    // Evaluate.
    let out = quito_eval::eval_program(&program)
        .map_err(|e| anyhow::anyhow!("evaluation error: {}", e.0))?;

    for line in &out.echoes {
        println!("{line}");
    }
    for w in &out.warnings {
        eprintln!("WARNING: {w}");
    }

    if cli.check {
        return Ok(());
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
        if manifold_ok { "" } else { ", WARNING: inward-facing" },
    );

    if let Some(path) = &cli.output {
        match cli.format {
            StlFormat::Binary => {
                std::fs::write(path, mesh.to_binary_stl())
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            StlFormat::Ascii => {
                let name = cli
                    .input
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("quito");
                std::fs::write(path, mesh.to_ascii_stl(name))
                    .with_context(|| format!("writing {}", path.display()))?;
            }
        }
        eprintln!("wrote {}", path.display());
    }

    Ok(())
}
