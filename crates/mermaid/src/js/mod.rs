//! What JavaScript does with numbers and strings, where Rust's own answer
//! differs and a drawing would come out different.

pub(crate) mod date;
mod json;
pub(crate) mod re;

pub use json::{JsValue, number_value, parse_json};

pub(crate) use re::{Lazy, Regex};

use std::fmt;

use crate::util::chars::is_space;

/// `Math.round`: halves go up, `-2.5` to `-2`.
pub(crate) fn round(x: f64) -> f64 {
    let floor = x.floor();
    if x - floor >= 0.5 { floor + 1.0 } else { floor }
}

/// `String(x)` for a number.
pub fn num_str(x: f64) -> String {
    Number(x).to_string()
}

/// [`num_str`] as a value `write!` writes in place.
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

pub(crate) fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

pub(crate) fn trim_start(text: &str) -> &str {
    text.trim_start_matches(is_space)
}

pub(crate) fn trim_end(text: &str) -> &str {
    text.trim_end_matches(is_space)
}

/// `text.length`: UTF-16 units.
pub(crate) fn len(text: &str) -> usize {
    // One unit per character, two for the four-byte ones.
    text.bytes()
        .map(|b| usize::from(b & 0xc0 != 0x80) + usize::from(b >= 0xf0))
        .sum()
}

/// The byte offset in `text` after its first `units` UTF-16 units, at the
/// end of the character holding the last of them; the length of `text` when
/// it is shorter.
pub(crate) fn units_end(text: &str, units: usize) -> usize {
    let mut taken = 0;
    for (at, c) in text.char_indices() {
        if taken >= units {
            return at;
        }
        taken += c.len_utf16();
    }
    text.len()
}

/// `list.sort()`: strings in order of their UTF-16 code units.
pub(crate) fn sort_strings(list: &mut [String]) {
    list.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
}

/// Whether [`len`] of `text` is over `limit`, counted only when its bytes
/// are: no character has fewer UTF-8 bytes than UTF-16 units.
pub(crate) fn len_over(text: &str, limit: usize) -> bool {
    text.len() > limit && len(text) > limit
}

/// The length of the longest decimal literal at the start of `text`:
/// sign, digits, a point, an exponent.
fn decimal_prefix(text: &str) -> usize {
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
pub(crate) fn parse_float(text: &str) -> f64 {
    let text = trim_start(text);
    let len = decimal_prefix(text);
    if len == 0 {
        return f64::NAN;
    }
    read_decimal(&text[..len])
}

/// `Number(text)`.
pub(crate) fn number(text: &str) -> f64 {
    let text = trim(text);
    if text.is_empty() {
        return 0.0;
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
            return f64::NAN;
        }
        return digits.chars().fold(0.0, |acc, c| {
            acc * radix as f64 + c.to_digit(radix).unwrap_or(0) as f64
        });
    }
    let len = decimal_prefix(text);
    if len != text.len() {
        return f64::NAN;
    }
    read_decimal(text)
}

/// `parseInt(text, radix)` for radix 10 or 16.
pub(crate) fn parse_int(text: &str, radix: u32) -> f64 {
    let text = trim_start(text);
    let (negative, rest) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let rest = if radix == 16 {
        rest.strip_prefix("0x")
            .or_else(|| rest.strip_prefix("0X"))
            .unwrap_or(rest)
    } else {
        rest
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
pub(crate) fn truthy(x: f64) -> bool {
    x != 0.0 && !x.is_nan()
}

/// `x || 1` for a number.
pub(crate) fn or_one(x: f64) -> f64 {
    if truthy(x) { x } else { 1.0 }
}

/// `x | 0`: ECMAScript's `ToInt32`, `0` for `NaN` and the infinities.
pub(crate) fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4294967296.0) as u32 as i32
}

/// `Math.sign`: `0` for `0`, `NaN` for `NaN`.
pub(crate) fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// `Math.hypot` of two numbers as V8 works it out: each scaled by the larger
/// and summed with Kahan's compensation, which can differ from the libm
/// `hypot` in the last bit.
pub(crate) fn hypot(a: f64, b: f64) -> f64 {
    if a.is_infinite() || b.is_infinite() {
        return f64::INFINITY;
    }
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    let (a, b) = (a.abs(), b.abs());
    let max = a.max(b);
    if max == 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for value in [a, b] {
        let n = value / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// What `Array.prototype.sort` makes of a comparator's number: its sign,
/// with `NaN` as equal.
pub(crate) fn compare(x: f64) -> std::cmp::Ordering {
    x.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
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

/// `x.toFixed(digits)`.
pub(crate) fn to_fixed(x: f64, digits: usize) -> String {
    if x.is_finite() && x.abs() < 1e21 && !halfway(x, digits as i64) {
        // Away from a tie, Rust's exact formatting rounds as JavaScript does.
        let body = format!("{:.*}", digits, x.abs());
        return if x < 0.0 { format!("-{body}") } else { body };
    }
    if !x.is_finite() || x.abs() >= 1e21 {
        return num_str(x);
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

/// `x.toExponential(digits)`.
pub(crate) fn to_exponential(x: f64, digits: usize) -> String {
    if !x.is_finite() {
        return num_str(x);
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

/// `Number(x.toPrecision(precision))`: `x` rounded to `precision`
/// significant digits.
pub(crate) fn to_precision_number(x: f64, precision: usize) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let (rounded, exponent) = rounded_significant(x, precision);
    let text = format!(
        "{}{}e{}",
        if x < 0.0 { "-" } else { "" },
        String::from_utf8(rounded).unwrap_or_default(),
        exponent - precision as i64 + 1
    );
    text.parse().unwrap_or(f64::NAN)
}

/// A number as `JSON.stringify` writes it, for `serialize_with`: integers
/// without a fraction, `NaN` and the infinities as `null`.
pub(crate) fn json_number<S: serde::Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
    if !x.is_finite() {
        s.serialize_unit()
    } else if x.fract() == 0.0 && x.abs() < 9e18 {
        s.serialize_i64(*x as i64)
    } else {
        s.serialize_f64(*x)
    }
}

/// [`json_number`] for an optional number.
pub(crate) fn json_number_opt<S: serde::Serializer>(
    x: &Option<f64>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match x {
        Some(x) => json_number(x, s),
        None => s.serialize_unit(),
    }
}

/// [`json_number`] for each of a list.
pub(crate) fn json_numbers<S: serde::Serializer>(xs: &[f64], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(xs.iter().map(|x| JsonNumber(*x)))
}

/// A number serialized by [`json_number`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct JsonNumber(pub f64);

impl serde::Serialize for JsonNumber {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        json_number(&self.0, s)
    }
}

/// [`json_number`] for each value of a map, as `JSON.stringify` writes a
/// `Map` turned into an object.
pub(crate) fn json_number_map<S: serde::Serializer, K: serde::Serialize>(
    map: &indexmap::IndexMap<K, f64>,
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_map(map.iter().map(|(k, v)| (k, JsonNumber(*v))))
}

/// `Math.max` of two: `NaN` when either is.
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

/// `Math.min` of two: `NaN` when either is.
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// An index as JavaScript code holds one, for `serialize_with`: -1 for none.
pub(crate) fn json_index<S: serde::Serializer>(
    index: &Option<usize>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match index {
        Some(index) => s.serialize_u64(*index as u64),
        None => s.serialize_i64(-1),
    }
}
