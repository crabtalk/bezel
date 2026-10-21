//! A field name as a path into a datum, read as Vega's `splitAccessPath`
//! reads it: `a.b`, `a[0]`, `a["b"]` and `a['b']`, with `\` escaping the
//! character after it.

use crate::vega_lite::format::Row;
use serde_json::Value;
use std::str::FromStr;

/// The keys a field name steps through, outermost first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Path(Vec<String>);

impl FromStr for Path {
    /// An unclosed bracket or quote, or a `]` with no `[`.
    type Err = ();

    fn from_str(path: &str) -> Result<Self, ()> {
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
                    return Err(());
                }
                if bracket > 0 {
                    keys.push(std::mem::take(&mut escaped) + &text(start, end));
                }
                bracket = 0;
                start = end + 1;
            }
            end += 1;
        }
        if bracket != 0 || quote.is_some() {
            return Err(());
        }
        if end > start {
            keys.push(escaped + &text(start, chars.len()));
        }
        Ok(Self(keys))
    }
}

impl Path {
    /// The value at the end of the path. An array takes its key as an index.
    pub(crate) fn get<'a>(&self, value: &'a Value) -> Option<&'a Value> {
        self.0.iter().try_fold(value, step)
    }

    /// The value at the end of the path from `row`.
    pub(crate) fn field<'a>(&self, row: &'a Row) -> Option<&'a Value> {
        let (first, rest) = self.0.split_first()?;
        rest.iter().try_fold(row.get(first)?, step)
    }
}

fn step<'a>(value: &'a Value, key: &String) -> Option<&'a Value> {
    match value {
        Value::Object(object) => object.get(key),
        Value::Array(items) => items.get(key.parse::<usize>().ok()?),
        _ => None,
    }
}
