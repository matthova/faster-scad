//! `text()` — turn a string into 2D glyph outlines (contours), using a bundled
//! copy of Liberation Sans (SIL OFL), the same font OpenSCAD ships as its
//! default, so glyph shapes match. Outlines come from `ttf-parser`; Bézier
//! segments are flattened to line segments.
//!
//! The result is a set of contours (outer boundaries and holes) that become a
//! `Node::Polygon`; even-odd triangulation in `quito-geom` turns them into a
//! filled 2D region (with holes) that can be rendered or extruded.

use std::sync::OnceLock;
use ttf_parser::Face;

/// Liberation Sans Regular (SIL Open Font License) — bundled so `text()` works
/// identically native and in the browser. See `fonts/LICENSE`.
static FONT_BYTES: &[u8] = include_bytes!("../fonts/LiberationSans-Regular.ttf");

fn face() -> &'static Face<'static> {
    static FACE: OnceLock<Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| Face::parse(FONT_BYTES, 0).expect("bundled font parses"))
}

/// Parameters for a `text()` call.
pub struct TextOpts<'a> {
    pub text: &'a str,
    pub size: f64,
    pub halign: &'a str,
    pub valign: &'a str,
    pub spacing: f64,
    pub direction: &'a str,
    /// Segments per Bézier curve (from `$fn`, clamped).
    pub segments: usize,
}

/// Flattens a glyph's outline into contours (in font units).
struct Outliner {
    contours: Vec<Vec<[f64; 2]>>,
    cur: Vec<[f64; 2]>,
    last: [f64; 2],
    seg: usize,
}

impl Outliner {
    fn new(seg: usize) -> Self {
        Outliner {
            contours: Vec::new(),
            cur: Vec::new(),
            last: [0.0, 0.0],
            seg: seg.max(1),
        }
    }
    fn flush(&mut self) {
        if self.cur.len() >= 2 {
            self.contours.push(std::mem::take(&mut self.cur));
        } else {
            self.cur.clear();
        }
    }
}

impl ttf_parser::OutlineBuilder for Outliner {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        self.last = [x as f64, y as f64];
        self.cur.push(self.last);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.last = [x as f64, y as f64];
        self.cur.push(self.last);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, c, p1) = (self.last, [x1 as f64, y1 as f64], [x as f64, y as f64]);
        for i in 1..=self.seg {
            let t = i as f64 / self.seg as f64;
            let u = 1.0 - t;
            self.cur.push([
                u * u * p0[0] + 2.0 * u * t * c[0] + t * t * p1[0],
                u * u * p0[1] + 2.0 * u * t * c[1] + t * t * p1[1],
            ]);
        }
        self.last = p1;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, c1, c2, p1) = (
            self.last,
            [x1 as f64, y1 as f64],
            [x2 as f64, y2 as f64],
            [x as f64, y as f64],
        );
        for i in 1..=self.seg {
            let t = i as f64 / self.seg as f64;
            let u = 1.0 - t;
            let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.cur.push([
                a * p0[0] + b * c1[0] + cc * c2[0] + d * p1[0],
                a * p0[1] + b * c1[1] + cc * c2[1] + d * p1[1],
            ]);
        }
        self.last = p1;
    }
    fn close(&mut self) {
        self.flush();
    }
}

/// Build the glyph contours for `opts` as `(points, paths)` suitable for a
/// `Node::Polygon`. Coordinates are in mm; the baseline is at y=0 for
/// `valign="baseline"`.
pub fn text_contours(opts: &TextOpts) -> (Vec<[f64; 2]>, Vec<Vec<u32>>) {
    let face = face();
    let upem = face.units_per_em() as f64;
    if upem <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    // OpenSCAD renders glyphs 100/72 larger than the nominal `size` (a FreeType
    // 72-DPI vs 100-unit-per-point convention); match it so text is the same
    // size as OpenSCAD's.
    let scale = opts.size / upem * (100.0 / 72.0);

    let chars: Vec<char> = opts.text.chars().collect();
    let advance = |c: char| -> f64 {
        face.glyph_index(c)
            .and_then(|g| face.glyph_hor_advance(g))
            .map(|a| a as f64 * scale * opts.spacing)
            .unwrap_or(0.0)
    };
    let widths: Vec<f64> = chars.iter().map(|&c| advance(c)).collect();
    let total: f64 = widths.iter().sum();

    let x0 = match opts.halign {
        "center" => -total / 2.0,
        "right" => -total,
        _ => 0.0,
    };
    let asc = face.ascender() as f64 * scale;
    let desc = face.descender() as f64 * scale; // negative
    let y0 = match opts.valign {
        "top" => -asc,
        "bottom" => -desc,
        "center" => -(asc + desc) / 2.0,
        _ => 0.0, // baseline
    };

    // Right-to-left just reverses the placement order.
    let rtl = opts.direction == "rtl";
    let order: Vec<usize> = if rtl {
        (0..chars.len()).rev().collect()
    } else {
        (0..chars.len()).collect()
    };

    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut paths: Vec<Vec<u32>> = Vec::new();
    let mut pen_x = x0;

    for &i in &order {
        let c = chars[i];
        if let Some(gid) = face.glyph_index(c) {
            let mut o = Outliner::new(opts.segments);
            if face.outline_glyph(gid, &mut o).is_some() {
                o.flush();
                for contour in &o.contours {
                    if contour.len() < 3 {
                        continue;
                    }
                    let start = points.len() as u32;
                    for p in contour {
                        points.push([p[0] * scale + pen_x, p[1] * scale + y0]);
                    }
                    paths.push((start..points.len() as u32).collect());
                }
            }
        }
        pen_x += widths[i];
    }
    (points, paths)
}
