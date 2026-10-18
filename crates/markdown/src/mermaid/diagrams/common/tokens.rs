//! Tokenizer for the diagram types Mermaid defines with Langium grammars.
//! Token types are tried in order and the first match wins, as in
//! Chevrotain. Pele's `diagrams/common/tokens.ts`.

use std::borrow::Cow;

use crate::mermaid::{
    detect::Kind,
    error::Error,
    js::{self, Lazy, Regex},
};

/// The end of a match at a position, or `None`.
pub type Scanner = Box<dyn FnMut(&str, usize) -> Option<usize>>;

pub struct TokenType {
    pub name: Cow<'static, str>,
    pub pattern: Regex,
    pub hidden: bool,
    /// Start of a construct that needs a closer. When this matches but the
    /// whole pattern does not, no closer exists in the rest of the text, so
    /// the pattern is not tried again.
    pub opener: Option<Regex>,
    /// Every ASCII character a match can start with, when that is known.
    pub first: Option<&'static str>,
    /// Chevrotain's `LONGER_ALT`, by name: tried at the same place when this
    /// matches, the first one matching more text becoming the token.
    pub longer: Vec<&'static str>,
    /// A matcher made once per text, for a pattern that could rescan it.
    pub scanner: Option<fn() -> Scanner>,
    /// Matches in place of the pattern, needing no memory between calls.
    pub matcher: Option<fn(&str, usize) -> Option<usize>>,
}

impl TokenType {
    /// A sticky regular expression.
    pub fn regex(name: &'static str, pattern: &str, first: Option<&'static str>) -> Self {
        Self {
            name: Cow::Borrowed(name),
            pattern: Regex::new(pattern, "y"),
            hidden: false,
            opener: None,
            first,
            longer: Vec::new(),
            scanner: None,
            matcher: None,
        }
    }

    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    pub fn named(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = name.into();
        self
    }

    pub fn opener(mut self, pattern: &str) -> Self {
        self.opener = Some(Regex::new(pattern, "y"));
        self
    }
}

/// A diagram's token types, with what each ASCII character can start.
pub struct Table {
    pub types: Vec<TokenType>,
    starting: Vec<Vec<usize>>,
    every: Vec<usize>,
    longer: Vec<Vec<usize>>,
}

impl Table {
    pub fn new(types: Vec<TokenType>) -> Self {
        let every: Vec<usize> = (0..types.len()).collect();
        let mut starting = Vec::with_capacity(128);
        for c in 0..128u8 {
            let ch = c as char;
            starting.push(
                every
                    .iter()
                    .copied()
                    .filter(|&k| types[k].first.is_none_or(|first| first.contains(ch)))
                    .collect(),
            );
        }
        let longer = types
            .iter()
            .map(|t| {
                t.longer
                    .iter()
                    .filter_map(|name| types.iter().position(|other| other.name == *name))
                    .collect()
            })
            .collect();
        Self {
            types,
            starting,
            every,
            longer,
        }
    }
}

pub struct Tokens<'s> {
    pub kinds: Vec<i32>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
    pub src: &'s str,
}

pub fn tokenize<'s>(src: &'s str, table: &Table, diagram: Kind) -> Result<Tokens<'s>, Error> {
    let types = &table.types;
    let mut kinds = Vec::new();
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut dead = vec![false; types.len()];
    let mut scanners: Vec<Option<Scanner>> = (0..types.len()).map(|_| None).collect();
    let n = src.len();

    let mut matcher = |k: usize, p: usize, dead: &mut Vec<bool>| -> Option<usize> {
        if dead[k] {
            return None;
        }
        let ty = &types[k];
        if let Some(matcher) = ty.matcher {
            return matcher(src, p);
        }
        if let Some(make) = ty.scanner {
            let scanner = scanners[k].get_or_insert_with(make);
            return scanner(src, p);
        }
        let end = ty.pattern.match_end(src, p);
        if end.is_none()
            && let Some(opener) = &ty.opener
            && opener.match_end(src, p).is_some()
        {
            dead[k] = true;
        }
        end
    };

    let mut p = 0;
    'scan: while p < n {
        let c = src.as_bytes()[p];
        let candidates = if c < 128 {
            &table.starting[c as usize]
        } else {
            &table.every
        };
        for &k in candidates {
            let Some(mut end) = matcher(k, p, &mut dead) else {
                continue;
            };
            if end <= p {
                continue;
            }
            let mut kind = k;
            for &alt in &table.longer[k] {
                if let Some(alt_end) = matcher(alt, p, &mut dead)
                    && alt_end > end
                {
                    kind = alt;
                    end = alt_end;
                    break;
                }
            }
            if !types[kind].hidden {
                kinds.push(kind as i32);
                starts.push(p);
                ends.push(end);
            }
            p = end;
            continue 'scan;
        }
        return Err(Error::at(diagram, src, p, "", true));
    }
    kinds.push(-1);
    starts.push(n);
    ends.push(n);
    Ok(Tokens {
        kinds,
        starts,
        ends,
        src,
    })
}

pub fn acc_descr() -> TokenType {
    TokenType::regex(
        "ACC_DESCR",
        r"[\t ]*accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*\{([^}]*)\})",
        Some("\t a"),
    )
}

