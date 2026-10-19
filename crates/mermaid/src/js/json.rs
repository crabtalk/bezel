//! JSON values as JavaScript reads them.

use serde_json::{Map, Number, Value};

/// `n` as a JSON value: an integral number as an integer, so it serializes
/// without a fraction as `JSON.stringify` writes it; `null` for NaN and the
/// infinities.
pub fn number_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9e18 {
        return Value::Number(Number::from(n as i64));
    }
    Number::from_f64(n).map_or(Value::Null, Value::Number)
}

/// `JSON.parse(text)`, or `None` where it would throw.
pub fn parse_json(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok().map(integral)
}

/// `value` with each integral number held as an integer.
fn integral(value: Value) -> Value {
    match value {
        Value::Number(n) if !n.is_i64() && !n.is_u64() => {
            number_value(n.as_f64().unwrap_or(f64::NAN))
        }
        Value::Array(items) => Value::Array(items.into_iter().map(integral).collect()),
        Value::Object(map) => {
            Value::Object(map.into_iter().map(|(k, v)| (k, integral(v))).collect())
        }
        other => other,
    }
}

/// What JavaScript makes of a JSON value.
pub trait JsValue {
    /// JavaScript's truth of the value.
    fn truthy(&self) -> bool;
    /// `String(value)`.
    fn to_js_string(&self) -> String;
    /// `Number(value)`.
    fn to_number(&self) -> f64;
    /// `JSON.stringify(value)`.
    fn to_json(&self) -> String;
}

impl JsValue for Value {
    fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => super::truthy(n.as_f64().unwrap_or(f64::NAN)),
            Value::String(text) => !text.is_empty(),
            Value::Array(_) | Value::Object(_) => true,
        }
    }

    fn to_js_string(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => super::num_str(n.as_f64().unwrap_or(f64::NAN)),
            Value::String(text) => text.clone(),
            Value::Array(items) => items
                .iter()
                .map(|item| match item {
                    Value::Null => String::new(),
                    other => other.to_js_string(),
                })
                .collect::<Vec<_>>()
                .join(","),
            Value::Object(_) => "[object Object]".into(),
        }
    }

    fn to_number(&self) -> f64 {
        match self {
            Value::Null => 0.0,
            Value::Bool(b) => f64::from(u8::from(*b)),
            Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
            Value::String(text) => super::number(text),
            Value::Array(items) => match items.as_slice() {
                [] => 0.0,
                [one] => one.to_number(),
                _ => f64::NAN,
            },
            Value::Object(_) => f64::NAN,
        }
    }

    fn to_json(&self) -> String {
        let mut out = String::new();
        write_json(self, &mut out);
        out
    }
}

fn write_json(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&super::num_str(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(text) => write_json_string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => write_json_object(map, out),
    }
}

fn write_json_object(map: &Map<String, Value>, out: &mut String) {
    out.push('{');
    for (i, (key, value)) in map.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_json_string(key, out);
        out.push(':');
        write_json(value, out);
    }
    out.push('}');
}

/// A string as `JSON.stringify` quotes it.
fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
