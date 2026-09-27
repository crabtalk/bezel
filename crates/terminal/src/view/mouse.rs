//! Pointer positions as cells, and pointer events as PTY bytes.

use super::*;

/// Minimum pointer travel before a press turns into a selection.
///
/// Without it, the click that focuses the host panel starts a one-cell
/// selection if the hand moves a pixel — and once anything copies on selection
/// change, that silently clobbers the clipboard. Matches the threshold zed uses
/// for the same reason (`SELECTION_DRAG_THRESHOLD`), which is gpui's own `div`
/// drag threshold.
pub const SELECTION_DRAG_THRESHOLD: f32 = 2.0;

/// Which cell a pointer landed on, and which edge of it a selection anchors to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellHit {
    pub row: usize,
    pub col: usize,
    /// Selections anchor to a cell *edge*, not a cell: pressing on the left
    /// half of a glyph includes it, the right half excludes it.
    pub side: Side,
}

/// Map a position *relative to the grid's top-left glyph* onto a cell.
///
/// Positions outside the grid clamp to the nearest cell rather than returning
/// `None`, because that is what a drag needs: the pointer routinely leaves the
/// panel mid-gesture, and the selection should extend to the edge it left
/// through instead of freezing at the last sample taken inside.
///
/// Overshoot also *forces the side*, which clamping alone does not give you.
/// Dragging past the bottom should take the last line whole, even when the
/// pointer drifted left of where it started — deriving the side from x there
/// would stop the selection mid-row. Same rule going up, mirrored. This is the
/// behaviour alacritty and zed's `grid_point_and_side` both implement.
pub fn cell_at(x: f32, y: f32, cell_w: f32, line_h: f32, cols: usize, rows: usize) -> CellHit {
    // Degenerate metrics (a zero-size grid, or a font probe that returned NaN)
    // would otherwise divide into garbage cell indices.
    let usable = |v: f32| v.is_finite() && v > 0.0;
    if cols == 0 || rows == 0 || !usable(cell_w) || !usable(line_h) {
        return CellHit {
            row: 0,
            col: 0,
            side: Side::Left,
        };
    }
    let x = if x.is_finite() { x } else { 0.0 };
    let y = if y.is_finite() { y } else { 0.0 };
    let last_col = cols - 1;
    let last_row = rows - 1;

    let raw_col = (x / cell_w).floor();
    let mut side = if x.max(0.0) % cell_w > cell_w / 2.0 {
        Side::Right
    } else {
        Side::Left
    };
    let col = if raw_col > last_col as f32 {
        side = Side::Right;
        last_col
    } else {
        raw_col.max(0.0) as usize
    };

    let raw_row = (y / line_h).floor();
    let row = if raw_row > last_row as f32 {
        side = Side::Right;
        last_row
    } else if raw_row < 0.0 {
        side = Side::Left;
        0
    } else {
        raw_row as usize
    };

    CellHit { row, col, side }
}

/// A pointer button, in the order the protocol numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// What the pointer did, in the vocabulary a report is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Press(MouseButton),
    Release(MouseButton),
    /// The button held through the move, or `None` for a bare move.
    Motion(Option<MouseButton>),
    /// A wheel gesture, already resolved to whole lines.
    Scroll {
        up: bool,
        lines: usize,
    },
}

/// Encode a pointer event as PTY bytes. `None` means the event is the user's —
/// the host should run its own selection or scrollback instead.
///
/// `row` and `col` are viewport cells, which is what [`cell_at`] answers.
/// Holding shift always answers `None`: it is how a user reaches the selection
/// under a program that has taken the pointer, and it costs that program the
/// shift modifier on every report.
pub fn mouse_bytes(
    action: MouseAction,
    row: usize,
    col: usize,
    mods: &Modifiers,
    mode: MouseMode,
) -> Option<Vec<u8>> {
    if mods.shift {
        return None;
    }
    match action {
        MouseAction::Scroll { up, lines } if mode.tracking == MouseTracking::Off => {
            // The alternate screen has no scrollback to move through, so a
            // wheel there drives whatever the arrow keys drive.
            let arrows = (mode.alternate_scroll && mode.alt_screen).then(|| {
                let one: &[u8] = match (up, mode.app_cursor) {
                    (true, false) => b"\x1b[A",
                    (true, true) => b"\x1bOA",
                    (false, false) => b"\x1b[B",
                    (false, true) => b"\x1bOB",
                };
                one.repeat(lines)
            })?;
            (!arrows.is_empty()).then_some(arrows)
        }
        MouseAction::Scroll { up, lines } => {
            let mut out = Vec::new();
            for _ in 0..lines {
                out.extend(report(
                    MouseAction::Scroll { up, lines: 1 },
                    row,
                    col,
                    mods,
                    mode,
                )?);
            }
            (!out.is_empty()).then_some(out)
        }
        MouseAction::Motion(held) => {
            let wanted = match mode.tracking {
                MouseTracking::Motion => true,
                MouseTracking::Drag => held.is_some(),
                MouseTracking::Off | MouseTracking::Click => false,
            };
            wanted.then(|| report(action, row, col, mods, mode))?
        }
        _ if mode.tracking == MouseTracking::Off => None,
        _ => report(action, row, col, mods, mode),
    }
}

/// One report. `None` for a cell the chosen encoding has no room to name.
pub(super) fn report(
    action: MouseAction,
    row: usize,
    col: usize,
    mods: &Modifiers,
    mode: MouseMode,
) -> Option<Vec<u8>> {
    let number = |button| match button {
        MouseButton::Left => 0u32,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    // Alt and control travel on every report; shift never does, having been
    // spent on the selection override.
    let modifiers = u32::from(mods.alt) * 8 + u32::from(mods.control) * 16;
    let (button, released) = match action {
        MouseAction::Press(button) => (number(button), false),
        MouseAction::Release(button) => (number(button), true),
        // 3 is "no button", which is what a bare move reports moving.
        MouseAction::Motion(held) => (held.map_or(3, number) + 32, false),
        MouseAction::Scroll { up, .. } => (if up { 64 } else { 65 }, false),
    };

    if mode.sgr {
        let final_byte = if released { 'm' } else { 'M' };
        return Some(
            format!(
                "\x1b[<{};{};{}{final_byte}",
                button + modifiers,
                col + 1,
                row + 1
            )
            .into_bytes(),
        );
    }

    // The original encoding numbers every release 3: there is no room in it to
    // say which button came up.
    let button = if released { 3 } else { button };
    let mut out = b"\x1b[M".to_vec();
    out.push(32 + (button + modifiers) as u8);
    for coordinate in [col, row] {
        let value = coordinate as u32 + 33;
        if mode.utf8 {
            // Two UTF-8 bytes, which is as far as `1005` reaches.
            if value > 0x7ff {
                return None;
            }
            let mut buffer = [0u8; 4];
            out.extend_from_slice(char::from_u32(value)?.encode_utf8(&mut buffer).as_bytes());
        } else {
            if value > 0xff {
                return None;
            }
            out.push(value as u8);
        }
    }
    Some(out)
}
