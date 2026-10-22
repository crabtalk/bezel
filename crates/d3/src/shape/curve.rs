//! d3-shape's curves: the path commands `d3.line` and `d3.area` write through
//! a run of points with each of d3's curves.

use std::mem;

use crate::{format::number, shape::path::Segment};

/// A d3 curve. The parameter is d3's: `Bundle`'s beta, a cardinal curve's
/// tension, a Catmull-Rom curve's alpha. A Catmull-Rom curve with alpha 0
/// is the cardinal curve with tension 0, and a bundle with beta 1 is
/// `Basis`, as in d3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    LinearClosed,
    Step,
    StepBefore,
    StepAfter,
    Basis,
    BasisOpen,
    BasisClosed,
    Bundle(f64),
    Cardinal(f64),
    CardinalOpen(f64),
    CardinalClosed(f64),
    CatmullRom(f64),
    CatmullRomOpen(f64),
    CatmullRomClosed(f64),
    MonotoneX,
    MonotoneY,
    Natural,
}

/// d3-shape's `epsilon`.
const EPSILON: f64 = 1e-12;

impl Curve {
    /// What `d3.line().curve(…)` writes through `points`, which have no gap.
    pub fn line(self, points: &[[f64; 2]]) -> Vec<Segment> {
        let mut stream = Stream::new(self);
        if points.is_empty() {
            return Vec::new();
        }
        stream.line_start();
        for &[x, y] in points {
            stream.point(stream.curve, x, y);
        }
        stream.line_end(stream.curve);
        stream.path
    }

    /// What `d3.area().curve(…)` writes for an area whose `top[i]` stands
    /// over `base[i]`: along the top, then back along the base.
    pub fn area(self, top: &[[f64; 2]], base: &[[f64; 2]]) -> Vec<Segment> {
        let mut stream = Stream::new(self);
        if top.is_empty() {
            return Vec::new();
        }
        stream.area_start();
        stream.line_start();
        for &[x, y] in top {
            stream.point(stream.curve, x, y);
        }
        stream.line_end(stream.curve);
        stream.line_start();
        for &[x, y] in base.iter().rev() {
            stream.point(stream.curve, x, y);
        }
        stream.line_end(stream.curve);
        stream.area_end();
        stream.path
    }

    fn closed(self) -> bool {
        matches!(
            self,
            Curve::LinearClosed
                | Curve::BasisClosed
                | Curve::CardinalClosed(_)
                | Curve::CatmullRomClosed(_)
        )
    }
}

/// A d3 curve's state as points stream through it. `line` is d3's `_line`:
/// `None` drawing a line, `Some(false)` an area's first side and
/// `Some(true)` its second.
struct Stream {
    curve: Curve,
    path: Vec<Segment>,
    line: Option<bool>,
    point: u8,
    x: [f64; 6],
    y: [f64; 6],
    /// `_l01_a`, `_l12_a`, `_l23_a`.
    l_a: [f64; 3],
    /// `_l01_2a`, `_l12_2a`, `_l23_2a`.
    l_2a: [f64; 3],
    /// Monotone's `_t0`.
    t0: f64,
    /// Step's `_t`.
    t: f64,
    /// What natural and bundle hold until the line ends.
    xs: Vec<f64>,
    ys: Vec<f64>,
}

impl Stream {
    fn new(curve: Curve) -> Self {
        let curve = match curve {
            Curve::Bundle(1.0) => Curve::Basis,
            Curve::CatmullRom(0.0) => Curve::Cardinal(0.0),
            Curve::CatmullRomOpen(0.0) => Curve::CardinalOpen(0.0),
            Curve::CatmullRomClosed(0.0) => Curve::CardinalClosed(0.0),
            curve => curve,
        };
        Self {
            curve,
            path: Vec::new(),
            line: None,
            point: 0,
            x: [f64::NAN; 6],
            y: [f64::NAN; 6],
            l_a: [0.0; 3],
            l_2a: [0.0; 3],
            t0: f64::NAN,
            t: match curve {
                Curve::StepBefore => 0.0,
                Curve::StepAfter => 1.0,
                _ => 0.5,
            },
            xs: Vec::new(),
            ys: Vec::new(),
        }
    }

