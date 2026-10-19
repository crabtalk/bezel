//! The geometry of Mermaid's Cynefin boundaries: seeded wavy lines between
//! the domains and the S-shaped cliff between Clear and Chaotic. Curves are
//! flat lists of numbers: a start point, then six numbers (two control points
//! and an end point) per cubic segment. Pele's
//! `diagrams/cynefin/boundaries.ts`.

use crate::{js, svg::builder::num};
use std::fmt::Write as _;

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
pub fn wave(
    from: f64,
    to: f64,
    center: f64,
    seed: f64,
    amplitude: f64,
    o: WaveOptions,
) -> Vec<f64> {
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
    let mut out = if o.vertical {
        vec![across[0], along[0]]
    } else {
        vec![along[0], across[0]]
    };
    for i in 0..o.segments {
        let mid = (along[i] + along[i + 1]) / 2.0;
        let offset = amplitude
            * 1.5
            * (if i % 2 == 0 { 1.0 } else { -1.0 })
            * seeded_random(seed + i as f64 * o.control_step + o.control_shift);
        if o.vertical {
            out.extend([
                across[i] + offset,
                mid,
                across[i + 1] - offset,
                mid,
                across[i + 1],
                along[i + 1],
            ]);
        } else {
            out.extend([
                mid,
                across[i] + offset,
                mid,
                across[i + 1] - offset,
                along[i + 1],
                across[i + 1],
            ]);
        }
    }
    out
}

/// The cliff: a steep S from (cx, top) down to (cx, bottom).
pub fn cliff(cx: f64, top: f64, bottom: f64, amplitude: f64) -> Vec<f64> {
    let h = bottom - top;
    vec![
        cx,
        top,
        cx + amplitude,
        top + h * 0.2,
        cx - amplitude * 1.5,
        top + h * 0.55,
        cx + amplitude * 0.5,
        top + h * 0.75,
        cx - amplitude,
        top + h * 0.85,
        cx + amplitude * 0.3,
        top + h * 0.95,
        cx,
        bottom,
    ]
}

/// Path commands that continue from the curve's start point to its end.
pub fn forward(curve: &[f64]) -> String {
    let mut d = String::new();
    let mut i = 2;
    while i < curve.len() {
        let _ = write!(
            d,
            "C{},{} {},{} {},{}",
            num(curve[i]),
            num(curve[i + 1]),
            num(curve[i + 2]),
            num(curve[i + 3]),
            num(curve[i + 4]),
            num(curve[i + 5])
        );
        i += 6;
    }
    d
}

/// Path commands that continue from the curve's end point back to its start.
pub fn backward(curve: &[f64]) -> String {
    let mut d = String::new();
    let mut i = curve.len() as isize - 6;
    while i >= 2 {
        let k = i as usize;
        let _ = write!(
            d,
            "C{},{} {},{} {},{}",
            num(curve[k + 2]),
            num(curve[k + 3]),
            num(curve[k]),
            num(curve[k + 1]),
            num(curve[k - 2]),
            num(curve[k - 1])
        );
        i -= 6;
    }
    d
}

pub fn curve_path(curve: &[f64]) -> String {
    format!("M{},{}{}", num(curve[0]), num(curve[1]), forward(curve))
}

pub fn ellipse_path(cx: f64, cy: f64, rx: f64, ry: f64) -> String {
    let r = format!("{},{}", num(rx), num(ry));
    format!(
        "M{},{}A{r} 0 1,1 {},{}A{r} 0 1,1 {},{}Z",
        num(cx - rx),
        num(cy),
        num(cx + rx),
        num(cy),
        num(cx - rx),
        num(cy)
    )
}
