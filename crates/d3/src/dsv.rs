//! d3-dsv: delimiter-separated values. A field in double quotes may hold
//! delimiters, line breaks and `""` for a quote, and a row ends at `\n`,
//! `\r\n` or `\r`.

/// `dsvFormat(delimiter).parse(text)`: a row per line after the first, which
/// names the fields, each row its fields in the first line's order. A field
/// a row lacks is `""`; a name written twice keeps its first place and its
/// later value.
pub fn parse(text: &str, delimiter: char) -> Vec<Vec<(String, String)>> {
    let mut rows = parse_rows(text, delimiter).into_iter();
    let Some(names) = rows.next() else {
        return Vec::new();
    };
    rows.map(|row| {
        let mut out: Vec<(String, String)> = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let value = row.get(index).cloned().unwrap_or_default();
            match out.iter_mut().find(|(n, _)| n == name) {
                Some((_, slot)) => *slot = value,
                None => out.push((name.clone(), value)),
            }
        }
        out
    })
    .collect()
}

/// `dsvFormat(delimiter).parseRows(text)`: each line's fields, a final line
/// break ignored.
pub fn parse_rows(text: &str, delimiter: char) -> Vec<Vec<String>> {
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
