//! d3-color: `color(specifier)` as RGB, a CSS colour named, in hex, or in
//! `rgb()`, `rgba()`, `hsl()` or `hsla()` notation; the HSL, CIELAB, HCL and
//! Cubehelix spaces and their conversions; and `formatRgb` as `Display`.

use crate::{math, number::Number, string};
use std::{f64::consts::PI, fmt, str::FromStr};

/// Channels in `0..=255` as written, which may fall outside it, and opacity
/// in `0..=1`. A colour with opacity at or below zero has `NaN` channels, as
/// `transparent` does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub opacity: f64,
}

/// The CSS colour keywords d3-color knows, sorted, each as `0xrrggbb`.
const NAMED: [(&str, u32); 148] = [
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

impl Rgb {
    fn rgba(r: f64, g: f64, b: f64, opacity: f64) -> Self {
        match opacity <= 0.0 {
            true => Self {
                r: f64::NAN,
                g: f64::NAN,
                b: f64::NAN,
                opacity,
            },
            false => Self { r, g, b, opacity },
        }
    }

    fn hex(n: u32) -> Self {
        let channel = |shift: u32| ((n >> shift) & 0xff) as f64;
        Self {
            r: channel(16),
            g: channel(8),
            b: channel(0),
            opacity: 1.0,
        }
    }
}

impl FromStr for Rgb {
    type Err = ();

    /// `None` where d3-color's `color` answers `null`.
    fn from_str(specifier: &str) -> Result<Self, ()> {
        let format = string::trim(specifier).to_lowercase();
        if let Some(digits) = format.strip_prefix('#') {
            if !(3..=8).contains(&digits.len()) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(());
            }
            let m = u32::from_str_radix(digits, 16).map_err(|_| ())?;
            let nibble = |shift: u32| ((m >> shift) & 0xf) as f64 * 17.0;
            return match digits.len() {
                6 => Ok(Self::hex(m)),
                3 => Ok(Self {
                    r: nibble(8),
                    g: nibble(4),
                    b: nibble(0),
                    opacity: 1.0,
                }),
                8 => Ok(Self::rgba(
                    (m >> 24 & 0xff) as f64,
                    (m >> 16 & 0xff) as f64,
                    (m >> 8 & 0xff) as f64,
                    (m & 0xff) as f64 / 255.0,
                )),
                4 => Ok(Self::rgba(
                    nibble(12),
                    nibble(8),
                    nibble(4),
                    nibble(0) / 255.0,
                )),
                _ => Err(()),
            };
        }
        if let Some((name, rest)) = format.split_once('(')
            && let Some(inner) = rest.strip_suffix(')')
        {
            let args: Vec<&str> = inner.split(',').collect();
            let all = |read: fn(&str) -> Option<f64>| -> Option<Vec<f64>> {
                args.iter().map(|arg| read(arg)).collect()
            };
            // d3 multiplies before it divides, which rounds differently.
            let rgb = |args: &[f64], percent: bool| match percent {
                true => [
                    args[0] * 255.0 / 100.0,
                    args[1] * 255.0 / 100.0,
                    args[2] * 255.0 / 100.0,
                ],
                false => [args[0], args[1], args[2]],
            };
            return match (name, args.len()) {
                ("rgb", 3) => {
                    if let Some(v) = all(integer) {
                        Ok(Self::rgba(v[0], v[1], v[2], 1.0))
                    } else {
                        let v = all(percent).ok_or(())?;
                        let [r, g, b] = rgb(&v, true);
                        Ok(Self {
                            r,
                            g,
                            b,
                            opacity: 1.0,
                        })
                    }
                }
                ("rgba", 4) => {
                    let opacity = number(args[3]).ok_or(())?;
                    let head = &args[..3];
                    let read = |f: fn(&str) -> Option<f64>| -> Option<Vec<f64>> {
                        head.iter().map(|arg| f(arg)).collect()
                    };
                    let [r, g, b] = match read(integer) {
                        Some(v) => rgb(&v, false),
                        None => rgb(&read(percent).ok_or(())?, true),
                    };
                    Ok(Self::rgba(r, g, b, opacity))
                }
                ("hsl" | "hsla", count) if count == if name == "hsl" { 3 } else { 4 } => {
                    let h = number(args[0]).ok_or(())?;
                    let s = percent(args[1]).ok_or(())?;
                    let l = percent(args[2]).ok_or(())?;
                    let opacity = match count {
                        4 => number(args[3]).ok_or(())?,
                        _ => 1.0,
                    };
                    Ok(Hsl::hsla(h, s / 100.0, l / 100.0, opacity).into())
                }
                _ => Err(()),
            };
        }
        if let Ok(index) = NAMED.binary_search_by_key(&format.as_str(), |(name, _)| name) {
            return Ok(Self::hex(NAMED[index].1));
        }
        match format.as_str() {
            "transparent" => Ok(Self::rgba(f64::NAN, f64::NAN, f64::NAN, 0.0)),
            _ => Err(()),
        }
    }
}

