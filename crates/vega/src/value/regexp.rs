//! JavaScript's `RegExp` objects, on the `regex` crate: a pattern in
//! JavaScript's syntax is rewritten to the crate's, with `\d`, `\w`, `\s`,
//! `\b` and `.` matching what they match in JavaScript.
//!
//! Lookaround, backreferences and the `v` flag are a `SyntaxError`.

use d3::string;
use regex::{Captures, Regex, RegexBuilder};
use std::{fmt, fmt::Write as _};

/// A `RegExp` object: its pattern, and the flags it was made with.
#[derive(Clone, Debug)]
pub struct RegExp {
    pub source: String,
    pub flags: String,
    regex: Regex,
    global: bool,
    sticky: bool,
}

/// A `SyntaxError` `new RegExp` throws, by its message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError(pub String);

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SyntaxError {}

impl RegExp {
    /// `new RegExp(source, flags)`, or the `SyntaxError` it throws.
    pub fn new(source: &str, flags: &str) -> Result<Self, SyntaxError> {
        let (mut global, mut ignore_case, mut multiline) = (false, false, false);
        let (mut dot_all, mut unicode, mut sticky, mut indices) = (false, false, false, false);
        for flag in flags.chars() {
            let set = match flag {
                'g' => &mut global,
                'i' => &mut ignore_case,
                'm' => &mut multiline,
                's' => &mut dot_all,
                'u' => &mut unicode,
                'y' => &mut sticky,
                'd' => &mut indices,
                _ => return Err(invalid_flags(flags)),
            };
            if *set {
                return Err(invalid_flags(flags));
            }
            *set = true;
        }
        let invalid = |message: &str| {
            SyntaxError(format!(
                "Invalid regular expression: /{source}/{flags}: {message}"
            ))
        };
        let pattern = translate(source, dot_all, unicode).map_err(invalid)?;
        let regex = RegexBuilder::new(&pattern)
            .case_insensitive(ignore_case)
            .multi_line(multiline)
            .crlf(multiline)
            .build()
            .map_err(|error| invalid(&error.to_string()))?;
        Ok(Self {
            source: source.to_string(),
            flags: flags.to_string(),
            regex,
            global,
            sticky,
        })
    }

    pub fn global(&self) -> bool {
        self.global
    }

    /// The match at or after byte `at`; for a sticky pattern only at `at`.
    fn captures_at<'t>(&self, text: &'t str, at: usize) -> Option<Captures<'t>> {
        let caps = self.regex.captures_at(text, at)?;
        (!self.sticky || caps.get_match().start() == at).then_some(caps)
    }

    /// `re.test(text)` from the start.
    pub fn test(&self, text: &str) -> bool {
        self.captures_at(text, 0).is_some()
    }

    /// `text.match(re)` for a global pattern: every match, an empty one
    /// stepping one character on.
    pub fn matches<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let mut found = Vec::new();
        let mut at = 0;
        while at <= text.len() {
            let Some(caps) = self.captures_at(text, at) else {
                break;
            };
            let m = caps.get_match();
            found.push(m.as_str());
            at = if m.is_empty() {
                m.end() + text[m.end()..].chars().next().map_or(1, char::len_utf8)
            } else {
                m.end()
            };
        }
        found
    }

    /// `text.replace(re, replacement)`: every match when the pattern is
    /// global, else the first, `replacement` read for `$` patterns. An empty
    /// match steps one character on.
    pub fn replace(&self, text: &str, replacement: &str) -> String {
        let names: Vec<(usize, &str)> = self
            .regex
            .capture_names()
            .enumerate()
            .filter_map(|(i, name)| Some((i, name?)))
            .collect();
        let mut out = String::with_capacity(text.len());
        let (mut last, mut at) = (0, 0);
        while at <= text.len() {
            let Some(caps) = self.captures_at(text, at) else {
                break;
            };
            let found = caps.get_match();
            let groups: Vec<_> = (1..caps.len())
                .map(|i| caps.get(i).map(|m| m.as_str()))
                .collect();
            let named: Vec<_> = names
                .iter()
                .map(|&(i, name)| (name, caps.get(i).map(|m| m.as_str())))
                .collect();
            out.push_str(&text[last..found.start()]);
            out.push_str(&string::substitute(
                replacement,
                text,
                found.range(),
                &groups,
                &named,
            ));
            last = found.end();
            if !self.global {
                break;
            }
            at = if found.is_empty() {
                found.end() + text[found.end()..].chars().next().map_or(1, char::len_utf8)
            } else {
                found.end()
            };
        }
        out.push_str(&text[last..]);
        out
    }

    /// `text.split(re)`: groups are spliced in between the pieces, a group
    /// that took no part as `None`.
    pub fn split<'t>(&self, text: &'t str) -> Vec<Option<&'t str>> {
        if text.is_empty() {
            return match self.regex.captures(text) {
                Some(caps) if caps.get_match().start() == 0 => Vec::new(),
                _ => vec![Some(text)],
            };
        }
        let mut out = Vec::new();
        let (mut p, mut q) = (0, 0);
        while q < text.len() {
            let Some(caps) = self.regex.captures_at(text, q) else {
                break;
            };
            let found = caps.get_match();
            if found.start() >= text.len() {
                break;
            }
            if found.end() == p {
                q = found.start()
                    + text[found.start()..]
                        .chars()
                        .next()
                        .map_or(1, char::len_utf8);
                continue;
            }
            out.push(Some(&text[p..found.start()]));
            out.extend((1..caps.len()).map(|i| caps.get(i).map(|m| m.as_str())));
            p = found.end();
            q = p;
        }
        out.push(Some(&text[p..]));
        out
    }
}

