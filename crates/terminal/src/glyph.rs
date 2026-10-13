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
    arms(ch).is_some() || matches!(ch, '═' | '║' | '\u{2580}'..='\u{259F}')
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

    if let Some([left, up, right, down]) = arms(ch) {
        // Each arm runs from the cell edge past the centre by half the
        // thickest arm, so arms meet in a square joint.
        let reach = light * f32::from(left.max(up).max(right).max(down)) / 2.0;
        let mut quads = Vec::with_capacity(4);
        if left > 0 {
            let t = light * f32::from(left);
            quads.push(fill(
                Bounds::new(
                    point(cell.left(), mid_y - t / 2.0),
                    size(mid_x + reach - cell.left(), t),
                ),
                color,
            ));
        }
        if right > 0 {
            let t = light * f32::from(right);
            quads.push(fill(
                Bounds::new(
                    point(mid_x - reach, mid_y - t / 2.0),
                    size(cell.right() - mid_x + reach, t),
                ),
                color,
            ));
        }
        if up > 0 {
            let t = light * f32::from(up);
            quads.push(fill(
                Bounds::new(
                    point(mid_x - t / 2.0, cell.top()),
                    size(t, mid_y + reach - cell.top()),
                ),
                color,
            ));
        }
        if down > 0 {
            let t = light * f32::from(down);
            quads.push(fill(
                Bounds::new(
                    point(mid_x - t / 2.0, mid_y - reach),
                    size(t, cell.bottom() - mid_y + reach),
                ),
                color,
            ));
        }
        return quads;
    }

    match ch {
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

/// `[left, up, right, down]` stroke weights of a single-line box-drawing
/// character: 0 none, 1 light, 2 heavy. `None` for every other character,
/// mixed light-and-heavy corners, dashes, rounded corners and diagonals
/// included.
fn arms(ch: char) -> Option<[u8; 4]> {
    Some(match ch {
        '─' => [1, 0, 1, 0],
        '━' => [2, 0, 2, 0],
        '│' => [0, 1, 0, 1],
        '┃' => [0, 2, 0, 2],
        '┌' => [0, 0, 1, 1],
        '┐' => [1, 0, 0, 1],
        '└' => [0, 1, 1, 0],
        '┘' => [1, 1, 0, 0],
        '├' => [0, 1, 1, 1],
        '┤' => [1, 1, 0, 1],
        '┬' => [1, 0, 1, 1],
        '┴' => [1, 1, 1, 0],
        '┼' => [1, 1, 1, 1],
        '┏' => [0, 0, 2, 2],
        '┓' => [2, 0, 0, 2],
        '┗' => [0, 2, 2, 0],
        '┛' => [2, 2, 0, 0],
        '┣' => [0, 2, 2, 2],
        '┫' => [2, 2, 0, 2],
        '┳' => [2, 0, 2, 2],
        '┻' => [2, 2, 2, 0],
        '╋' => [2, 2, 2, 2],
        '╴' => [1, 0, 0, 0],
        '╵' => [0, 1, 0, 0],
        '╶' => [0, 0, 1, 0],
        '╷' => [0, 0, 0, 1],
        '╸' => [2, 0, 0, 0],
        '╹' => [0, 2, 0, 0],
        '╺' => [0, 0, 2, 0],
        '╻' => [0, 0, 0, 2],
        '╼' => [1, 0, 2, 0],
        '╽' => [0, 1, 0, 2],
        '╾' => [2, 0, 1, 0],
        '╿' => [0, 2, 0, 1],
        _ => return None,
    })
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
