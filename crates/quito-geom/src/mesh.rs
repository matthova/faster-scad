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
            v += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
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

    /// The `3D/3dmodel.model` XML for a minimal (core-spec) 3MF package.
    pub fn to_3mf_model(&self) -> String {
        let mut s = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <model unit=\"millimeter\" xml:lang=\"en-US\" \
             xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n\
             \x20<resources>\n\
             \x20 <object id=\"1\" type=\"model\">\n\
             \x20  <mesh>\n\
             \x20   <vertices>\n",
        );
        for v in &self.verts {
            s.push_str(&format!(
                "    <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                v[0], v[1], v[2]
            ));
        }
        s.push_str("    </vertices>\n    <triangles>\n");
        for t in &self.tris {
            s.push_str(&format!(
                "    <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
                t[0], t[1], t[2]
            ));
        }
        s.push_str(
            "    </triangles>\n\
             \x20  </mesh>\n\
             \x20 </object>\n\
             \x20</resources>\n\
             \x20<build>\n\
             \x20 <item objectid=\"1\"/>\n\
             \x20</build>\n\
             </model>\n",
        );
        s
    }

    /// Serialize as AMF (Additive Manufacturing Format) — plain XML.
    pub fn to_amf(&self) -> String {
        let mut s = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <amf unit=\"millimeter\">\n\
             \x20<object id=\"0\">\n\
             \x20 <mesh>\n\
             \x20  <vertices>\n",
        );
        for v in &self.verts {
            s.push_str(&format!(
                "   <vertex><coordinates><x>{}</x><y>{}</y><z>{}</z></coordinates></vertex>\n",
                v[0], v[1], v[2]
            ));
        }
        s.push_str("   </vertices>\n   <volume>\n");
        for t in &self.tris {
            s.push_str(&format!(
                "    <triangle><v1>{}</v1><v2>{}</v2><v3>{}</v3></triangle>\n",
                t[0], t[1], t[2]
            ));
        }
        s.push_str("   </volume>\n  </mesh>\n </object>\n</amf>\n");
        s
    }

    /// Serialize as a 3MF package (a ZIP of the model XML plus the OPC
    /// content-types and relationships parts).
    pub fn to_3mf(&self) -> Vec<u8> {
        const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n\
            \x20<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n\
            \x20<Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\n\
            </Types>\n";
        const RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
            <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
            \x20<Relationship Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\" Target=\"/3D/3dmodel.model\" Id=\"rel0\"/>\n\
            </Relationships>\n";
        let mut zip = Zip::new();
        zip.add("[Content_Types].xml", CONTENT_TYPES.as_bytes());
        zip.add("_rels/.rels", RELS.as_bytes());
        zip.add("3D/3dmodel.model", self.to_3mf_model().as_bytes());
        zip.finish()
    }

    /// Parse a 3MF package: read the ZIP, inflate `3D/3dmodel.model`, and build
    /// an indexed mesh from its `<vertex>`/`<triangle>` elements. Returns an
    /// empty mesh if the archive or model can't be read.
    pub fn from_3mf(bytes: &[u8]) -> Mesh {
        let Some(model) = zip_read_entry(bytes, "3D/3dmodel.model") else {
            return Mesh::new();
        };
        let xml = String::from_utf8_lossy(&model);
        let mut verts: Vec<[f64; 3]> = Vec::new();
        let mut tris: Vec<[u32; 3]> = Vec::new();
        for tag in xml_tags(&xml, "vertex") {
            verts.push([
                xml_attr_f64(tag, "x").unwrap_or(0.0),
                xml_attr_f64(tag, "y").unwrap_or(0.0),
                xml_attr_f64(tag, "z").unwrap_or(0.0),
            ]);
        }
        for tag in xml_tags(&xml, "triangle") {
            if let (Some(a), Some(b), Some(c)) = (
                xml_attr_f64(tag, "v1"),
                xml_attr_f64(tag, "v2"),
                xml_attr_f64(tag, "v3"),
            ) {
                tris.push([a as u32, b as u32, c as u32]);
            }
        }
        Mesh { verts, tris }
    }

    /// Parse an AMF document (plain XML) into an indexed mesh.
    pub fn from_amf(bytes: &[u8]) -> Mesh {
        let xml = String::from_utf8_lossy(bytes);
        let mut verts: Vec<[f64; 3]> = Vec::new();
        // Each <vertex> holds a <coordinates> with <x>/<y>/<z> child elements.
        for v in split_elements(&xml, "vertex") {
            verts.push([
                xml_child_f64(v, "x").unwrap_or(0.0),
                xml_child_f64(v, "y").unwrap_or(0.0),
                xml_child_f64(v, "z").unwrap_or(0.0),
            ]);
        }
        let mut tris: Vec<[u32; 3]> = Vec::new();
        for t in split_elements(&xml, "triangle") {
            if let (Some(a), Some(b), Some(c)) = (
                xml_child_f64(t, "v1"),
                xml_child_f64(t, "v2"),
                xml_child_f64(t, "v3"),
            ) {
                tris.push([a as u32, b as u32, c as u32]);
            }
        }
        Mesh { verts, tris }
    }

    /// Parse a binary or ASCII STL into an indexed mesh (welding coincident
    /// vertices at 1e-6 precision).
    pub fn from_stl(bytes: &[u8]) -> Mesh {
        // ASCII if it starts with "solid" and contains "facet".
        let is_ascii =
            bytes.starts_with(b"solid") && bytes.windows(5).take(512).any(|w| w == b"facet");
        let raw_tris: Vec<[[f64; 3]; 3]> = if is_ascii {
            parse_ascii_stl(&String::from_utf8_lossy(bytes))
        } else {
            parse_binary_stl(bytes)
        };
        let mut mesh = Mesh::new();
        let mut map: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
        let key = |p: [f64; 3]| {
            [
                (p[0] * 1e6).round() as i64,
                (p[1] * 1e6).round() as i64,
                (p[2] * 1e6).round() as i64,
            ]
        };
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
        // Bound the loops by the tokens actually present, not by the (untrusted)
        // header counts — a bogus `nv`/`nf` of billions must not push billions
        // of default vertices/faces and OOM.
        for _ in 0..nv {
            let Some(x) = nums.next().map(|t| t.parse().unwrap_or(0.0)) else {
                break;
            };
            let y = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0.0);
            let z = nums.next().and_then(|t| t.parse().ok()).unwrap_or(0.0);
            mesh.verts.push([x, y, z]);
        }
        for _ in 0..nf {
            let Some(k) = nums.next().map(|t| t.parse::<usize>().unwrap_or(0)) else {
                break;
            };
            let idx: Vec<u32> = (0..k)
                .map_while(|_| nums.next())
                .filter_map(|t| t.parse().ok())
                .collect();
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
                    let resolve = |i: i64| {
                        if i < 0 {
                            (n + i) as u32
                        } else {
                            (i - 1) as u32
                        }
                    };
                    for j in 1..idx.len().saturating_sub(1) {
                        mesh.tris
                            .push([resolve(idx[0]), resolve(idx[j]), resolve(idx[j + 1])]);
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
    // Don't trust the header count: cap the pre-allocation to the triangles the
    // input could actually hold (50 bytes each after the 84-byte header), so a
    // bogus count can't request a multi-gigabyte allocation.
    let max_tris = bytes.len().saturating_sub(84) / 50;
    let mut out = Vec::with_capacity(n.min(max_tris));
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
            let nums: Vec<f64> = rest
                .split_whitespace()
                .filter_map(|t| t.parse().ok())
                .collect();
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

// ---------------------------------------------------------------------------
// Minimal store-only ZIP writer (for 3MF packaging). No compression, no deps,
// so it builds identically on native and wasm.
// ---------------------------------------------------------------------------

struct Zip {
    out: Vec<u8>,
    entries: Vec<ZipEntry>,
}

struct ZipEntry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

impl Zip {
    fn new() -> Self {
        Zip {
            out: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// Append a stored (uncompressed) file entry.
    fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32(data);
        let offset = self.out.len() as u32;
        // Local file header (signature 0x04034b50).
        self.out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        self.out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        self.out.extend_from_slice(&0u16.to_le_bytes()); // flags
        self.out.extend_from_slice(&0u16.to_le_bytes()); // method: store
        self.out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        self.out.extend_from_slice(&0u16.to_le_bytes()); // mod date
        self.out.extend_from_slice(&crc.to_le_bytes());
        self.out
            .extend_from_slice(&(data.len() as u32).to_le_bytes()); // comp size
        self.out
            .extend_from_slice(&(data.len() as u32).to_le_bytes()); // uncomp size
        self.out
            .extend_from_slice(&(name.len() as u16).to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        self.out.extend_from_slice(name.as_bytes());
        self.out.extend_from_slice(data);
        self.entries.push(ZipEntry {
            name: name.to_string(),
            crc,
            size: data.len() as u32,
            offset,
        });
    }

    /// Write the central directory + end record and return the ZIP bytes.
    fn finish(mut self) -> Vec<u8> {
        let cd_start = self.out.len() as u32;
        for e in &self.entries {
            self.out.extend_from_slice(&0x0201_4b50u32.to_le_bytes()); // central sig
            self.out.extend_from_slice(&20u16.to_le_bytes()); // version made by
            self.out.extend_from_slice(&20u16.to_le_bytes()); // version needed
            self.out.extend_from_slice(&0u16.to_le_bytes()); // flags
            self.out.extend_from_slice(&0u16.to_le_bytes()); // method: store
            self.out.extend_from_slice(&0u16.to_le_bytes()); // mod time
            self.out.extend_from_slice(&0u16.to_le_bytes()); // mod date
            self.out.extend_from_slice(&e.crc.to_le_bytes());
            self.out.extend_from_slice(&e.size.to_le_bytes()); // comp size
            self.out.extend_from_slice(&e.size.to_le_bytes()); // uncomp size
            self.out
                .extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            self.out.extend_from_slice(&0u16.to_le_bytes()); // extra len
            self.out.extend_from_slice(&0u16.to_le_bytes()); // comment len
            self.out.extend_from_slice(&0u16.to_le_bytes()); // disk number
            self.out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            self.out.extend_from_slice(&0u32.to_le_bytes()); // external attrs
            self.out.extend_from_slice(&e.offset.to_le_bytes()); // local header offset
            self.out.extend_from_slice(e.name.as_bytes());
        }
        let cd_size = self.out.len() as u32 - cd_start;
        // End of central directory record.
        self.out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes()); // disk number
        self.out.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
        self.out
            .extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        self.out
            .extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        self.out.extend_from_slice(&cd_size.to_le_bytes());
        self.out.extend_from_slice(&cd_start.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes()); // comment len
        self.out
    }
}

/// Standard CRC-32 (IEEE 802.3, polynomial 0xEDB88320), computed on the fly.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// ---------------------------------------------------------------------------
// ZIP reading (for 3MF import) + tiny XML helpers.
// ---------------------------------------------------------------------------

/// Read and decompress a single named entry from a ZIP archive via its central
/// directory. Handles stored (method 0) and deflate (method 8). Returns None on
/// any malformation or if the entry is absent.
fn zip_read_entry(zip: &[u8], name: &str) -> Option<Vec<u8>> {
    // Offsets below come straight from the (untrusted) archive, so every
    // addition is checked: a bogus 64-bit offset must yield None, not an
    // arithmetic-overflow panic (found by fuzzing).
    let rd_u16 = |o: usize| -> Option<usize> {
        zip.get(o..o.checked_add(2)?)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let rd_u32 = |o: usize| -> Option<usize> {
        zip.get(o..o.checked_add(4)?)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let rd_u64 = |o: usize| -> Option<usize> {
        zip.get(o..o.checked_add(8)?)
            .map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as usize)
    };
    // Find the End Of Central Directory record (scan backward for its signature).
    let eocd = (0..zip.len().saturating_sub(21))
        .rev()
        .find(|&i| zip[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]))?;
    let mut count = rd_u16(eocd + 10)?;
    let mut cd = rd_u32(eocd + 16)?;
    // ZIP64: sentinels in the classic EOCD mean the real values are in a ZIP64
    // EOCD record, pointed to by the ZIP64 locator just before the EOCD.
    // (OpenSCAD's 3MF writer uses ZIP64.)
    if cd == 0xFFFF_FFFF || count == 0xFFFF {
        let loc = eocd.checked_sub(20)?;
        if zip
            .get(loc..loc + 4)?
            .starts_with(&[0x50, 0x4b, 0x06, 0x07])
        {
            let z64 = rd_u64(loc + 8)?;
            if zip
                .get(z64..z64.checked_add(4)?)?
                .starts_with(&[0x50, 0x4b, 0x06, 0x06])
            {
                count = rd_u64(z64.checked_add(32)?)?;
                cd = rd_u64(z64.checked_add(48)?)?;
            }
        }
    }
    const Z64: usize = 0xFFFF_FFFF;
    for _ in 0..count {
        if !zip
            .get(cd..cd.checked_add(4)?)?
            .starts_with(&[0x50, 0x4b, 0x01, 0x02])
        {
            return None;
        }
        let method = rd_u16(cd.checked_add(10)?)?;
        let mut comp_size = rd_u32(cd.checked_add(20)?)?;
        let raw_ucomp = rd_u32(cd.checked_add(24)?)?;
        let name_len = rd_u16(cd.checked_add(28)?)?;
        let extra_len = rd_u16(cd.checked_add(30)?)?;
        let comment_len = rd_u16(cd.checked_add(32)?)?;
        let mut local_off = rd_u32(cd.checked_add(42)?)?;
        let name_start = cd.checked_add(46)?;
        let entry_name =
            std::str::from_utf8(zip.get(name_start..name_start.checked_add(name_len)?)?).ok()?;
        // ZIP64 extended-information extra field (id 0x0001): the sentinel
        // fields are stored here, in order (uncompressed, compressed, offset).
        if comp_size == Z64 || local_off == Z64 {
            let mut e = name_start.checked_add(name_len)?;
            let extra_end = e.checked_add(extra_len)?;
            while e.checked_add(4)? <= extra_end {
                let id = rd_u16(e)?;
                let sz = rd_u16(e.checked_add(2)?)?;
                if id == 0x0001 {
                    let mut p = e.checked_add(4)?;
                    if raw_ucomp == Z64 {
                        p = p.checked_add(8)?; // skip uncompressed size
                    }
                    if comp_size == Z64 {
                        comp_size = rd_u64(p)?;
                        p = p.checked_add(8)?;
                    }
                    if local_off == Z64 {
                        local_off = rd_u64(p)?;
                    }
                    break;
                }
                e = e.checked_add(4)?.checked_add(sz)?;
            }
        }
        if entry_name == name {
            // Jump to the local header to find where the data begins.
            if !zip
                .get(local_off..local_off.checked_add(4)?)?
                .starts_with(&[0x50, 0x4b, 0x03, 0x04])
            {
                return None;
            }
            let lh_name = rd_u16(local_off.checked_add(26)?)?;
            let lh_extra = rd_u16(local_off.checked_add(28)?)?;
            let data_start = local_off
                .checked_add(30)?
                .checked_add(lh_name)?
                .checked_add(lh_extra)?;
            let data = zip.get(data_start..data_start.checked_add(comp_size)?)?;
            return match method {
                0 => Some(data.to_vec()),
                8 => miniz_oxide::inflate::decompress_to_vec(data).ok(),
                _ => None,
            };
        }
        cd = cd
            .checked_add(46)?
            .checked_add(name_len)?
            .checked_add(extra_len)?
            .checked_add(comment_len)?;
    }
    None
}

/// True if `xml[at..]` begins a tag named `name` at a word boundary.
fn tag_boundary(xml: &str, at: usize, name_len: usize) -> bool {
    matches!(
        xml.as_bytes().get(at + name_len),
        Some(&b) if b.is_ascii_whitespace() || b == b'>' || b == b'/'
    )
}

/// The attribute region (between `<name` and `>`) of each `<name ...>` tag.
fn xml_tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(rel) = xml[i..].find(&open) {
        let tstart = i + rel;
        if !tag_boundary(xml, tstart + 1, name.len()) {
            i = tstart + open.len();
            continue;
        }
        let attr_start = tstart + open.len();
        match xml[attr_start..].find('>') {
            Some(end) => {
                out.push(&xml[attr_start..attr_start + end]);
                i = attr_start + end + 1;
            }
            None => break,
        }
    }
    out
}

/// Parse a numeric XML attribute (`attr="..."` / `attr='...'`) from a tag region.
fn xml_attr_f64(tag: &str, attr: &str) -> Option<f64> {
    let mut from = 0;
    while let Some(rel) = tag[from..].find(attr) {
        let i = from + rel;
        let before_ok = i == 0 || tag.as_bytes()[i - 1].is_ascii_whitespace();
        let rest = tag[i + attr.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let rest = rest[1..].trim_start();
            let q = rest.chars().next()?;
            if q == '"' || q == '\'' {
                let end = rest[1..].find(q)?;
                return rest[1..1 + end].trim().parse().ok();
            }
        }
        from = i + attr.len();
    }
    None
}

/// The inner content of each `<name>...</name>` element (non-self-closing).
fn split_elements<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(rel) = xml[i..].find(&open) {
        let tstart = i + rel;
        if !tag_boundary(xml, tstart + 1, name.len()) {
            i = tstart + open.len();
            continue;
        }
        let Some(gt) = xml[tstart..].find('>') else {
            break;
        };
        let content_start = tstart + gt + 1;
        if xml.as_bytes().get(tstart + gt - 1) == Some(&b'/') {
            i = content_start; // self-closing, no content
            continue;
        }
        match xml[content_start..].find(&close) {
            Some(crel) => {
                out.push(&xml[content_start..content_start + crel]);
                i = content_start + crel + close.len();
            }
            None => break,
        }
    }
    out
}

/// Parse the numeric text of a `<name>value</name>` child element.
fn xml_child_f64(block: &str, name: &str) -> Option<f64> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let s = block.find(&open)? + open.len();
    let e = block[s..].find(&close)? + s;
    block[s..e].trim().parse().ok()
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

    #[test]
    fn amf_has_all_geometry() {
        let m = cube();
        let amf = m.to_amf();
        assert_eq!(amf.matches("<vertex>").count(), m.verts.len());
        assert_eq!(amf.matches("<triangle>").count(), m.tris.len());
        assert!(amf.contains("<amf unit=\"millimeter\">"));
    }

    #[test]
    fn threemf_is_a_valid_zip_with_all_parts() {
        let m = cube();
        let zip = m.to_3mf();
        // Local-file-header signature at the start, EOCD signature at the end.
        assert_eq!(&zip[0..4], b"PK\x03\x04");
        assert!(
            zip.windows(4).any(|w| w == b"PK\x05\x06"),
            "no end-of-central-directory"
        );
        // All three OPC parts present as stored entries (their names appear raw).
        for part in ["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"] {
            assert!(
                zip.windows(part.len()).any(|w| w == part.as_bytes()),
                "missing part {part}"
            );
        }
        // The model XML carries every vertex/triangle.
        let model = m.to_3mf_model();
        assert_eq!(model.matches("<vertex ").count(), m.verts.len());
        assert_eq!(model.matches("<triangle ").count(), m.tris.len());
    }

    /// CRC-32 of "123456789" is the well-known check value 0xCBF43926.
    #[test]
    fn crc32_check_value() {
        assert_eq!(super::crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn threemf_roundtrip() {
        // to_3mf packages a store-only ZIP; from_3mf reads it back exactly.
        let m = cube();
        let back = Mesh::from_3mf(&m.to_3mf());
        assert_eq!(back.verts.len(), m.verts.len());
        assert_eq!(back.tris.len(), m.tris.len());
        assert!(
            (back.volume() - 480.0).abs() < 1e-6,
            "vol {}",
            back.volume()
        );
    }

    #[test]
    fn amf_roundtrip() {
        let m = cube();
        let back = Mesh::from_amf(m.to_amf().as_bytes());
        assert_eq!(back.verts.len(), m.verts.len());
        assert!(
            (back.volume() - 480.0).abs() < 1e-6,
            "vol {}",
            back.volume()
        );
    }
}
