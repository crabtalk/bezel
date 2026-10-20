//! A small YAML reader covering what Mermaid feeds to js-yaml with the JSON
//! schema: block and flow collections, quoted and plain scalars, and block
//! scalars. Ported from Pele's `util/yaml.ts`.

use super::{
    config::{Map, Value},
    js::{self, Lazy},
};
use crate::{js::JsValue, util::chars};

const MAX_DEPTH: usize = 64;

static RE_INT: Lazy = Lazy::new(r"^[-+]?(?:0|[1-9][0-9]*)$", "");
static RE_HEX: Lazy = Lazy::new(r"^[-+]?0x[0-9a-fA-F]+$", "");
static RE_OCT: Lazy = Lazy::new(r"^[-+]?0o[0-7]+$", "");
static RE_BIN: Lazy = Lazy::new(r"^[-+]?0b[01]+$", "");
static RE_FLOAT: Lazy = Lazy::new(
    r"^[-+]?(?:[0-9][0-9]*(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][-+]?[0-9]+)?$",
    "",
);
static RE_INF: Lazy = Lazy::new(r"^[-+]?\.(?:inf|Inf|INF)$", "");
static RE_NAN: Lazy = Lazy::new(r"^\.(?:nan|NaN|NAN)$", "");
static RE_BLOCK_HEADER: Lazy = Lazy::new(r"^[|>][-+0-9]*\s*(?:#.*)?$", "");
static RE_CONTINUED: Lazy = Lazy::new(r"\\\n[ \t]*", "g");

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

fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// The end of the run of blanks at byte `from` of `bytes`.
fn blanks_end(bytes: &[u8], from: usize) -> usize {
    from + bytes
        .get(from..)
        .map_or(0, |rest| rest.iter().take_while(|&&b| is_blank(b)).count())
}

/// Whether `c` ends a plain scalar after a `:`.
fn ends_plain(c: char) -> bool {
    chars::is_space(c) || matches!(c, ',' | '[' | ']' | '{' | '}')
}

fn leading(line: &str) -> usize {
    line.len() - js::trim_start(line).len()
}

/// Splits `key: rest` at the start of a line: the raw key and where the
/// value starts.
fn split_key(line: &str) -> Option<(&str, usize)> {
    let bytes = line.as_bytes();
    let first = *bytes.first()?;
    let end;
    let colon;
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
        colon = blanks_end(bytes, end);
        if bytes.get(colon) != Some(&b':') {
            return None;
        }
    } else {
        let first_char = line.chars().next()?;
        if chars::is_space(first_char) || "#:,[]{}&*!|>'\"%@`".contains(first_char) {
            return None;
        }
        if first == b'-'
            && bytes
                .get(1)
                .is_none_or(|&b| is_blank(b) || matches!(b, b':' | b'#'))
        {
            return None;
        }
        colon = 1 + bytes[1..]
            .iter()
            .take_while(|&&b| !matches!(b, b':' | b'#'))
            .count();
        if bytes.get(colon) != Some(&b':') {
            return None;
        }
        end = bytes[..colon]
            .iter()
            .rposition(|&b| !is_blank(b))
            .map_or(0, |at| at + 1);
    }
    if bytes.get(colon + 1).is_some_and(|&b| !is_blank(b)) {
        return None;
    }
    Some((&line[..end], blanks_end(bytes, colon + 1)))
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
    let push_breaks = |out: &mut String, breaks: usize| match breaks {
        0 => {}
        1 => out.push(' '),
        _ => out.push_str(&"\n".repeat(breaks - 1)),
    };
    let mut breaks = 0;
    for c in joined.chars() {
        if c == '\n' {
            breaks += 1;
            continue;
        }
        push_breaks(&mut out, breaks);
        breaks = 0;
        out.push(c);
    }
    push_breaks(&mut out, breaks);
    out
}

fn unquote_double(body: &str) -> Result<String> {
    let s = fold(&RE_CONTINUED.replace(body, |_| String::new()));
    let mut chars = s.chars();
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(e) = chars.next() else {
            return error("YAML: unknown escape sequence");
        };
        let digits = match e {
            'x' => 2,
            'u' => 4,
            'U' => 8,
            _ => {
                let Some(c) = escape(e) else {
                    return error("YAML: unknown escape sequence");
                };
                out.push(c);
                continue;
            }
        };
        let digits: String = chars.by_ref().take(digits).collect();
        let code = js::parse_int(&digits, 16);
        match (code.is_finite() && code >= 0.0)
            .then(|| char::from_u32(code as u32))
            .flatten()
        {
            Some(c) => out.push(c),
            None => return error("Invalid code point"),
        }
    }
    Ok(out)
}

fn skip(s: &str, mut pos: usize) -> usize {
    let bytes = s.as_bytes();
    while pos < bytes.len() {
        let c = bytes[pos];
        if is_blank(c) {
            pos += 1;
        } else if c == b'#'
            && (pos == 0 || s[..pos].chars().next_back().is_some_and(chars::is_space))
        {
            pos = s[pos..].find('\n').map_or(s.len(), |nl| pos + nl);
        } else {
            break;
        }
    }
    pos
}

