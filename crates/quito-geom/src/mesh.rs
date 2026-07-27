//! An indexed triangle mesh plus geometric utilities and STL export.

/// An indexed triangle mesh with f64 vertices.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub verts: Vec<[f64; 3]>,
    pub tris: Vec<[u32; 3]>,
}

impl Mesh {
    pub fn new() -> Self {
        Mesh::default()
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Signed volume (positive when triangles are wound outward / CCW).
    pub fn signed_volume(&self) -> f64 {
        let mut v = 0.0;
        for t in &self.tris {
            let a = self.verts[t[0] as usize];
            let b = self.verts[t[1] as usize];
            let c = self.verts[t[2] as usize];
            v += a[0] * (b[1] * c[2] - b[2] * c[1])
                - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
        v / 6.0
    }

    pub fn volume(&self) -> f64 {
        self.signed_volume().abs()
    }

    /// Total surface area.
    pub fn surface_area(&self) -> f64 {
        let mut area = 0.0;
        for t in &self.tris {
            let a = self.verts[t[0] as usize];
            let b = self.verts[t[1] as usize];
            let c = self.verts[t[2] as usize];
            let ab = sub(b, a);
            let ac = sub(c, a);
            area += norm(cross(ab, ac)) * 0.5;
        }
        area
    }

    /// Axis-aligned bounding box (min, max), or None if empty.
    pub fn bbox(&self) -> Option<([f64; 3], [f64; 3])> {
        let mut it = self.verts.iter();
        let first = *it.next()?;
        let mut lo = first;
        let mut hi = first;
        for v in it {
            for i in 0..3 {
                lo[i] = lo[i].min(v[i]);
                hi[i] = hi[i].max(v[i]);
            }
        }
        Some((lo, hi))
    }

    /// Reverse triangle winding if the signed volume is negative, guaranteeing
    /// outward-facing normals for a consistently-oriented closed mesh.
    pub fn ensure_outward(&mut self) {
        if self.signed_volume() < 0.0 {
            self.flip_winding();
        }
    }

    pub fn flip_winding(&mut self) {
        for t in &mut self.tris {
            t.swap(1, 2);
        }
    }

    /// Expand to a non-indexed triangle soup with per-face (flat) normals,
    /// as f32, for direct upload to a GPU buffer. Returns `(positions,
    /// normals)`, each with 9 floats per triangle.
    pub fn to_triangle_soup_f32(&self) -> (Vec<f32>, Vec<f32>) {
        let n = self.tris.len();
        let mut positions = Vec::with_capacity(n * 9);
        let mut normals = Vec::with_capacity(n * 9);
        for t in &self.tris {
            let a = self.verts[t[0] as usize];
            let b = self.verts[t[1] as usize];
            let c = self.verts[t[2] as usize];
            let nrm = normalize(cross(sub(b, a), sub(c, a)));
            for v in [a, b, c] {
                positions.extend_from_slice(&[v[0] as f32, v[1] as f32, v[2] as f32]);
                normals.extend_from_slice(&[nrm[0] as f32, nrm[1] as f32, nrm[2] as f32]);
            }
        }
        (positions, normals)
    }

    /// Serialize as binary STL.
    pub fn to_binary_stl(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(84 + self.tris.len() * 50);
        out.extend_from_slice(&[0u8; 80]); // header
        out.extend_from_slice(&(self.tris.len() as u32).to_le_bytes());
        for t in &self.tris {
            let a = self.verts[t[0] as usize];
            let b = self.verts[t[1] as usize];
            let c = self.verts[t[2] as usize];
            let n = normalize(cross(sub(b, a), sub(c, a)));
            for comp in n {
                out.extend_from_slice(&(comp as f32).to_le_bytes());
            }
            for v in [a, b, c] {
                for comp in v {
                    out.extend_from_slice(&(comp as f32).to_le_bytes());
                }
            }
            out.extend_from_slice(&[0u8, 0u8]); // attribute byte count
        }
        out
    }

    /// Serialize as OFF (Object File Format).
    pub fn to_off(&self) -> String {
        let mut s = format!("OFF\n{} {} 0\n", self.verts.len(), self.tris.len());
        for v in &self.verts {
            s.push_str(&format!("{} {} {}\n", v[0], v[1], v[2]));
        }
        for t in &self.tris {
            s.push_str(&format!("3 {} {} {}\n", t[0], t[1], t[2]));
        }
        s
    }

    /// Serialize as Wavefront OBJ (1-indexed faces).
    pub fn to_obj(&self) -> String {
        let mut s = String::new();
        for v in &self.verts {
            s.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
        }
        for t in &self.tris {
            s.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
        }
        s
    }

    /// Parse a binary or ASCII STL into an indexed mesh (welding coincident
    /// vertices at 1e-6 precision).
    pub fn from_stl(bytes: &[u8]) -> Mesh {
        // ASCII if it starts with "solid" and contains "facet".
        let is_ascii = bytes.starts_with(b"solid")
            && bytes.windows(5).take(512).any(|w| w == b"facet");
        let raw_tris: Vec<[[f64; 3]; 3]> = if is_ascii {
            parse_ascii_stl(&String::from_utf8_lossy(bytes))
        } else {
            parse_binary_stl(bytes)
        };
        let mut mesh = Mesh::new();
        let mut map: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
        let key = |p: [f64; 3]| [
            (p[0] * 1e6).round() as i64,
            (p[1] * 1e6).round() as i64,
            (p[2] * 1e6).round() as i64,
        ];
        for tri in raw_tris {
            let mut idx = [0u32; 3];
            for (k, p) in tri.iter().enumerate() {
                let e = *map.entry(key(*p)).or_insert_with(|| {
                    mesh.verts.push(*p);
                    (mesh.verts.len() - 1) as u32
                });
                idx[k] = e;
            }
            mesh.tris.push(idx);
        }
        mesh
    }

    /// Parse an OFF file.
    pub fn from_off(text: &str) -> Mesh {
        let mut mesh = Mesh::new();
        let mut nums = text
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .flat_map(|l| l.split_whitespace())
            .filter(|t| *t != "OFF");
        let nv: usize = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0);
        let nf: usize = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0);
        let _edges = nums.next();
        for _ in 0..nv {
            let x = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0.0);
            let y = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0.0);
            let z = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0.0);
            mesh.verts.push([x, y, z]);
        }
        for _ in 0..nf {
            let k: usize = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0);
            let idx: Vec<u32> = (0..k).filter_map(|_| nums.next()?.parse().ok()).collect();
            for j in 1..idx.len().saturating_sub(1) {
                mesh.tris.push([idx[0], idx[j], idx[j + 1]]);
            }
        }
        mesh
    }

    /// Parse a Wavefront OBJ file (vertices and triangulated faces).
    pub fn from_obj(text: &str) -> Mesh {
        let mut mesh = Mesh::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("v") => {
                    let c: Vec<f64> = it.filter_map(|t| t.parse().ok()).collect();
                    if c.len() >= 3 {
                        mesh.verts.push([c[0], c[1], c[2]]);
                    }
                }
                Some("f") => {
                    // face indices may be `i`, `i/j`, `i//k`; take the vertex index.
                    let idx: Vec<i64> = it
                        .filter_map(|t| t.split('/').next()?.parse().ok())
                        .collect();
                    let n = mesh.verts.len() as i64;
                    let resolve = |i: i64| if i < 0 { (n + i) as u32 } else { (i - 1) as u32 };
                    for j in 1..idx.len().saturating_sub(1) {
                        mesh.tris.push([resolve(idx[0]), resolve(idx[j]), resolve(idx[j + 1])]);
                    }
                }
                _ => {}
            }
        }
        mesh
    }

    /// Serialize as ASCII STL.
    pub fn to_ascii_stl(&self, name: &str) -> String {
        let mut s = format!("solid {name}\n");
        for t in &self.tris {
            let a = self.verts[t[0] as usize];
            let b = self.verts[t[1] as usize];
            let c = self.verts[t[2] as usize];
            let n = normalize(cross(sub(b, a), sub(c, a)));
            s.push_str(&format!("  facet normal {} {} {}\n", n[0], n[1], n[2]));
            s.push_str("    outer loop\n");
            for v in [a, b, c] {
                s.push_str(&format!("      vertex {} {} {}\n", v[0], v[1], v[2]));
            }
            s.push_str("    endloop\n  endfacet\n");
        }
        s.push_str(&format!("endsolid {name}\n"));
        s
    }
}

