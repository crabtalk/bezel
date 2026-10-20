//! Pele's `diagrams/flowchart/lexer.ts`, and the rules the agentflow lexer
//! shares with it. Positions are byte offsets.

use std::borrow::Cow;

use super::tokens::T;
use crate::{
    diagrams::common::directions::Directions,
    js::Lazy,
    token::{Cursor, Token},
    util::{
        chars::{char_at, digits_end, is_space, is_word, width_at},
        unicode::is_unicode_letter,
    },
};

static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle\s*:\s*", "y");
static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr\s*:\s*", "y");
static RE_ACC_DESCR_ML: Lazy = Lazy::new(r"accDescr\s*\{\s*", "y");
static RE_CALL: Lazy = Lazy::new(r"call\s+", "y");
static RE_CLICK: Lazy = Lazy::new(r"click\s+", "y");
static RE_NODIR: Lazy = Lazy::new(r"(?:\r?\n)*\s*\n", "y");
static RE_DIR: Lazy = Lazy::new(r"\s*(?:LR|RL|TB|BT|TD|BR)\b", "y");
static RE_DIR_SYM: Lazy = Lazy::new(r"\s*[<>^]", "y");
static RE_DIR_V: Lazy = Lazy::new(r"\s*v\b", "y");
static DIRECTIONS: [(T, Lazy); 5] = [
    (T::DirectionTb, Lazy::new(r".*direction\s+TB[^\n]*", "y")),
    (T::DirectionBt, Lazy::new(r".*direction\s+BT[^\n]*", "y")),
    (T::DirectionRl, Lazy::new(r".*direction\s+RL[^\n]*", "y")),
    (T::DirectionLr, Lazy::new(r".*direction\s+LR[^\n]*", "y")),
    (T::DirectionTd, Lazy::new(r".*direction\s+TD[^\n]*", "y")),
];
static RE_EMPTY_CALL: Lazy = Lazy::new(r"\(\s*\)", "y");
static RE_BR: Lazy = Lazy::new(r"\n\s*", "g");

static RE_LINK: Lazy = Lazy::new(r"\s*[xo<]?--+[-xo>]\s*", "y");
static RE_START_LINK: Lazy = Lazy::new(r"\s*[xo<]?--\s*", "y");
static RE_THICK_LINK: Lazy = Lazy::new(r"\s*[xo<]?==+[=xo>]\s*", "y");
static RE_THICK_START: Lazy = Lazy::new(r"\s*[xo<]?==\s*", "y");
static RE_DOTTED_LINK: Lazy = Lazy::new(r"\s*[xo<]?-?\.+-[xo>]?\s*", "y");
static RE_DOTTED_START: Lazy = Lazy::new(r"\s*[xo<]?-\.\s*", "y");
static RE_INVIS_LINK: Lazy = Lazy::new(r"\s*~~~+\s*", "y");
static RE_NEWLINE: Lazy = Lazy::new(r"(?:\r?\n)+", "y");
static RE_SPACE: Lazy = Lazy::new(r"[^\S\n\r]+", "y");

/// Jison's start conditions of the flowchart and agentflow lexers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum S {
    Initial,
    String,
    MdString,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Dir,
    Text,
    EllipseText,
    TrapText,
    EdgeText,
    ThickEdgeText,
    DottedEdgeText,
    Click,
    Callbackname,
    Callbackargs,
    ShapeData,
    ShapeDataStr,
}

impl S {
    /// The token the condition gives at the end of the input.
    fn end_token(self) -> Option<T> {
        match self {
            Self::Initial => Some(T::Eof),
            Self::AccTitle => Some(T::AccTitleValue),
            Self::AccDescr => Some(T::AccDescrValue),
            Self::AccDescrMultiline => Some(T::AccDescrMultilineValue),
            Self::Click => Some(T::Click),
            Self::Callbackname => Some(T::Callbackname),
            Self::Callbackargs => Some(T::Callbackargs),
            _ => None,
        }
    }
}

fn is_node_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!\"#$%&'*+.`?\\_/".contains(c)
}

/// The end of the node string at `p`: node characters, and each `-` with a
/// character after it other than `>`, `-` or `.`.
fn node_string_end(src: &str, mut p: usize) -> usize {
    loop {
        match char_at(src, p) {
            Some(c) if is_node_char(c) => p += 1,
            Some('-') if !matches!(char_at(src, p + 1), None | Some('>' | '-' | '.')) => p += 1,
            _ => return p,
        }
    }
}

