//! Vega's string functions. Lengths and indices count UTF-16 code units, as
//! JavaScript's do.

use js::{string, value::Value};

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn text(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// JavaScript's `slice(start, end)` bounds on a sequence `length` long:
/// negative counts from the end, `undefined` end is the length.
pub(crate) fn bounds(length: usize, start: &Value, end: &Value) -> (usize, usize) {
    let length = length as f64;
    let at = |value: &Value, default: f64| {
        let n = match value {
            Value::Undefined => default,
            _ => value.to_integer(),
        };
        (if n < 0.0 {
            (length + n).max(0.0)
        } else {
            n.min(length)
        }) as usize
    };
    let from = at(start, 0.0);
    (from, at(end, length).max(from))
}

fn slice(units: &[u16], start: f64, end: f64) -> String {
    let (from, to) = bounds(units.len(), &Value::Number(start), &Value::Number(end));
    text(&units[from..to])
}

/// `text.slice(start, end)`.
pub(crate) fn slice_text(text: &str, start: &Value, end: &Value) -> String {
    let units = units(text);
    let (from, to) = bounds(units.len(), start, end);
    self::text(&units[from..to])
}

/// The first, or if `last` the last, UTF-16 offset of `needle` in `haystack`.
pub(crate) fn index_of(haystack: &str, needle: &str, last: bool) -> Option<usize> {
    let (haystack, needle) = (units(haystack), units(needle));
    let fits = haystack.len().checked_sub(needle.len())?;
    let hit = |&i: &usize| haystack[i..i + needle.len()] == needle[..];
    if last {
        (0..=fits).rev().find(hit)
    } else {
        (0..=fits).find(hit)
    }
}

/// vega-util's `pad`: `text` padded with `fill` (a space if falsy) to `length`,
/// on the right unless `align` is `left` or `center`.
pub(crate) fn pad(text: &str, length: f64, fill: &Value, align: &Value) -> String {
    let fill = if fill.truthy() {
        fill.to_string()
    } else {
        " ".to_string()
    };
    let n = length - string::len(text) as f64;
    let short = n > 0.0;
    if !short {
        return text.to_string();
    }
    let repeat = |count: f64| fill.repeat(count.floor() as usize);
    match align {
        Value::String(a) if &**a == "left" => repeat(n) + text,
        Value::String(a) if &**a == "center" => {
            repeat((n / 2.0).trunc()) + text + &repeat((n / 2.0).ceil())
        }
        _ => text.to_string() + &repeat(n),
    }
}

/// vega-util's `truncate`: `text` cut to `length` with `ellipsis` (`…` if
/// null or undefined) on the right, or the left or center per `align`.
pub(crate) fn truncate(text: &str, length: f64, align: &Value, ellipsis: &Value) -> String {
    let ellipsis = if ellipsis.is_nullish() {
        "\u{2026}".to_string()
    } else {
        ellipsis.to_string()
    };
    let units = units(text);
    let n = units.len() as f64;
    let l = (length - string::len(&ellipsis) as f64).max(0.0);
    if n <= length {
        return text.to_string();
    }
    match align {
        Value::String(a) if &**a == "left" => ellipsis + &slice(&units, n - l, n),
        Value::String(a) if &**a == "center" => {
            slice(&units, 0.0, (l / 2.0).ceil())
                + &ellipsis
                + &slice(&units, n - (l / 2.0).trunc(), n)
        }
        _ => slice(&units, 0.0, l) + &ellipsis,
    }
}

/// `text.replace(pattern, replacement)`: every match of a global regular
/// expression, else the first match or occurrence of the string.
pub(crate) fn replace(text: &str, pattern: &Value, replacement: &str) -> String {
    match pattern {
        Value::RegExp(regexp) => regexp.regex.replace_str(text, replacement),
        pattern => string::replace(text, &pattern.to_string(), replacement),
    }
}

/// `text.split(separator, limit)`.
pub(crate) fn split(text: &str, separator: &Value, limit: &Value) -> Value {
    let limit = match limit {
        Value::Undefined => u32::MAX as usize,
        other => other.to_uint32() as usize,
    };
    let pieces: Vec<Option<String>> = match separator {
        _ if limit == 0 => Vec::new(),
        Value::RegExp(regexp) => regexp
            .regex
            .split(text)
            .into_iter()
            .take(limit)
            .map(|piece| piece.map(str::to_string))
            .collect(),
        Value::Undefined => vec![Some(text.to_string())],
        separator => {
            let separator = separator.to_string();
            if separator.is_empty() {
                text.encode_utf16()
                    .take(limit)
                    .map(|unit| Some(self::text(&[unit])))
                    .collect()
            } else {
                text.split(separator.as_str())
                    .take(limit)
                    .map(|piece| Some(piece.to_string()))
                    .collect()
            }
        }
    };
    pieces
        .into_iter()
        .map(|piece| piece.map_or(Value::Undefined, Value::from))
        .collect::<Vec<_>>()
        .into()
}

/// JavaScript's `substring`: bounds clamped to the string and swapped if
/// reversed.
pub(crate) fn substring(text: &str, start: &Value, end: &Value) -> String {
    let units = units(text);
    let length = units.len() as f64;
    let clamp = |value: &Value| value.to_integer().clamp(0.0, length) as usize;
    let from = clamp(start);
    let to = match end {
        Value::Undefined => units.len(),
        end => clamp(end),
    };
    self::text(&units[from.min(to)..from.max(to)])
}
