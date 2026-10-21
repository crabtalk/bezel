//! d3-color's `color(specifier)`, as RGB: a CSS colour named, in hex, or in
//! `rgb()`, `rgba()`, `hsl()` or `hsla()` notation.

use crate::string;
use std::str::FromStr;

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

    /// d3-color's `hsl(h, s, l, opacity).rgb()`, `s` and `l` in `0..=1`.
    fn hsla(h: f64, s: f64, l: f64, opacity: f64) -> Self {
        let (h, s, l) = if opacity <= 0.0 {
            (f64::NAN, f64::NAN, f64::NAN)
        } else if l <= 0.0 || l >= 1.0 {
            (f64::NAN, f64::NAN, l)
        } else if s <= 0.0 {
            (f64::NAN, s, l)
        } else {
            (h, s, l)
        };
        let h = h % 360.0 + if h < 0.0 { 360.0 } else { 0.0 };
        let s = if h.is_nan() || s.is_nan() { 0.0 } else { s };
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
            opacity,
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
                    Ok(Self::hsla(h, s / 100.0, l / 100.0, opacity))
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