/// Where link ids end, found ahead of the cursor.
struct LinkIds {
    /// The next `@` at or after the last run measured.
    next: Option<usize>,
    /// The end of the last run measured.
    run_end: usize,
    /// The `@` that ends the last run's link id.
    run_at: Option<usize>,
}

impl LinkIds {
    /// The end of the link id at `p`: the run of characters up to a quote or
    /// whitespace, through its last `@` with neither `{` nor `"` after it.
    fn end(&mut self, src: &str, p: usize) -> Option<usize> {
        let c = char_at(src, p)?;
        if self.next.is_none() || c == '"' || is_space(c) {
            return None;
        }
        if p >= self.run_end {
            if self.next.is_some_and(|at| at < p) {
                self.next = src[p..].find('@').map(|at| p + at);
            }
            let end = src[p..]
                .find(|c: char| c == '"' || is_space(c))
                .map_or(src.len(), |at| p + at);
            self.run_end = end;
            self.run_at = None;
            if let Some(next) = self.next
                && next < end
            {
                // The last character of the source is never the `@`.
                let last = end.min(src.len() - 1);
                let bytes = src.as_bytes();
                self.run_at = (next..last).rev().find(|&at| {
                    bytes[at] == b'@' && !matches!(char_at(src, at + 1), Some('{' | '"'))
                });
            }
        }
        self.run_at.filter(|&at| at > p).map(|at| at + 1)
    }
}

/// What both lexers keep: the cursor, and what `INITIAL` looks ahead for.
pub(crate) struct Core<'s> {
    pub cur: Cursor<'s, T, S>,
    /// Whether the diagram keyword has been read.
    graph_read: bool,
    directions: Directions<T>,
    link_ids: LinkIds,
    /// The last run of whitespace measured, from and to.
    ws: Option<(usize, usize)>,
}

impl<'s> Core<'s> {
    pub fn new(src: &'s str) -> Self {
        Self {
            cur: Cursor::new(src, S::Initial),
            graph_read: false,
            directions: Directions::new(src, &DIRECTIONS),
            link_ids: LinkIds {
                next: src.find('@'),
                run_end: 0,
                run_at: None,
            },
            ws: None,
        }
    }

    /// The end of the run of whitespace at `q`.
    pub fn space_end(&mut self, q: usize) -> usize {
        if let Some((from, to)) = self.ws
            && (from..=to).contains(&q)
        {
            return to;
        }
        let end = self.cur.space_end(q);
        self.ws = Some((q, end));
        end
    }
}

