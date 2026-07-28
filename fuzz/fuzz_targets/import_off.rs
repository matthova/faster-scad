#![no_main]
//! `Mesh::from_off` takes text and must never panic on arbitrary input.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = quito_geom::Mesh::from_off(text);
    }
});