/// `[+-]?\d+`, with whitespace either side.
fn integer(arg: &str) -> Option<f64> {
    let text = string::trim(arg);
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// `[+-]?(\d*\.)?\d+([eE][+-]?\d+)?`, with whitespace either side.
fn number(arg: &str) -> Option<f64> {
    let text = string::trim(arg);
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
        None => (unsigned, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let fraction = match mantissa.split_once('.') {
        Some((whole, fraction)) => whole.bytes().all(|b| b.is_ascii_digit()) && digits(fraction),
        None => digits(mantissa),
    };
    let exponent = exponent.is_none_or(|e| digits(e.strip_prefix(['+', '-']).unwrap_or(e)));
    (fraction && exponent).then(|| text.parse().ok()).flatten()
}

/// A number then `%`, with whitespace either side.
fn percent(arg: &str) -> Option<f64> {
    number(string::trim(arg).strip_suffix('%')?)
}

/// `formatRgb`: [`Rgb::written`]'s channels, `rgba()` below full opacity.
impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Rgb { r, g, b, opacity } = self.written();
        match opacity == 1.0 {
            true => write!(f, "rgb({r}, {g}, {b})"),
            false => write!(f, "rgba({r}, {g}, {b}, {})", Number(opacity)),
        }
    }
}

impl Rgb {
    /// The colour as `formatRgb` writes it, which is how d3-interpolate
    /// hands a colour on: channels rounded into `0..=255`, `NaN` as 0, and
    /// opacity held in `0..=1`, `NaN` as 1.
    pub fn written(self) -> Self {
        // `|| 0` takes NaN and -0 to 0.
        let channel = |v: f64| match math::round(v) {
            v if v.is_nan() || v == 0.0 => 0.0,
            v => v.clamp(0.0, 255.0),
        };
        Self {
            r: channel(self.r),
            g: channel(self.g),
            b: channel(self.b),
            opacity: match self.opacity.is_nan() {
                true => 1.0,
                false => self.opacity.clamp(0.0, 1.0),
            },
        }
    }
}

const RADIANS: f64 = PI / 180.0;
const DEGREES: f64 = 180.0 / PI;

/// A colour in HSL, `s` and `l` in `0..=1`. `NaN` hue or saturation where
/// the colour has none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl {
    pub h: f64,
    pub s: f64,
    pub l: f64,
    pub opacity: f64,
}

impl Hsl {
    /// d3-color's `hsla`: no hue, saturation or lightness for a transparent
    /// colour, no hue or saturation for black or white, no hue for a grey.
    pub fn hsla(h: f64, s: f64, l: f64, opacity: f64) -> Self {
        let (h, s, l) = if opacity <= 0.0 {
            (f64::NAN, f64::NAN, f64::NAN)
        } else if l <= 0.0 || l >= 1.0 {
            (f64::NAN, f64::NAN, l)
        } else if s <= 0.0 {
            (f64::NAN, s, l)
        } else {
            (h, s, l)
        };
        Self { h, s, l, opacity }
    }
}

