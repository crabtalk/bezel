//! Box-drawing and block characters painted as cell geometry.
//!
//! A font draws these inside its own line box, so a column of them breaks at
//! every row edge. Painting them as rectangles that fill the cell joins one
//! row to the next whatever the font is.

use gpui::{Bounds, Hsla, PaintQuad, Pixels, fill, point, px, size};

/// Block elements are laid out on this grid: eight columns by twenty-four
/// lines per cell.
const SUB_COLUMNS: f32 = 8.0;
const SUB_LINES: f32 = 24.0;

/// Light strokes are one eighth of the cell width, at least one pixel.
const LIGHT_STROKE_DIVISOR: f32 = 8.0;

/// Whether [`fills`] paints this character. The text run skips these.
pub(crate) fn covers(ch: char) -> bool {
    matches!(
        ch,
        '─' | '━' | '│' | '┃' | '═' | '║' | '\u{2580}'..='\u{259F}'
    )
}

/// The quads that paint `ch` filling `cell` in `color`. Empty for characters
/// [`covers`] does not return true for.
pub(crate) fn fills(ch: char, cell: Bounds<Pixels>, color: Hsla) -> Vec<PaintQuad> {
    let light = px((f32::from(cell.size.width) / LIGHT_STROKE_DIVISOR).max(1.0));
    let mid_x = cell.left() + cell.size.width / 2.0;
    let mid_y = cell.top() + cell.size.height / 2.0;
    let hbar = |center: Pixels, thickness: Pixels| {
        fill(
            Bounds::new(
                point(cell.left(), center - thickness / 2.0),
                size(cell.size.width, thickness),
            ),
            color,
        )
    };
    let vbar = |center: Pixels, thickness: Pixels| {
        fill(
            Bounds::new(
                point(center - thickness / 2.0, cell.top()),
                size(thickness, cell.size.height),
            ),
            color,
        )
    };

    match ch {
        '─' => vec![hbar(mid_y, light)],
        '━' => vec![hbar(mid_y, light * 2.0)],
        '│' => vec![vbar(mid_x, light)],
        '┃' => vec![vbar(mid_x, light * 2.0)],
        '═' => vec![hbar(mid_y - light, light), hbar(mid_y + light, light)],
        '║' => vec![vbar(mid_x - light, light), vbar(mid_x + light, light)],
        '\u{2591}' => vec![fill(
            cell,
            Hsla {
                a: color.a * 0.25,
                ..color
            },
        )],
        '\u{2592}' => vec![fill(
            cell,
            Hsla {
                a: color.a * 0.5,
                ..color
            },
        )],
        '\u{2593}' => vec![fill(
            cell,
            Hsla {
                a: color.a * 0.75,
                ..color
            },
        )],
        '\u{2580}'..='\u{259F}' => block_subcells(ch)
            .into_iter()
            .map(|(column, line, columns, lines)| {
                let at = Bounds::new(
                    point(
                        cell.left() + cell.size.width * column / SUB_COLUMNS,
                        cell.top() + cell.size.height * line / SUB_LINES,
                    ),
                    size(
                        cell.size.width * columns / SUB_COLUMNS,
                        cell.size.height * lines / SUB_LINES,
                    ),
                );
                fill(at, color)
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `(column, line, columns, lines)` rectangles in subcell units for a block
/// element. Quadrants are the four subcell blocks their bits name, in order
/// upper-left, upper-right, lower-left, lower-right.
fn block_subcells(ch: char) -> Vec<(f32, f32, f32, f32)> {
    let codepoint = ch as u32;
    match codepoint {
        0x2580 => vec![(0.0, 0.0, 8.0, 12.0)],
        0x2581..=0x2588 => {
            let eighths = (codepoint - 0x2580) as f32;
            vec![(0.0, 24.0 - eighths * 3.0, 8.0, eighths * 3.0)]
        }
        0x2589..=0x258F => vec![(0.0, 0.0, (0x2590 - codepoint) as f32, 24.0)],
        0x2590 => vec![(4.0, 0.0, 4.0, 24.0)],
        0x2594 => vec![(0.0, 0.0, 8.0, 3.0)],
        0x2595 => vec![(7.0, 0.0, 1.0, 24.0)],
        0x2596..=0x259F => {
            const QUADRANTS: [u8; 10] = [
                0b0100, // ▖
                0b1000, // ▗
                0b0001, // ▘
                0b1101, // ▙
                0b1001, // ▚
                0b0111, // ▛
                0b1011, // ▜
                0b0010, // ▝
                0b0110, // ▞
                0b1110, // ▟
            ];
            let filled = QUADRANTS[(codepoint - 0x2596) as usize];
            (0..4)
                .filter(|bit| filled & (1 << bit) != 0)
                .map(|bit| ((bit % 2) as f32 * 4.0, (bit / 2) as f32 * 12.0, 4.0, 12.0))
                .collect()
        }
        _ => Vec::new(),
    }
}
