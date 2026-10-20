//! Pele's `diagrams/block/arrows.ts`.

use std::fmt::Write as _;

use crate::{
    geom::{Axis, Point, Size},
    js,
    svg::shapes::poly,
};

/// How far a head reaches past each side of its shaft.
pub const OVERHANG: f64 = 7.0;
const MAX_DEPTH: f64 = 20.0;
const NUB: f64 = 12.0;

/// The directions a block arrow points in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Heads {
    pub right: bool,
    pub left: bool,
    pub up: bool,
    pub down: bool,
}

pub struct ArrowSize {
    pub size: Size,
    /// Shift of the label from the center, away from the heads.
    pub shift: Point,
}

fn bit(on: bool) -> f64 {
    if on { 1.0 } else { 0.0 }
}

impl Heads {
    /// Mermaid compares directions exactly, so `( up )` draws no arrow there.
    /// Here spaces are ignored.
    pub fn from_directions(directions: Option<&[String]>) -> Self {
        let mut heads = Heads::default();
        for direction in directions.unwrap_or_default() {
            match js::trim(direction) {
                "right" => heads.right = true,
                "left" => heads.left = true,
                "up" => heads.up = true,
                "down" => heads.down = true,
                "x" => (heads.right, heads.left) = (true, true),
                "y" => (heads.up, heads.down) = (true, true),
                _ => {}
            }
        }
        heads
    }

    /// The axis of an arrow that points along one axis only. Such an arrow
    /// has a shaft and heads wider than it; any other is a box with a small
    /// point on each side it names.
    pub fn axis(self) -> Option<Axis> {
        let across = self.right || self.left;
        let along = self.up || self.down;
        match (across, along) {
            (true, false) => Some(Axis::X),
            (false, true) => Some(Axis::Y),
            _ => None,
        }
    }

    /// The arrow around a label of `label`.
    pub fn size(self, label: Size) -> ArrowSize {
        let (tw, th) = (label.w, label.h);
        let across = bit(self.left) - bit(self.right);
        let along = bit(self.up) - bit(self.down);
        match self.axis() {
            Some(Axis::X) => {
                let h = (th + 8.0).max(14.0) + 2.0 * OVERHANG;
                let depth = (h / 2.0).min(MAX_DEPTH);
                let heads = if self.right && self.left { 2.0 } else { 1.0 };
                ArrowSize {
                    size: Size::new((tw + 12.0).max(16.0) + depth * heads, h),
                    shift: Point::new((across * depth) / 2.0, 0.0),
                }
            }
            Some(Axis::Y) => {
                let w = (tw + 16.0).max(14.0) + 2.0 * OVERHANG;
                let depth = (w / 2.0).min(MAX_DEPTH);
                let heads = if self.up && self.down { 2.0 } else { 1.0 };
                ArrowSize {
                    size: Size::new(w, (th + 8.0).max(16.0) + depth * heads),
                    shift: Point::new(0.0, (along * depth) / 2.0),
                }
            }
            None => ArrowSize {
                size: Size::new(
                    (tw + 16.0).max(24.0) + NUB * (bit(self.left) + bit(self.right)),
                    (th + 8.0).max(24.0) + NUB * (bit(self.up) + bit(self.down)),
                ),
                shift: Point::new((across * NUB) / 2.0, (along * NUB) / 2.0),
            },
        }
    }

    /// The arrow at `size` around the origin, appended to `out`.
    pub fn draw(self, out: &mut String, size: Size, attrs: &str) {
        let x = size.w / 2.0;
        let y = size.h / 2.0;
        let mut p: Vec<Point>;
        match self.axis() {
            Some(Axis::X) => {
                let depth = y.min(MAX_DEPTH);
                let s = y - OVERHANG;
                let x0 = if self.left { -x + depth } else { -x };
                let x1 = if self.right { x - depth } else { x };
                p = vec![Point::new(x0, -s), Point::new(x1, -s)];
                if self.right {
                    p.extend([Point::new(x1, -y), Point::new(x, 0.0), Point::new(x1, y)]);
                }
                p.extend([Point::new(x1, s), Point::new(x0, s)]);
                if self.left {
                    p.extend([Point::new(x0, y), Point::new(-x, 0.0), Point::new(x0, -y)]);
                }
            }
            Some(Axis::Y) => {
                let depth = x.min(MAX_DEPTH);
                let s = x - OVERHANG;
                let y0 = if self.up { -y + depth } else { -y };
                let y1 = if self.down { y - depth } else { y };
                p = vec![Point::new(s, y0), Point::new(s, y1)];
                if self.down {
                    p.extend([Point::new(x, y1), Point::new(0.0, y), Point::new(-x, y1)]);
                }
                p.extend([Point::new(-s, y1), Point::new(-s, y0)]);
                if self.up {
                    p.extend([Point::new(-x, y0), Point::new(0.0, -y), Point::new(x, y0)]);
                }
            }
            None => {
                let x0 = if self.left { -x + NUB } else { -x };
                let x1 = if self.right { x - NUB } else { x };
                let y0 = if self.up { -y + NUB } else { -y };
                let y1 = if self.down { y - NUB } else { y };
                let cx = (x0 + x1) / 2.0;
                let cy = (y0 + y1) / 2.0;
                let kx = 10f64.min((x1 - x0) / 2.0);
                let ky = 10f64.min((y1 - y0) / 2.0);
                p = vec![Point::new(x0, y0)];
                if self.up {
                    p.extend([
                        Point::new(cx - kx, y0),
                        Point::new(cx, -y),
                        Point::new(cx + kx, y0),
                    ]);
                }
                p.push(Point::new(x1, y0));
                if self.right {
                    p.extend([
                        Point::new(x1, cy - ky),
                        Point::new(x, cy),
                        Point::new(x1, cy + ky),
                    ]);
                }
                p.push(Point::new(x1, y1));
                if self.down {
                    p.extend([
                        Point::new(cx + kx, y1),
                        Point::new(cx, y),
                        Point::new(cx - kx, y1),
                    ]);
                }
                p.push(Point::new(x0, y1));
                if self.left {
                    p.extend([
                        Point::new(x0, cy + ky),
                        Point::new(-x, cy),
                        Point::new(x0, cy - ky),
                    ]);
                }
            }
        }
        let _ = write!(
            out,
            "{}",
            poly(&p, format_args!(" stroke-linejoin=\"round\"{attrs}"))
        );
    }
}
