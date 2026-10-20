//! Tokenizer for the diagram types Mermaid defines with Langium grammars.
//! Token types are tried in order and the first match wins, as in
//! Chevrotain. Pele's `diagrams/common/tokens.ts`.

use crate::util::chars::{self, char_at, digits_end};

use crate::{
    detect::Kind,
    diagrams::common::db::Titles,
    error::Error,
    js::{self, Lazy, Regex},
};

/// The end of a match at a position, or `None`.
pub type Scanner = Box<dyn FnMut(&str, usize) -> Option<usize>>;

/// A grammar's token kinds, in the order the lexer tries them. Declare one
/// with [`token_kinds!`].
pub trait Kinds: Copy + Eq + Send + Sync + 'static {
    /// Every kind, in table order.
    const ALL: &'static [Self];

    fn table() -> &'static Table<Self>;

    /// The kind's place in [`Kinds::ALL`] and in the table.
    fn index(self) -> usize;
}

/// Declares a token kind enum and its [`Kinds`], each variant with the token
/// type it stands for, in the order the lexer tries them.
macro_rules! token_kinds {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$variant_meta:meta])* $variant:ident = $token:expr,)*
    }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        $vis enum $name {
            $($(#[$variant_meta])* $variant,)*
        }

        impl $crate::diagrams::common::tokens::Kinds for $name {
            const ALL: &'static [Self] = &[$(Self::$variant,)*];

            fn table() -> &'static $crate::diagrams::common::tokens::Table<Self> {
                static TABLE: ::std::sync::LazyLock<
                    $crate::diagrams::common::tokens::Table<$name>,
                > = ::std::sync::LazyLock::new(|| {
                    $crate::diagrams::common::tokens::Table::new(vec![$($token,)*])
                });
                &TABLE
            }

            fn index(self) -> usize {
                self as usize
            }
        }
    };
}
pub(crate) use token_kinds;

enum Pattern {
    Literal(&'static str),
    Regex(Regex),
}

/// Which of Mermaid's title and accessibility statements a token is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heading {
    Title,
    AccTitle,
    AccDescr,
}

pub struct TokenType<K> {
    name: &'static str,
    pattern: Pattern,
    hidden: bool,
    /// Start of a construct that needs a closer. When this matches but the
    /// whole pattern does not, no closer exists in the rest of the text, so
    /// the pattern is not tried again.
    opener: Option<Regex>,
    /// Every ASCII character a match can start with, when that is known.
    first: Option<&'static str>,
    /// Chevrotain's `LONGER_ALT`: tried at the same place when this matches,
    /// the first one matching more text becoming the token.
    longer: Vec<K>,
    /// A matcher made once per text, for a pattern that could rescan it.
    scanner: Option<fn() -> Scanner>,
    /// Matches in place of the pattern, needing no memory between calls.
    matcher: Option<fn(&str, usize) -> Option<usize>>,
    heading: Option<Heading>,
}

impl<K> TokenType<K> {
    fn new(name: &'static str, pattern: Pattern, first: Option<&'static str>) -> Self {
        Self {
            name,
            pattern,
            hidden: false,
            opener: None,
            first,
            longer: Vec::new(),
            scanner: None,
            matcher: None,
            heading: None,
        }
    }

    /// `text` itself, named by it.
    pub fn literal(text: &'static str) -> Self {
        Self::new(text, Pattern::Literal(text), None)
    }

    /// A sticky regular expression.
    pub fn regex(name: &'static str, pattern: &str, first: Option<&'static str>) -> Self {
        Self::new(name, Pattern::Regex(Regex::new(pattern, "y")), first)
    }

    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    pub fn named(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }

    pub fn first(mut self, first: &'static str) -> Self {
        self.first = Some(first);
        self
    }

    pub fn longer(mut self, kinds: &[K]) -> Self
    where
        K: Copy,
    {
        self.longer = kinds.to_vec();
        self
    }

    pub fn opener(mut self, pattern: &str) -> Self {
        self.opener = Some(Regex::new(pattern, "y"));
        self
    }

