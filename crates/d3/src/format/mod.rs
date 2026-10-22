//! d3-format's number formatting with its en-US locale.

use crate::{array, format::number::Number, string};
use std::{error, fmt, str::FromStr};

pub mod number;

/// The SI prefixes from 1e-24 to 1e24, a step of 1e3 apart.
const PREFIXES: [&str; 17] = [
    "y", "z", "a", "f", "p", "n", "µ", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y",
];
/// en-US's minus sign.
const MINUS: &str = "\u{2212}";
const CURRENCY: &str = "$";

/// Where padding goes: `<`, `>`, `^`, or `=` between the sign and the digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
    AfterSign,
}

/// What a sign is written as: `-` only for a negative number, `+` for either
/// sign, `(` for parentheses around a negative number, ` ` for a space before
/// a positive one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Minus,
    Plus,
    Parentheses,
    Space,
}

/// `$` for the currency symbol, `#` for `0b`, `0o` and `0x` before binary,
/// octal and hexadecimal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    None,
    Currency,
    Base,
}

/// d3's `formatSpecifier`: `[[fill]align][sign][symbol][0][width][,][.precision][~][type]`.
/// `Display` writes it as d3's `toString` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Specifier {
    pub fill: char,
    pub align: Align,
    pub sign: Sign,
    pub symbol: Symbol,
    pub zero: bool,
    pub width: Option<usize>,
    pub comma: bool,
    pub precision: Option<usize>,
    pub trim: bool,
    /// The type, any ASCII letter or `%`.
    pub kind: Option<char>,
}

impl Default for Specifier {
    fn default() -> Self {
        Self {
            fill: ' ',
            align: Align::Right,
            sign: Sign::Minus,
            symbol: Symbol::None,
            zero: false,
            width: None,
            comma: false,
            precision: None,
            trim: false,
            kind: None,
        }
    }
}

/// A specifier d3 throws on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidSpecifier(pub String);

impl fmt::Display for InvalidSpecifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid format: {}", self.0)
    }
}

impl error::Error for InvalidSpecifier {}

fn align(c: char) -> Option<Align> {
    match c {
        '<' => Some(Align::Left),
        '>' => Some(Align::Right),
        '^' => Some(Align::Center),
        '=' => Some(Align::AfterSign),
        _ => None,
    }
}

/// A run of ASCII digits at the start of `text` as a number, saturating, and
/// the rest of `text`.
fn digits(text: &str) -> (Option<usize>, &str) {
    let end = text.bytes().take_while(u8::is_ascii_digit).count();
    if end == 0 {
        return (None, text);
    }
    let value = text[..end].bytes().fold(0usize, |n, b| {
        n.saturating_mul(10).saturating_add(usize::from(b - b'0'))
    });
    (Some(value), &text[end..])
}

/// Whether `text` starts with `c`, which is then taken off it.
fn take(text: &mut &str, c: char) -> bool {
    text.strip_prefix(c).inspect(|rest| *text = rest).is_some()
}

impl Specifier {
    /// Everything after `[[fill]align]`, or `None` where the rest of the
    /// grammar does not match.
    fn rest(mut self, mut text: &str) -> Option<Self> {
        let sign = [
            ('+', Sign::Plus),
            ('-', Sign::Minus),
            ('(', Sign::Parentheses),
            (' ', Sign::Space),
        ];
        if let Some(&(_, sign)) = sign.iter().find(|(c, _)| take(&mut text, *c)) {
            self.sign = sign;
        }
        if take(&mut text, '$') {
            self.symbol = Symbol::Currency;
        } else if take(&mut text, '#') {
            self.symbol = Symbol::Base;
        }
        self.zero = take(&mut text, '0');
        (self.width, text) = digits(text);
        self.comma = take(&mut text, ',');
        if take(&mut text, '.') {
            let precision;
            (precision, text) = digits(text);
            self.precision = Some(precision?);
        }
        self.trim = take(&mut text, '~');
        let mut chars = text.chars();
        self.kind = chars.next();
        let kind = self
            .kind
            .is_none_or(|c| c.is_ascii_alphabetic() || c == '%');
        (kind && chars.next().is_none()).then_some(self)
    }
}

impl FromStr for Specifier {
    type Err = InvalidSpecifier;

