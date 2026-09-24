//! Kitty's Unicode placeholders: cells of `U+10EEEE` that each show one
//! cell's slice of a virtual placement.
//!
//! The cells are ordinary text to `alacritty_terminal`, so they scroll, clip
//! and reflow with the rest of the grid. What a cell shows is read off the
//! cell itself: the image id from its foreground color, the placement id from
//! its underline color, and its row and column in the placement — plus the
//! image id's high byte — from its combining marks.

use alacritty_terminal::{term::cell::Cell, vte::ansi::Color};

/// The placeholder character.
pub const PLACEHOLDER: char = '\u{10EEEE}';

/// Kitty's `rowcolumn-diacritics.txt`, in its order: the mark at index `n`
/// encodes `n`.
const DIACRITICS: [u32; 297] = [
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A, 0x034B, 0x034C,
    0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F, 0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592,
    0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1,
    0x05A8, 0x05A9, 0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6, 0x06D7, 0x06D8,
    0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2, 0x06E4, 0x06E7, 0x06E8, 0x06EB,
    0x06EC, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736, 0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074A, 0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3,
    0x0816, 0x0817, 0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951, 0x0953, 0x0954,
    0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD, 0x193A, 0x1A17, 0x1A75, 0x1A76,
    0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C, 0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F, 0x1B70, 0x1B71,
    0x1B72, 0x1B73, 0x1CD0, 0x1CD1, 0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4,
    0x1DC5, 0x1DC6, 0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1, 0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5,
    0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF, 0x1DE0, 0x1DE1,
    0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1, 0x20D4, 0x20D5, 0x20D6, 0x20D7,
    0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9, 0x20F0, 0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2,
    0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6, 0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE,
    0x2DEF, 0x2DF0, 0x2DF1, 0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1, 0xA8E0, 0xA8E1,
    0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9, 0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED,
    0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2, 0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1,
    0xFE20, 0xFE21, 0xFE22, 0xFE23, 0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186,
    0x1D187, 0x1D188, 0x1D189, 0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

/// The number a combining mark encodes, if it is one of [`DIACRITICS`].
fn diacritic(mark: char) -> Option<u32> {
    DIACRITICS
        .iter()
        .position(|&known| known == mark as u32)
        .map(|at| at as u32)
}

/// A color read as an id: an indexed color is its index, a direct one its 24
/// bits. A named color, the default included, carries none.
fn color_id(color: Color) -> Option<u32> {
    match color {
        Color::Indexed(index) => Some(index as u32),
        Color::Spec(rgb) => Some((rgb.r as u32) << 16 | (rgb.g as u32) << 8 | rgb.b as u32),
        Color::Named(_) => None,
    }
}

/// One placeholder cell, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub image: u32,
    /// Zero when the cell names none, which leaves the choice of virtual
    /// placement to the terminal.
    pub placement: u32,
    pub row: u32,
    pub col: u32,
}

/// What the placeholder to the left said, for the marks a cell leaves out.
#[derive(Debug, Clone, Copy)]
struct Left {
    image: u32,
    placement: u32,
    row: u32,
    col: u32,
    high: u32,
}

/// Decodes one grid row left to right. A fresh one per row: nothing is
/// inherited across a line break.
#[derive(Debug, Default)]
pub struct RowDecoder {
    left: Option<Left>,
}

impl RowDecoder {
    /// The next cell along. `None` for anything that is not a placeholder
    /// naming an image.
    pub fn cell(&mut self, cell: &Cell) -> Option<Slot> {
        let image = (cell.c == PLACEHOLDER).then(|| color_id(cell.fg)).flatten();
        let Some(image) = image else {
            self.left = None;
            return None;
        };
        let placement = cell.underline_color().and_then(color_id).unwrap_or(0);
        let marks: Vec<u32> = cell
            .zerowidth()
            .unwrap_or(&[])
            .iter()
            .filter_map(|&mark| diacritic(mark))
            .collect();
        // The cell to the left counts only if it carries the same colors.
        let left = self
            .left
            .filter(|left| left.image == image && left.placement == placement);
        let (row, col, high) = match marks[..] {
            [] => left.map_or((0, 0, 0), |left| (left.row, left.col + 1, left.high)),
            [row] => left
                .filter(|left| left.row == row)
                .map_or((row, 0, 0), |left| (row, left.col + 1, left.high)),
            [row, col] => {
                let high = left
                    .filter(|left| left.row == row && left.col + 1 == col)
                    .map_or(0, |left| left.high);
                (row, col, high)
            }
            [row, col, high, ..] => (row, col, high),
        };
        self.left = Some(Left {
            image,
            placement,
            row,
            col,
            high,
        });
        Some(Slot {
            image: high << 24 | image,
            placement,
            row,
            col,
        })
    }
}
