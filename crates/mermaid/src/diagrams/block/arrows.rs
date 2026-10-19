//! Pele's `diagrams/block/arrows.ts`.

use std::fmt::Write as _;

use crate::{js, svg::builder::push_num};

pub const RIGHT: u32 = 1;
pub const LEFT: u32 = 2;
pub const UP: u32 = 4;
pub const DOWN: u32 = 8;
/// How far a head reaches past each side of its shaft.
pub const OVERHANG: f64 = 7.0;
const MAX_DEPTH: f64 = 20.0;
const NUB: f64 = 12.0;

pub struct ArrowSize {
    pub w: f64,
    pub h: f64,
    /// Shift of the label from the center, away from the heads.
    pub dx: f64,
    pub dy: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    None,
}

/// Mermaid compares directions exactly, so `( up )` draws no arrow there.
/// Here spaces are ignored.
pub fn arrow_mask(directions: Option<&[String]>) -> u32 {
    let mut mask = 0;
    for direction in directions.unwrap_or_default() {
        match js::trim(direction) {
            "right" => mask |= RIGHT,
            "left" => mask |= LEFT,
            "up" => mask |= UP,
            "down" => mask |= DOWN,
            "x" => mask |= RIGHT | LEFT,
            "y" => mask |= UP | DOWN,
            _ => {}
        }
    }
    mask
}

fn bit(mask: u32, flag: u32) -> f64 {
    if mask & flag != 0 { 1.0 } else { 0.0 }
}

/// An arrow along one axis has a shaft and heads wider than it. One that
/// points along both axes is a box with a small point on each side it names.
pub fn arrow_axis(mask: u32) -> Axis {
    if mask != 0 && mask & (UP | DOWN) == 0 {
        return Axis::X;
    }
    if mask != 0 && mask & (RIGHT | LEFT) == 0 {
        return Axis::Y;
    }
    Axis::None
}

pub fn arrow_size(mask: u32, tw: f64, th: f64) -> ArrowSize {
    let axis = arrow_axis(mask);
    let across = bit(mask, LEFT) - bit(mask, RIGHT);
    let along = bit(mask, UP) - bit(mask, DOWN);
    match axis {
        Axis::X => {
            let h = (th + 8.0).max(14.0) + 2.0 * OVERHANG;
            let depth = (h / 2.0).min(MAX_DEPTH);
            let heads = if mask == RIGHT | LEFT { 2.0 } else { 1.0 };
            ArrowSize {
                w: (tw + 12.0).max(16.0) + depth * heads,
                h,
                dx: (across * depth) / 2.0,
                dy: 0.0,
            }
        }
        Axis::Y => {
            let w = (tw + 16.0).max(14.0) + 2.0 * OVERHANG;
            let depth = (w / 2.0).min(MAX_DEPTH);
            let heads = if mask == UP | DOWN { 2.0 } else { 1.0 };
            ArrowSize {
                w,
                h: (th + 8.0).max(16.0) + depth * heads,
                dx: 0.0,
                dy: (along * depth) / 2.0,
            }
        }
        Axis::None => ArrowSize {
            w: (tw + 16.0).max(24.0) + NUB * (bit(mask, LEFT) + bit(mask, RIGHT)),
            h: (th + 8.0).max(24.0) + NUB * (bit(mask, UP) + bit(mask, DOWN)),
            dx: (across * NUB) / 2.0,
            dy: (along * NUB) / 2.0,
        },
    }
}

pub fn draw_arrow(out: &mut String, mask: u32, w: f64, h: f64, attrs: &str) {
    let axis = arrow_axis(mask);
    let x = w / 2.0;
    let y = h / 2.0;
    let has = |flag: u32| mask & flag != 0;
    let mut p: Vec<f64>;
    match axis {
        Axis::X => {
            let depth = y.min(MAX_DEPTH);
            let s = y - OVERHANG;
            let x0 = if has(LEFT) { -x + depth } else { -x };
            let x1 = if has(RIGHT) { x - depth } else { x };
            p = vec![x0, -s, x1, -s];
            if has(RIGHT) {
                p.extend([x1, -y, x, 0.0, x1, y]);
            }
            p.extend([x1, s, x0, s]);
            if has(LEFT) {
                p.extend([x0, y, -x, 0.0, x0, -y]);
            }
        }
        Axis::Y => {
            let depth = x.min(MAX_DEPTH);
            let s = x - OVERHANG;
            let y0 = if has(UP) { -y + depth } else { -y };
            let y1 = if has(DOWN) { y - depth } else { y };
            p = vec![s, y0, s, y1];
            if has(DOWN) {
                p.extend([x, y1, 0.0, y, -x, y1]);
            }
            p.extend([-s, y1, -s, y0]);
            if has(UP) {
                p.extend([-x, y0, 0.0, -y, x, y0]);
            }
        }
        Axis::None => {
            let x0 = if has(LEFT) { -x + NUB } else { -x };
            let x1 = if has(RIGHT) { x - NUB } else { x };
            let y0 = if has(UP) { -y + NUB } else { -y };
            let y1 = if has(DOWN) { y - NUB } else { y };
            let cx = (x0 + x1) / 2.0;
            let cy = (y0 + y1) / 2.0;
            let kx = 10f64.min((x1 - x0) / 2.0);
            let ky = 10f64.min((y1 - y0) / 2.0);
            p = vec![x0, y0];
            if has(UP) {
                p.extend([cx - kx, y0, cx, -y, cx + kx, y0]);
            }
            p.extend([x1, y0]);
            if has(RIGHT) {
                p.extend([x1, cy - ky, x, cy, x1, cy + ky]);
            }
            p.extend([x1, y1]);
            if has(DOWN) {
                p.extend([cx + kx, y1, cx, y, cx - kx, y1]);
            }
            p.extend([x0, y1]);
            if has(LEFT) {
                p.extend([x0, cy + ky, -x, cy, x0, cy - ky]);
            }
        }
    }
    out.push_str("<polygon points=\"");
    for (i, pair) in p.chunks(2).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        push_num(out, pair[0]);
        out.push(',');
        push_num(out, pair[1]);
    }
    let _ = write!(out, "\" stroke-linejoin=\"round\"{attrs}/>");
}