    pub fn scanner(mut self, scanner: fn() -> Scanner) -> Self {
        self.scanner = Some(scanner);
        self
    }

    pub fn matcher(mut self, matcher: fn(&str, usize) -> Option<usize>) -> Self {
        self.matcher = Some(matcher);
        self
    }

    fn heading(mut self, heading: Heading) -> Self {
        self.heading = Some(heading);
        self
    }
}

/// A grammar's token types, with what each ASCII character can start.
pub struct Table<K> {
    types: Vec<TokenType<K>>,
    starting: Vec<Vec<usize>>,
    every: Vec<usize>,
    longer: Vec<Vec<usize>>,
}

impl<K: Kinds> Table<K> {
    /// `types` holds one token type per kind, in [`Kinds::ALL`] order.
    pub fn new(types: Vec<TokenType<K>>) -> Self {
        assert_eq!(types.len(), K::ALL.len(), "one token type per kind");
        let every: Vec<usize> = (0..types.len()).collect();
        let starting = (0..128u8)
            .map(|c| {
                let ch = char::from(c);
                every
                    .iter()
                    .copied()
                    .filter(|&k| match &types[k].pattern {
                        Pattern::Literal(text) => text.starts_with(ch),
                        Pattern::Regex(_) => types[k].first.is_none_or(|first| first.contains(ch)),
                    })
                    .collect()
            })
            .collect();
        let longer = types
            .iter()
            .map(|t| t.longer.iter().map(|kind| kind.index()).collect())
            .collect();
        Self {
            types,
            starting,
            every,
            longer,
        }
    }

    fn get(&self, kind: K) -> &TokenType<K> {
        &self.types[kind.index()]
    }

    /// The visible tokens of `src`.
    fn tokenize(&self, src: &str, diagram: Kind) -> Result<Vec<Token<K>>, Error> {
        let types = &self.types;
        let mut tokens = Vec::new();
        let mut dead = vec![false; types.len()];
        let mut scanners: Vec<Option<Scanner>> = (0..types.len()).map(|_| None).collect();

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
            match &ty.pattern {
                Pattern::Literal(text) => src[p..].starts_with(text).then(|| p + text.len()),
                Pattern::Regex(re) => {
                    let end = re.match_end(src, p);
                    if end.is_none()
                        && let Some(opener) = &ty.opener
                        && opener.match_end(src, p).is_some()
                    {
                        dead[k] = true;
                    }
                    end
                }
            }
        };

        let mut p = 0;
        'scan: while p < src.len() {
            let c = src.as_bytes()[p];
            let candidates = if c.is_ascii() {
                &self.starting[usize::from(c)]
            } else {
                &self.every
            };
            for &k in candidates {
                let Some(mut end) = matcher(k, p, &mut dead) else {
                    continue;
                };
                if end <= p {
                    continue;
                }
                let mut kind = k;
                for &alt in &self.longer[k] {
                    if let Some(alt_end) = matcher(alt, p, &mut dead)
                        && alt_end > end
                    {
                        kind = alt;
                        end = alt_end;
                        break;
                    }
                }
                if !types[kind].hidden {
                    tokens.push(Token {
                        kind: K::ALL[kind],
                        start: p,
                        end,
                    });
                }
                p = end;
                continue 'scan;
            }
            return Err(Error::at(diagram, src, p, "", true));
        }
        Ok(tokens)
    }
}

pub fn acc_descr<K>() -> TokenType<K> {
    TokenType::regex(
        "ACC_DESCR",
        r"[\t ]*accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*\{([^}]*)\})",
        Some("\t a"),
    )
    .heading(Heading::AccDescr)
}

pub fn acc_title<K>() -> TokenType<K> {
    TokenType::regex(
        "ACC_TITLE",
        r"[\t ]*accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)",
        Some("\t a"),
    )
    .heading(Heading::AccTitle)
}

pub fn title<K>() -> TokenType<K> {
    TokenType::regex(
        "TITLE",
        r"[\t ]*title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)",
        Some("\t t"),
    )
    .heading(Heading::Title)
}