/// A lexer of the flowchart family: how it places its tokens, and the
/// rules where the grammars differ. The rest is read alike.
pub(crate) trait Lex<'s> {
    /// Whether `%%` starts a comment, and `%` ends a run of text.
    const COMMENTS: bool;

    fn core(&self) -> &Core<'s>;
    fn core_mut(&mut self) -> &mut Core<'s>;
    /// A token from the cursor to `end`.
    fn emit(&mut self, kind: T, end: usize);
    /// [`Lex::emit`], for a token with no line break.
    fn emit_flat(&mut self, kind: T, end: usize) {
        self.emit(kind, end);
    }
    /// [`Lex::emit`], for a run Jison matches a character at a time.
    fn emit_chars(&mut self, kind: T, end: usize) {
        self.emit(kind, end);
    }
    /// A token at the cursor whose text is `text`; reading goes on from
    /// `end`.
    fn emit_text(&mut self, kind: T, text: Cow<'s, str>, end: usize);
    /// Moves the cursor to `end` with no token.
    fn skip(&mut self, end: usize);
    /// The empty token of `kind` at `at` that ends the token list.
    fn close(&mut self, kind: T, at: usize);
    /// The end of an invisible link at `q`.
    fn invis_link_end(&mut self, _q: usize) -> Option<usize> {
        None
    }
    /// The end of a comment at `q`.
    fn comment_end(&self, _q: usize) -> Option<usize> {
        None
    }
    /// Reads one token in `INITIAL` or a condition only this lexer has.
    fn own(&mut self) -> bool;

    fn p(&self) -> usize {
        self.core().cur.p
    }

    fn src(&self) -> &'s str {
        self.core().cur.src
    }

    fn at(&self, q: usize) -> Option<char> {
        self.core().cur.char_at(q)
    }

    fn word(&self, word: &str) -> bool {
        self.core().cur.keyword(word)
    }

    fn push(&mut self, state: S) {
        self.core_mut().cur.push(state);
    }

    fn pop(&mut self) {
        self.core_mut().cur.pop();
    }

    /// A `"`, or `` "` `` opening a markdown string.
    fn quote(&mut self) {
        let p = self.p();
        if self.at(p + 1) == Some('`') {
            self.push(S::MdString);
            self.skip(p + 2);
        } else {
            self.push(S::String);
            self.skip(p + 1);
        }
    }

    /// A shape's opening bracket.
    fn opener(&mut self) -> bool {
        let p = self.p();
        let next = self.at(p + 1);
        let (state, kind, len) = match self.at(p) {
            Some('(') => match next {
                Some('-') => (S::EllipseText, T::EllipseStart, 2),
                Some('[') => (S::Text, T::Stadiumstart, 2),
                Some('(') if self.at(p + 2) == Some('(') => (S::Text, T::Doublecirclestart, 3),
                _ => (S::Text, T::Ps, 1),
            },
            Some('[') => match next {
                Some('[') => (S::Text, T::Subroutinestart, 2),
                Some('(') => (S::Text, T::Cylinderstart, 2),
                Some('/') => (S::TrapText, T::Trapstart, 2),
                Some('\\') => (S::TrapText, T::Invtrapstart, 2),
                _ => (S::Text, T::Sqs, 1),
            },
            Some('|') => (S::Text, T::Pipe, 1),
            Some('{') => (S::Text, T::DiamondStart, 1),
            _ => return false,
        };
        self.push(state);
        self.emit_flat(kind, p + len);
        true
    }

    fn node_string(&mut self) -> bool {
        let p = self.p();
        let end = node_string_end(self.src(), p);
        if end == p {
            return false;
        }
        self.emit_flat(T::NodeString, end);
        true
    }

    fn invis_link(&mut self) -> bool {
        let Some(end) = self.invis_link_end(self.p()) else {
            return false;
        };
        self.emit(T::Link, end);
        true
    }

    /// A comment, skipped.
    fn comment(&mut self) -> bool {
        let Some(end) = self.comment_end(self.p()) else {
            return false;
        };
        self.skip(end);
        true
    }

    /// The diagram keyword, through `end`. The first one opens `DIR`.
    fn graph(&mut self, end: usize) {
        if !self.core().graph_read {
            self.core_mut().graph_read = true;
            self.push(S::Dir);
        }
        self.emit_flat(T::Graph, end);
    }

    /// A keyword or statement start both grammars have.
    fn keyword(&mut self) -> bool {
        let p = self.p();
        let src = self.src();
        match self.at(p) {
            Some('a') if src[p..].starts_with("acc") => {
                if let Some(end) = RE_ACC_TITLE.match_end(src, p) {
                    self.push(S::AccTitle);
                    self.emit(T::AccTitle, end);
                    return true;
                }
                if let Some(end) = RE_ACC_DESCR.match_end(src, p) {
                    self.push(S::AccDescr);
                    self.emit(T::AccDescr, end);
                    return true;
                }
                if let Some(end) = RE_ACC_DESCR_ML.match_end(src, p) {
                    self.push(S::AccDescrMultiline);
                    self.skip(end);
                    return true;
                }
            }
            Some('@') if self.at(p + 1) == Some('{') => {
                self.push(S::ShapeData);
                self.emit_text(T::ShapeData, "".into(), p + 2);
                return true;
            }
            Some('c') => {
                if let Some(end) = RE_CALL.match_end(src, p) {
                    self.push(S::Callbackname);
                    self.skip(end);
                    return true;
                }
                if self.word("classDef") {
                    self.emit_flat(T::Classdef, p + 8);
                    return true;
                }
                if self.word("class") {
                    self.emit_flat(T::Class, p + 5);
                    return true;
                }
                if let Some(end) = RE_CLICK.match_end(src, p) {
                    self.push(S::Click);
                    self.skip(end);
                    return true;
                }
            }
            Some('"') => {
                self.quote();
                return true;
            }
            Some('h') if src[p..].starts_with("href") && self.at(p + 4).is_some_and(is_space) => {
                self.emit(T::Href, p + 4 + width_at(src, p + 4));
                return true;
            }
            _ => {}
        }
        let words = [
            ("style", T::Style),
            ("default", T::Default),
            ("linkStyle", T::Linkstyle),
            ("interpolate", T::Interpolate),
            ("_self", T::LinkTarget),
            ("_blank", T::LinkTarget),
            ("_parent", T::LinkTarget),
            ("_top", T::LinkTarget),
        ];
        for (word, kind) in words {
            if self.word(word) {
                self.emit_flat(kind, p + word.len());
                return true;
            }
        }
        false
    }

    fn direction(&mut self) -> bool {
        let (p, src) = (self.p(), self.src());
        let Some((kind, end)) = self.core_mut().directions.at(src, p) else {
            return false;
        };
        self.emit(kind, end);
        true
    }

    fn link_id(&mut self) -> bool {
        let (p, src) = (self.p(), self.src());
        let Some(end) = self.core_mut().link_ids.end(src, p) else {
            return false;
        };
        self.emit_flat(T::LinkId, end);
        true
    }

    fn number(&mut self) -> bool {
        let p = self.p();
        if !self.at(p).is_some_and(|c| c.is_ascii_digit()) {
            return false;
        }
        self.emit_flat(T::Num, digits_end(self.src(), p));
        true
    }

    /// Punctuation, and the characters that open shapes; `None` at any
    /// other character.
    fn symbol(&mut self) -> Option<bool> {
        let p = self.p();
        let next = self.at(p + 1);
        let (kind, len) = match self.at(p)? {
            '#' => (T::Brkt, 1),
            ':' if next == Some(':') && self.at(p + 2) == Some(':') => (T::StyleSeparator, 3),
            ':' => (T::Colon, 1),
            '&' => (T::Amp, 1),
            ';' => (T::Semi, 1),
            ',' => (T::Comma, 1),
            '*' => (T::Mult, 1),
            '[' if next == Some('|') => (T::VertexWithPropsStart, 2),
            '(' | '[' | '|' | '{' => return Some(self.opener()),
            '>' => {
                self.push(S::Text);
                (T::Tagend, 1)
            }
            '^' => (T::Up, 1),
            '\\' if next == Some('|') => (T::Sep, 2),
            'v' if !next.is_some_and(is_word) => (T::Down, 1),
            '\\' | 'v' => return Some(self.node_string()),
            _ => return None,
        };
        self.emit_flat(kind, p + len);
        Some(true)
    }

    /// A node string, or a letter outside ASCII.
    fn text_char(&mut self) -> bool {
        let p = self.p();
        match self.at(p) {
            Some(c) if c.is_ascii() => self.node_string(),
            Some(c) if is_unicode_letter(u32::from(c)) => {
                self.emit_flat(T::UnicodeText, p + c.len_utf8());
                true
            }
            _ => false,
        }
    }

    fn dir(&mut self) -> bool {
        let (p, src) = (self.p(), self.src());
        if self.at(p) == Some('"') {
            self.quote();
            return true;
        }
        if let Some(end) = RE_NODIR.match_end(src, p) {
            self.pop();
            self.emit(T::Nodir, end);
            return true;
        }
        if let Some(end) = RE_DIR
            .match_end(src, p)
            .or_else(|| RE_DIR_SYM.match_end(src, p))
            .or_else(|| RE_DIR_V.match_end(src, p))
        {
            self.pop();
            self.emit(T::Dir, end);
            return true;
        }
        self.invis_link() || self.opener()
    }

    /// The label inside most shapes.
    fn text(&mut self) -> bool {
        let (p, src) = (self.p(), self.src());
        let next = self.at(p + 1);
        let (kind, len) = match self.at(p) {
            Some('"') => {
                self.quote();
                return true;
            }
            Some('(' | '[' | '{') => return self.opener(),
            Some(']') => match next {
                Some(')') => (T::Stadiumend, 2),
                Some(']') => (T::Subroutineend, 2),
                _ => (T::Sqe, 1),
            },
            Some(')') => match next {
                Some(']') => (T::Cylinderend, 2),
                Some(')') if self.at(p + 2) == Some(')') => (T::Doublecircleend, 3),
                _ => (T::Pe, 1),
            },
            Some('|') => (T::Pipe, 1),
            Some('}') => (T::DiamondStop, 1),
            // Jison drops the lone `%` from Mermaid's text pattern, so only
            // a comment may start with one.
            Some('%') if Self::COMMENTS => return self.comment(),
            c => {
                if c.is_some_and(|c| c == '~' || is_space(c)) && self.invis_link() {
                    return true;
                }
                let end = self.core().cur.until(p + width_at(src, p), |c| {
                    matches!(c, '[' | ']' | '(' | ')' | '{' | '}' | '|' | '"')
                        || (Self::COMMENTS && c == '%')
                });
                self.emit(T::Text, end);
                return true;
            }
        };
        self.pop();
        self.emit_flat(kind, p + len);
        true
    }

    /// The label of an ellipse, `(-` to `-)`.
    fn ellipse_text(&mut self) -> bool {
        let (p, src) = (self.p(), self.src());
        let c = self.at(p);
        if c == Some('"') {
            self.quote();
            return true;
        }
        if c.is_some_and(|c| c == '~' || is_space(c)) && self.invis_link() {
            return true;
        }
        if c == Some('%')
            && let Some(end) = self.comment_end(p)
        {
            self.skip(end);
            return true;
        }
        if matches!(c, Some('-' | '/' | ')')) && self.at(p + 1) == Some(')') {
            self.pop();
            self.emit_flat(T::EllipseEnd, p + 2);
            return true;
        }
        match c {
            Some('(' | '[' | '{') => return self.opener(),
            Some(')' | ']' | '}') => return false,
            _ => {}
        }
        let mut q = p + width_at(src, p);
        while let Some(ch) = self.at(q) {
            if matches!(ch, '"' | '(' | ')' | '[' | ']' | '{' | '}')
                || (matches!(ch, '-' | '/') && self.at(q + 1) == Some(')'))
                || (ch == '%' && self.comment_end(q).is_some())
                || ((ch == '~' || is_space(ch)) && self.invis_link_end(q).is_some())
            {
                break;
            }
            q += ch.len_utf8();
        }
        self.emit_chars(T::Text, q);
        true
    }

    /// The label of a trapezoid or a slanted shape.
    fn trap_text(&mut self) -> bool {
        let (p, src) = (self.p(), self.src());
        let c = self.at(p);
        let next = self.at(p + 1);
        if c == Some('"') {
            self.quote();
            return true;
        }
        if c.is_some_and(|c| c == '~' || is_space(c)) && self.invis_link() {
            return true;
        }
        let (kind, len) = match c {
            Some('(') if next == Some(']') => (T::Trapend, 2),
            Some('(' | '[' | '{') => return self.opener(),
            Some('\\' | ']' | ')' | '?' | '=') if next == Some(']') => (T::Trapend, 2),
            Some('/') if next == Some(']') => (T::Invtrapend, 2),
            Some('\\' | '/') => {
                self.emit_flat(T::Text, p + 1);
                return true;
            }
            Some(']' | ')' | '}') => return false,
            Some('%') if Self::COMMENTS => return self.comment(),
            _ => {
                let end = self.core().cur.until(p + width_at(src, p), |c| {
                    matches!(c, '\\' | '[' | ']' | '(' | ')' | '{' | '}' | '/')
                        || (Self::COMMENTS && c == '%')
                });
                self.emit(T::Text, end);
                return true;
            }
        };
        self.pop();
        self.emit_flat(kind, p + len);
        true
    }

    /// Reads one token; false where none matches.
    fn step(&mut self) -> bool {
        let (p, src) = (self.p(), self.src());
        let c = self.at(p);
        let cur = &self.core().cur;
        let rest_from = p + width_at(src, p);
        match cur.state {
            S::String => match c {
                Some('"') if self.at(p + 1) == Some('`') => {
                    self.push(S::MdString);
                    self.skip(p + 2);
                }
                Some('"') => {
                    self.pop();
                    self.skip(p + 1);
                }
                _ => {
                    let end = cur.until(p, |c| c == '"');
                    self.emit(T::Str, end);
                }
            },
            S::MdString => match c {
                Some('`') if self.at(p + 1) == Some('"') => {
                    self.pop();
                    self.skip(p + 2);
                }
                Some('`') => return false,
                Some('"') => self.quote(),
                _ => {
                    let end = cur.until(rest_from, |c| c == '`' || c == '"');
                    self.emit(T::MdStr, end);
                }
            },
            S::AccTitle | S::AccDescr => {
                let kind = if cur.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                let end = cur.line_end(p);
                self.pop();
                self.emit(kind, end);
            }
            S::AccDescrMultiline => {
                if c == Some('}') {
                    self.pop();
                    self.skip(p + 1);
                } else {
                    let end = cur.until(p, |c| c == '}');
                    self.emit(T::AccDescrMultilineValue, end);
                }
            }
            S::Dir => return self.dir(),
            S::Text => return self.text(),
            S::EllipseText => return self.ellipse_text(),
            S::TrapText => return self.trap_text(),
            S::Click => match c {
                Some('"') => self.quote(),
                Some(c) if is_space(c) => {
                    self.pop();
                    self.skip(rest_from);
                }
                _ => {
                    let end = cur.until(rest_from, is_space);
                    self.emit_flat(T::Click, end);
                }
            },
            S::Callbackname => {
                if c == Some('(') {
                    let empty = RE_EMPTY_CALL.match_end(src, p);
                    self.pop();
                    match empty {
                        Some(end) => self.skip(end),
                        None => {
                            self.push(S::Callbackargs);
                            self.skip(p + 1);
                        }
                    }
                } else {
                    let end = cur.until(p, |c| c == '(');
                    self.emit(T::Callbackname, end);
                }
            }
            S::Callbackargs => {
                if c == Some(')') {
                    self.pop();
                    self.skip(p + 1);
                } else {
                    let end = cur.until(p, |c| c == ')');
                    self.emit(T::Callbackargs, end);
                }
            }
            S::ShapeData => match c {
                Some('"') => {
                    self.push(S::ShapeDataStr);
                    self.emit_flat(T::ShapeData, p + 1);
                }
                Some('}') => {
                    self.pop();
                    self.skip(p + 1);
                }
                Some('^') => return false,
                _ => {
                    let end = cur.until(rest_from, |c| matches!(c, '}' | '^' | '"'));
                    self.emit(T::ShapeData, end);
                }
            },
            S::ShapeDataStr => {
                if c == Some('"') {
                    self.pop();
                    self.emit_flat(T::ShapeData, p + 1);
                } else {
                    let end = cur.until(p, |c| c == '"');
                    let text = RE_BR.replace(&src[p..end], |_| "<br/>".into());
                    self.emit_text(T::ShapeData, text.into(), end);
                }
            }
            S::Initial | S::EdgeText | S::ThickEdgeText | S::DottedEdgeText => return self.own(),
        }
        true
    }

    /// Reads the whole source.
    fn lex(&mut self) {
        let len = self.src().len();
        while self.p() < len {
            if !self.step() {
                let p = self.p();
                self.close(T::Error, p);
                return;
            }
        }
        if let Some(kind) = self.core().cur.state.end_token() {
            self.emit_flat(kind, len);
        }
        self.close(T::End, len);
    }
}

