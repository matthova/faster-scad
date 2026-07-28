#![no_main]
//! `Mesh::from_stl` must never panic on arbitrary bytes — binary and ASCII STL
//! both flow through here, and `import()` in the playground can fetch
//! user-supplied files.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = quito_geom::Mesh::from_stl(data);
});