    fn from_str(specifier: &str) -> Result<Self, Self::Err> {
        let mut chars = specifier.chars();
        let first = chars.next();
        let second = chars.next();
        let base = Self::default();
        // A fill is one UTF-16 unit, not a line terminator.
        let fill = first.filter(|&c| c.len_utf16() == 1 && !string::is_line_end(c));
        let filled = fill.zip(second.and_then(align)).and_then(|(fill, align)| {
            let rest = &specifier[fill.len_utf8() + 1..];
            Self {
                fill,
                align,
                ..base
            }
            .rest(rest)
        });
        let aligned = || {
            first
                .and_then(align)
                .and_then(|align| Self { align, ..base }.rest(&specifier[1..]))
        };
        filled
            .or_else(aligned)
            .or_else(|| base.rest(specifier))
            .ok_or_else(|| InvalidSpecifier(specifier.to_string()))
    }
}

impl fmt::Display for Specifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let align = match self.align {
            Align::Left => '<',
            Align::Right => '>',
            Align::Center => '^',
            Align::AfterSign => '=',
        };
        let sign = match self.sign {
            Sign::Minus => '-',
            Sign::Plus => '+',
            Sign::Parentheses => '(',
            Sign::Space => ' ',
        };
        let symbol = match self.symbol {
            Symbol::None => "",
            Symbol::Currency => "$",
            Symbol::Base => "#",
        };
        write!(f, "{}{align}{sign}{symbol}", self.fill)?;
        if self.zero {
            f.write_str("0")?;
        }
        if let Some(width) = self.width {
            write!(f, "{}", number::to_int32(width as f64).max(1))?;
        }
        if self.comma {
            f.write_str(",")?;
        }
        if let Some(precision) = self.precision {
            write!(f, ".{}", number::to_int32(precision as f64).max(0))?;
        }
        if self.trim {
            f.write_str("~")?;
        }
        if let Some(kind) = self.kind {
            write!(f, "{kind}")?;
        }
        Ok(())
    }
}

/// d3's `format(specifier)`: formats numbers as the specifier says.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    /// The specifier as `newFormat` adjusts it.
    specifier: Specifier,
    kind: char,
    precision: usize,
    prefix: String,
    suffix: String,
    /// What a number is multiplied by before it is formatted.
    scale: f64,
}

impl From<Specifier> for NumberFormat {
    fn from(specifier: Specifier) -> Self {
        Self::new(specifier, "", 1.0)
    }
}

impl FromStr for NumberFormat {
    type Err = InvalidSpecifier;

    fn from_str(specifier: &str) -> Result<Self, Self::Err> {
        specifier.parse::<Specifier>().map(Self::from)
    }
}

impl NumberFormat {
    /// d3's `newFormat`, with `si` after the suffix and numbers multiplied by
    /// `scale`.
    fn new(mut specifier: Specifier, si: &str, scale: f64) -> Self {
        let mut kind = specifier.kind.unwrap_or_default();
        if kind == 'n' {
            specifier.comma = true;
            kind = 'g';
        } else if !"%bcdefgoprsXx".contains(kind) {
            specifier.precision.get_or_insert(12);
            specifier.trim = true;
            kind = 'g';
        }
        if specifier.zero || (specifier.fill == '0' && specifier.align == Align::AfterSign) {
            specifier.zero = true;
            specifier.fill = '0';
            specifier.align = Align::AfterSign;
        }
        let prefix = match specifier.symbol {
            Symbol::Currency => CURRENCY.to_string(),
            Symbol::Base if "boxX".contains(kind) => format!("0{}", kind.to_ascii_lowercase()),
            _ => String::new(),
        };
        let suffix = match specifier.symbol {
            Symbol::Currency => String::new(),
            _ if "%p".contains(kind) => "%".to_string(),
            _ => String::new(),
        } + si;
        let precision = match specifier.precision {
            None => 6,
            Some(p) if "gprs".contains(kind) => p.clamp(1, 21),
            Some(p) => p.min(20),
        };
        Self {
            specifier,
            kind,
            precision,
            prefix,
            suffix,
            scale,
        }
    }