fn parse_binary_stl(bytes: &[u8]) -> Vec<[[f64; 3]; 3]> {
    if bytes.len() < 84 {
        return Vec::new();
    }
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let mut out = Vec::with_capacity(n);
    let f = |b: &[u8]| f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64;
    for i in 0..n {
        let o = 84 + i * 50;
        if o + 50 > bytes.len() {
            break;
        }
        let mut tri = [[0.0; 3]; 3];
        for (k, v) in tri.iter_mut().enumerate() {
            let vo = o + 12 + k * 12;
            *v = [f(&bytes[vo..]), f(&bytes[vo + 4..]), f(&bytes[vo + 8..])];
        }
        out.push(tri);
    }
    out
}

fn parse_ascii_stl(s: &str) -> Vec<[[f64; 3]; 3]> {
    let mut out = Vec::new();
    let mut cur: Vec<[f64; 3]> = Vec::new();
    for line in s.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("vertex ") {
            let nums: Vec<f64> = rest.split_whitespace().filter_map(|t| t.parse().ok()).collect();
            if nums.len() == 3 {
                cur.push([nums[0], nums[1], nums[2]]);
                if cur.len() == 3 {
                    out.push([cur[0], cur[1], cur[2]]);
                    cur.clear();
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod io_tests {
    use super::*;

    fn cube() -> Mesh {
        crate::cube([10.0, 8.0, 6.0], false)
    }

    #[test]
    fn stl_roundtrip() {
        let m = Mesh::from_stl(&cube().to_binary_stl());
        assert!((m.volume() - 480.0).abs() < 1e-6);
        assert_eq!(m.verts.len(), 8); // welded
    }

    #[test]
    fn off_obj_roundtrip() {
        let off = Mesh::from_off(&cube().to_off());
        assert!((off.volume() - 480.0).abs() < 1e-6, "off {}", off.volume());
        let obj = Mesh::from_obj(&cube().to_obj());
        assert!((obj.volume() - 480.0).abs() < 1e-6, "obj {}", obj.volume());
    }
}

pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn norm(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

pub(crate) fn normalize(a: [f64; 3]) -> [f64; 3] {
    let n = norm(a);
    if n == 0.0 {
        [0.0, 0.0, 0.0]
    } else {
        [a[0] / n, a[1] / n, a[2] / n]
    }
}
