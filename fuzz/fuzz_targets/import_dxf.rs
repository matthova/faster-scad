#![no_main]
//! `import_dxf` must never panic on arbitrary bytes — `import("...dxf")` in the
//! playground can fetch user-supplied files.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = quito_geom::import_dxf(data);
});
