//! How wide a run of text is drawn, which every layout is built on.

use std::sync::OnceLock;

use super::metrics::{BOLD, REGULAR};

/// Emphasis bits on a run of text.
pub struct Style;

impl Style {
    pub const BOLD: u8 = 1;
    pub const ITALIC: u8 = 2;
    pub const MONO: u8 = 4;
}

/// What the page a drawing is shown in provides: its fonts' widths and its
/// icons. A diagram is laid out in the widths it answers, so it must answer
/// as the fonts the drawing is shown in.
pub trait Host {
    /// `text`'s advance at `size` px, under the [`Style`] bits in `style`.
    fn width(&self, text: &str, size: f64, style: u8) -> f64;

    /// The SVG markup icon `name` is drawn with, inside a 24×24 view box, or
    /// `None` for a name the host has no icon for. Called during layout and
    /// again during drawing for each use.
    fn icon(&self, name: &str) -> Option<String> {
        let _ = name;
        None
    }
}

/// Widths estimated from Arial's advances, where the host has no fonts to
/// ask: what Pele measures with outside a browser.
pub struct Metrics;

static REGULAR_TABLE: OnceLock<[u16; 95]> = OnceLock::new();
static BOLD_TABLE: OnceLock<[u16; 95]> = OnceLock::new();

fn unpack(packed: &str) -> [u16; 95] {
    let mut out = [0; 95];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u16::from_str_radix(&packed[i * 2..i * 2 + 2], 36).unwrap_or(0);
    }
    out
}

fn is_wide(c: u16) -> bool {
    (0x1100..=0x115f).contains(&c)
        || (0x2e80..=0xa4cf).contains(&c)
        || (0xac00..=0xd7a3).contains(&c)
        || (0xf900..=0xfaff).contains(&c)
        || (0xfe30..=0xfe4f).contains(&c)
        || (0xff00..=0xff60).contains(&c)
        || (0xffe0..=0xffe6).contains(&c)
        || (0xd800..=0xdbff).contains(&c)
}

fn is_zero_width(c: u16) -> bool {
    (0x300..=0x36f).contains(&c)
        || (0x200b..=0x200f).contains(&c)
        || (0xdc00..=0xdfff).contains(&c)
        || (0xfe00..=0xfe0f).contains(&c)
        || c == 0xfeff
}

impl Host for Metrics {
    fn width(&self, text: &str, size: f64, style: u8) -> f64 {
        let mono = style & Style::MONO != 0;
        let table = if style & Style::BOLD != 0 {
            BOLD_TABLE.get_or_init(|| unpack(BOLD))
        } else {
            REGULAR_TABLE.get_or_init(|| unpack(REGULAR))
        };
        let mut units = 0.0;
        for c in text.encode_utf16() {
            if c < 32 {
                continue;
            }
            units += if c < 127 {
                if mono {
                    600.0
                } else {
                    f64::from(table[(c - 32) as usize])
                }
            } else if is_zero_width(c) {
                continue;
            } else if is_wide(c) {
                if mono { 1200.0 } else { 1000.0 }
            } else if mono {
                600.0
            } else {
                580.0
            };
        }
        units * size / 1000.0
    }
}
