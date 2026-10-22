//! What Mermaid does to source before a diagram's parser sees it: front
//! matter, `%%{init}%%` directives, `%%` comments, and entity codes.

use super::{
    config::{Map, Value, merge},
    error::Error,
    util::chars::{self, is_inline_space, width_at},
    yaml::parse_yaml,
};
use crate::{json, json::JsValue};
use d3::string;
use regex::{Captures, Regex};
use std::{ops::Range, sync::LazyLock};

static RE_HTML_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<([0-9A-Z_a-z]+)([^>]*)>").expect("pattern"));
static RE_ATTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"="([^"]*)""#).expect("pattern"));
static RE_ENTITY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#[0-9A-Z_a-z]+;").expect("pattern"));

pub struct Preprocessed {
    pub text: String,
    pub title: Option<String>,
    pub config: Map,
    /// The text before its comment lines are removed.
    pub with_comments: String,
    /// The number of front matter lines removed before it.
    pub line_offset: usize,
}

/// `text` without its `%%` comment lines, as Mermaid's
/// `/^\s*%%(?!\{)[^\n]+\n?/gm` removes them, and its leading whitespace.
pub fn cleanup_comments(text: &str) -> String {
    if !text.contains("%%") {
        return string::trim_start(text).to_string();
    }
    let mut out = String::new();
    let mut last = 0;
    let mut line = 0;
    loop {
        let at = line + (text.len() - line - string::trim_start(&text[line..]).len());
        let rest = &text.as_bytes()[at..];
        if rest.starts_with(b"%%") && rest.get(2).is_some_and(|&c| c != b'{' && c != b'\n') {
            let end = memchr::memchr(b'\n', &rest[2..]).map_or(text.len(), |n| at + 2 + n + 1);
            out.push_str(&text[last..line]);
            last = end;
            line = end;
            continue;
        }
        match text[line..].find(|c: char| string::is_line_end(c)) {
            Some(n) => line += n + width_at(text, line + n),
            None => break,
        }
    }
    out.push_str(&text[last..]);
    string::trim_start(&out).to_string()
}

/// A `%%{…}%%` directive, as Mermaid's
/// `/%{2}\{\s*(?:(\w+)\s*:|(\w+))\s*(?:(\w+)|((?:(?!\}%{2}).|\r?\n)*))?\s*(?:\}%{2})?/gi`
/// matches it.
struct Directive<'t> {
    range: Range<usize>,
    kind: &'t str,
    /// What follows the kind when it is not a single word.
    args: Option<&'t str>,
}

impl<'t> Directive<'t> {
    /// The first directive in `text` from byte `from`.
    fn find(text: &'t str, mut from: usize) -> Option<Self> {
        let bytes = text.as_bytes();
        let space = |at: usize| at + (text.len() - at - string::trim_start(&text[at..]).len());
        let word = |at: usize| {
            at + bytes[at..]
                .iter()
                .take_while(|&&b| string::is_word(b))
                .count()
        };
        loop {
            let start = from + memchr::memmem::find(&bytes[from..], b"%%{")?;
            from = start + 1;
            let kind_start = space(start + 3);
            let kind_end = word(kind_start);
            if kind_end == kind_start {
                continue;
            }
            let colon = space(kind_end);
            let mut at = if bytes.get(colon) == Some(&b':') {
                colon + 1
            } else {
                kind_end
            };
            at = space(at);
            let mut args = None;
            let value_end = word(at);
            if value_end > at {
                at = value_end;
            } else {
                let args_start = at;
                loop {
                    if bytes[at..].starts_with(b"}%%") {
                        break;
                    }
                    match text[at..].chars().next() {
                        Some('\r') if bytes.get(at + 1) == Some(&b'\n') => at += 2,
                        Some(c) if !string::is_line_end(c) => at += c.len_utf8(),
                        Some('\n') => at += 1,
                        _ => break,
                    }
                }
                args = Some(&text[args_start..at]);
            }
            at = space(at);
            if bytes[at..].starts_with(b"}%%") {
                at += 3;
            }
            return Some(Self {
                range: start..at,
                kind: &text[kind_start..kind_end],
                args,
            });
        }
    }

    /// Every directive in `text`, in order.
    fn all(text: &'t str) -> impl Iterator<Item = Self> {
        let mut from = 0;
        std::iter::from_fn(move || {
            let found = Self::find(text, from)?;
            from = found.range.end;
            Some(found)
        })
    }
}

/// Byte offsets of the line breaks in the run of whitespace at `from`.
fn breaks_in_space(text: &str, from: usize) -> impl Iterator<Item = usize> + '_ {
    text[from..]
        .char_indices()
        .take_while(|&(_, c)| string::is_space(c))
        .filter(|&(_, c)| matches!(c, '\n' | '\r'))
        .map(move |(at, _)| from + at)
}

