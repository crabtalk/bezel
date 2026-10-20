//! The geometry of Mermaid's Cynefin boundaries: seeded wavy lines between
//! the domains and the S-shaped cliff between Clear and Chaotic. Pele's
//! `diagrams/cynefin/boundaries.ts`.

use crate::geom::Point;
use crate::{js, svg::builder::fragment};
use std::fmt;

pub const FOLD: WaveOptions = WaveOptions {
    segments: 7,
    vertical: true,
    point_step: 17.0,
    control_step: 31.0,
    control_shift: 7.0,
    pin_start: false,
    pin_end: false,
};
pub const HORIZON: WaveOptions = WaveOptions {
    segments: 7,
    vertical: false,
    point_step: 23.0,
    control_step: 37.0,
    control_shift: 11.0,
    pin_start: false,
    pin_end: false,
};

/// mulberry32, one step.
pub fn seeded_random(seed: f64) -> f64 {
    let mut t = js::to_int32(seed + f64::from(0x6d2b79f5u32));
    t = (t ^ ((t as u32) >> 15) as i32).wrapping_mul(t | 1);
    t ^= t.wrapping_add((t ^ ((t as u32) >> 7) as i32).wrapping_mul(t | 61));
    f64::from((t ^ ((t as u32) >> 14) as i32) as u32) / 4294967296.0
}

pub fn hash_string(text: &str) -> i32 {
    let mut hash: i32 = 0;
    for unit in text.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(i32::from(unit));
    }
    hash
}

/// A configured seed other than zero wins; otherwise the seed comes from the
/// diagram's id.
pub fn resolve_seed(configured: f64, id: &str) -> f64 {
    if configured.is_finite() && configured != 0.0 {
        return configured;
    }
    f64::from(hash_string(id))
}

#[derive(Clone, Copy)]
pub struct WaveOptions {
    pub segments: usize,
    pub vertical: bool,
    /// Multipliers Mermaid uses to vary the seed per point and per control
    /// point.
    pub point_step: f64,
    pub control_step: f64,
    pub control_shift: f64,
    pub pin_start: bool,
    pub pin_end: bool,
}

/// A wavy line along one axis from `from` to `to`, centered on `center` in
/// the other axis. Points jitter by up to the amplitude; a pinned end stays on
/// the center line.
pub fn wave(from: f64, to: f64, center: f64, seed: f64, amplitude: f64, o: WaveOptions) -> Curve {
    let step = (to - from) / o.segments as f64;
    let mut along = Vec::with_capacity(o.segments + 1);
    let mut across = Vec::with_capacity(o.segments + 1);
    for i in 0..=o.segments {
        let pinned = (i == 0 && o.pin_start) || (i == o.segments && o.pin_end);
        along.push(from + i as f64 * step);
        across.push(if pinned {
            center
        } else {
            center + seeded_random(seed + i as f64 * o.point_step) * amplitude * 2.0 - amplitude
        });
    }
    let point = |along: f64, across: f64| {
        if o.vertical {
            Point::new(across, along)
        } else {
            Point::new(along, across)
        }
    };
    let mut points = Vec::with_capacity(3 * o.segments + 1);
    points.push(point(along[0], across[0]));
    for i in 0..o.segments {
        let mid = (along[i] + along[i + 1]) / 2.0;
        let offset = amplitude
            * 1.5
            * (if i % 2 == 0 { 1.0 } else { -1.0 })
            * seeded_random(seed + i as f64 * o.control_step + o.control_shift);
        points.extend([
            point(mid, across[i] + offset),
            point(mid, across[i + 1] - offset),
            point(along[i + 1], across[i + 1]),
        ]);
    }
    Curve(points)
}

/// The cliff: a steep S from `top` straight down to `bottom`.
pub fn cliff(top: Point, bottom: f64, amplitude: f64) -> Curve {
    let Point { x: cx, y: top } = top;
    let h = bottom - top;
    Curve(vec![
        Point::new(cx, top),
        Point::new(cx + amplitude, top + h * 0.2),
        Point::new(cx - amplitude * 1.5, top + h * 0.55),
        Point::new(cx + amplitude * 0.5, top + h * 0.75),
        Point::new(cx - amplitude, top + h * 0.85),
        Point::new(cx + amplitude * 0.3, top + h * 0.95),
        Point::new(cx, bottom),
    ])
}

/// A start point, then two control points and an end point per cubic
/// segment.
pub struct Curve(pub Vec<Point>);

impl Curve {
    pub fn start(&self) -> Point {
        self.0[0]
    }

    pub fn end(&self) -> Point {
        self.0[self.0.len() - 1]
    }

    /// Path commands that continue from the start point to the end.
    pub fn forward(&self) -> impl fmt::Display + '_ {
        fragment(move |f| {
            for c in self.0[1..].as_chunks::<3>().0 {
                write!(f, "C{} {} {}", c[0], c[1], c[2])?;
            }
            Ok(())
        })
    }

    /// Path commands that continue from the end point back to the start.
    pub fn backward(&self) -> impl fmt::Display + '_ {
        fragment(move |f| {
            for c in self.0.windows(4).rev().step_by(3) {
                write!(f, "C{} {} {}", c[2], c[1], c[0])?;
            }
            Ok(())
        })
    }

    pub fn path(&self) -> impl fmt::Display + '_ {
        fragment(move |f| write!(f, "M{}{}", self.start(), self.forward()))
    }
}

pub fn ellipse_path(center: Point, radii: Point) -> impl fmt::Display {
    fragment(move |f| {
        let across = Point::new(radii.x, 0.0);
        write!(
            f,
            "M{}A{radii} 0 1,1 {}A{radii} 0 1,1 {}Z",
            center - across,
            center + across,
            center - across
        )
    })
}
