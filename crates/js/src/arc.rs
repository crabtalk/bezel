//! d3-shape's arc: the path `d3.arc()` writes for a circular or annular
//! sector, with padding and rounded corners.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::{
    math,
    path::{Path, Segment},
};

/// d3-shape's `epsilon`.
const EPSILON: f64 = 1e-12;

/// What `d3.arc()` reads from a datum. Angles are radians clockwise from
/// twelve o'clock.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sector {
    pub inner: f64,
    pub outer: f64,
    pub start: f64,
    pub end: f64,
    /// The angle between neighbouring sectors, half taken from each side.
    pub pad: f64,
    pub corner: f64,
}

/// d3's `cornerTangents`: a corner circle of radius `rc` touching the line
/// from `p0` to `p1` and the circle of radius `r1`.
struct Tangents {
    cx: f64,
    cy: f64,
    x01: f64,
    y01: f64,
    x11: f64,
    y11: f64,
}

impl Tangents {
    fn new([x0, y0]: [f64; 2], [x1, y1]: [f64; 2], r1: f64, rc: f64, cw: bool) -> Self {
        let (x01, y01) = (x0 - x1, y0 - y1);
        let lo = (if cw { rc } else { -rc }) / (x01 * x01 + y01 * y01).sqrt();
        let (ox, oy) = (lo * y01, -lo * x01);
        let (x11, y11) = (x0 + ox, y0 + oy);
        let (x10, y10) = (x1 + ox, y1 + oy);
        let (x00, y00) = ((x11 + x10) / 2.0, (y11 + y10) / 2.0);
        let (dx, dy) = (x10 - x11, y10 - y11);
        let d2 = dx * dx + dy * dy;
        let r = r1 - rc;
        let big_d = x11 * y10 - x10 * y11;
        let d =
            (if dy < 0.0 { -1.0 } else { 1.0 }) * math::max(0.0, r * r * d2 - big_d * big_d).sqrt();
        let (mut cx, mut cy) = ((big_d * dy - dx * d) / d2, (-big_d * dx - dy * d) / d2);
        let (cx1, cy1) = ((big_d * dy + dx * d) / d2, (-big_d * dx + dy * d) / d2);
        let (dx0, dy0) = (cx - x00, cy - y00);
        let (dx1, dy1) = (cx1 - x00, cy1 - y00);
        // The closer of the two intersections.
        if dx0 * dx0 + dy0 * dy0 > dx1 * dx1 + dy1 * dy1 {
            (cx, cy) = (cx1, cy1);
        }
        Self {
            cx,
            cy,
            x01: -ox,
            y01: -oy,
            x11: cx * (r1 / r - 1.0),
            y11: cy * (r1 / r - 1.0),
        }
    }

    fn center(&self) -> [f64; 2] {
        [self.cx, self.cy]
    }
}

/// d3's `intersect`: where the line through `p0` and `p1` meets the line
/// through `p2` and `p3`, `None` for parallel lines.
fn intersect(
    [x0, y0]: [f64; 2],
    [x1, y1]: [f64; 2],
    [x2, y2]: [f64; 2],
    [x3, y3]: [f64; 2],
) -> Option<[f64; 2]> {
    let (x10, y10) = (x1 - x0, y1 - y0);
    let (x32, y32) = (x3 - x2, y3 - y2);
    let t = y32 * x10 - x32 * y10;
    if t * t < EPSILON {
        return None;
    }
    let t = (x32 * (y0 - y2) - y32 * (x0 - x2)) / t;
    Some([x0 + t * x10, y0 + t * y10])
}

/// d3-shape's `acos`, clamped to its domain.
fn acos(x: f64) -> f64 {
    if x > 1.0 {
        0.0
    } else if x < -1.0 {
        PI
    } else {
        x.acos()
    }
}

/// d3-shape's `asin`, clamped to its domain.
fn asin(x: f64) -> f64 {
    if x >= 1.0 {
        FRAC_PI_2
    } else if x <= -1.0 {
        -FRAC_PI_2
    } else {
        x.asin()
    }
}