struct FrontMatter<'t> {
    indent: &'t str,
    body: &'t str,
    length: usize,
}

/// A `---` fenced YAML block at the start of the text, found the way
/// Mermaid's front matter pattern finds it.
fn match_front_matter(text: &str) -> Option<FrontMatter<'_>> {
    let indent_len = text.len() - text.trim_start_matches(|c: char| is_inline_space(c)).len();
    let indent = &text[..indent_len];
    if !text[indent_len..].starts_with("---") {
        return None;
    }

    let opening_ends: Vec<usize> = breaks_in_space(text, indent_len + 3).collect();
    let first_opening = *opening_ends.first()?;

    let needle = format!("\n{indent}---");
    let closing_end = |i: usize| breaks_in_space(text, i + needle.len()).last();

    let mut last_fence = None;
    let mut found = text.rfind(&needle);
    while let Some(i) = found {
        if i <= first_opening {
            break;
        }
        if closing_end(i).is_some() {
            last_fence = Some(i);
            break;
        }
        // `lastIndexOf(needle, i - 1)`: the last match starting before `i`.
        found = text[..i - 1 + needle.len()].rfind(&needle);
    }
    let last_fence = last_fence?;

    let opening_end = opening_ends
        .iter()
        .copied()
        .rev()
        .find(|&end| end < last_fence)
        .unwrap_or(first_opening);

    let body_start = opening_end + 1;
    let mut from = opening_end;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find(&needle)) {
        let i = from + found;
        if i > last_fence {
            break;
        }
        if i >= body_start
            && let Some(end) = closing_end(i)
        {
            return Some(FrontMatter {
                indent,
                body: &text[body_start..i],
                length: end + 1,
            });
        }
        from = i + 1;
    }
    None
}

fn directives(text: &str, config: &mut Map) -> String {
    if !text.contains("%%{") {
        return text.to_string();
    }
    let normalized = string::trim(text).replace('\'', "\"");
    for directive in Directive::all(&normalized) {
        match directive.kind {
            "wrap" => {
                config.insert("wrap".into(), Value::Bool(true));
            }
            "init" | "initialize" => {
                if let Some(args) = directive.args.filter(|args| !args.is_empty())
                    && let Some(Value::Object(args)) = json::parse_json(string::trim(args))
                {
                    // Mermaid ignores an init directive whose body is not
                    // valid JSON.
                    merge(config, &args);
                }
            }
            _ => {}
        }
    }
    let mut out = String::new();
    let mut last = 0;
    for directive in Directive::all(text) {
        out.push_str(&text[last..directive.range.start]);
        last = directive.range.end;
    }
    out + &text[last..]
}

