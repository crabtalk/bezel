//! Delimiter-separated values as d3-dsv's `parse` reads them: the first row
//! names the fields, a field in double quotes may hold delimiters, line breaks
//! and `""` for a quote, and a row ends at `\n`, `\r\n` or `\r`.

use serde_json::{Map, Value};

/// A row per line after the first, every field a string. A field a row lacks
/// is `""`; of two fields with one name, the later one's value is kept.
pub(crate) fn parse(text: &str, delimiter: char) -> Vec<Map<String, Value>> {
    let mut rows = rows(text, delimiter).into_iter();
    let Some(names) = rows.next() else {
        return Vec::new();
    };
    rows.map(|row| {
        let mut out = Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = row.get(index).cloned().unwrap_or_default();
            out.insert(name.clone(), Value::String(value));
        }
        out
    })
    .collect()
}

/// Each line's fields, a final line break ignored.
fn rows(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let text = text.strip_suffix('\n').unwrap_or(text);
    let text = text.strip_suffix('\r').unwrap_or(text);
    let mut rows = Vec::new();
    if text.is_empty() {
        return rows;
    }
    let mut chars = text.chars().peekable();
    let mut row = Vec::new();
    loop {
        let mut field = String::new();
        // What ended the field: `None` for the end of the text.
        let mut end = None;
        if chars.peek() == Some(&'"') {
            chars.next();
            while let Some(c) = chars.next() {
                if c != '"' {
                    field.push(c);
                } else if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    break;
                }
            }
            // The character after a closing quote ends the field, whatever it is.
            end = match chars.next() {
                Some(c @ ('\n' | '\r')) => Some(c),
                Some(_) => Some(delimiter),
                None => None,
            };
        } else {
            for c in chars.by_ref() {
                if c == delimiter || c == '\n' || c == '\r' {
                    end = Some(c);
                    break;
                }
                field.push(c);
            }
        }
        if end == Some('\r') && chars.peek() == Some(&'\n') {
            chars.next();
        }
        row.push(field);
        if end != Some(delimiter) {
            rows.push(std::mem::take(&mut row));
        }
        if end.is_none() {
            return rows;
        }
    }
}
