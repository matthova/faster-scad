#![no_main]
//! The parser must never panic on arbitrary input — only return a
//! `SyntaxError`. The playground parses untrusted public source on every
//! keystroke, so a panic here is a browser-worker crash (and a native crash in
//! the CLI/desktop).
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = std::str::from_utf8(data) {
        let _ = openrscad_syntax::parse(src);
    }
});
