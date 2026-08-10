// Shared helpers for reading the test corpus and reporting byte differences in
// a form a person can read.
//
// Each file in tests/ is compiled as its own binary, so helpers only some of
// them use look unused to the others.
#![allow(dead_code)]
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::fs;
use std::path::PathBuf;

use comma::document::{Dialect, Document};

pub fn read(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

/// The dialect each corpus file is written in.
pub fn dialect(name: &str) -> Dialect {
    match name {
        "semicolon.csv" => Dialect::semicolon(),
        "tabs.tsv" => Dialect::tab(),
        _ => Dialect::comma(),
    }
}

pub fn load(name: &str) -> Document {
    Document::from_bytes(&read(name), dialect(name))
        .unwrap_or_else(|error| panic!("could not load {name}: {error}"))
}

/// Renders bytes with line endings and other control characters made visible,
/// so a failed comparison says which byte moved instead of printing two blobs
/// that look identical.
pub fn visible(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &byte in bytes {
        match byte {
            b'\n' => out.push_str("\\n\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7E => out.push(byte as char),
            _ => out.push_str(&format!("\\x{byte:02x}")),
        }
    }
    out
}