impl From<Rgb> for Hsl {
    fn from(o: Rgb) -> Self {
        let (r, g, b) = (o.r / 255.0, o.g / 255.0, o.b / 255.0);
        let min = math::min(math::min(r, g), b);
        let max = math::max(math::max(r, g), b);
        let mut h = f64::NAN;
        let mut s = max - min;
        let l = (max + min) / 2.0;
        if s != 0.0 && !s.is_nan() {
            h = if r == max {
                (g - b) / s + if g < b { 6.0 } else { 0.0 }
            } else if g == max {
                (b - r) / s + 2.0
            } else {
                (r - g) / s + 4.0
            };
            s /= if l < 0.5 { max + min } else { 2.0 - max - min };
            h *= 60.0;
        } else {
            s = if l > 0.0 && l < 1.0 { 0.0 } else { h };
        }
        Self {
            h,
            s,
            l,
            opacity: o.opacity,
        }
    }
}

impl From<Hsl> for Rgb {
    fn from(o: Hsl) -> Self {
        let h = o.h % 360.0 + if o.h < 0.0 { 360.0 } else { 0.0 };
        let s = if h.is_nan() || o.s.is_nan() { 0.0 } else { o.s };
        let l = o.l;
        let m2 = l + if l < 0.5 { l } else { 1.0 - l } * s;
        let m1 = 2.0 * l - m2;
        let channel = |h: f64| {
            let v = if h < 60.0 {
                m1 + (m2 - m1) * h / 60.0
            } else if h < 180.0 {
                m2
            } else if h < 240.0 {
                m1 + (m2 - m1) * (240.0 - h) / 60.0
            } else {
                m1
            };
            v * 255.0
        };
        Self {
            r: channel(if h >= 240.0 { h - 240.0 } else { h + 120.0 }),
            g: channel(h),
            b: channel(if h < 120.0 { h + 240.0 } else { h - 120.0 }),
            opacity: o.opacity,
        }
    }
}

/// D50 white, as d3-color's CIELAB takes it.
const XN: f64 = 0.96422;
const YN: f64 = 1.0;
const ZN: f64 = 0.82521;
const T0: f64 = 4.0 / 29.0;
const T1: f64 = 6.0 / 29.0;
const T2: f64 = 3.0 * T1 * T1;
const T3: f64 = T1 * T1 * T1;

/// A colour in CIELAB, under D50 white.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
    pub opacity: f64,
}

fn xyz_to_lab(t: f64) -> f64 {
    match t > T3 {
        true => math::pow(t, 1.0 / 3.0),
        false => t / T2 + T0,
    }
}

fn lab_to_xyz(t: f64) -> f64 {
    match t > T1 {
        true => t * t * t,
        false => T2 * (t - T0),
    }
}

fn linear_to_rgb(x: f64) -> f64 {
    255.0
        * match x <= 0.0031308 {
            true => 12.92 * x,
            false => 1.055 * math::pow(x, 1.0 / 2.4) - 0.055,
        }
}

fn rgb_to_linear(x: f64) -> f64 {
    let x = x / 255.0;
    match x <= 0.04045 {
        true => x / 12.92,
        false => math::pow((x + 0.055) / 1.055, 2.4),
    }
}

impl From<Rgb> for Lab {
    fn from(o: Rgb) -> Self {
        let (r, g, b) = (rgb_to_linear(o.r), rgb_to_linear(o.g), rgb_to_linear(o.b));
        let y = xyz_to_lab((0.2225045 * r + 0.7168786 * g + 0.0606169 * b) / YN);
        let (x, z) = match r == g && g == b {
            true => (y, y),
            false => (
                xyz_to_lab((0.4360747 * r + 0.3850649 * g + 0.1430804 * b) / XN),
                xyz_to_lab((0.0139322 * r + 0.0971045 * g + 0.7141733 * b) / ZN),
            ),
        };
        Self {
            l: 116.0 * y - 16.0,
            a: 500.0 * (x - y),
            b: 200.0 * (y - z),
            opacity: o.opacity,
        }
    }
}