/// Reads the entries of a flow collection from `pos`, past its opening
/// bracket, with `entry`, which answers where each ends. Answers the index
/// after `close`.
fn flow_entries(
    s: &str,
    mut pos: usize,
    close: u8,
    mut entry: impl FnMut(usize) -> Result<usize>,
) -> Result<usize> {
    let bytes = s.as_bytes();
    let mut separated = true;
    loop {
        pos = skip(s, pos);
        let Some(&c) = bytes.get(pos) else {
            return Err(Failure::Incomplete(
                "YAML: unexpected end of the stream within a flow collection".into(),
            ));
        };
        if c == close {
            return Ok(pos + 1);
        }
        // Entries are separated by exactly one comma, as js-yaml requires.
        if !separated {
            return error("YAML: missed comma between flow collection entries");
        }
        if c == b',' {
            return error("YAML: expected the node content, but found ','");
        }
        pos = skip(s, entry(pos)?);
        separated = bytes.get(pos) == Some(&b',');
        if separated {
            pos += 1;
        }
    }
}

/// Parses a flow value starting at `pos`: the value and the index after it.
fn flow(s: &str, pos: usize, stops: &str, depth: usize) -> Result<(Value, usize)> {
    if depth > MAX_DEPTH {
        return error("YAML: nesting is too deep");
    }
    let bytes = s.as_bytes();
    let pos = skip(s, pos);
    let ch = bytes.get(pos).copied();
    if ch == Some(b'{') {
        let mut out = Map::new();
        let end = flow_entries(s, pos + 1, b'}', |pos| {
            let (key, after) = flow(s, pos, ":,}", depth + 1)?;
            let mut pos = skip(s, after);
            let mut value = Value::Null;
            if bytes.get(pos) == Some(&b':') {
                (value, pos) = flow(s, pos + 1, ",}", depth + 1)?;
            }
            out.insert(key.to_js_string(), value);
            Ok(pos)
        })?;
        return Ok((Value::Object(out), end));
    }
    if ch == Some(b'[') {
        let mut out = Vec::new();
        let end = flow_entries(s, pos + 1, b']', |pos| {
            let (value, after) = flow(s, pos, ",]", depth + 1)?;
            out.push(value);
            Ok(after)
        })?;
        return Ok((Value::Array(out), end));
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
                || s[end + 1..].chars().next().is_some_and(ends_plain))
        {
            break;
        }
        if c == b'#' && end > pos && s[..end].chars().next_back().is_some_and(chars::is_space) {
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
    let mut quote = None;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = quote {
            if c == b'\\' && q == b'"' {
                i += 1;
            } else if c == q {
                quote = None;
            }
        } else if c == b'"' || c == b'\'' {
            quote = Some(c);
        } else if c == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return &s[..i];
        }
        i += 1;
    }
    s
}

/// A block scalar's chomping indicator: what becomes of its final line
/// breaks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Chomp {
    /// `-`: none are kept.
    Strip,
    /// No indicator: one is kept.
    Clip,
    /// `+`: all are kept.
    Keep,
}

impl Chomp {
    /// The indicator in a block scalar's `header`.
    fn of(header: &str) -> Self {
        if header.contains('-') {
            Self::Strip
        } else if header.contains('+') {
            Self::Keep
        } else {
            Self::Clip
        }
    }
}

fn is_seq(line: &str) -> bool {
    line == "-" || line.starts_with("- ")
}

