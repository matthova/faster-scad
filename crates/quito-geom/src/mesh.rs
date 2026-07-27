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
