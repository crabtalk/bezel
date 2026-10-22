//! JavaScript numbers: how they print and how text reads as one.

use crate::string;
use std::{cmp::Ordering, fmt};

/// A JavaScript number. `Display` writes it as `String(x)` does.
#[derive(Clone, Copy, Debug)]
pub struct Number(pub f64);

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = self.0;
        if x.is_nan() {
            return f.write_str("NaN");
        }
        if x.is_infinite() {
            return f.write_str(if x > 0.0 { "Infinity" } else { "-Infinity" });
        }
        if x == 0.0 {
            return f.write_str("0");
        }
        let abs = x.abs();
        if abs < 9_007_199_254_740_992.0 && x.fract() == 0.0 {
            return write!(f, "{}", x as i64);
        }
        // In this range JavaScript writes the shortest digits without an
        // exponent, as Rust's `Display` does.
        if (1e-6..1e21).contains(&abs) {
            return write!(f, "{x}");
        }
        f.write_str(&scientific(x))
    }
}

/// `Number(text)`.
impl From<&str> for Number {
    fn from(text: &str) -> Self {
        let text = string::trim(text);
        if text.is_empty() {
            return Self(0.0);
        }
        let radix = match text.get(..2) {
            Some("0x" | "0X") => Some(16),
            Some("0o" | "0O") => Some(8),
            Some("0b" | "0B") => Some(2),
            _ => None,
        };
        if let Some(radix) = radix {
            let digits = &text[2..];
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return Self(f64::NAN);
            }
            return Self(digits.chars().fold(0.0, |acc, c| {
                acc * radix as f64 + c.to_digit(radix).unwrap_or(0) as f64
            }));
        }
        let len = decimal_prefix(text);
        if len != text.len() {
            return Self(f64::NAN);
        }
        Self(read_decimal(text))
    }
}

impl Number {
    /// `x.toFixed(digits)`.
    pub fn to_fixed(self, digits: usize) -> String {
        let x = self.0;
        if x.is_finite() && x.abs() < 1e21 && !halfway(x, digits as i64) {
            // Away from a tie, Rust's exact formatting rounds as JavaScript does.
            let body = format!("{:.*}", digits, x.abs());
            return if x < 0.0 { format!("-{body}") } else { body };
        }
        if !x.is_finite() || x.abs() >= 1e21 {
            return self.to_string();
        }
        let (int, frac) = exact_digits(x);
        let mut all = int.clone().into_bytes();
        all.extend(frac.bytes());
        let keep = int.len() + digits;
        let (rounded, carried) = round_digits(&all, keep);
        let int_len = int.len() + usize::from(carried);
        let text = String::from_utf8(rounded).unwrap_or_default();
        let (int, frac) = text.split_at(int_len);
        let int = int.trim_start_matches('0');
        let int = if int.is_empty() { "0" } else { int };
        let body = if digits == 0 {
            int.to_string()
        } else {
            format!("{int}.{frac}")
        };
        // A negative number rounded to zero keeps its sign.
        if x < 0.0 { format!("-{body}") } else { body }
    }

    /// `x.toExponential(digits)`.
    pub fn to_exponential(self, digits: usize) -> String {
        let x = self.0;
        if !x.is_finite() {
            return self.to_string();
        }
        let (mantissa, exponent) = if x == 0.0 {
            (vec![b'0'; digits + 1], 0)
        } else {
            rounded_significant(x, digits + 1)
        };
        let mantissa = String::from_utf8(mantissa).unwrap_or_default();
        let body = if digits == 0 {
            mantissa
        } else {
            format!("{}.{}", &mantissa[..1], &mantissa[1..])
        };
        let sign = if x < 0.0 { "-" } else { "" };
        let e = if exponent >= 0 {
            format!("+{exponent}")
        } else {
            exponent.to_string()
        };
        format!("{sign}{body}e{e}")
    }

    /// `x.toPrecision(digits)`, `digits` from 1.
    pub fn to_precision(self, digits: usize) -> String {
        let x = self.0;
        if !x.is_finite() {
            return self.to_string();
        }
        let (mantissa, exponent) = if x == 0.0 {
            (vec![b'0'; digits], 0)
        } else {
            rounded_significant(x, digits)
        };
        let m = String::from_utf8(mantissa).unwrap_or_default();
        let p = digits as i64;
        let body = if exponent < -6 || exponent >= p {
            let e = if exponent >= 0 {
                format!("+{exponent}")
            } else {
                exponent.to_string()
            };
            if digits == 1 {
                format!("{m}e{e}")
            } else {
                format!("{}.{}e{e}", &m[..1], &m[1..])
            }
        } else if exponent == p - 1 {
            m
        } else if exponent >= 0 {
            let point = exponent as usize + 1;
            format!("{}.{}", &m[..point], &m[point..])
        } else {
            format!("0.{}{m}", "0".repeat((-exponent - 1) as usize))
        };
        if x < 0.0 { format!("-{body}") } else { body }
    }

