//! What Mermaid does to source before a diagram's parser sees it: front
//! matter, `%%{init}%%` directives, `%%` comments, and entity codes.

use super::{
    config::{Map, Value, merge},
    error::Error,
    js::{self, JsValue, Lazy, parse_json},
    yaml::parse_yaml,
};

static RE_COMMENT: Lazy = Lazy::new(r"^\s*%%(?!\{)[^\n]+\n?", "gm");
static RE_DIRECTIVE: Lazy = Lazy::new(
    r"%{2}\{\s*(?:(\w+)\s*:|(\w+))\s*(?:(\w+)|((?:(?!\}%{2}).|\r?\n)*))?\s*(?:\}%{2})?",
    "gi",
);
static RE_HTML_TAG: Lazy = Lazy::new(r"<(\w+)([^>]*)>", "g");
static RE_ATTR: Lazy = Lazy::new(r#"="([^"]*)""#, "g");
static RE_ENTITY: Lazy = Lazy::new(r"#\w+;", "g");
static RE_INT: Lazy = Lazy::new(r"^\+?\d+$", "");

pub struct Preprocessed {
    pub text: String,
    pub title: Option<String>,
    pub config: Map,
    /// The text before its comment lines are removed.
    pub with_comments: String,
    /// The number of front matter lines removed before it.
    pub line_offset: usize,
}

pub fn cleanup_comments(text: &str) -> String {
    let text = if text.contains("%%") {
        RE_COMMENT.replace(text, |_| String::new())
    } else {
        text.to_string()
    };
    js::trim_start(&text).to_string()
}

fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..)?.chars().next()
}

fn is_line_break(c: Option<char>) -> bool {
    matches!(c, Some('\n' | '\r'))
}

fn is_space(c: Option<char>) -> bool {
    c.is_some_and(js::is_space)
}

fn step(text: &str, at: usize) -> usize {
    at + char_at(text, at).map_or(1, char::len_utf8)
}

struct FrontMatter {
    indent: String,
    body: String,
    length: usize,
}

/// A `---` fenced YAML block at the start of the text, found the way
/// Mermaid's front matter pattern finds it.
fn match_front_matter(text: &str) -> Option<FrontMatter> {
    let mut cursor = 0;
    while cursor < text.len()
        && is_space(char_at(text, cursor))
        && !is_line_break(char_at(text, cursor))
    {
        cursor = step(text, cursor);
    }
    let indent = &text[..cursor];
    if !text[cursor..].starts_with("---") {
        return None;
    }

    let mut opening_ends = Vec::new();
    let mut i = cursor + 3;
    while i < text.len() && is_space(char_at(text, i)) {
        if is_line_break(char_at(text, i)) {
            opening_ends.push(i);
        }
        i = step(text, i);
    }
    let first_opening = *opening_ends.first()?;

    let fence = format!("{indent}---");
    let closing_end = |i: usize| -> Option<usize> {
        if !is_line_break(char_at(text, i)) || !text[i + 1..].starts_with(&fence) {
            return None;
        }
        let mut last = None;
        let mut j = i + 1 + fence.len();
        while j < text.len() && is_space(char_at(text, j)) {
            if is_line_break(char_at(text, j)) {
                last = Some(j);
            }
            j = step(text, j);
        }
        last
    };

    let needle = format!("\n{fence}");
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

    let mut opening_end = first_opening;
    for &candidate in &opening_ends {
        if candidate < last_fence {
            opening_end = candidate;
        }
    }

    let body_start = opening_end + 1;
    let first_at = if body_start > 0 { body_start - 1 } else { 0 };
    let mut from = first_at;
    while let Some(found) = text.get(from..).and_then(|rest| rest.find(&needle)) {
        let i = from + found;
        if i > last_fence {
            break;
        }
        if i >= body_start
            && let Some(end) = closing_end(i)
        {
            return Some(FrontMatter {
                indent: indent.to_string(),
                body: text[body_start..i].to_string(),
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
    let normalized = js::trim(text).replace('\'', "\"");
    for caps in RE_DIRECTIVE.captures_iter(&normalized) {
        let kind = caps.get(1).or_else(|| caps.get(2));
        match kind {
            Some("wrap") => {
                config.insert("wrap".into(), Value::Bool(true));
            }
            Some("init" | "initialize") => {
                if let Some(args) = caps.get(4).filter(|args| !args.is_empty())
                    && let Some(Value::Object(args)) = parse_json(js::trim(args))
                {
                    // Mermaid ignores an init directive whose body is not
                    // valid JSON.
                    merge(config, &args);
                }
            }
            _ => {}
        }
    }
    RE_DIRECTIVE.replace(text, |_| String::new())
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
        let head = RE_HTML_TAG.replace(&text[..=last_close], |caps| {
            let attrs = RE_ATTR.replace(caps.str(2), |attr| format!("='{}'", attr.str(1)));
            format!("<{}{}>", caps.str(1), attrs)
        });
        text = head + &text[last_close + 1..];
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
            front.body.clone()
        } else {
            front
                .body
                .split('\n')
                .map(|line| line.strip_prefix(front.indent.as_str()).unwrap_or(line))
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
        line_offset = text[..front.length.min(text.len())].matches('\n').count();
        text = text[front.length.min(text.len())..].to_string();
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
            } else if js::is_space(c) {
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
        let next_line = line_end + char_at(text, line_end).map_or(1, char::len_utf8);
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
    RE_ENTITY.replace(&text, |caps| {
        let s = caps.str(0);
        let inner = &s[1..s.len() - 1];
        let open = if RE_INT.test(inner) {
            "ﬂ°°"
        } else {
            "ﬂ°"
        };
        format!("{open}{inner}¶ß")
    })
}