    /// d3's `formatPrefix(specifier, value)`: fixed-point, scaled to the SI
    /// prefix of `value`, which every number it formats takes.
    pub fn prefix(mut specifier: Specifier, value: f64) -> Self {
        specifier.kind = Some('f');
        match exponent(value) {
            Some(e) => {
                let e = (e.div_euclid(3)).clamp(-8, 8) * 3;
                let si = PREFIXES[(8 + e / 3) as usize];
                Self::new(specifier, si, 10f64.powf(f64::from(-e)))
            }
            None => Self::new(specifier, "", f64::NAN),
        }
    }

    /// vega-format's `formatSpan(start, stop, count, specifier)`: `specifier`,
    /// `,f` without one, at the precision the ticks `d3.tickStep` takes
    /// from `start` to `stop` need where it gives none.
    pub fn span(start: f64, stop: f64, count: f64, specifier: Option<Specifier>) -> Self {
        let mut specifier = specifier.unwrap_or(Specifier {
            comma: true,
            kind: Some('f'),
            ..Specifier::default()
        });
        let step = array::tick_step(start, stop, count);
        let value = start.abs().max(stop.abs());
        if specifier.precision.is_none() {
            match specifier.kind {
                Some('s') => {
                    specifier.precision = precision_prefix(step, value);
                    return Self::prefix(specifier, value);
                }
                None | Some('e' | 'g' | 'p' | 'r') => {
                    let less = usize::from(specifier.kind == Some('e'));
                    specifier.precision =
                        precision_round(step, value).map(|p| p.saturating_sub(less));
                }
                Some('f' | '%') => {
                    let less = 2 * usize::from(specifier.kind == Some('%'));
                    specifier.precision = precision_fixed(step).map(|p| p.saturating_sub(less));
                }
                _ => {}
            }
        }
        specifier.into()
    }

    /// vega-format's `formatFloat(specifier)`: `specifier`, `,` without one,
    /// to 12 significant digits with trailing zeros dropped where it gives
    /// no precision.
    pub fn float(specifier: Option<Specifier>) -> Self {
        let mut specifier = specifier.unwrap_or(Specifier {
            comma: true,
            ..Specifier::default()
        });
        if specifier.precision.is_none() {
            specifier.precision = Some(match specifier.kind {
                Some('%') => 10,
                Some('e') => 11,
                _ => 12,
            });
            specifier.trim = true;
        }
        specifier.into()
    }

    /// The number as text.
    pub fn format(&self, value: f64) -> String {
        let s = &self.specifier;
        let value = self.scale * value;
        let mut prefix = self.prefix.clone();
        let mut suffix = self.suffix.clone();
        let mut digits;
        if self.kind == 'c' {
            suffix = Number(value).to_string() + &suffix;
            digits = String::new();
        } else {
            let mut negative = value < 0.0 || (value == 0.0 && value.is_sign_negative());
            let mut si = None;
            digits = if value.is_nan() {
                "NaN".to_string()
            } else {
                let (text, e) = self.format_type(value.abs());
                si = e;
                text
            };
            if s.trim {
                digits = trim(&digits);
            }
            // A negative number that rounds to zero loses its sign.
            if negative && Number::from(digits.as_str()).0 == 0.0 && s.sign != Sign::Plus {
                negative = false;
            }
            let sign = match (negative, s.sign) {
                (true, Sign::Parentheses) => "(",
                (true, _) => MINUS,
                (false, Sign::Plus) => "+",
                (false, Sign::Space) => " ",
                (false, _) => "",
            };
            prefix = format!("{sign}{prefix}");
            let si = si.map_or("", |e| PREFIXES[(8 + e / 3) as usize]);
            let close = if negative && s.sign == Sign::Parentheses {
                ")"
            } else {
                ""
            };
            suffix = format!("{si}{suffix}{close}");
            if "defgprs%".contains(self.kind)
                && let Some(i) = digits.find(|c: char| !c.is_ascii_digit())
            {
                suffix = format!("{}{suffix}", &digits[i..]);
                digits.truncate(i);
            }
        }
        if s.comma && !s.zero {
            digits = group(&digits, i64::MAX);
        }
        let width = s.width.map_or(0, |w| w as i64);
        let length = (string::len(&prefix) + string::len(&digits) + string::len(&suffix)) as i64;
        let mut padding = if length < width {
            s.fill.to_string().repeat((width - length) as usize)
        } else {
            String::new()
        };
        if s.comma && s.zero {
            let limit = if padding.is_empty() {
                i64::MAX
            } else {
                width - string::len(&suffix) as i64
            };
            digits = group(&(padding + &digits), limit);
            padding = String::new();
        }
        match s.align {
            Align::Left => format!("{prefix}{digits}{suffix}{padding}"),
            Align::AfterSign => format!("{prefix}{padding}{digits}{suffix}"),
            Align::Center => {
                let half = padding.chars().count() / 2;
                let split = padding
                    .char_indices()
                    .nth(half)
                    .map_or(padding.len(), |(i, _)| i);
                let (before, after) = padding.split_at(split);
                format!("{before}{prefix}{digits}{suffix}{after}")
            }
            Align::Right => format!("{padding}{prefix}{digits}{suffix}"),
        }
    }

