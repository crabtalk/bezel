//! d3-path: the commands d3-shape's generators write, and `arc`'s join from
//! the current point.

use std::f64::consts::{PI, TAU};

/// d3-path's `epsilon`.
const EPSILON: f64 = 1e-6;

/// One path command, as d3-path writes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Move([f64; 2]),
    Line([f64; 2]),
    /// Two control points, then the end.
    Cubic([f64; 2], [f64; 2], [f64; 2]),
    /// SVG's `A` with both radii `radius` and no rotation: to `to`, the
    /// longer way round when `large`, clockwise with y down when `sweep`.
    Arc {
        radius: f64,
        large: bool,
        sweep: bool,
        to: [f64; 2],
    },
    Close,
}

/// d3-path's `Path`: its segments, and the points `arc` and `close_path`
/// read.
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub segments: Vec<Segment>,
    /// The current subpath's start.
    start: Option<[f64; 2]>,
    /// The current point.
    end: Option<[f64; 2]>,
}

impl Path {
    pub fn move_to(&mut self, to: [f64; 2]) {
        (self.start, self.end) = (Some(to), Some(to));
        self.segments.push(Segment::Move(to));
    }

    pub fn line_to(&mut self, to: [f64; 2]) {
        self.end = Some(to);
        self.segments.push(Segment::Line(to));
    }

    pub fn close_path(&mut self) {
        if self.end.is_some() {
            self.end = self.start;
            self.segments.push(Segment::Close);
        }
    }

    /// `arc(x, y, r, a0, a1, ccw)`: a line or move to the arc's start unless
    /// the path is already there, then the arc. Angles are radians clockwise
    /// from three o'clock with y down.
    pub fn arc(&mut self, [x, y]: [f64; 2], radius: f64, a0: f64, a1: f64, ccw: bool) {
        let (dx, dy) = (radius * a0.cos(), radius * a0.sin());
        let (x0, y0) = (x + dx, y + dy);
        let sweep = !ccw;
        let mut da = if ccw { a0 - a1 } else { a1 - a0 };
        // Neither command moves the current point, as in d3-path.
        match self.end {
            None => self.segments.push(Segment::Move([x0, y0])),
            Some([x1, y1]) if (x1 - x0).abs() > EPSILON || (y1 - y0).abs() > EPSILON => {
                self.segments.push(Segment::Line([x0, y0]))
            }
            Some(_) => {}
        }
        if radius == 0.0 || radius.is_nan() {
            return;
        }
        if da < 0.0 {
            da = da % TAU + TAU;
        }
        if da > TAU - EPSILON {
            self.segments.push(Segment::Arc {
                radius,
                large: true,
                sweep,
                to: [x - dx, y - dy],
            });
            self.segments.push(Segment::Arc {
                radius,
                large: true,
                sweep,
                to: [x0, y0],
            });
            self.end = Some([x0, y0]);
        } else if da > EPSILON {
            let to = [x + radius * a1.cos(), y + radius * a1.sin()];
            self.segments.push(Segment::Arc {
                radius,
                large: da >= PI,
                sweep,
                to,
            });
            self.end = Some(to);
        }
    }
}