    fn emit(&mut self, segment: Segment) {
        // MonotoneY runs MonotoneX over points with x and y swapped.
        let segment = match self.curve {
            Curve::MonotoneY => {
                let flip = |[x, y]: [f64; 2]| [y, x];
                match segment {
                    Segment::Move(p) => Segment::Move(flip(p)),
                    Segment::Line(p) => Segment::Line(flip(p)),
                    Segment::Cubic(a, b, p) => Segment::Cubic(flip(a), flip(b), flip(p)),
                    Segment::Arc {
                        radius,
                        large,
                        sweep,
                        to,
                    } => Segment::Arc {
                        radius,
                        large,
                        sweep: !sweep,
                        to: flip(to),
                    },
                    Segment::Close => Segment::Close,
                }
            }
            _ => segment,
        };
        self.path.push(segment);
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.emit(Segment::Move([x, y]));
    }

    fn line_to(&mut self, x: f64, y: f64) {
        self.emit(Segment::Line([x, y]));
    }

    fn cubic(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) {
        self.emit(Segment::Cubic([x1, y1], [x2, y2], [x, y]));
    }

    /// d3-path closes nothing before its first move.
    fn close(&mut self) {
        if self.path.iter().any(|s| matches!(s, Segment::Move(_))) {
            self.emit(Segment::Close);
        }
    }

    /// d3's `this._line ? lineTo : moveTo`.
    fn begin(&mut self, x: f64, y: f64) {
        match self.line == Some(true) {
            true => self.line_to(x, y),
            false => self.move_to(x, y),
        }
    }

    /// Closes the path where d3 does, `single` being d3's test for a run
    /// too short to draw, and turns to an area's other side.
    fn finish(&mut self, single: bool) {
        if self.line == Some(true) || (self.line != Some(false) && single) {
            self.close();
        }
        self.line = self.line.map(|line| !line);
    }

    fn area_start(&mut self) {
        if !self.curve.closed() {
            self.line = Some(false);
        }
    }

    fn area_end(&mut self) {
        if !self.curve.closed() {
            self.line = None;
        }
    }

    fn line_start(&mut self) {
        self.x = [f64::NAN; 6];
        self.y = [f64::NAN; 6];
        self.l_a = [0.0; 3];
        self.l_2a = [0.0; 3];
        self.t0 = f64::NAN;
        self.point = 0;
        self.xs.clear();
        self.ys.clear();
    }

    /// Shifts the last three points along, `(x, y)` the newest.
    fn shift3(&mut self, x: f64, y: f64) {
        self.x[0] = self.x[1];
        self.x[1] = self.x[2];
        self.x[2] = x;
        self.y[0] = self.y[1];
        self.y[1] = self.y[2];
        self.y[2] = y;
    }

    /// Shifts the last two points along.
    fn shift2(&mut self, x: f64, y: f64) {
        self.x[0] = self.x[1];
        self.x[1] = x;
        self.y[0] = self.y[1];
        self.y[1] = y;
    }

    fn basis_point(&mut self, x: f64, y: f64) {
        let ([x0, x1, ..], [y0, y1, ..]) = (self.x, self.y);
        self.cubic(
            (2.0 * x0 + x1) / 3.0,
            (2.0 * y0 + y1) / 3.0,
            (x0 + 2.0 * x1) / 3.0,
            (y0 + 2.0 * y1) / 3.0,
            (x0 + 4.0 * x1 + x) / 6.0,
            (y0 + 4.0 * y1 + y) / 6.0,
        );
    }

    fn cardinal_point(&mut self, tension: f64, x: f64, y: f64) {
        let k = (1.0 - tension) / 6.0;
        let ([x0, x1, x2, ..], [y0, y1, y2, ..]) = (self.x, self.y);
        self.cubic(
            x1 + k * (x2 - x0),
            y1 + k * (y2 - y0),
            x2 + k * (x1 - x),
            y2 + k * (y1 - y),
            x2,
            y2,
        );
    }