    /// d3's `formatTypes[type](x, precision)` for a number not negative and
    /// not `NaN`, and the exponent of the SI prefix type `s` chose.
    fn format_type(&self, x: f64) -> (String, Option<i32>) {
        let p = self.precision;
        let text = match self.kind {
            '%' => Number(x * 100.0).to_fixed(p),
            'b' => radix(number::round(x), 1),
            'd' => decimal(x),
            'e' => Number(x).to_exponential(p),
            'f' => Number(x).to_fixed(p),
            'o' => radix(number::round(x), 3),
            'p' => rounded(x * 100.0, p),
            'r' => rounded(x, p),
            's' => return prefix_auto(x, p),
            'X' => radix(number::round(x), 4).to_ascii_uppercase(),
            'x' => radix(number::round(x), 4),
            _ => Number(x).to_precision(p),
        };
        (text, None)
    }
}

/// d3's `formatDecimalParts`: the significant digits of `x`, rounded to `p`
/// of them or as few as read back as `x` where `p` is 0, and the exponent of
/// the first. `None` for `0`, `NaN` and the infinities.
fn decimal_parts(x: f64, p: usize) -> Option<(String, i32)> {
    if !x.is_finite() || x == 0.0 {
        return None;
    }
    // Rust's `{:e}` writes the shortest digits, as `toExponential()` does.
    let text = if p > 0 {
        Number(x).to_exponential(p - 1)
    } else {
        format!("{x:e}")
    };
    let (mantissa, exponent) = text.split_once('e')?;
    let digits = mantissa.chars().filter(char::is_ascii_digit).collect();
    Some((digits, exponent.parse().ok()?))
}

/// The decimal exponent of `x`'s first significant digit.
fn exponent(x: f64) -> Option<i32> {
    decimal_parts(x.abs(), 0).map(|(_, e)| e)
}

/// d3's `formatDecimal`: `x` rounded to an integer and written out in full.
fn decimal(x: f64) -> String {
    let x = number::round(x);
    if x.abs() < 1e21 {
        return Number(x).to_string();
    }
    // `toLocaleString("en")` without its separators.
    if x.is_infinite() {
        return "∞".to_string();
    }
    match decimal_parts(x, 0) {
        Some((digits, e)) => {
            let zeros = (e as usize + 1).saturating_sub(digits.len());
            format!("{digits}{}", "0".repeat(zeros))
        }
        None => Number(x).to_string(),
    }
}

/// d3's `formatRounded`: `x` rounded to `p` significant digits, in fixed
/// point.
fn rounded(x: f64, p: usize) -> String {
    let Some((digits, e)) = decimal_parts(x, p) else {
        return Number(x).to_string();
    };
    if e < 0 {
        format!("0.{}{digits}", "0".repeat((-e - 1) as usize))
    } else if digits.len() > e as usize + 1 {
        let (int, frac) = digits.split_at(e as usize + 1);
        format!("{int}.{frac}")
    } else {
        format!("{digits}{}", "0".repeat(e as usize + 1 - digits.len()))
    }
}

