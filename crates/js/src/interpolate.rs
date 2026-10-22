//! d3-interpolate: numbers, colours in each of d3-color's spaces, and
//! `piecewise`. A colour comes out as d3 hands it on, written as
//! `formatRgb` writes it and read back.

use crate::{
    color::{Cubehelix, Hcl, Hsl, Lab, Rgb},
    math,
};
use std::str::FromStr;

/// A colour interpolator, by Vega's names: `rgb`, `hsl`, `hsl-long`, `lab`,
/// `hcl`, `hcl-long`, `cubehelix` and `cubehelix-long`. `gamma` is 1 for
/// none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Interpolate {
    Rgb { gamma: f64 },
    Hsl,
    HslLong,
    Lab,
    Hcl,
    HclLong,
    Cubehelix { gamma: f64 },
    CubehelixLong { gamma: f64 },
}

impl FromStr for Interpolate {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        Ok(match name.to_lowercase().as_str() {
            "rgb" => Self::Rgb { gamma: 1.0 },
            "hsl" => Self::Hsl,
            "hsl-long" => Self::HslLong,
            "lab" => Self::Lab,
            "hcl" => Self::Hcl,
            "hcl-long" => Self::HclLong,
            "cubehelix" => Self::Cubehelix { gamma: 1.0 },
            "cubehelix-long" => Self::CubehelixLong { gamma: 1.0 },
            _ => return Err(()),
        })
    }
}

/// d3's `interpolateNumber(a, b)(t)`.
pub fn number(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}

/// d3's `interpolateRound(a, b)(t)`.
pub fn round(a: f64, b: f64, t: f64) -> f64 {
    math::round(number(a, b, t))
}

/// d3-interpolate's `color`: a straight run, or `a` throughout where it
/// does not move, `b` where `a` is `NaN`.
fn linear(a: f64, b: f64, t: f64) -> f64 {
    let d = b - a;
    match d != 0.0 && !d.is_nan() {
        true => a + t * d,
        false if a.is_nan() => b,
        false => a,
    }
}

/// d3-interpolate's `hue`: the shorter way round.
fn hue(a: f64, b: f64, t: f64) -> f64 {
    let d = b - a;
    match d != 0.0 && !d.is_nan() {
        true => {
            let d = match !(-180.0..=180.0).contains(&d) {
                true => d - 360.0 * math::round(d / 360.0),
                false => d,
            };
            a + t * d
        }
        false if a.is_nan() => b,
        false => a,
    }
}

/// d3-interpolate's `gamma(y)`: a straight run in `x^y`.
fn gamma(a: f64, b: f64, y: f64, t: f64) -> f64 {
    if y == 1.0 {
        return linear(a, b, t);
    }
    if b - a == 0.0 || (b - a).is_nan() {
        return if a.is_nan() { b } else { a };
    }
    let a = math::pow(a, y);
    let b = math::pow(b, y) - a;
    math::pow(a + t * b, 1.0 / y)
}

impl Interpolate {
    /// The interpolator `gamma` takes where it takes one, as Vega's
    /// `interpolateGamma` does.
    pub fn with_gamma(self, gamma: f64) -> Self {
        match self {
            Self::Rgb { .. } => Self::Rgb { gamma },
            Self::Cubehelix { .. } => Self::Cubehelix { gamma },
            Self::CubehelixLong { .. } => Self::CubehelixLong { gamma },
            other => other,
        }
    }

    /// The colour `t` of the way from `a` to `b`.
    pub fn color(self, a: Rgb, b: Rgb, t: f64) -> Rgb {
        let color: Rgb = match self {
            Self::Rgb { gamma: y } => Rgb {
                r: gamma(a.r, b.r, y, t),
                g: gamma(a.g, b.g, y, t),
                b: gamma(a.b, b.b, y, t),
                opacity: linear(a.opacity, b.opacity, t),
            },
            Self::Hsl | Self::HslLong => {
                let (a, b) = (Hsl::from(a), Hsl::from(b));
                Hsl {
                    h: match self {
                        Self::Hsl => hue(a.h, b.h, t),
                        _ => linear(a.h, b.h, t),
                    },
                    s: linear(a.s, b.s, t),
                    l: linear(a.l, b.l, t),
                    opacity: linear(a.opacity, b.opacity, t),
                }
                .into()
            }
            Self::Lab => {
                let (a, b) = (Lab::from(a), Lab::from(b));
                Lab {
                    l: linear(a.l, b.l, t),
                    a: linear(a.a, b.a, t),
                    b: linear(a.b, b.b, t),
                    opacity: linear(a.opacity, b.opacity, t),
                }
                .into()
            }
            Self::Hcl | Self::HclLong => {
                let (a, b) = (Hcl::from(a), Hcl::from(b));
                Hcl {
                    h: match self {
                        Self::Hcl => hue(a.h, b.h, t),
                        _ => linear(a.h, b.h, t),
                    },
                    c: linear(a.c, b.c, t),
                    l: linear(a.l, b.l, t),
                    opacity: linear(a.opacity, b.opacity, t),
                }
                .into()
            }
            Self::Cubehelix { gamma: y } | Self::CubehelixLong { gamma: y } => {
                let (a, b) = (Cubehelix::from(a), Cubehelix::from(b));
                Cubehelix {
                    h: match self {
                        Self::Cubehelix { .. } => hue(a.h, b.h, t),
                        _ => linear(a.h, b.h, t),
                    },
                    s: linear(a.s, b.s, t),
                    l: linear(a.l, b.l, math::pow(t, y)),
                    opacity: linear(a.opacity, b.opacity, t),
                }
                .into()
            }
        };
        color.written()
    }

    /// d3's `piecewise(interpolate, colors)(t)`: `colors` evenly across
    /// `0..=1`, this interpolator between each and the next.
    pub fn piecewise(self, colors: &[Rgb], t: f64) -> Rgb {
        let n = colors.len().saturating_sub(1);
        if n == 0 {
            return colors.first().copied().unwrap_or(Rgb {
                r: f64::NAN,
                g: f64::NAN,
                b: f64::NAN,
                opacity: f64::NAN,
            });
        }
        let t = t * n as f64;
        let i = (t.floor().min(n as f64 - 1.0)).max(0.0);
        let i = if i.is_nan() { 0 } else { i as usize };
        self.color(colors[i], colors[i + 1], t - i as f64)
    }
}