pub fn preprocess(source: &str) -> Result<Preprocessed, Error> {
    let mut text = if source.contains('\r') {
        source.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        source.to_string()
    };
    // A tag needs a closing bracket, so nothing after the last one can match.
    if text.contains('<')
        && let Some(last_close) = text.rfind('>')
        && last_close > 0
    {
        let head = RE_HTML_TAG.replace_all(&text[..=last_close], |caps: &Captures| {
            let attrs = RE_ATTR.replace_all(&caps[2], "='$1'");
            format!("<{}{}>", &caps[1], attrs)
        });
        text = head.into_owned() + &text[last_close + 1..];
    }

    let mut config = Map::new();
    let mut title = None;
    let mut line_offset = 0;

    let front = if text.contains("---") {
        match_front_matter(&text)
    } else {
        None
    };
    if let Some(front) = front {
        let body = if front.indent.is_empty() {
            front.body.to_string()
        } else {
            front
                .body
                .split('\n')
                .map(|line| line.strip_prefix(front.indent).unwrap_or(line))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let parsed = parse_yaml(&body).map_err(|error| {
            Error::syntax(format!("Front matter is not valid YAML. {}", error.0))
        })?;
        if let Value::Object(parsed) = parsed {
            if let Some(found) = parsed.get("title").filter(|value| value.truthy()) {
                title = Some(found.to_js_string());
            }
            if let Some(Value::Object(found)) = parsed.get("config") {
                merge(&mut config, found);
            }
            if let Some(mode) = parsed.get("displayMode").filter(|value| value.truthy()) {
                if !config.get("gantt").is_some_and(Value::is_object) {
                    config.insert("gantt".into(), Value::Object(Map::new()));
                }
                if let Some(Value::Object(gantt)) = config.get_mut("gantt") {
                    gantt.insert("displayMode".into(), Value::String(mode.to_js_string()));
                }
            }
        }
        line_offset = text[..front.length].matches('\n').count();
        text = text[front.length..].to_string();
    }

    let text = directives(&text, &mut config);
    Ok(Preprocessed {
        text: cleanup_comments(&text),
        title,
        config,
        with_comments: text,
        line_offset,
    })
}

/// Mermaid's `text.replace(/keyword.*:\S*#.*;/g, …)`: on a line where the
/// keyword is followed by a colon, a colour-like `#`, and later a semicolon,
/// the last semicolon goes.
fn drop_color_semicolons(text: &str, keyword: &str) -> String {
    let Some(mut from) = text.find(keyword) else {
        return text.to_string();
    };
    let mut out = String::new();
    let mut last = 0;
    loop {
        let line_end = text[from..]
            .find(['\n', '\r', '\u{2028}', '\u{2029}'])
            .map_or(text.len(), |at| from + at);
        let mut after_colon = false;
        let mut hash = None;
        for (j, c) in text[from + keyword.len()..line_end].char_indices() {
            let j = from + keyword.len() + j;
            if c == ':' {
                after_colon = true;
            } else if c == '#' {
                if after_colon {
                    hash = Some(j);
                    break;
                }
            } else if string::is_space(c) {
                after_colon = false;
            }
        }
        if let Some(hash) = hash
            && let Some(semi) = text[..line_end].rfind(';')
            && semi > hash
        {
            out.push_str(&text[last..semi]);
            last = semi + 1;
        }
        if line_end >= text.len() {
            break;
        }
        let next_line = line_end + width_at(text, line_end);
        match text[next_line..].find(keyword) {
            Some(at) => from = next_line + at,
            None => break,
        }
    }
    if last == 0 {
        return text.to_string();
    }
    out + &text[last..]
}

/// Mermaid hides `#name;` entity codes behind placeholder characters so `#`
/// and `;` survive parsing.
pub fn encode_entities(text: &str) -> String {
    if !text.contains('#') {
        return text.to_string();
    }
    let text = drop_color_semicolons(&drop_color_semicolons(text, "style"), "classDef");
    RE_ENTITY
        .replace_all(&text, |caps: &Captures| {
            let s = &caps[0];
            let inner = chars::inner(s);
            let open = if inner.bytes().all(|b| b.is_ascii_digit()) {
                "ﬂ°°"
            } else {
                "ﬂ°"
            };
            format!("{open}{inner}¶ß")
        })
        .into_owned()
}