impl From<Lab> for Rgb {
    fn from(o: Lab) -> Self {
        let y = (o.l + 16.0) / 116.0;
        let x = if o.a.is_nan() { y } else { y + o.a / 500.0 };
        let z = if o.b.is_nan() { y } else { y - o.b / 200.0 };
        let (x, y, z) = (XN * lab_to_xyz(x), YN * lab_to_xyz(y), ZN * lab_to_xyz(z));
        Self {
            r: linear_to_rgb(3.1338561 * x - 1.6168667 * y - 0.4906146 * z),
            g: linear_to_rgb(-0.9787684 * x + 1.9161415 * y + 0.0334540 * z),
            b: linear_to_rgb(0.0719453 * x - 0.2289914 * y + 1.4052427 * z),
            opacity: o.opacity,
        }
    }
}

/// A colour in CIELCh, hue in degrees. `NaN` hue for a grey.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hcl {
    pub h: f64,
    pub c: f64,
    pub l: f64,
    pub opacity: f64,
}

impl From<Lab> for Hcl {
    fn from(o: Lab) -> Self {
        if o.a == 0.0 && o.b == 0.0 {
            let c = if 0.0 < o.l && o.l < 100.0 {
                0.0
            } else {
                f64::NAN
            };
            return Self {
                h: f64::NAN,
                c,
                l: o.l,
                opacity: o.opacity,
            };
        }
        let h = o.b.atan2(o.a) * DEGREES;
        Self {
            h: if h < 0.0 { h + 360.0 } else { h },
            c: (o.a * o.a + o.b * o.b).sqrt(),
            l: o.l,
            opacity: o.opacity,
        }
    }
}

impl From<Hcl> for Lab {
    fn from(o: Hcl) -> Self {
        if o.h.is_nan() {
            return Self {
                l: o.l,
                a: 0.0,
                b: 0.0,
                opacity: o.opacity,
            };
        }
        let h = o.h * RADIANS;
        Self {
            l: o.l,
            a: h.cos() * o.c,
            b: h.sin() * o.c,
            opacity: o.opacity,
        }
    }
}

impl From<Rgb> for Hcl {
    fn from(o: Rgb) -> Self {
        Lab::from(o).into()
    }
}

impl From<Hcl> for Rgb {
    fn from(o: Hcl) -> Self {
        Lab::from(o).into()
    }
}

const A: f64 = -0.14861;
const B: f64 = 1.78277;
const C: f64 = -0.29227;
const D: f64 = -0.90649;
const E: f64 = 1.97294;
const ED: f64 = E * D;
const EB: f64 = E * B;
const BC_DA: f64 = B * C - D * A;

/// A colour in Green's Cubehelix, hue in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cubehelix {
    pub h: f64,
    pub s: f64,
    pub l: f64,
    pub opacity: f64,
}

impl From<Rgb> for Cubehelix {
    fn from(o: Rgb) -> Self {
        let (r, g, b) = (o.r / 255.0, o.g / 255.0, o.b / 255.0);
        let l = (BC_DA * b + ED * r - EB * g) / (BC_DA + ED - EB);
        let bl = b - l;
        let k = (E * (g - l) - C * bl) / D;
        let s = (k * k + bl * bl).sqrt() / (E * l * (1.0 - l));
        let h = match s != 0.0 && !s.is_nan() {
            true => k.atan2(bl) * DEGREES - 120.0,
            false => f64::NAN,
        };
        Self {
            h: if h < 0.0 { h + 360.0 } else { h },
            s,
            l,
            opacity: o.opacity,
        }
    }
}

impl From<Cubehelix> for Rgb {
    fn from(o: Cubehelix) -> Self {
        let h = if o.h.is_nan() {
            0.0
        } else {
            (o.h + 120.0) * RADIANS
        };
        let l = o.l;
        let a = if o.s.is_nan() {
            0.0
        } else {
            o.s * l * (1.0 - l)
        };
        let (cosh, sinh) = (h.cos(), h.sin());
        Self {
            r: 255.0 * (l + a * (A * cosh + B * sinh)),
            g: 255.0 * (l + a * (C * cosh + D * sinh)),
            b: 255.0 * (l + a * (E * cosh)),
            opacity: o.opacity,
        }
    }
}
