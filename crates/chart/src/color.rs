//! Colours between bezel-js's d3-color and gpui.

use gpui::{Hsla, Rgba};
use js::color::Rgb;

/// `rgb` as gpui paints it, its channels held in range. A colour with no
/// channels or opacity, as `transparent` has none, is transparent.
pub(crate) fn hsla(rgb: Rgb) -> Hsla {
    if rgb.opacity.is_nan() || rgb.r.is_nan() {
        return Hsla::transparent_black();
    }
    let channel = |v: f64| (v / 255.0).clamp(0.0, 1.0) as f32;
    Rgba {
        r: channel(rgb.r),
        g: channel(rgb.g),
        b: channel(rgb.b),
        a: rgb.opacity.clamp(0.0, 1.0) as f32,
    }
    .into()
}

/// `color` as d3-color holds it.
pub(crate) fn rgb(color: Hsla) -> Rgb {
    let Rgba { r, g, b, a } = color.into();
    Rgb {
        r: f64::from(r) * 255.0,
        g: f64::from(g) * 255.0,
        b: f64::from(b) * 255.0,
        opacity: f64::from(a),
    }
}