    /// `Number(x.toPrecision(digits))`: `x` rounded to `digits` significant
    /// digits. `0`, `-0`, `NaN` and the infinities are kept as they are.
    pub fn round_significant(self, digits: usize) -> Self {
        let x = self.0;
        if !x.is_finite() || x == 0.0 {
            return self;
        }
        let (rounded, exponent) = rounded_significant(x, digits);
        let text = format!(
            "{}{}e{}",
            if x < 0.0 { "-" } else { "" },
            String::from_utf8(rounded).unwrap_or_default(),
            exponent - digits as i64 + 1
        );
        Self(text.parse().unwrap_or(f64::NAN))
    }
}

fn scientific(x: f64) -> String {
    let sign = if x < 0.0 { "-" } else { "" };
    // Rust's `{:e}` writes the shortest digits that read back as `x`, which
    // is what JavaScript picks too; only the layout differs.
    let sci = format!("{:e}", x.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exponent.parse::<i32>().unwrap_or(0) + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let e = if e >= 0 {
            format!("+{e}")
        } else {
            e.to_string()
        };
        if k == 1 {
            format!("{digits}e{e}")
        } else {
            format!("{}.{}e{e}", &digits[..1], &digits[1..])
        }
    };
    format!("{sign}{body}")
}

/// The length of the longest decimal literal at the start of `text`:
/// sign, digits, a point, an exponent, or a signed `Infinity`.
pub fn decimal_prefix(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i = 1;
    }
    if text[i..].starts_with("Infinity") {
        return i + "Infinity".len();
    }
    let digits_from = i;
    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    let mut seen = i > digits_from;
    if bytes.get(i) == Some(&b'.') {
        let after = i + 1;
        let mut j = after;
        while bytes.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        if seen || j > after {
            i = j;
            seen = true;
        }
    }
    if !seen {
        return 0;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(bytes.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let from = j;
        while bytes.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        if j > from {
            i = j;
        }
    }
    i
}

fn read_decimal(literal: &str) -> f64 {
    let unsigned = literal.trim_start_matches(['+', '-']);
    let negative = literal.starts_with('-');
    let value = if unsigned == "Infinity" {
        f64::INFINITY
    } else {
        let unsigned = if unsigned.starts_with('.') {
            format!("0{unsigned}")
        } else {
            unsigned.to_string()
        };
        let unsigned = unsigned
            .strip_suffix('.')
            .map_or(unsigned.clone(), str::to_string);
        unsigned.parse::<f64>().unwrap_or(f64::NAN)
    };
    if negative { -value } else { value }
}

/// `parseFloat(text)`.
pub fn parse_float(text: &str) -> f64 {
    let text = string::trim_start(text);
    let len = decimal_prefix(text);
    if len == 0 {
        return f64::NAN;
    }
    read_decimal(&text[..len])
}

/// `parseInt(text, radix)`, `radix` after `ToInt32`: `0` reads base 10, or
/// 16 after `0x`.
pub fn parse_int(text: &str, radix: i32) -> f64 {
    let text = string::trim_start(text);
    let (negative, rest) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if radix != 0 && !(2..=36).contains(&radix) {
        return f64::NAN;
    }
    let hex = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X"));
    let (rest, radix) = match hex {
        Some(hex) if radix == 0 || radix == 16 => (hex, 16),
        _ if radix == 0 => (rest, 10),
        _ => (rest, radix as u32),
    };
    let mut value = 0.0;
    let mut any = false;
    for c in rest.chars() {
        let Some(d) = c.to_digit(radix) else { break };
        value = value * radix as f64 + d as f64;
        any = true;
    }
    if !any {
        return f64::NAN;
    }
    if negative { -value } else { value }
}

/// A number's truth in JavaScript: anything but `0` and `NaN`.
pub fn truthy(x: f64) -> bool {
    x != 0.0 && !x.is_nan()
}

/// `x || y` for numbers.
pub fn or(x: f64, y: f64) -> f64 {
    if truthy(x) { x } else { y }
}

/// `x | 0`: ECMAScript's `ToInt32`, `0` for `NaN` and the infinities.
pub fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4294967296.0) as u32 as i32
}

/// `ToIntegerOrInfinity`: `NaN` is `0`, the rest truncate.
pub fn to_integer(x: f64) -> f64 {
    if x.is_nan() { 0.0 } else { x.trunc() + 0.0 }
}

/// What `Array.prototype.sort` makes of a comparator's number: its sign,
/// with `NaN` as equal.
pub fn compare(x: f64) -> Ordering {
    x.partial_cmp(&0.0).unwrap_or(Ordering::Equal)
}