    fn catmull_rom_point(&mut self, x: f64, y: f64) {
        let ([x0, mut x1, mut x2, ..], [y0, mut y1, mut y2, ..]) = (self.x, self.y);
        let ([l01_a, l12_a, l23_a], [l01_2a, l12_2a, l23_2a]) = (self.l_a, self.l_2a);
        if l01_a > EPSILON {
            let a = 2.0 * l01_2a + 3.0 * l01_a * l12_a + l12_2a;
            let n = 3.0 * l01_a * (l01_a + l12_a);
            x1 = (x1 * a - x0 * l12_2a + self.x[2] * l01_2a) / n;
            y1 = (y1 * a - y0 * l12_2a + self.y[2] * l01_2a) / n;
        }
        if l23_a > EPSILON {
            let b = 2.0 * l23_2a + 3.0 * l23_a * l12_a + l12_2a;
            let m = 3.0 * l23_a * (l23_a + l12_a);
            x2 = (x2 * b + self.x[1] * l23_2a - x * l12_2a) / m;
            y2 = (y2 * b + self.y[1] * l23_2a - y * l12_2a) / m;
        }
        self.cubic(x1, y1, x2, y2, self.x[2], self.y[2]);
    }

    /// Catmull-Rom's distance from the last point to `(x, y)`.
    fn catmull_rom_measure(&mut self, alpha: f64, x: f64, y: f64) {
        if self.point > 0 {
            let (x23, y23) = (self.x[2] - x, self.y[2] - y);
            self.l_2a[2] = number::pow(x23 * x23 + y23 * y23, alpha);
            self.l_a[2] = self.l_2a[2].sqrt();
        }
    }

    fn catmull_rom_shift(&mut self, x: f64, y: f64) {
        self.l_a = [self.l_a[1], self.l_a[2], self.l_a[2]];
        self.l_2a = [self.l_2a[1], self.l_2a[2], self.l_2a[2]];
        self.shift3(x, y);
    }

    /// Steffen's slope at the middle of the last two points and `(x2, y2)`.
    fn slope3(&self, x2: f64, y2: f64) -> f64 {
        let ([x0, x1, ..], [y0, y1, ..]) = (self.x, self.y);
        let (h0, h1) = (x1 - x0, x2 - x1);
        // JavaScript's `h0 || h1 < 0 && -0`.
        let or = |h: f64, other: f64| match h != 0.0 && !h.is_nan() {
            true => h,
            false if other < 0.0 => -0.0,
            false => 0.0,
        };
        let s0 = (y1 - y0) / or(h0, h1);
        let s1 = (y2 - y1) / or(h1, h0);
        let p = (s0 * h1 + s1 * h0) / (h0 + h1);
        let sign = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
        let slope =
            (sign(s0) + sign(s1)) * number::min(number::min(s0.abs(), s1.abs()), 0.5 * p.abs());
        match slope.is_nan() {
            true => 0.0,
            false => slope,
        }
    }

    /// A one-sided slope.
    fn slope2(&self, t: f64) -> f64 {
        let h = self.x[1] - self.x[0];
        match h != 0.0 && !h.is_nan() {
            true => (3.0 * (self.y[1] - self.y[0]) / h - t) / 2.0,
            false => t,
        }
    }

    fn monotone_point(&mut self, t0: f64, t1: f64) {
        let ([x0, x1, ..], [y0, y1, ..]) = (self.x, self.y);
        let dx = (x1 - x0) / 3.0;
        self.cubic(x0 + dx, y0 + dx * t0, x1 - dx, y1 - dx * t1, x1, y1);
    }

