#![no_main]
//! `Mesh::from_3mf` must never panic on arbitrary bytes (3MF is a zip container
//! of XML — classic fuzz-finds-crashes territory).
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = quito_geom::Mesh::from_3mf(data);
});