pub fn acc_title() -> TokenType {
    TokenType::regex(
        "ACC_TITLE",
        r"[\t ]*accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)",
        Some("\t a"),
    )
}

pub fn title() -> TokenType {
    TokenType::regex(
        "TITLE",
        r"[\t ]*title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)",
        Some("\t t"),
    )
}

pub fn newline() -> TokenType {
    TokenType::regex("NEWLINE", r"\r?\n", Some("\r\n"))
}

pub fn whitespace() -> TokenType {
    TokenType::regex("WHITESPACE", r"[\t ]+", Some("\t ")).hidden()
}

pub fn yaml() -> TokenType {
    TokenType::regex(
        "YAML",
        r"---[\t ]*\r?\n(?:[\S\s]*?\r?\n)?---(?:\r?\n|(?!\S))",
        Some("-"),
    )
    .hidden()
    .opener(r"---[\t ]*\r?\n")
}

pub fn directive() -> TokenType {
    TokenType::regex(
        "DIRECTIVE",
        r"[\t ]*%%\{[\S\s]*?\}%%(?:\r?\n|(?!\S))",
        Some("\t %"),
    )
    .hidden()
    .opener(r"[\t ]*%%\{")
}

pub fn single_line_comment() -> TokenType {
    TokenType::regex("SINGLE_LINE_COMMENT", r"[\t ]*%%[^\n\r]*", Some("\t %")).hidden()
}

/// A keyword, which Mermaid ends only at whitespace, a comment, or the end of
/// input.
pub fn keyword(word: &'static str) -> TokenType {
    let mut escaped = String::new();
    for c in word.chars() {
        if ".*+?^${}()|[]\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    let first = &word[..word.chars().next().map_or(0, char::len_utf8)];
    let mut ty = TokenType::regex("", &format!(r"{escaped}(?:(?=%%)|(?!\S))"), None).named(word);
    ty.first = Some(first);
    ty
}

static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr(?:[\t ]*:([^\n\r]*)|\s*\{([^}]*)\})", "");
static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle[\t ]*:([^\n\r]*)", "");
static RE_TITLE: Lazy = Lazy::new(r"title([\t ][^\n\r]*|)", "");
static RE_SPACES: Lazy = Lazy::new(r"[\t ]{2,}", "gm");
static RE_LEAD: Lazy = Lazy::new(r"^\s*", "gm");
static RE_TRAIL: Lazy = Lazy::new(r"\s+$", "gm");
static RE_BREAKS: Lazy = Lazy::new(r"[\n\r]{2,}", "gm");

fn single(text: &str) -> String {
    RE_SPACES.replace(js::trim(text), |_| " ".into())
}

pub fn title_value(text: &str) -> String {
    single(RE_TITLE.exec(text).map_or("", |m| m.str(1)))
}

pub fn acc_title_value(text: &str) -> String {
    single(RE_ACC_TITLE.exec(text).map_or("", |m| m.str(1)))
}

pub fn acc_descr_value(text: &str) -> String {
    let Some(m) = RE_ACC_DESCR.exec(text) else {
        return String::new();
    };
    if let Some(line) = m.get(1) {
        return single(line);
    }
    let body = RE_LEAD.replace(m.str(2), |_| String::new());
    let body = RE_TRAIL.replace(&body, |_| String::new());
    let body = RE_SPACES.replace(&body, |_| " ".into());
    RE_BREAKS.replace(&body, |_| "\n".into())
}

/// Reads tokens for a hand-written parser, with Chevrotain-style messages on
/// failure.
pub struct Reader<'t, 's> {
    pub i: usize,
    pub tokens: &'t Tokens<'s>,
    pub table: &'t Table,
    pub diagram: Kind,
}

impl<'t, 's> Reader<'t, 's> {
    pub fn new(tokens: &'t Tokens<'s>, table: &'t Table, diagram: Kind) -> Self {
        Self {
            i: 0,
            tokens,
            table,
            diagram,
        }
    }

    pub fn kind(&self) -> i32 {
        self.tokens.kinds[self.i]
    }

    pub fn text_at(&self, index: usize) -> &'s str {
        &self.tokens.src[self.tokens.starts[index]..self.tokens.ends[index]]
    }

    pub fn text(&self) -> &'s str {
        self.text_at(self.i)
    }

    pub fn take(&mut self) -> &'s str {
        let text = self.text_at(self.i);
        self.i += 1;
        text
    }

    pub fn accept(&mut self, kind: i32) -> bool {
        if self.kind() != kind {
            return false;
        }
        self.i += 1;
        true
    }

    pub fn expect(&mut self, kind: i32) -> Result<&'s str, Error> {
        if self.kind() != kind {
            return Err(self.fail(&format!(
                "token of type '{}'",
                self.table.types[kind as usize].name
            )));
        }
        Ok(self.take())
    }

    pub fn fail(&self, expected: &str) -> Error {
        let found = if self.kind() == -1 {
            "end of input".to_string()
        } else {
            format!("`{}`", self.text())
        };
        Error::at(
            self.diagram,
            self.tokens.src,
            self.tokens.starts[self.i],
            &format!("Expecting {expected} but found {found}."),
            false,
        )
    }
}