struct Lexer<'s> {
    core: Core<'s>,
}

impl<'s> Lex<'s> for Lexer<'s> {
    const COMMENTS: bool = false;

    fn core(&self) -> &Core<'s> {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core<'s> {
        &mut self.core
    }

    fn emit(&mut self, kind: T, end: usize) {
        let cur = &mut self.core.cur;
        cur.emit(kind, cur.p, end);
    }

    fn emit_text(&mut self, kind: T, text: Cow<'s, str>, end: usize) {
        let cur = &mut self.core.cur;
        cur.emit_text(kind, text, cur.p, end);
    }

    fn skip(&mut self, end: usize) {
        self.core.cur.p = end;
    }

    fn close(&mut self, kind: T, at: usize) {
        self.core.cur.tokens.push(Token::new(kind, "", at));
    }

    fn invis_link_end(&mut self, q: usize) -> Option<usize> {
        let at = self.core.space_end(q);
        if self.at(at) != Some('~') {
            return None;
        }
        RE_INVIS_LINK.match_end(self.src(), q)
    }

    fn own(&mut self) -> bool {
        match self.core.cur.state {
            S::EdgeText => self.edge_text(&RE_LINK, '-', None),
            S::ThickEdgeText => self.edge_text(&RE_THICK_LINK, '=', None),
            S::DottedEdgeText => self.edge_text(&RE_DOTTED_LINK, '.', Some('-')),
            _ => self.initial(),
        }
    }
}