impl Reader {
    fn block_scalar(&mut self, header: &str, parent_indent: usize) -> String {
        let folded = header.starts_with('>');
        let chomp = Chomp::of(header);
        let mut indent = header
            .chars()
            .find_map(|c| c.to_digit(10).filter(|&d| d > 0))
            .map(|d| parent_indent + d as usize);
        let mut out: Vec<String> = Vec::new();
        while self.i < self.lines.len() {
            let line = self.lines[self.i].clone();
            if js::trim(&line).is_empty() {
                out.push(String::new());
                self.i += 1;
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
            self.i += 1;
        }
        let mut trailing = 0;
        while out.last().is_some_and(String::is_empty) {
            out.pop();
            trailing += 1;
        }
        let text = if folded {
            let mut text = String::new();
            let mut prev: Option<&String> = None;
            for cur in &out {
                if let Some(prev) = prev {
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
                prev = Some(cur);
            }
            text
        } else {
            out.join("\n")
        };
        if out.is_empty() {
            return if chomp == Chomp::Keep {
                "\n".repeat(trailing)
            } else {
                String::new()
            };
        }
        match chomp {
            Chomp::Strip => text,
            Chomp::Keep => text + &"\n".repeat(trailing + 1),
            Chomp::Clip => text + "\n",
        }
    }

    /// The value `rest` starts, with the lines after it that are indented
    /// past `parent`; with no `parent`, every line.
    fn inline_value(&mut self, rest: &str, parent: Option<usize>) -> Result<Value> {
        let first = rest.as_bytes().first().copied();
        if matches!(first, Some(b'|' | b'>')) && RE_BLOCK_HEADER.test(rest) {
            return Ok(Value::String(self.block_scalar(rest, parent.unwrap_or(0))));
        }
        if matches!(first, Some(b'{' | b'[' | b'"' | b'\'')) {
            // The value may continue on later lines: read them in growing
            // batches until it closes.
            let start = self.i;
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
                        self.i = start + used;
                        return Ok(value);
                    }
                    Err(Failure::Incomplete(message)) => {
                        if start + taken >= self.lines.len() {
                            return Err(Failure::Error(message));
                        }
                        let to = (start + taken + batch).min(self.lines.len());
                        let more = &self.lines[start + taken..to];
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
        while self.i < self.lines.len() {
            let line = &self.lines[self.i];
            if js::trim(line).is_empty() {
                let mut j = self.i;
                while j < self.lines.len() && js::trim(&self.lines[j]).is_empty() {
                    j += 1;
                }
                if j >= self.lines.len()
                    || parent.is_some_and(|parent| leading(&self.lines[j]) <= parent)
                {
                    break;
                }
                text.push_str(&"\n".repeat(j - self.i));
                self.i = j;
                continue;
            }
            if parent.is_some_and(|parent| leading(line) <= parent) {
                break;
            }
            let more = js::trim(strip_comment(line)).to_string();
            if !text.ends_with('\n') {
                text.push(' ');
            }
            text.push_str(&more);
            self.i += 1;
        }
        Ok(scalar(&text))
    }

    fn block(&mut self, min_indent: usize, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return error("YAML: nesting is too deep");
        }
        self.skip_blank();
        if self.i >= self.lines.len() {
            return Ok(Value::Null);
        }
        let indent = self.indent()?;
        if indent < min_indent {
            return Ok(Value::Null);
        }
        let line = self.lines[self.i][indent..].to_string();

        if is_seq(&line) {
            let mut out = Vec::new();
            while self.i < self.lines.len() {
                self.skip_blank();
                if self.i >= self.lines.len() || self.indent()? != indent {
                    break;
                }
                let cur = self.lines[self.i][indent..].to_string();
                if !is_seq(&cur) {
                    break;
                }
                let rest = &cur[1..];
                let pad = rest.len() - js::trim_start(rest).len() + 1;
                let body = js::trim(strip_comment(rest));
                if body.is_empty() {
                    self.i += 1;
                    out.push(self.block(indent + 1, depth + 1)?);
                } else if split_key(body).is_some() {
                    self.lines[self.i] =
                        format!("{}{}", " ".repeat(indent + pad), js::trim_start(rest));
                    out.push(self.block(indent + pad, depth + 1)?);
                } else {
                    self.i += 1;
                    out.push(self.inline_value(js::trim(rest), Some(indent))?);
                }
            }
            return Ok(Value::Array(out));
        }

        if split_key(&line).is_none() {
            self.i += 1;
            return self.inline_value(js::trim(&line), indent.checked_sub(1));
        }

        let mut out = Map::new();
        while self.i < self.lines.len() {
            self.skip_blank();
            if self.i >= self.lines.len() {
                break;
            }
            let k = self.indent()?;
            if k < indent {
                break;
            }
            if k > indent {
                return error("YAML: bad indentation of a mapping entry");
            }
            let cur = self.lines[self.i][indent..].to_string();
            let Some((raw_key, length)) = split_key(&cur) else {
                return error("YAML: can not read a block mapping entry");
            };
            let key = flow(raw_key, 0, "", 0)?.0.to_js_string();
            let rest = cur[length..].to_string();
            self.i += 1;
            if js::trim(strip_comment(&rest)).is_empty() {
                self.skip_blank();
                if self.i < self.lines.len() {
                    let next = self.indent()?;
                    let next_line = self.lines[self.i][next..].to_string();
                    let seq = is_seq(&next_line);
                    let value = if next > indent || (seq && next == indent) {
                        self.block(if seq { indent } else { indent + 1 }, depth + 1)?
                    } else {
                        Value::Null
                    };
                    out.insert(key, value);
                } else {
                    out.insert(key, Value::Null);
                }
            } else {
                let value = self.inline_value(js::trim(&rest), Some(indent))?;
                out.insert(key, value);
            }
        }
        Ok(Value::Object(out))
    }
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
    let value = r.block(0, 0).map_err(fail)?;
    r.skip_blank();
    if r.i < r.lines.len() && js::trim(&r.lines[r.i]) != "..." {
        return Err(YamlError(
            "YAML: end of the stream or a document separator is expected".into(),
        ));
    }
    Ok(value)
}
