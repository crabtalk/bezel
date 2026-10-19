//! A small YAML reader covering what Mermaid feeds to js-yaml with the JSON
//! schema: block and flow collections, quoted and plain scalars, and block
//! scalars. Ported from Pele's `util/yaml.ts`.

use super::{
    config::{Map, Value},
    js::{self, Lazy},
};
use crate::js::JsValue;

const MAX_DEPTH: usize = 64;

static RE_INT: Lazy = Lazy::new(r"^[-+]?(?:0|[1-9][0-9]*)$", "");
static RE_HEX: Lazy = Lazy::new(r"^[-+]?0x[0-9a-fA-F]+$", "");
static RE_OCT: Lazy = Lazy::new(r"^[-+]?0o[0-7]+$", "");
static RE_BIN: Lazy = Lazy::new(r"^[-+]?0b[01]+$", "");
static RE_FLOAT: Lazy = Lazy::new(
    r"^[-+]?(?:[0-9][0-9]*(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][-+]?[0-9]+)?$",
    "",
);
static RE_KEY_START: Lazy = Lazy::new(r#"[\s#:,[\]{}&*!|>'"%@`]"#, "");
static RE_INF: Lazy = Lazy::new(r"^[-+]?\.(?:inf|Inf|INF)$", "");
static RE_NAN: Lazy = Lazy::new(r"^\.(?:nan|NaN|NAN)$", "");
static RE_BLOCK_HEADER: Lazy = Lazy::new(r"^[|>][-+0-9]*\s*(?:#.*)?$", "");
static RE_CONTINUED: Lazy = Lazy::new(r"\\\n[ \t]*", "g");
static RE_PLAIN_END: Lazy = Lazy::new(r"[\s,[\]{}]", "");

/// Why the text is not YAML: js-yaml's message, which the error shows.
#[derive(Debug)]
pub struct YamlError(pub String);

enum Failure {
    /// A quoted scalar or flow collection runs past the text read so far.
    Incomplete(String),
    Error(String),
}

type Result<T> = std::result::Result<T, Failure>;

fn error<T>(message: &str) -> Result<T> {
    Err(Failure::Error(message.into()))
}

struct Reader {
    lines: Vec<String>,
    i: usize,
}

impl Reader {
    fn new(src: &str) -> Self {
        let src = src.replace("\r\n", "\n").replace('\r', "\n");
        Self {
            lines: src.split('\n').map(str::to_string).collect(),
            i: 0,
        }
    }

    fn skip_blank(&mut self) {
        while self.i < self.lines.len() {
            let t = js::trim(&self.lines[self.i]);
            if !t.is_empty() && !t.starts_with('#') && t != "---" {
                break;
            }
            self.i += 1;
        }
    }

    fn indent(&self) -> Result<usize> {
        let line = self.lines[self.i].as_bytes();
        let mut k = 0;
        while k < line.len() && line[k] == b' ' {
            k += 1;
        }
        if line.get(k) == Some(&b'\t') {
            return error("YAML: tabs are not allowed for indentation");
        }
        Ok(k)
    }
}

fn is_blank(ch: Option<u8>) -> bool {
    matches!(ch, Some(b' ' | b'\t' | b'\n' | b'\r'))
}

fn leading(line: &str) -> usize {
    line.len() - js::trim_start(line).len()
}

/// Splits `key: rest` at the start of a line: the raw key and where the
/// value starts.
fn split_key(line: &str) -> Option<(String, usize)> {
    let bytes = line.as_bytes();
    let first = *bytes.first()?;
    let end;
    let mut colon;
    if first == b'"' || first == b'\'' {
        let mut i = 1;
        loop {
            if i >= bytes.len() {
                return None;
            }
            let c = bytes[i];
            if first == b'"' && c == b'\\' {
                i += 2;
            } else if c == first {
                if first == b'"' || bytes.get(i + 1) != Some(&b'\'') {
                    break;
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        end = i + 1;
        colon = end;
        while is_blank(bytes.get(colon).copied()) {
            colon += 1;
        }
        if bytes.get(colon) != Some(&b':') {
            return None;
        }
    } else {
        let first_char = line.chars().next()?;
        if RE_KEY_START.test(first_char.encode_utf8(&mut [0; 4])) {
            return None;
        }
        if first == b'-'
            && (bytes.len() < 2
                || is_blank(bytes.get(1).copied())
                || bytes[1] == b':'
                || bytes[1] == b'#')
        {
            return None;
        }
        colon = 1;
        while colon < bytes.len() && bytes[colon] != b':' && bytes[colon] != b'#' {
            colon += 1;
        }
        if bytes.get(colon) != Some(&b':') {
            return None;
        }
        let mut e = colon;
        while e > 0 && is_blank(bytes.get(e - 1).copied()) {
            e -= 1;
        }
        end = e;
    }
    let mut length = colon + 1;
    if length < bytes.len() && !is_blank(bytes.get(length).copied()) {
        return None;
    }
    while is_blank(bytes.get(length).copied()) {
        length += 1;
    }
    Some((line[..end].to_string(), length))
}

fn scalar(raw: &str) -> Value {
    let s = js::trim(raw);
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Value::Null,
        "true" | "True" | "TRUE" => return Value::Bool(true),
        "false" | "False" | "FALSE" => return Value::Bool(false),
        _ => {}
    }
    if RE_INT.test(s) || RE_FLOAT.test(s) {
        return js::number_value(js::number(s));
    }
    if RE_HEX.test(s) || RE_OCT.test(s) || RE_BIN.test(s) {
        let neg = s.starts_with('-');
        let v = js::number(if neg || s.starts_with('+') {
            &s[1..]
        } else {
            s
        });
        return js::number_value(if neg { -v } else { v });
    }
    if RE_INF.test(s) {
        return js::number_value(if s.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if RE_NAN.test(s) {
        return js::number_value(f64::NAN);
    }
    Value::String(s.to_string())
}

fn escape(e: char) -> Option<char> {
    Some(match e {
        '0' => '\0',
        'a' => '\u{7}',
        'b' => '\u{8}',
        't' => '\t',
        'n' => '\n',
        'v' => '\u{b}',
        'f' => '\u{c}',
        'r' => '\r',
        'e' => '\u{1b}',
        ' ' => ' ',
        '"' => '"',
        '/' => '/',
        '\\' => '\\',
        'N' => '\u{85}',
        '_' => '\u{a0}',
        'L' => '\u{2028}',
        'P' => '\u{2029}',
        _ => return None,
    })
}

/// Folds a scalar's line breaks: blanks around a break go, one break becomes
/// a space, and each further break stays.
fn fold(text: &str) -> String {
    if !text.contains('\n') {
        return text.to_string();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let last = lines.len() - 1;
    let mut trimmed = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let mut line = *line;
        if i > 0 {
            line = line.trim_start_matches([' ', '\t']);
        }
        if i < last {
            line = line.trim_end_matches([' ', '\t']);
        }
        trimmed.push(line);
    }
    let joined = trimmed.join("\n");
    let mut out = String::with_capacity(joined.len());
    let mut breaks = 0;
    for c in joined.chars() {
        if c == '\n' {
            breaks += 1;
            continue;
        }
        if breaks > 0 {
            if breaks == 1 {
                out.push(' ');
            } else {
                out.push_str(&"\n".repeat(breaks - 1));
            }
            breaks = 0;
        }
        out.push(c);
    }
    if breaks == 1 {
        out.push(' ');
    } else if breaks > 1 {
        out.push_str(&"\n".repeat(breaks - 1));
    }
    out
}

fn unquote_double(body: &str) -> Result<String> {
    let s = fold(&RE_CONTINUED.replace(body, |_| String::new()));
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch != '\\' {
            out.push(ch);
            i += 1;
            continue;
        }
        i += 1;
        let Some(&e) = chars.get(i) else {
            return error("YAML: unknown escape sequence");
        };
        if e == 'x' || e == 'u' || e == 'U' {
            let len = match e {
                'x' => 2,
                'u' => 4,
                _ => 8,
            };
            let digits: String = chars[(i + 1).min(chars.len())..(i + 1 + len).min(chars.len())]
                .iter()
                .collect();
            let code = js::parse_int(&digits, 16);
            match (code.is_finite() && code >= 0.0)
                .then(|| char::from_u32(code as u32))
                .flatten()
            {
                Some(c) => out.push(c),
                None => return error("Invalid code point"),
            }
            i += len + 1;
        } else if let Some(c) = escape(e) {
            out.push(c);
            i += 1;
        } else {
            return error("YAML: unknown escape sequence");
        }
    }
    Ok(out)
}

fn skip(s: &str, mut pos: usize) -> usize {
    let bytes = s.as_bytes();
    while pos < bytes.len() {
        let c = bytes[pos];
        if c == b' ' || c == b'\n' || c == b'\t' || c == b'\r' {
            pos += 1;
        } else if c == b'#' && (pos == 0 || s[..pos].chars().next_back().is_some_and(js::is_space))
        {
            pos = s[pos..].find('\n').map_or(s.len(), |nl| pos + nl);
        } else {
            break;
        }
    }
    pos
}

/// Entries of a flow collection are separated by exactly one comma, as
/// js-yaml requires.
fn entry_start(ch: u8, separated: bool) -> Result<()> {
    if !separated {
        return error("YAML: missed comma between flow collection entries");
    }
    if ch == b',' {
        return error("YAML: expected the node content, but found ','");
    }
    Ok(())
}

/// Parses a flow value starting at `pos`: the value and the index after it.
fn flow(s: &str, pos: usize, stops: &str, depth: usize) -> Result<(Value, usize)> {
    if depth > MAX_DEPTH {
        return error("YAML: nesting is too deep");
    }
    let bytes = s.as_bytes();
    let mut pos = skip(s, pos);
    let ch = bytes.get(pos).copied();
    if ch == Some(b'{') {
        let mut out = Map::new();
        pos += 1;
        let mut separated = true;
        loop {
            pos = skip(s, pos);
            if pos >= bytes.len() {
                return Err(Failure::Incomplete(
                    "YAML: unexpected end of the stream within a flow collection".into(),
                ));
            }
            if bytes[pos] == b'}' {
                return Ok((Value::Object(out), pos + 1));
            }
            entry_start(bytes[pos], separated)?;
            let (key, after) = flow(s, pos, ":,}", depth + 1)?;
            pos = skip(s, after);
            let mut value = Value::Null;
            if bytes.get(pos) == Some(&b':') {
                let (v, after) = flow(s, pos + 1, ",}", depth + 1)?;
                value = v;
                pos = after;
            }
            out.insert(key.to_js_string(), value);
            pos = skip(s, pos);
            separated = bytes.get(pos) == Some(&b',');
            if separated {
                pos += 1;
            }
        }
    }
    if ch == Some(b'[') {
        let mut out = Vec::new();
        pos += 1;
        let mut separated = true;
        loop {
            pos = skip(s, pos);
            if pos >= bytes.len() {
                return Err(Failure::Incomplete(
                    "YAML: unexpected end of the stream within a flow collection".into(),
                ));
            }
            if bytes[pos] == b']' {
                return Ok((Value::Array(out), pos + 1));
            }
            entry_start(bytes[pos], separated)?;
            let (value, after) = flow(s, pos, ",]", depth + 1)?;
            out.push(value);
            pos = skip(s, after);
            separated = bytes.get(pos) == Some(&b',');
            if separated {
                pos += 1;
            }
        }
    }
    if ch == Some(b'"') {
        let mut end = pos + 1;
        while end < bytes.len() && bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        if end >= bytes.len() {
            return Err(Failure::Incomplete(
                "YAML: unexpected end of the stream within a double quoted scalar".into(),
            ));
        }
        let body = String::from_utf8_lossy(&bytes[pos + 1..end]);
        return Ok((Value::String(unquote_double(&body)?), end + 1));
    }
    if ch == Some(b'\'') {
        let mut end = pos + 1;
        while end < bytes.len() {
            if bytes[end] == b'\'' {
                if bytes.get(end + 1) != Some(&b'\'') {
                    break;
                }
                end += 1;
            }
            end += 1;
        }
        if end >= bytes.len() {
            return Err(Failure::Incomplete(
                "YAML: unexpected end of the stream within a single quoted scalar".into(),
            ));
        }
        return Ok((
            Value::String(fold(&s[pos + 1..end]).replace("''", "'")),
            end + 1,
        ));
    }
    let mut end = pos;
    while end < bytes.len() {
        let c = bytes[end];
        if stops.as_bytes().contains(&c)
            && (c != b':'
                || end + 1 >= bytes.len()
                || s[end + 1..]
                    .chars()
                    .next()
                    .is_some_and(|next| RE_PLAIN_END.test(next.encode_utf8(&mut [0; 4]))))
        {
            break;
        }
        if c == b'#' && end > pos && s[..end].chars().next_back().is_some_and(js::is_space) {
            let nl = s[end..].find('\n').map_or(s.len(), |nl| end + nl);
            return Ok((scalar(&fold(js::trim(&s[pos..end]))), nl));
        }
        end += 1;
    }
    while !s.is_char_boundary(end) {
        end += 1;
    }
    Ok((scalar(&fold(js::trim(&s[pos..end]))), end))
}

fn strip_comment(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut quote = 0u8;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == b'\\' && quote == b'"' {
                i += 1;
            } else if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return &s[..i];
        }
        i += 1;
    }
    s
}

fn block_scalar(r: &mut Reader, header: &str, parent_indent: usize) -> String {
    let folded = header.starts_with('>');
    let chomp = if header.contains('-') {
        '-'
    } else if header.contains('+') {
        '+'
    } else {
        ' '
    };
    let explicit = header.chars().find(|c| ('1'..='9').contains(c));
    let mut indent: Option<usize> =
        explicit.map(|d| parent_indent + d.to_digit(10).unwrap_or(0) as usize);
    let mut out: Vec<String> = Vec::new();
    while r.i < r.lines.len() {
        let line = r.lines[r.i].clone();
        if js::trim(&line).is_empty() {
            out.push(String::new());
            r.i += 1;
            continue;
        }
        let k = leading(&line);
        let indent_now = match indent {
            Some(indent) => indent,
            None => {
                if k <= parent_indent {
                    break;
                }
                indent = Some(k);
                k
            }
        };
        if k < indent_now {
            break;
        }
        out.push(line.get(indent_now..).unwrap_or("").to_string());
        r.i += 1;
    }
    let mut trailing = 0;
    while out.last().is_some_and(String::is_empty) {
        out.pop();
        trailing += 1;
    }
    let text = if folded {
        let mut text = String::new();
        for i in 0..out.len() {
            let cur = &out[i];
            if i > 0 {
                let prev = &out[i - 1];
                if cur.is_empty() {
                    text.push('\n');
                } else if !prev.is_empty() {
                    let indented = |s: &str| s.starts_with([' ', '\t']);
                    text.push(if indented(prev) || indented(cur) {
                        '\n'
                    } else {
                        ' '
                    });
                }
            }
            text.push_str(cur);
        }
        text
    } else {
        out.join("\n")
    };
    if out.is_empty() {
        return if chomp == '+' {
            "\n".repeat(trailing)
        } else {
            String::new()
        };
    }
    match chomp {
        '-' => text,
        '+' => text + &"\n".repeat(trailing + 1),
        _ => text + "\n",
    }
}

fn inline_value(r: &mut Reader, rest: &str, indent: isize) -> Result<Value> {
    let first = rest.as_bytes().first().copied();
    if matches!(first, Some(b'|' | b'>')) && RE_BLOCK_HEADER.test(rest) {
        return Ok(Value::String(block_scalar(r, rest, indent.max(0) as usize)));
    }
    if matches!(first, Some(b'{' | b'[' | b'"' | b'\'')) {
        // The value may continue on later lines: read them in growing
        // batches until it closes.
        let start = r.i;
        let mut text = rest.to_string();
        let mut taken = 0;
        let mut batch = 1;
        loop {
            match flow(&text, 0, "", 0) {
                Ok((value, end)) => {
                    let used = text[..end.min(text.len())].matches('\n').count();
                    let line_end = text[end..].find('\n').map_or(text.len(), |nl| end + nl);
                    if !js::trim(strip_comment(&text[end..line_end])).is_empty() {
                        return error("YAML: unexpected content after a value");
                    }
                    r.i = start + used;
                    return Ok(value);
                }
                Err(Failure::Incomplete(message)) => {
                    if start + taken >= r.lines.len() {
                        return Err(Failure::Error(message));
                    }
                    let to = (start + taken + batch).min(r.lines.len());
                    let more = &r.lines[start + taken..to];
                    text.push('\n');
                    text.push_str(&more.join("\n"));
                    taken += more.len();
                    batch *= 2;
                }
                Err(other) => return Err(other),
            }
        }
    }
    let mut text = js::trim(strip_comment(rest)).to_string();
    while r.i < r.lines.len() {
        let line = &r.lines[r.i];
        if js::trim(line).is_empty() {
            let mut j = r.i;
            while j < r.lines.len() && js::trim(&r.lines[j]).is_empty() {
                j += 1;
            }
            if j >= r.lines.len() || leading(&r.lines[j]) as isize <= indent {
                break;
            }
            text.push_str(&"\n".repeat(j - r.i));
            r.i = j;
            continue;
        }
        if leading(line) as isize <= indent {
            break;
        }
        let more = js::trim(strip_comment(line)).to_string();
        if !text.ends_with('\n') {
            text.push(' ');
        }
        text.push_str(&more);
        r.i += 1;
    }
    Ok(scalar(&text))
}

fn is_seq(line: &str) -> bool {
    line == "-" || line.starts_with("- ")
}

fn block(r: &mut Reader, min_indent: usize, depth: usize) -> Result<Value> {
    if depth > MAX_DEPTH {
        return error("YAML: nesting is too deep");
    }
    r.skip_blank();
    if r.i >= r.lines.len() {
        return Ok(Value::Null);
    }
    let indent = r.indent()?;
    if indent < min_indent {
        return Ok(Value::Null);
    }
    let line = r.lines[r.i][indent..].to_string();

    if is_seq(&line) {
        let mut out = Vec::new();
        while r.i < r.lines.len() {
            r.skip_blank();
            if r.i >= r.lines.len() || r.indent()? != indent {
                break;
            }
            let cur = r.lines[r.i][indent..].to_string();
            if !is_seq(&cur) {
                break;
            }
            let rest = &cur[1..];
            let pad = rest.len() - js::trim_start(rest).len() + 1;
            let body = js::trim(strip_comment(rest));
            if body.is_empty() {
                r.i += 1;
                out.push(block(r, indent + 1, depth + 1)?);
            } else if split_key(body).is_some() {
                r.lines[r.i] = format!("{}{}", " ".repeat(indent + pad), js::trim_start(rest));
                out.push(block(r, indent + pad, depth + 1)?);
            } else {
                r.i += 1;
                out.push(inline_value(r, js::trim(rest), indent as isize)?);
            }
        }
        return Ok(Value::Array(out));
    }

    if split_key(&line).is_none() {
        r.i += 1;
        return inline_value(r, js::trim(&line), indent as isize - 1);
    }

    let mut out = Map::new();
    while r.i < r.lines.len() {
        r.skip_blank();
        if r.i >= r.lines.len() {
            break;
        }
        let k = r.indent()?;
        if k < indent {
            break;
        }
        if k > indent {
            return error("YAML: bad indentation of a mapping entry");
        }
        let cur = r.lines[r.i][indent..].to_string();
        let Some((raw_key, length)) = split_key(&cur) else {
            return error("YAML: can not read a block mapping entry");
        };
        let key = flow(&raw_key, 0, "", 0)?.0.to_js_string();
        let rest = cur[length..].to_string();
        r.i += 1;
        if js::trim(strip_comment(&rest)).is_empty() {
            r.skip_blank();
            if r.i < r.lines.len() {
                let next = r.indent()?;
                let next_line = r.lines[r.i][next..].to_string();
                let seq = is_seq(&next_line);
                let value = if next > indent || (seq && next == indent) {
                    block(r, if seq { indent } else { indent + 1 }, depth + 1)?
                } else {
                    Value::Null
                };
                out.insert(key, value);
            } else {
                out.insert(key, Value::Null);
            }
        } else {
            let value = inline_value(r, js::trim(&rest), indent as isize)?;
            out.insert(key, value);
        }
    }
    Ok(Value::Object(out))
}

pub fn parse_yaml(src: &str) -> std::result::Result<Value, YamlError> {
    let fail = |failure: Failure| match failure {
        Failure::Incomplete(message) | Failure::Error(message) => YamlError(message),
    };
    let trimmed = js::trim(src);
    if trimmed.starts_with(['{', '[']) {
        let (value, end) = flow(trimmed, 0, "", 0).map_err(fail)?;
        if skip(trimmed, end) == trimmed.len() {
            return Ok(value);
        }
    }
    let mut r = Reader::new(src);
    let value = block(&mut r, 0, 0).map_err(fail)?;
    r.skip_blank();
    if r.i < r.lines.len() && js::trim(&r.lines[r.i]) != "..." {
        return Err(YamlError(
            "YAML: end of the stream or a document separator is expected".into(),
        ));
    }
    Ok(value)
}
