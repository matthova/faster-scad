//! `text()` — turn a string into 2D glyph outlines (contours) using the bundled
//! Liberation family (Sans/Serif/Mono × Regular/Bold/Italic/BoldItalic, SIL OFL)
//! — the exact files OpenSCAD ships — so `font=` selects the same face and glyph
//! shapes match. Outlines come from `ttf-parser`; Bézier segments are flattened
//! to line segments.
//!
//! The result is a set of contours (outer boundaries and holes) that become a
//! `Node::Polygon`; even-odd triangulation in `openrscad-geom` turns them into a
//! filled 2D region (with holes) that can be rendered or extruded.

use std::sync::OnceLock;
use ttf_parser::Face;

/// One bundled Liberation face. The whole family (SIL Open Font License) is
/// bundled — the exact Liberation 2.00.1 files OpenSCAD ships — so `text(font=)`
/// works identically native and in the browser *and* matches OpenSCAD's glyph
/// shapes byte-for-byte. `family`/`style` are lowercase, `style` space-stripped
/// (`bolditalic`). See `fonts/LICENSE`.
struct BundledFont {
    family: &'static str,
    style: &'static str,
    bytes: &'static [u8],
}

macro_rules! font {
    ($family:literal, $style:literal, $file:literal) => {
        BundledFont {
            family: $family,
            style: $style,
            bytes: include_bytes!(concat!("../fonts/", $file)),
        }
    };
}

static FONTS: &[BundledFont] = &[
    font!("liberation sans", "regular", "LiberationSans-Regular.ttf"),
    font!("liberation sans", "bold", "LiberationSans-Bold.ttf"),
    font!("liberation sans", "italic", "LiberationSans-Italic.ttf"),
    font!(
        "liberation sans",
        "bolditalic",
        "LiberationSans-BoldItalic.ttf"
    ),
    font!("liberation serif", "regular", "LiberationSerif-Regular.ttf"),
    font!("liberation serif", "bold", "LiberationSerif-Bold.ttf"),
    font!("liberation serif", "italic", "LiberationSerif-Italic.ttf"),
    font!(
        "liberation serif",
        "bolditalic",
        "LiberationSerif-BoldItalic.ttf"
    ),
    font!("liberation mono", "regular", "LiberationMono-Regular.ttf"),
    font!("liberation mono", "bold", "LiberationMono-Bold.ttf"),
    font!("liberation mono", "italic", "LiberationMono-Italic.ttf"),
    font!(
        "liberation mono",
        "bolditalic",
        "LiberationMono-BoldItalic.ttf"
    ),
];

/// The parsed faces, one per [`FONTS`] entry (same order), parsed once.
fn faces() -> &'static [Face<'static>] {
    static FACES: OnceLock<Vec<Face<'static>>> = OnceLock::new();
    FACES.get_or_init(|| {
        FONTS
            .iter()
            .map(|f| Face::parse(f.bytes, 0).expect("bundled font parses"))
            .collect()
    })
}

/// Resolve an OpenSCAD `font` string (`"Family"` or `"Family:style=Style"`) to a
/// bundled face. Returns the face and whether the requested *family* is one we
/// bundle — `false` means we fell back to Liberation Sans and the caller should
/// warn. An unavailable style within a known family silently uses that family's
/// regular. Matching is case-insensitive; the style's spaces are ignored.
pub fn resolve_font(font: &str) -> (&'static Face<'static>, bool) {
    let (family_part, attrs) = font.split_once(':').unwrap_or((font, ""));
    let family = family_part.trim().to_ascii_lowercase();
    let style = attrs
        .split(':')
        .find_map(|a| a.trim().strip_prefix("style="))
        .map(|s| s.trim().to_ascii_lowercase().replace(' ', ""))
        .unwrap_or_default();
    let style = if style.is_empty() { "regular" } else { &style };

    let find = |fam: &str, sty: &str| FONTS.iter().position(|f| f.family == fam && f.style == sty);
    // Empty family means the default (Liberation Sans) — a match, not a fallback.
    let family = if family.is_empty() {
        "liberation sans"
    } else {
        &family
    };
    let known_family = FONTS.iter().any(|f| f.family == family);
    let idx = find(family, style)
        .or_else(|| find(family, "regular"))
        .unwrap_or(0);
    (&faces()[idx], known_family)
}

/// Parameters for a `text()` call.
pub struct TextOpts<'a> {
    pub text: &'a str,
    /// The resolved font face (see [`resolve_font`]).
    pub face: &'a Face<'a>,
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
    let face = opts.face;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn advance(f: &Face, c: char) -> u16 {
        f.glyph_hor_advance(f.glyph_index(c).unwrap()).unwrap()
    }

    #[test]
    fn resolve_font_selects_family_and_reports_unknown() {
        // A known family resolves and reports availability; an unknown one falls
        // back to Liberation Sans and reports `false` so the caller can warn.
        assert!(resolve_font("Liberation Serif").1);
        assert!(resolve_font("Liberation Mono").1);
        assert!(resolve_font("").1); // empty == default Liberation Sans
        assert!(!resolve_font("Arial").1);

        // Mono is fixed-width; Sans is proportional — proves distinct faces.
        let mono = resolve_font("Liberation Mono").0;
        let sans = resolve_font("Liberation Sans").0;
        assert_eq!(advance(mono, 'i'), advance(mono, 'M'));
        assert_ne!(advance(sans, 'i'), advance(sans, 'M'));
    }

    #[test]
    fn resolve_font_style_matching_is_case_and_space_insensitive() {
        // "Bold Italic", "bolditalic", mixed case all select the same face.
        let a = resolve_font("Liberation Sans:style=Bold Italic").0;
        let b = resolve_font("liberation sans:style=bolditalic").0;
        let regular = resolve_font("Liberation Sans").0;
        assert_eq!(advance(a, 'A'), advance(b, 'A'));
        // The styled face is genuinely different from regular.
        assert_ne!(advance(a, 'A'), advance(regular, 'A'));
    }
}