/// The exact decimal digits of `x.abs()`: the integer part and the fraction.
fn exact_digits(x: f64) -> (String, String) {
    // A double's fraction has at most 1074 binary digits, so as many decimal
    // digits write it exactly.
    let text = format!("{:.1074}", x.abs());
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    (int.to_string(), frac.trim_end_matches('0').to_string())
}

/// `digits` rounded to its first `keep` digits, a half going up, as a
/// string of `keep` digits and whether it carried into a new leading digit.
fn round_digits(digits: &[u8], keep: usize) -> (Vec<u8>, bool) {
    let mut head: Vec<u8> = (0..keep)
        .map(|i| digits.get(i).copied().unwrap_or(b'0'))
        .collect();
    if digits.get(keep).is_some_and(|&d| d >= b'5') {
        let mut i = keep;
        loop {
            if i == 0 {
                head.insert(0, b'1');
                return (head, true);
            }
            i -= 1;
            if head[i] == b'9' {
                head[i] = b'0';
            } else {
                head[i] += 1;
                break;
            }
        }
    }
    (head, false)
}

/// Whether `x * 10^k` lies exactly halfway between two integers, where
/// JavaScript rounds up and Rust's formatting to even.
fn halfway(x: f64, k: i64) -> bool {
    let bits = x.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1 << 52) - 1);
    let (m, e) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    };
    // x * 10^k = m * 5^k * 2^(e + k), and 5^k is odd: the fraction is
    // one half exactly when m's lowest set bit is the one that halves.
    let shift = -(e + k);
    m != 0 && shift >= 1 && i64::from(m.trailing_zeros()) == shift - 1
}

/// The significant digits of `x.abs()`, not zero, and the decimal exponent of
/// the first.
fn significant(x: f64) -> (Vec<u8>, i64) {
    let (int, frac) = exact_digits(x);
    let int = int.trim_start_matches('0');
    if !int.is_empty() {
        let mut all = int.as_bytes().to_vec();
        all.extend(frac.bytes());
        (all, int.len() as i64 - 1)
    } else {
        let zeros = frac.bytes().take_while(|&b| b == b'0').count();
        (frac.as_bytes()[zeros..].to_vec(), -(zeros as i64) - 1)
    }
}

/// The first `keep` significant digits of `x.abs()`, not zero, rounded,
/// and the decimal exponent of the first. JavaScript rounds a tie up.
fn rounded_significant(x: f64, keep: usize) -> (Vec<u8>, i64) {
    if keep > 0 {
        let text = format!("{:.*e}", keep - 1, x.abs());
        if let Some((mantissa, exponent)) = text.split_once('e')
            && let Ok(exponent) = exponent.parse::<i64>()
        {
            // Rust rounds a tie to even. A tie sits at the digit kept last
            // under this exponent, or under the one before a carry.
            let k = keep as i64 - 1 - exponent;
            if !halfway(x, k) && !halfway(x, k + 1) {
                let digits = mantissa.bytes().filter(|&b| b != b'.').collect();
                return (digits, exponent);
            }
        }
    }
    let (all, exponent) = significant(x);
    let (mut rounded, carried) = round_digits(&all, keep);
    rounded.truncate(keep);
    (rounded, exponent + i64::from(carried))
}

/// `Math.round`: halves go up, `-2.5` to `-2`, and `-0.5` to `-0`.
pub fn round(x: f64) -> f64 {
    let floor = x.floor();
    let rounded = if x - floor >= 0.5 { floor + 1.0 } else { floor };
    if rounded == 0.0 && x < 0.0 {
        -0.0
    } else {
        rounded
    }
}

/// `Math.sign`: `0` for `0`, `NaN` for `NaN`.
pub fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// `Math.hypot(...values)` as V8 works it out: each scaled by the largest
/// and summed with Kahan's compensation, which can differ from the libm
/// `hypot` in the last bit. `Infinity` when any is infinite, even with a
/// `NaN`.
pub fn hypot(values: &[f64]) -> f64 {
    if values.iter().any(|v| v.is_infinite()) {
        return f64::INFINITY;
    }
    if values.iter().any(|v| v.is_nan()) {
        return f64::NAN;
    }
    let max = values.iter().fold(0.0, |max: f64, v| max.max(v.abs()));
    if max == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for value in values {
        let n = value.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// `Math.max` of two: `NaN` when either is, `0` over `-0`.
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b || (a == b && a.is_sign_positive()) {
        a
    } else {
        b
    }
}

/// `Math.min` of two: `NaN` when either is, `-0` under `0`.
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

/// `Math.pow`: `NaN` for a `NaN` exponent, and for `1` or `-1` to an
/// infinite power.
pub fn pow(base: f64, exponent: f64) -> f64 {
    if exponent.is_nan() || (base.abs() == 1.0 && exponent.is_infinite()) {
        f64::NAN
    } else {
        base.powf(exponent)
    }
}