impl Sector {
    /// What `d3.arc()` writes for it about the origin, its pad radius d3's
    /// default.
    pub fn path(self) -> Vec<Segment> {
        let mut p = Path::default();
        let (mut r0, mut r1) = (self.inner, self.outer);
        let a0 = self.start - FRAC_PI_2;
        let a1 = self.end - FRAC_PI_2;
        let da = (a1 - a0).abs();
        let cw = a1 > a0;
        if r1 < r0 {
            (r0, r1) = (r1, r0);
        }
        let at = |r: f64, a: f64| [r * a.cos(), r * a.sin()];

        if r1.is_nan() || r1 <= EPSILON {
            p.move_to([0.0, 0.0]);
        } else if da > TAU - EPSILON {
            p.move_to(at(r1, a0));
            p.arc([0.0, 0.0], r1, a0, a1, !cw);
            if r0 > EPSILON {
                p.move_to(at(r0, a1));
                p.arc([0.0, 0.0], r0, a1, a0, cw);
            }
        } else {
            let (mut a01, mut a11, mut a00, mut a10) = (a0, a1, a0, a1);
            let (mut da0, mut da1) = (da, da);
            let ap = self.pad / 2.0;
            let rp = match ap > EPSILON {
                true => (r0 * r0 + r1 * r1).sqrt(),
                false => 0.0,
            };
            let rc = math::min((r1 - r0).abs() / 2.0, self.corner);
            let (mut rc0, mut rc1) = (rc, rc);

            if rp > EPSILON {
                let p0 = asin(rp / r0 * ap.sin());
                let p1 = asin(rp / r1 * ap.sin());
                da0 -= p0 * 2.0;
                if da0 > EPSILON {
                    let p0 = if cw { p0 } else { -p0 };
                    a00 += p0;
                    a10 -= p0;
                } else {
                    da0 = 0.0;
                    (a00, a10) = ((a0 + a1) / 2.0, (a0 + a1) / 2.0);
                }
                da1 -= p1 * 2.0;
                if da1 > EPSILON {
                    let p1 = if cw { p1 } else { -p1 };
                    a01 += p1;
                    a11 -= p1;
                } else {
                    da1 = 0.0;
                    (a01, a11) = ((a0 + a1) / 2.0, (a0 + a1) / 2.0);
                }
            }

            let p01 = at(r1, a01);
            let p10 = at(r0, a10);
            let p11 = at(r1, a11);
            let p00 = at(r0, a00);

            // The corner radius the sector's angle leaves room for, or none
            // where the sides do not meet.
            if rc > EPSILON && da < PI {
                match intersect(p01, p00, p11, p10) {
                    Some(oc) => {
                        let (ax, ay) = (p01[0] - oc[0], p01[1] - oc[1]);
                        let (bx, by) = (p11[0] - oc[0], p11[1] - oc[1]);
                        let cos = (ax * bx + ay * by)
                            / ((ax * ax + ay * ay).sqrt() * (bx * bx + by * by).sqrt());
                        let kc = 1.0 / (acos(cos) / 2.0).sin();
                        let lc = (oc[0] * oc[0] + oc[1] * oc[1]).sqrt();
                        rc0 = math::min(rc, (r0 - lc) / (kc - 1.0));
                        rc1 = math::min(rc, (r1 - lc) / (kc + 1.0));
                    }
                    None => (rc0, rc1) = (0.0, 0.0),
                }
            }

            let angle = |y: f64, x: f64| y.atan2(x);
            if da1.is_nan() || da1 <= EPSILON {
                p.move_to(p01);
            } else if rc1 > EPSILON {
                let t0 = Tangents::new(p00, p01, r1, rc1, cw);
                let t1 = Tangents::new(p11, p10, r1, rc1, cw);
                p.move_to([t0.cx + t0.x01, t0.cy + t0.y01]);
                if rc1 < rc {
                    // The corners have merged.
                    let (from, to) = (angle(t0.y01, t0.x01), angle(t1.y01, t1.x01));
                    p.arc(t0.center(), rc1, from, to, !cw);
                } else {
                    let (from, to) = (angle(t0.y01, t0.x01), angle(t0.y11, t0.x11));
                    p.arc(t0.center(), rc1, from, to, !cw);
                    let from = angle(t0.cy + t0.y11, t0.cx + t0.x11);
                    let to = angle(t1.cy + t1.y11, t1.cx + t1.x11);
                    p.arc([0.0, 0.0], r1, from, to, !cw);
                    let (from, to) = (angle(t1.y11, t1.x11), angle(t1.y01, t1.x01));
                    p.arc(t1.center(), rc1, from, to, !cw);
                }
            } else {
                p.move_to(p01);
                p.arc([0.0, 0.0], r1, a01, a11, !cw);
            }

            if r0.is_nan() || r0 <= EPSILON || da0.is_nan() || da0 <= EPSILON {
                p.line_to(p10);
            } else if rc0 > EPSILON {
                let t0 = Tangents::new(p10, p11, r0, -rc0, cw);
                let t1 = Tangents::new(p01, p00, r0, -rc0, cw);
                p.line_to([t0.cx + t0.x01, t0.cy + t0.y01]);
                if rc0 < rc {
                    let (from, to) = (angle(t0.y01, t0.x01), angle(t1.y01, t1.x01));
                    p.arc(t0.center(), rc0, from, to, !cw);
                } else {
                    let (from, to) = (angle(t0.y01, t0.x01), angle(t0.y11, t0.x11));
                    p.arc(t0.center(), rc0, from, to, !cw);
                    let from = angle(t0.cy + t0.y11, t0.cx + t0.x11);
                    let to = angle(t1.cy + t1.y11, t1.cx + t1.x11);
                    p.arc([0.0, 0.0], r0, from, to, cw);
                    let (from, to) = (angle(t1.y11, t1.x11), angle(t1.y01, t1.x01));
                    p.arc(t1.center(), rc0, from, to, !cw);
                }
            } else {
                p.arc([0.0, 0.0], r0, a10, a00, cw);
            }
        }
        p.close_path();
        p.segments
    }
}
