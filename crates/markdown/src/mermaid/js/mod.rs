//! What JavaScript does with numbers and strings, where Rust's own answer
//! differs and a drawing would come out different.

pub(crate) mod re;

pub(crate) use re::{Lazy, Regex};

/// `Math.round`: halves go up, `-2.5` to `-2`.
pub(crate) fn round(x: f64) -> f64 {
    let floor = x.floor();
    if x - floor >= 0.5 { floor + 1.0 } else { floor }
}

/// `String(x)` for a number.
pub(crate) fn num_str(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return "0".into();
    }
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

/// The JavaScript whitespace `trim` removes.
pub(crate) fn is_space(c: char) -> bool {
    re::is_space(c)
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
    text.chars().map(char::len_utf16).sum()
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
