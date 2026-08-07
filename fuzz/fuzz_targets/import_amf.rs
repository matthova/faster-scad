#![no_main]
//! `Mesh::from_amf` must never panic on arbitrary bytes (AMF is XML).
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = openrscad_geom::Mesh::from_amf(data);
});
