#![no_main]
//! `import_svg` must never panic on arbitrary bytes — `import("...svg")` in the
//! playground can fetch user-supplied files.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = openrscad_geom::import_svg(data);
});