impl fmt::Display for RegExp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let source = if self.source.is_empty() {
            "(?:)"
        } else {
            &self.source
        };
        write!(f, "/{source}/{}", self.flags)
    }
}

fn invalid_flags(flags: &str) -> SyntaxError {
    SyntaxError(format!(
        "Invalid flags supplied to RegExp constructor '{flags}'"
    ))
}

/// `source`, a pattern in JavaScript's syntax, in the `regex` crate's.
fn translate(source: &str, dot_all: bool, unicode: bool) -> Result<String, &'static str> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut class = false;
    let mut i = 0;
    let hex = |from: usize, len: usize| -> Option<u32> {
        let digits: String = chars.get(from..from + len)?.iter().collect();
        digits
            .chars()
            .all(|c| c.is_ascii_hexdigit())
            .then(|| u32::from_str_radix(&digits, 16).ok())
            .flatten()
    };
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            '\\' => {
                let Some(&e) = chars.get(i) else {
                    return Err("\\ at end of pattern");
                };
                i += 1;
                match e {
                    'd' => out.push_str("[0-9]"),
                    'D' => out.push_str("[^0-9]"),
                    'w' => out.push_str("[0-9A-Z_a-z]"),
                    'W' => out.push_str("[^0-9A-Z_a-z]"),
                    's' => {
                        let _ = write!(out, "[{}]", string::SPACE);
                    }
                    'S' => {
                        let _ = write!(out, "[^{}]", string::SPACE);
                    }
                    'b' if class => out.push_str(r"\x08"),
                    'b' => out.push_str(r"(?-u:\b)"),
                    'B' => out.push_str(r"(?-u:\B)"),
                    'n' => out.push_str(r"\n"),
                    'r' => out.push_str(r"\r"),
                    't' => out.push_str(r"\t"),
                    'f' => out.push_str(r"\x0C"),
                    'v' => out.push_str(r"\x0B"),
                    '0' if !chars.get(i).is_some_and(char::is_ascii_digit) => {
                        out.push_str(r"\x00");
                    }
                    '1'..='9' if !class => return Err("Backreferences are not supported"),
                    'k' if !class && chars.get(i) == Some(&'<') => {
                        return Err("Backreferences are not supported");
                    }
                    'x' => match hex(i, 2) {
                        Some(code) => {
                            let _ = write!(out, r"\x{{{code:X}}}");
                            i += 2;
                        }
                        None => out.push('x'),
                    },
                    'u' if unicode && chars.get(i) == Some(&'{') => {
                        let close = chars[i..]
                            .iter()
                            .position(|&c| c == '}')
                            .ok_or("Invalid Unicode escape")?;
                        let code = hex(i + 1, close - 1).ok_or("Invalid Unicode escape")?;
                        char::from_u32(code).ok_or("Invalid Unicode escape")?;
                        let _ = write!(out, r"\x{{{code:X}}}");
                        i += close + 1;
                    }
                    'u' => match hex(i, 4) {
                        Some(high @ 0xd800..=0xdbff)
                            if chars.get(i + 4) == Some(&'\\')
                                && chars.get(i + 5) == Some(&'u')
                                && hex(i + 6, 4)
                                    .is_some_and(|low| (0xdc00..=0xdfff).contains(&low)) =>
                        {
                            let low = hex(i + 6, 4).unwrap_or_default();
                            let code = 0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00);
                            let _ = write!(out, r"\x{{{code:X}}}");
                            i += 10;
                        }
                        Some(0xd800..=0xdfff) => return Err("Lone surrogates are not supported"),
                        Some(code) => {
                            let _ = write!(out, r"\x{{{code:X}}}");
                            i += 4;
                        }
                        None => out.push('u'),
                    },
                    'c' if chars.get(i).is_some_and(char::is_ascii_alphabetic) => {
                        let _ = write!(out, r"\x{{{:X}}}", u32::from(chars[i]) % 32);
                        i += 1;
                    }
                    'p' | 'P' if unicode => {
                        out.push('\\');
                        out.push(e);
                    }
                    e if e.is_ascii_punctuation() => {
                        out.push('\\');
                        out.push(e);
                    }
                    e => out.push_str(&regex::escape(&e.to_string())),
                }
            }
            '[' if class => out.push_str(r"\["),
            '[' => {
                match (chars.get(i), chars.get(i + 1)) {
                    (Some(']'), _) => {
                        out.push_str("[a&&b]");
                        i += 1;
                        continue;
                    }
                    (Some('^'), Some(']')) => {
                        out.push_str("(?s:.)");
                        i += 2;
                        continue;
                    }
                    _ => {}
                }
                class = true;
                out.push('[');
                if chars.get(i) == Some(&'^') {
                    out.push('^');
                    i += 1;
                }
            }
            ']' if class => {
                class = false;
                out.push(']');
            }
            '&' | '~' if class => {
                out.push('\\');
                out.push(c);
            }
            '-' if class && chars.get(i) == Some(&'-') => out.push_str(r"\-"),
            _ if class => out.push(c),
            ']' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '{' => {
                // A quantifier, else a literal brace.
                let close = chars[i..].iter().position(|&c| c == '}');
                let quantifier = close.is_some_and(|close| {
                    let inside: String = chars[i..i + close].iter().collect();
                    let (min, max) = inside.split_once(',').unwrap_or((&inside, "0"));
                    !min.is_empty()
                        && min.chars().all(|c| c.is_ascii_digit())
                        && max.chars().all(|c| c.is_ascii_digit())
                });
                match close {
                    Some(close) if quantifier => {
                        out.extend(&chars[i - 1..=i + close]);
                        i += close + 1;
                    }
                    _ => out.push_str(r"\{"),
                }
            }
            '.' if dot_all => out.push_str("(?s:.)"),
            '.' => out.push_str("[^\n\r\u{2028}\u{2029}]"),
            '(' if chars.get(i) == Some(&'?') => match (chars.get(i + 1), chars.get(i + 2)) {
                (Some('=' | '!'), _) | (Some('<'), Some('=' | '!')) => {
                    return Err("Lookaround is not supported");
                }
                _ => out.push('('),
            },
            _ => out.push(c),
        }
    }
    if class {
        return Err("Unterminated character class");
    }
    Ok(out)
}