    fn point(&mut self, curve: Curve, x: f64, y: f64) {
        match curve {
            Curve::Linear => match self.point {
                0 => {
                    self.point = 1;
                    self.begin(x, y);
                }
                _ => {
                    self.point = 2;
                    self.line_to(x, y);
                }
            },
            Curve::LinearClosed => match self.point {
                0 => {
                    self.point = 1;
                    self.move_to(x, y);
                }
                _ => self.line_to(x, y),
            },
            Curve::Step | Curve::StepBefore | Curve::StepAfter => {
                match self.point {
                    0 => {
                        self.point = 1;
                        self.begin(x, y);
                    }
                    _ => {
                        self.point = 2;
                        let (px, py) = (self.x[0], self.y[0]);
                        if self.t <= 0.0 {
                            self.line_to(px, y);
                            self.line_to(x, y);
                        } else {
                            let x1 = px * (1.0 - self.t) + x * self.t;
                            self.line_to(x1, py);
                            self.line_to(x1, y);
                        }
                    }
                }
                self.x[0] = x;
                self.y[0] = y;
            }
            Curve::Basis => {
                match self.point {
                    0 => {
                        self.point = 1;
                        self.begin(x, y);
                    }
                    1 => self.point = 2,
                    2 => {
                        self.point = 3;
                        let ([x0, x1, ..], [y0, y1, ..]) = (self.x, self.y);
                        self.line_to((5.0 * x0 + x1) / 6.0, (5.0 * y0 + y1) / 6.0);
                        self.basis_point(x, y);
                    }
                    _ => self.basis_point(x, y),
                }
                self.shift2(x, y);
            }
            Curve::BasisOpen => {
                match self.point {
                    0 => self.point = 1,
                    1 => self.point = 2,
                    2 => {
                        self.point = 3;
                        let ([x0, x1, ..], [y0, y1, ..]) = (self.x, self.y);
                        self.begin((x0 + 4.0 * x1 + x) / 6.0, (y0 + 4.0 * y1 + y) / 6.0);
                    }
                    _ => {
                        self.point = 4;
                        self.basis_point(x, y);
                    }
                }
                self.shift2(x, y);
            }
            Curve::BasisClosed => {
                match self.point {
                    0 => {
                        self.point = 1;
                        (self.x[2], self.y[2]) = (x, y);
                    }
                    1 => {
                        self.point = 2;
                        (self.x[3], self.y[3]) = (x, y);
                    }
                    2 => {
                        self.point = 3;
                        (self.x[4], self.y[4]) = (x, y);
                        let ([x0, x1, ..], [y0, y1, ..]) = (self.x, self.y);
                        self.move_to((x0 + 4.0 * x1 + x) / 6.0, (y0 + 4.0 * y1 + y) / 6.0);
                    }
                    _ => self.basis_point(x, y),
                }
                self.shift2(x, y);
            }
            Curve::Bundle(_) | Curve::Natural => {
                self.xs.push(x);
                self.ys.push(y);
            }
            Curve::Cardinal(tension) => {
                match self.point {
                    0 => {
                        self.point = 1;
                        self.begin(x, y);
                    }
                    1 => {
                        self.point = 2;
                        (self.x[1], self.y[1]) = (x, y);
                    }
                    _ => {
                        self.point = 3;
                        self.cardinal_point(tension, x, y);
                    }
                }
                self.shift3(x, y);
            }
            Curve::CardinalOpen(tension) => {
                match self.point {
                    0 => self.point = 1,
                    1 => self.point = 2,
                    2 => {
                        self.point = 3;
                        let (x2, y2) = (self.x[2], self.y[2]);
                        self.begin(x2, y2);
                    }
                    _ => {
                        self.point = 4;
                        self.cardinal_point(tension, x, y);
                    }
                }
                self.shift3(x, y);
            }
            Curve::CardinalClosed(tension) => {
                match self.point {
                    0 => {
                        self.point = 1;
                        (self.x[3], self.y[3]) = (x, y);
                    }
                    1 => {
                        self.point = 2;
                        (self.x[4], self.y[4]) = (x, y);
                        self.move_to(x, y);
                    }
                    2 => {
                        self.point = 3;
                        (self.x[5], self.y[5]) = (x, y);
                    }
                    _ => self.cardinal_point(tension, x, y),
                }
                self.shift3(x, y);
            }
            Curve::CatmullRom(alpha) => {
                self.catmull_rom_measure(alpha, x, y);
                match self.point {
                    0 => {
                        self.point = 1;
                        self.begin(x, y);
                    }
                    1 => self.point = 2,
                    _ => {
                        self.point = 3;
                        self.catmull_rom_point(x, y);
                    }
                }
                self.catmull_rom_shift(x, y);
            }
            Curve::CatmullRomOpen(alpha) => {
                self.catmull_rom_measure(alpha, x, y);
                match self.point {
                    0 => self.point = 1,
                    1 => self.point = 2,
                    2 => {
                        self.point = 3;
                        let (x2, y2) = (self.x[2], self.y[2]);
                        self.begin(x2, y2);
                    }
                    _ => {
                        self.point = 4;
                        self.catmull_rom_point(x, y);
                    }
                }
                self.catmull_rom_shift(x, y);
            }
            Curve::CatmullRomClosed(alpha) => {
                self.catmull_rom_measure(alpha, x, y);
                match self.point {
                    0 => {
                        self.point = 1;
                        (self.x[3], self.y[3]) = (x, y);
                    }
                    1 => {
                        self.point = 2;
                        (self.x[4], self.y[4]) = (x, y);
                        self.move_to(x, y);
                    }
                    2 => {
                        self.point = 3;
                        (self.x[5], self.y[5]) = (x, y);
                    }
                    _ => self.catmull_rom_point(x, y),
                }
                self.catmull_rom_shift(x, y);
            }
            Curve::MonotoneX | Curve::MonotoneY => {
                let (x, y) = match curve {
                    Curve::MonotoneY => (y, x),
                    _ => (x, y),
                };
                if x == self.x[1] && y == self.y[1] {
                    return;
                }
                let mut t1 = f64::NAN;
                match self.point {
                    0 => {
                        self.point = 1;
                        self.begin(x, y);
                    }
                    1 => self.point = 2,
                    2 => {
                        self.point = 3;
                        t1 = self.slope3(x, y);
                        let t0 = self.slope2(t1);
                        self.monotone_point(t0, t1);
                    }
                    _ => {
                        t1 = self.slope3(x, y);
                        self.monotone_point(self.t0, t1);
                    }
                }
                self.shift2(x, y);
                self.t0 = t1;
            }
        }
    }

