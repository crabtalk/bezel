//! The blobatar palette: six authored tones, hue from the seed, and the
//! contrast floors enforced against real sRGB luminance.

use gpui::hsla;
use theme::rgb_to_hsl;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

fn to_linear(c: Oklch) -> (f64, f64, f64) {
    let r = c.h.to_radians();
    let a = c.c * r.cos();
    let b = c.c * r.sin();

    let l_ = c.l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = c.l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = c.l - 0.0894841775 * a - 1.291485548 * b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
    )
}

fn in_gamut((r, g, b): (f64, f64, f64)) -> bool {
    (-1e-4..=1.0 + 1e-4).contains(&r)
        && (-1e-4..=1.0 + 1e-4).contains(&g)
        && (-1e-4..=1.0 + 1e-4).contains(&b)
}

/// In-gamut linear sRGB, chroma reduced by binary search when needed. Chroma
/// is the right axis to give up: lowering it desaturates, while clipping
/// channels shifts hue — a clipped vivid blue turns purple.
fn resolve(c: Oklch) -> (f64, f64, f64) {
    let mut rgb = to_linear(c);
    if !in_gamut(rgb) {
        let mut lo = 0.0;
        let mut hi = c.c;
        for _ in 0..12 {
            let mid = (lo + hi) / 2.0;
            if in_gamut(to_linear(Oklch { c: mid, ..c })) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        rgb = to_linear(Oklch { c: lo, ..c });
    }
    (
        rgb.0.clamp(0.0, 1.0),
        rgb.1.clamp(0.0, 1.0),
        rgb.2.clamp(0.0, 1.0),
    )
}

/// WCAG relative luminance. `resolve` output is already linear-light sRGB,
/// which is exactly what WCAG's piecewise transfer function produces.
fn luminance(c: Oklch) -> f64 {
    let (r, g, b) = resolve(c);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

pub fn contrast(a: Oklch, b: Oklch) -> f64 {
    let x = luminance(a);
    let y = luminance(b);
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Pushes `fg`'s lightness away from `bg` until the pair clears `min`,
/// walking the direction it already leans first.
fn ensure_contrast(fg: Oklch, bg: Oklch, min: f64) -> Oklch {
    if contrast(fg, bg) >= min {
        return fg;
    }
    let lean = if fg.l >= bg.l { 1.0 } else { -1.0 };
    for dir in [lean, -lean] {
        let mut probe = fg;
        for _ in 0..60 {
            probe.l = (probe.l + dir * 0.02).clamp(0.0, 1.0);
            if contrast(probe, bg) >= min {
                return probe;
            }
            if probe.l == 0.0 || probe.l == 1.0 {
                break;
            }
        }
    }
    let black = Oklch {
        l: 0.0,
        c: 0.0,
        h: fg.h,
    };
    let white = Oklch {
        l: 1.0,
        c: 0.0,
        h: fg.h,
    };
    if contrast(black, bg) >= contrast(white, bg) {
        black
    } else {
        white
    }
}

/// Gamma-encoded 8-bit sRGB — the hex values blobatar serializes.
pub fn to_rgb8(c: Oklch) -> [u8; 3] {
    let (r, g, b) = resolve(c);
    let enc = |v: f64| {
        let s = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        // JS `Math.round` semantics: half rounds toward +∞.
        (s * 255.0 + 0.5).floor() as u8
    };
    [enc(r), enc(g), enc(b)]
}

/// The palette as gpui paints it, bridged through the same 8-bit values.
pub fn to_hsla(c: Oklch) -> gpui::Hsla {
    let [r, g, b] = to_rgb8(c);
    let (h, s, l) = rgb_to_hsl(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    hsla(h, s, l, 1.0)
}

/// Six authored swatches: the same discipline as a designer handing you a
/// set, rather than a slider. Thresholds are cumulative, so pale and mid
/// tones dominate and the near-black body stays a rare find.
const TONES: [(f64, f64, f64); 6] = [
    (0.2, 0.86, 0.085),
    (0.36, 0.9, 0.028),
    (0.62, 0.73, 0.135),
    (0.8, 0.62, 0.165),
    (0.93, 0.87, 0.16),
    (1.0, 0.34, 0.035),
];

const DARK_SURFACE: Oklch = Oklch {
    l: 0.145,
    c: 0.0,
    h: 0.0,
};
const SURFACE_FLOOR: f64 = 1.5;

/// The authored ramp: the seed picks a hue and a tone, and everything follows.
fn ramp(hue: f64, tone: f64) -> (Oklch, Oklch) {
    let (l, c) = TONES
        .iter()
        .find(|(edge, _, _)| tone < *edge)
        .map(|(_, l, c)| (*l, *c))
        .unwrap_or((TONES[0].1, TONES[0].2));
    let head = ensure_contrast(Oklch { l, c, h: hue }, DARK_SURFACE, SURFACE_FLOOR);
    // Polarity follows the body: dark eyes on a light body, light eyes on a
    // dark one.
    let eye = if head.l >= 0.5 {
        Oklch {
            l: 0.17,
            c: 0.02,
            h: hue,
        }
    } else {
        Oklch {
            l: 0.97,
            c: 0.012,
            h: hue,
        }
    };
    (head, eye)
}

/// The seed → (head, eye) palette, floors enforced in blobatar's order:
/// head against its light bg, then the eye against the head.
pub fn ramp_for(hue: f64, tone: f64) -> (Oklch, Oklch) {
    let (mut head, mut eye) = ramp(hue, tone);
    let bg = Oklch {
        l: 0.965,
        c: 0.01,
        h: hue,
    };
    head = ensure_contrast(head, bg, 1.25);
    eye = ensure_contrast(eye, head, 4.5);
    (head, eye)
}
