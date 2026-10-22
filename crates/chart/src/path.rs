//! A field name as a path into a datum, read as vega-util's
//! `splitAccessPath` reads it: `a.b`, `a[0]`, `a["b"]` and `a['b']`, with
//! `\` escaping the character after it.

use js::value::{TypeError, Value};
use std::{ops::Deref, str::FromStr};

/// The keys a field name steps through, outermost first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Path(Vec<String>);

impl FromStr for Path {
    /// vega-util's message for an unclosed bracket or quote, or a `]` with
    /// no `[`.
    type Err = String;

    fn from_str(path: &str) -> Result<Self, String> {
        let chars: Vec<char> = path.chars().collect();
        let mut keys = Vec::new();
        let mut quote = None;
        // In a bracket from this index; -1 just after a closing quote.
        let mut bracket: isize = 0;
        let mut escaped = String::new();
        let mut start = 0;
        let mut end = 0;
        let text = |from: usize, to: usize| chars[from..to].iter().collect::<String>();
        while end < chars.len() {
            let c = chars[end];
            if c == '\\' {
                escaped += &text(start, end);
                end += 1;
                start = end;
            } else if Some(c) == quote {
                keys.push(std::mem::take(&mut escaped) + &text(start, end));
                start = end + 1;
                quote = None;
                bracket = -1;
            } else if quote.is_some() {
            } else if start as isize == bracket && (c == '"' || c == '\'') {
                start = end + 1;
                quote = Some(c);
            } else if c == '.' && bracket == 0 {
                if end > start {
                    keys.push(std::mem::take(&mut escaped) + &text(start, end));
                }
                start = end + 1;
            } else if c == '[' {
                if end > start {
                    keys.push(std::mem::take(&mut escaped) + &text(start, end));
                }
                start = end + 1;
                bracket = start as isize;
            } else if c == ']' {
                if bracket == 0 {
                    return Err(format!("Access path missing open bracket: {path}"));
                }
                if bracket > 0 {
                    keys.push(std::mem::take(&mut escaped) + &text(start, end));
                }
                bracket = 0;
                start = end + 1;
            }
            end += 1;
        }
        if bracket != 0 {
            return Err(format!("Access path missing closing bracket: {path}"));
        }
        if quote.is_some() {
            return Err(format!("Access path missing closing quote: {path}"));
        }
        if end > start {
            keys.push(escaped + &text(start, chars.len()));
        }
        Ok(Self(keys))
    }
}

impl From<&[String]> for Path {
    fn from(keys: &[String]) -> Self {
        Self(keys.to_vec())
    }
}

impl Deref for Path {
    type Target = [String];

    fn deref(&self) -> &[String] {
        &self.0
    }
}

impl Path {
    /// The value at the end of the path from `value`, JavaScript's
    /// `value[key]…`.
    pub(crate) fn get(&self, value: &Value) -> Result<Value, TypeError> {
        self.0
            .iter()
            .try_fold(value.clone(), |value, key| value.get(&key.as_str().into()))
    }
}

#[cfg(feature = "vega-lite")]
impl Path {
    /// Vega-Lite's `removePathFromField`: the keys joined by `.`.
    pub(crate) fn flat(&self) -> String {
        self.0.join(".")
    }

    /// The JSON at the end of the path from `value`. An array takes its key
    /// as an index.
    pub(crate) fn json<'a>(&self, value: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
        self.0.iter().try_fold(value, step)
    }

    /// The JSON at the end of the path from a JSON object's fields.
    pub(crate) fn field<'a>(
        &self,
        row: &'a serde_json::Map<String, serde_json::Value>,
    ) -> Option<&'a serde_json::Value> {
        let (first, rest) = self.0.split_first()?;
        rest.iter().try_fold(row.get(first)?, step)
    }
}

#[cfg(feature = "vega-lite")]
fn step<'a>(value: &'a serde_json::Value, key: &String) -> Option<&'a serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => object.get(key),
        serde_json::Value::Array(items) => items.get(key.parse::<usize>().ok()?),
        _ => None,
    }
}