pub fn string<K>() -> TokenType<K> {
    TokenType::regex("STRING", r#""([^"\\]|\\.)*"|'([^'\\]|\\.)*'"#, Some("\"'"))
}

pub fn newline<K>() -> TokenType<K> {
    TokenType::regex("NEWLINE", r"\r?\n", Some("\r\n"))
}

pub fn whitespace<K>() -> TokenType<K> {
    TokenType::regex("WHITESPACE", r"[\t ]+", Some("\t ")).hidden()
}

pub fn yaml<K>() -> TokenType<K> {
    TokenType::regex(
        "YAML",
        r"---[\t ]*\r?\n(?:[\S\s]*?\r?\n)?---(?:\r?\n|(?!\S))",
        Some("-"),
    )
    .hidden()
    .opener(r"---[\t ]*\r?\n")
}

pub fn directive<K>() -> TokenType<K> {
    TokenType::regex(
        "DIRECTIVE",
        r"[\t ]*%%\{[\S\s]*?\}%%(?:\r?\n|(?!\S))",
        Some("\t %"),
    )
    .hidden()
    .opener(r"[\t ]*%%\{")
}

pub fn single_line_comment<K>() -> TokenType<K> {
    TokenType::regex("SINGLE_LINE_COMMENT", r"[\t ]*%%[^\n\r]*", Some("\t %")).hidden()
}

/// A keyword, which Mermaid ends only at whitespace, a comment, or the end of
/// input.
pub fn keyword<K>(word: &'static str) -> TokenType<K> {
    let mut escaped = String::new();
    for c in word.chars() {
        if ".*+?^${}()|[]\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    let first = &word[..word.chars().next().map_or(0, char::len_utf8)];
    TokenType::regex(word, &format!(r"{escaped}(?:(?=%%)|(?!\S))"), Some(first))
}

/// Matches as `-?(?:[0-9]+\.[0-9]+(?!\.)|(?:0|[1-9][0-9]*)(?!\.))` does,
/// reading each run of digits once: the pattern reads to the end of a run
/// from every position in it, which is quadratic for a long run of zeros.
/// Without `signed` there is no `-`; with `lone_zero` a `0` matches even
/// before a point.
pub fn decimal(signed: bool, lone_zero: bool) -> Scanner {
    let mut from = 0;
    let mut end = 0;
    Box::new(move |src, p| {
        let at = if signed && char_at(src, p) == Some('-') {
            p + 1
        } else {
            p
        };
        if at < from || at >= end {
            from = at;
            end = digits_end(src, at);
        }
        if end == at {
            return None;
        }
        let dot = char_at(src, end) == Some('.');
        if dot {
            let stop = digits_end(src, end + 1);
            let length = stop - end - 1;
            if length > 0 && char_at(src, stop) != Some('.') {
                return Some(stop);
            }
            if length > 1 {
                return Some(stop - 1);
            }
        }
        if char_at(src, at) == Some('0') {
            return (lone_zero || char_at(src, at + 1) != Some('.')).then_some(at + 1);
        }
        if !dot {
            return Some(end);
        }
        (end - 1 > at).then_some(end - 1)
    })
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

/// A quoted string's text, its escapes read.
pub fn string_value(text: &str) -> String {
    let inner = chars::inner(text);
    if !text.contains('\\') {
        return inner.to_string();
    }
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('v') => out.push('\u{b}'),
            Some('0') => out.push('\0'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// A diagram's title and accessibility statements, their values read.
#[derive(Default)]
pub struct Header {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
}

impl Header {
    /// Keeps the value of a statement's token `text`.
    pub fn read(&mut self, heading: Heading, text: &str) {
        match heading {
            Heading::Title => self.title = Some(title_value(text)),
            Heading::AccTitle => self.acc_title = Some(acc_title_value(text)),
            Heading::AccDescr => self.acc_descr = Some(acc_descr_value(text)),
        }
    }

    /// A statement with an empty value counts as absent, and `fallback`
    /// stands in for an absent title.
    pub fn titles(self, fallback: Option<&str>) -> Titles {
        let mut titles = Titles::default();
        match self.title.filter(|text| !text.is_empty()) {
            Some(title) => titles.title = Some(title),
            None => titles.title = fallback.map(str::to_string),
        }
        if let Some(text) = self.acc_title.filter(|text| !text.is_empty()) {
            titles.set_acc_title(&text);
        }
        if let Some(text) = self.acc_descr.filter(|text| !text.is_empty()) {
            titles.set_acc_descr(&text);
        }
        titles
    }
}

struct Token<K> {
    kind: K,
    start: usize,
    end: usize,
}

/// Reads tokens for a hand-written parser, with Chevrotain-style messages on
/// failure.
pub struct Reader<'s, K> {
    src: &'s str,
    tokens: Vec<Token<K>>,
    i: usize,
    diagram: Kind,
}

impl<'s, K: Kinds> Reader<'s, K> {
    /// The tokens of `src`, or the lexical error in `diagram` where none
    /// matches.
    pub fn new(src: &'s str, diagram: Kind) -> Result<Self, Error> {
        Ok(Self {
            src,
            tokens: K::table().tokenize(src, diagram)?,
            i: 0,
            diagram,
        })
    }

    /// The kind of the token here, or `None` at the end of input.
    pub fn kind(&self) -> Option<K> {
        self.peek(0)
    }

    /// The kind of the token `ahead` tokens on.
    pub fn peek(&self, ahead: usize) -> Option<K> {
        self.tokens.get(self.i + ahead).map(|token| token.kind)
    }

    pub fn at(&self, kind: K) -> bool {
        self.kind() == Some(kind)
    }

    pub fn at_end(&self) -> bool {
        self.i >= self.tokens.len()
    }

    /// Byte offset of the token here; the source's length at the end.
    pub fn start(&self) -> usize {
        self.tokens.get(self.i).map_or(self.src.len(), |t| t.start)
    }

    /// The token's text; empty at the end.
    pub fn text(&self) -> &'s str {
        self.tokens
            .get(self.i)
            .map_or("", |t| &self.src[t.start..t.end])
    }

    /// Steps past the token here.
    pub fn bump(&mut self) {
        self.i += 1;
    }

    pub fn take(&mut self) -> &'s str {
        let text = self.text();
        self.bump();
        text
    }

    pub fn accept(&mut self, kind: K) -> bool {
        if !self.at(kind) {
            return false;
        }
        self.bump();
        true
    }

    pub fn expect(&mut self, kind: K) -> Result<&'s str, Error> {
        if !self.at(kind) {
            return Err(self.expected(kind));
        }
        Ok(self.take())
    }

    /// A `kind` token's number.
    pub fn number(&mut self, kind: K) -> Result<f64, Error> {
        Ok(self.expect(kind)?.parse().unwrap_or(f64::NAN))
    }

    /// Steps past every `kind` token here.
    pub fn skip(&mut self, kind: K) {
        while self.accept(kind) {}
    }

    /// The end of a statement: the end of input, or `newline` tokens.
    pub fn end_of_line(&mut self, newline: K) -> Result<(), Error> {
        if self.at_end() {
            return Ok(());
        }
        if !self.at(newline) {
            return Err(self.fail("a line break or the end of input"));
        }
        self.skip(newline);
        Ok(())
    }

    /// Reads a title or accessibility statement here into `header`, if
    /// there is one.
    pub fn heading(&mut self, header: &mut Header) -> bool {
        let Some(heading) = self.kind().and_then(|kind| K::table().get(kind).heading) else {
            return false;
        };
        header.read(heading, self.take());
        true
    }

    /// The error for a missing `kind` token here.
    pub fn expected(&self, kind: K) -> Error {
        self.fail(&format!("token of type '{}'", K::table().get(kind).name))
    }

    pub fn fail(&self, expected: &str) -> Error {
        let found = if self.at_end() {
            "end of input".to_string()
        } else {
            format!("`{}`", self.text())
        };
        Error::at(
            self.diagram,
            self.src,
            self.start(),
            &format!("Expecting {expected} but found {found}."),
            false,
        )
    }
}
