//! Helpers the graphics tests share.
#![allow(dead_code)]

use terminal::{
    emulator::Emulator,
    scanner::{Scanner, Segment},
};

/// `ESC _ G` … `ESC \` around a body.
pub fn apc(body: &str) -> Vec<u8> {
    format!("\x1b_G{body}\x1b\\").into_bytes()
}

/// Base64 of `bytes`, standard alphabet with padding — what a client sends.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut bits = 0u32;
        for (ix, &byte) in chunk.iter().enumerate() {
            bits |= (byte as u32) << (16 - 8 * ix);
        }
        for ix in 0..4 {
            if ix <= chunk.len() {
                out.push(ALPHABET[(bits >> (18 - 6 * ix)) as usize & 0x3f] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// One red pixel, RGBA.
pub fn pixel() -> Vec<u8> {
    vec![0xff, 0x00, 0x00, 0xff]
}

/// Text runs a scanner passed through, joined — what the ANSI parser would
/// have seen.
pub fn passed(scanner: &mut Scanner, bytes: &[u8]) -> Vec<u8> {
    scanner
        .feed(bytes)
        .iter()
        .filter_map(|segment| match segment {
            Segment::Text(text) => Some(*text),
            _ => None,
        })
        .fold(Vec::new(), |mut out, text| {
            out.extend_from_slice(text);
            out
        })
}

/// A terminal whose cells are 10x20 pixels, which is what turns an image's
/// pixel size into the rows it covers.
pub fn placed_emulator(cols: u16, rows: u16) -> Emulator {
    let mut emulator = Emulator::new(cols, rows);
    emulator.set_cell_size(10.0, 20.0);
    emulator
}