    fn line_end(&mut self, curve: Curve) {
        match curve {
            Curve::Linear => self.finish(self.point == 1),
            Curve::LinearClosed => {
                if self.point > 0 {
                    self.close();
                }
            }
            Curve::Step | Curve::StepBefore | Curve::StepAfter => {
                if 0.0 < self.t && self.t < 1.0 && self.point == 2 {
                    self.line_to(self.x[0], self.y[0]);
                }
                if self.line.is_some() {
                    self.t = 1.0 - self.t;
                }
                self.finish(self.point == 1);
            }
            Curve::Basis => {
                let (x1, y1) = (self.x[1], self.y[1]);
                if self.point == 3 {
                    self.basis_point(x1, y1);
                }
                if matches!(self.point, 2 | 3) {
                    self.line_to(x1, y1);
                }
                self.finish(self.point == 1);
            }
            Curve::BasisOpen => self.finish(self.point == 3),
            Curve::BasisClosed => match self.point {
                1 => {
                    self.move_to(self.x[2], self.y[2]);
                    self.close();
                }
                2 => {
                    let ([.., x2, x3, _, _], [.., y2, y3, _, _]) = (self.x, self.y);
                    self.move_to((x2 + 2.0 * x3) / 3.0, (y2 + 2.0 * y3) / 3.0);
                    self.line_to((x3 + 2.0 * x2) / 3.0, (y3 + 2.0 * y2) / 3.0);
                    self.close();
                }
                3 => {
                    let ([.., x2, x3, x4, _], [.., y2, y3, y4, _]) = (self.x, self.y);
                    self.point(curve, x2, y2);
                    self.point(curve, x3, y3);
                    self.point(curve, x4, y4);
                }
                _ => {}
            },
            Curve::Bundle(beta) => {
                let (xs, ys) = (mem::take(&mut self.xs), mem::take(&mut self.ys));
                if let Some(j) = xs.len().checked_sub(1).filter(|&j| j > 0) {
                    let (x0, y0) = (xs[0], ys[0]);
                    let (dx, dy) = (xs[j] - x0, ys[j] - y0);
                    for i in 0..=j {
                        let t = i as f64 / j as f64;
                        self.point(
                            Curve::Basis,
                            beta * xs[i] + (1.0 - beta) * (x0 + t * dx),
                            beta * ys[i] + (1.0 - beta) * (y0 + t * dy),
                        );
                    }
                }
                self.line_end(Curve::Basis);
            }
            Curve::Cardinal(tension) => {
                match self.point {
                    2 => self.line_to(self.x[2], self.y[2]),
                    3 => {
                        let (x1, y1) = (self.x[1], self.y[1]);
                        self.cardinal_point(tension, x1, y1);
                    }
                    _ => {}
                }
                self.finish(self.point == 1);
            }
            Curve::CardinalOpen(_) | Curve::CatmullRomOpen(_) => self.finish(self.point == 3),
            Curve::CardinalClosed(_) | Curve::CatmullRomClosed(_) => match self.point {
                1 => {
                    self.move_to(self.x[3], self.y[3]);
                    self.close();
                }
                2 => {
                    self.line_to(self.x[3], self.y[3]);
                    self.close();
                }
                3 => {
                    let ([.., x3, x4, x5], [.., y3, y4, y5]) = (self.x, self.y);
                    self.point(curve, x3, y3);
                    self.point(curve, x4, y4);
                    self.point(curve, x5, y5);
                }
                _ => {}
            },
            Curve::CatmullRom(_) => {
                match self.point {
                    2 => self.line_to(self.x[2], self.y[2]),
                    3 => {
                        let (x2, y2) = (self.x[2], self.y[2]);
                        self.point(curve, x2, y2);
                    }
                    _ => {}
                }
                self.finish(self.point == 1);
            }
            Curve::MonotoneX | Curve::MonotoneY => {
                match self.point {
                    2 => self.line_to(self.x[1], self.y[1]),
                    3 => {
                        let t0 = self.t0;
                        let t1 = self.slope2(t0);
                        self.monotone_point(t0, t1);
                    }
                    _ => {}
                }
                self.finish(self.point == 1);
            }
            Curve::Natural => {
                let (xs, ys) = (mem::take(&mut self.xs), mem::take(&mut self.ys));
                let n = xs.len();
                if n > 0 {
                    self.begin(xs[0], ys[0]);
                    if n == 2 {
                        self.line_to(xs[1], ys[1]);
                    } else {
                        let (px, py) = (control_points(&xs), control_points(&ys));
                        for i in 1..n {
                            self.cubic(
                                px.0[i - 1],
                                py.0[i - 1],
                                px.1[i - 1],
                                py.1[i - 1],
                                xs[i],
                                ys[i],
                            );
                        }
                    }
                }
                self.finish(n == 1);
            }
        }
    }
}

/// A natural cubic spline's two control points per segment through `x`.
fn control_points(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len() - 1;
    let (mut a, mut b, mut r) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    if n == 0 {
        return (a, b);
    }
    (a[0], b[0], r[0]) = (0.0, 2.0, x[0] + 2.0 * x[1]);
    for i in 1..n.saturating_sub(1) {
        (a[i], b[i], r[i]) = (1.0, 4.0, 4.0 * x[i] + 2.0 * x[i + 1]);
    }
    (a[n - 1], b[n - 1], r[n - 1]) = (2.0, 7.0, 8.0 * x[n - 1] + x[n]);
    for i in 1..n {
        let m = a[i] / b[i - 1];
        b[i] -= m;
        r[i] -= m * r[i - 1];
    }
    a[n - 1] = r[n - 1] / b[n - 1];
    for i in (0..n - 1).rev() {
        a[i] = (r[i] - a[i + 1]) / b[i];
    }
    b[n - 1] = (x[n] + a[n - 1]) / 2.0;
    for i in 0..n - 1 {
        b[i] = 2.0 * x[i + 1] - a[i + 1];
    }
    (a, b)
}