impl Lexer<'_> {
    /// The first character after any whitespace at `q`, past one `x`, `o`
    /// or `<`.
    fn link_char(&mut self, q: usize) -> Option<char> {
        let at = self.core.space_end(q);
        match self.at(at) {
            Some('x' | 'o' | '<') => self.at(at + 1),
            c => c,
        }
    }

    /// The first of `rules` matching at the cursor, a link or the start of
    /// one that opens the given condition.
    fn link_rule(&mut self, rules: &[(&Lazy, Option<S>)]) -> bool {
        let (p, src) = (self.p(), self.src());
        for &(rule, opens) in rules {
            if let Some(end) = rule.match_end(src, p) {
                match opens {
                    Some(state) => {
                        self.push(state);
                        self.emit(T::StartLink, end);
                    }
                    None => self.emit(T::Link, end),
                }
                return true;
            }
        }
        false
    }

    fn link(&mut self) -> bool {
        let p = self.p();
        let found = match self.link_char(p) {
            Some('-') => self.link_rule(&[
                (&RE_LINK, None),
                (&RE_START_LINK, Some(S::EdgeText)),
                (&RE_DOTTED_LINK, None),
                (&RE_DOTTED_START, Some(S::DottedEdgeText)),
            ]),
            Some('=') => self.link_rule(&[
                (&RE_THICK_LINK, None),
                (&RE_THICK_START, Some(S::ThickEdgeText)),
            ]),
            Some('.') => self.link_rule(&[(&RE_DOTTED_LINK, None)]),
            _ => false,
        };
        found || (!matches!(self.at(p), Some('-' | '=' | '.')) && self.invis_link())
    }

    /// A token of what `rule` matches at the cursor.
    fn rule(&mut self, rule: &Lazy, kind: T) -> bool {
        let Some(end) = rule.match_end(self.src(), self.p()) else {
            return false;
        };
        self.emit(kind, end);
        true
    }

    fn flowchart_keyword(&mut self) -> bool {
        let p = self.p();
        if self.word("swimlane-beta") || self.word("flowchart-elk") {
            self.graph(p + 13);
        } else if self.word("flowchart") {
            self.graph(p + 9);
        } else if self.word("graph") {
            self.graph(p + 5);
        } else if self.word("subgraph") {
            self.emit(T::Subgraph, p + 8);
        } else if self.word("end") {
            let end = self.core.cur.space_end(p + 3);
            self.emit(T::EndKw, end);
        } else {
            return false;
        }
        true
    }

    fn initial(&mut self) -> bool {
        if self.keyword()
            || self.flowchart_keyword()
            || self.direction()
            || self.link_id()
            || self.number()
        {
            return true;
        }
        let p = self.p();
        match self.at(p) {
            Some('x' | 'o' | '.') => return self.link() || self.node_string(),
            Some('<') => {
                if !self.link() {
                    self.emit(T::Tagstart, p + 1);
                }
                return true;
            }
            Some('-') => {
                if !(self.link() || self.node_string()) {
                    self.emit(T::Minus, p + 1);
                }
                return true;
            }
            Some('=') => return self.link(),
            Some('~') => return self.invis_link(),
            Some(c) if is_space(c) => {
                return self.link()
                    || self.rule(&RE_NEWLINE, T::Newline)
                    || self.rule(&RE_SPACE, T::Space);
            }
            _ => {}
        }
        if let Some(found) = self.symbol() {
            return found;
        }
        self.text_char()
    }

    /// The label of a link, up to the `line` that closes it, or `also` where
    /// a closing link may start with that.
    fn edge_text(&mut self, link: &Lazy, line: char, also: Option<char>) -> bool {
        let (p, src) = (self.p(), self.src());
        let c = self.at(p);
        if c == Some('"') {
            self.quote();
            return true;
        }
        let closes = |c: Option<char>| c.is_some() && (c == Some(line) || c == also);
        if closes(self.link_char(p))
            && let Some(end) = link.match_end(src, p)
        {
            self.pop();
            self.emit(T::Link, end);
            return true;
        }
        if c == Some(line) && (line != '-' || self.at(p + 1) == Some('-')) {
            return false;
        }
        let mut q = p + width_at(src, p);
        while let Some(ch) = self.at(q) {
            if ch == '"' {
                break;
            }
            if ch == line {
                if line != '-' || self.at(q + 1) == Some('-') {
                    break;
                }
            } else if (matches!(ch, 'x' | 'o' | '<') || Some(ch) == also || is_space(ch))
                && closes(self.link_char(q))
                && link.match_end(src, q).is_some()
            {
                break;
            }
            q += ch.len_utf8();
        }
        self.emit(T::EdgeText, q);
        true
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let mut lexer = Lexer {
        core: Core::new(src),
    };
    lexer.lex();
    lexer.core.cur.tokens
}
