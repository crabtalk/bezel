//! vega-scenegraph's text metrics without a canvas: widths are its
//! estimate, `0.8` of the font size a UTF-16 unit.

use crate::{scene::Item, value::Value};
use d3::string;

/// `fontSize(item)`: 11 where unset.
pub fn font_size(item: &Item) -> f64 {
    match item.get("fontSize") {
        Value::Undefined | Value::Null => 11.0,
        size => {
            let size = size.to_number();
            if size.is_nan() { 0.0 } else { size }
        }
    }
}

/// `lineHeight(item)`: the font size and 2 where unset.
pub fn line_height(item: &Item) -> f64 {
    match item.get("lineHeight") {
        Value::Undefined | Value::Null => font_size(item) + 2.0,
        height => height.to_number(),
    }
}

/// The estimated width of `text` at `size`.
pub fn estimate(text: &str, size: f64) -> f64 {
    (0.8 * string::len(text) as f64 * size).trunc()
}

/// `textMetrics.width(item, text)`.
pub fn width(item: &Item, text: &str) -> f64 {
    estimate(&text_value(item, Some(text)), font_size(item))
}

/// `textValue(item, line)`: the line trimmed, and truncated to the item's
/// `limit`.
pub fn text_value(item: &Item, line: Option<&str>) -> String {
    let text = line.map_or("", |line| string::trim(line));
    let limit = item.get("limit").to_number();
    if limit > 0.0 && !text.is_empty() {
        truncate(item, text, limit)
    } else {
        text.to_string()
    }
}

fn truncate(item: &Item, text: &str, limit: f64) -> String {
    let size = font_size(item);
    let width = |text: &str| estimate(text, size);
    if width(text) < limit {
        return text.to_string();
    }
    let ellipsis = match item.get("ellipsis") {
        Value::String(e) if !e.is_empty() => e.to_string(),
        _ => "\u{2026}".to_string(),
    };
    let limit = limit - width(&ellipsis);
    let units: Vec<u16> = text.encode_utf16().collect();
    let slice = |from: usize, to: usize| String::from_utf16_lossy(&units[from..to]);
    let rtl = matches!(item.get("dir"), Value::String(d) if &**d == "rtl");
    let (mut lo, mut hi) = (0, units.len());
    if rtl {
        while lo < hi {
            let mid = (lo + hi) >> 1;
            if width(&slice(mid, units.len())) > limit {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        ellipsis + &slice(lo, units.len())
    } else {
        while lo < hi {
            let mid = 1 + ((lo + hi) >> 1);
            if width(&slice(0, mid)) < limit {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        slice(0, lo) + &ellipsis
    }
}

/// `textLines(item)`: the item's text split at its `lineBreak`, or its
/// list of lines; `None` for one line, given as `text`.
pub fn lines(item: &Item) -> Result<Vec<String>, String> {
    let text = item.get("text");
    let one = |value: &Value| match value {
        Value::Undefined | Value::Null => String::new(),
        value => value.to_string(),
    };
    match text {
        Value::Array(lines) if lines.len() > 1 => Ok(lines.iter().map(one).collect()),
        Value::Array(lines) => Err(lines.first().map(one).unwrap_or_default()),
        _ => {
            let break_at = item.get("lineBreak");
            let text = one(text);
            match break_at {
                Value::String(at) if !at.is_empty() && !text.is_empty() => {
                    let lines: Vec<String> = text.split(&**at).map(str::to_string).collect();
                    match lines.len() {
                        1 => Err(text),
                        _ => Ok(lines),
                    }
                }
                _ => Err(text),
            }
        }
    }
}

/// `offset(item)`: the baseline's shift from the anchor, rounded.
pub fn offset(item: &Item) -> f64 {
    let h = font_size(item);
    let baseline = match item.get("baseline") {
        Value::String(b) => b.to_string(),
        _ => String::new(),
    };
    d3::format::number::round(match baseline.as_str() {
        "top" => 0.79 * h,
        "middle" => 0.30 * h,
        "bottom" => -0.21 * h,
        "line-top" => 0.29 * h + 0.5 * line_height(item),
        "line-bottom" => 0.29 * h - 0.5 * line_height(item),
        _ => 0.0,
    })
}