/// d3's `formatPrefixAuto`: `x` to `p` significant digits, scaled to an SI
/// prefix, and that prefix's exponent.
fn prefix_auto(x: f64, p: usize) -> (String, Option<i32>) {
    let Some((digits, e)) = decimal_parts(x, p) else {
        return (Number(x).to_precision(p), None);
    };
    let si = e.div_euclid(3).clamp(-8, 8) * 3;
    let i = e - si + 1;
    let n = digits.len() as i32;
    let text = if i == n {
        digits
    } else if i > n {
        format!("{digits}{}", "0".repeat((i - n) as usize))
    } else if i > 0 {
        let (int, frac) = digits.split_at(i as usize);
        format!("{int}.{frac}")
    } else {
        let p = (p as i32 + i - 1).max(0) as usize;
        let rest = decimal_parts(x, p).map(|(d, _)| d).unwrap_or_default();
        format!("0.{}{rest}", "0".repeat((-i) as usize))
    };
    (text, Some(si))
}

/// d3's `formatTrim`: insignificant trailing zeros removed, `1.2000k` to
/// `1.2k`.
fn trim(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let (mut i0, mut i1) = (-1i64, 0usize);
    for (i, &c) in chars.iter().enumerate().skip(1) {
        match c {
            '.' => {
                i0 = i as i64;
                i1 = i;
            }
            '0' => {
                if i0 == 0 {
                    i0 = i as i64;
                }
                i1 = i;
            }
            _ => {
                if !c.is_ascii_digit() {
                    break;
                }
                if i0 > 0 {
                    i0 = 0;
                }
            }
        }
    }
    if i0 > 0 {
        chars[..i0 as usize]
            .iter()
            .chain(&chars[i1 + 1..])
            .collect()
    } else {
        s.to_string()
    }
}

/// d3's `formatGroup` with groups of three and `,`: as many groups from the
/// right as fit in `width`.
fn group(value: &str, width: i64) -> String {
    let chars: Vec<char> = value.chars().collect();
    let mut i = chars.len() as i64;
    let mut groups = Vec::new();
    let mut g = 3i64;
    let mut length = 0i64;
    while i > 0 && g > 0 {
        if length.saturating_add(g + 1) > width {
            g = (width - length).max(1);
        }
        let start = (i - g).max(0) as usize;
        groups.push(chars[start..i as usize].iter().collect::<String>());
        i -= g;
        length += g + 1;
        if length > width {
            break;
        }
        g = 3;
    }
    groups.reverse();
    groups.join(",")
}

/// `x.toString(2 ^ bits)` for `x` an integer not negative, or infinite.
fn radix(x: f64, bits: usize) -> String {
    if x.is_infinite() {
        return Number(x).to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    let raw = x.to_bits();
    let exponent = ((raw >> 52) & 0x7ff) as i64;
    let fraction = raw & ((1 << 52) - 1);
    let (mantissa, shift) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    };
    let mut binary: Vec<u8> = if shift >= 0 {
        let mut b = format!("{mantissa:b}").into_bytes();
        b.extend(std::iter::repeat_n(b'0', shift as usize));
        b
    } else {
        format!("{:b}", mantissa >> -shift).into_bytes()
    };
    let pad = (bits - binary.len() % bits) % bits;
    binary.splice(0..0, std::iter::repeat_n(b'0', pad));
    binary
        .chunks(bits)
        .map(|chunk| {
            let digit = chunk.iter().fold(0, |n, b| n * 2 + u32::from(b - b'0'));
            char::from_digit(digit, 16).unwrap_or('0')
        })
        .collect()
}

/// d3's `precisionFixed(step)`: the precision `f` needs for multiples of
/// `step`. `None` where d3 returns `NaN`.
pub fn precision_fixed(step: f64) -> Option<usize> {
    exponent(step.abs()).map(|e| (-e).max(0) as usize)
}

/// d3's `precisionPrefix(step, value)`: the precision
/// [`NumberFormat::prefix`] with `value` needs for multiples of `step`.
pub fn precision_prefix(step: f64, value: f64) -> Option<usize> {
    let e = exponent(value)?;
    let step = exponent(step.abs())?;
    Some((e.div_euclid(3).clamp(-8, 8) * 3 - step).max(0) as usize)
}

/// d3's `precisionRound(step, max)`: the precision `r` needs for multiples
/// of `step` up to `max`.
pub fn precision_round(step: f64, max: f64) -> Option<usize> {
    let step = step.abs();
    let max = max.abs() - step;
    Some((exponent(max)? - exponent(step)?).max(0) as usize + 1)
}
