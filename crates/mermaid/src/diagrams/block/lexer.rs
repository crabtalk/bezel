//! Pele's `diagrams/block/lexer.ts`.

use super::tokens::T;
use crate::{
    token::{Cursor, Token},
    util::chars::{digits_end, is_space, is_word},
};

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    String,
    MdString,
    Node,
    BlockArrow,
    ArrowDir,
    Llabel,
    Classdef,
    ClassdefId,
    Class,
    ClassStyle,
    StyleStatement,
    StyleDefinition,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

const DIRECTIONS: [&str; 6] = ["right", "left", "x", "y", "up", "down"];

/// Whether `c` ends a node id: ( ) [ { } < > - : = and whitespace.
fn ends_id(c: char) -> bool {
    matches!(c, '(' | ')' | '[' | '{' | '}' | '<' | '>' | '-' | ':' | '=') || is_space(c)
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
}

impl<'s> Lexer<'s> {
    /// A token of `text` at the cursor; reading goes on from `end`.
    fn put(&mut self, kind: T, text: &'s str, end: usize) -> bool {
        self.cur.emit_text(kind, text, self.cur.p, end);
        true
    }

    /// A token from the cursor to `end`.
    fn emit(&mut self, kind: T, end: usize) -> bool {
        self.cur.emit_to(kind, end);
        true
    }

    fn quote(&mut self) -> bool {
        if self.cur.char_at(self.cur.p + 1) == Some('`') {
            self.cur.push(S::MdString);
            self.cur.p += 2;
        } else {
            self.cur.push(S::String);
            self.cur.p += 1;
        }
        true
    }

    /// End of `.+-[xo>]?` at q.
    fn dotted(&self, mut q: usize) -> Option<usize> {
        if self.cur.char_at(q) != Some('.') {
            return None;
        }
        while self.cur.char_at(q) == Some('.') {
            q += 1;
        }
        if self.cur.char_at(q) != Some('-') {
            return None;
        }
        Some(
            if matches!(self.cur.char_at(q + 1), Some('x' | 'o' | '>')) {
                q + 2
            } else {
                q + 1
            },
        )
    }

    /// End of a complete link at q, before its trailing whitespace.
    fn link_end(&self, mut q: usize) -> Option<usize> {
        if matches!(self.cur.char_at(q), Some('x' | 'o' | '<')) {
            q += 1;
        }
        let line = match self.cur.char_at(q) {
            Some('.') => return self.dotted(q),
            Some(line @ ('-' | '=')) => line,
            _ => return None,
        };
        let mut k = q + 1;
        while self.cur.char_at(k) == Some(line) {
            k += 1;
        }
        if k - q == 1 {
            return if line == '-' { self.dotted(k) } else { None };
        }
        if matches!(self.cur.char_at(k), Some('x' | 'o' | '>')) {
            return Some(k + 1);
        }
        if k - q >= 3 { Some(k) } else { None }
    }

    fn link(&mut self) -> bool {
        if let Some(e) = self.link_end(self.cur.p) {
            let end = self.cur.space_end(e);
            return self.emit(T::Link, end);
        }
        let q = if self.cur.char_at(self.cur.p) == Some('<') {
            self.cur.p + 1
        } else {
            self.cur.p
        };
        if matches!(
            (self.cur.char_at(q), self.cur.char_at(q + 1)),
            (Some('-'), Some('-' | '.')) | (Some('='), Some('='))
        ) {
            self.cur.push(S::Llabel);
            let end = self.cur.space_end(q + 2);
            return self.emit(T::StartLink, end);
        }
        false
    }

    fn line_state(&mut self, kind: T) -> bool {
        let e = self.cur.line_end(self.cur.p);
        self.cur.pop();
        self.emit(kind, e)
    }

    /// `\w+(,\s*\w+)*`
    fn id_list(&mut self, kind: T, next: S) -> bool {
        let mut k = self.cur.word_end(self.cur.p);
        if k == self.cur.p {
            return false;
        }
        while self.cur.char_at(k) == Some(',') {
            let from = self.cur.space_end(k + 1);
            let to = self.cur.word_end(from);
            if to == from {
                break;
            }
            k = to;
        }
        self.cur.state = next;
        self.emit(kind, k)
    }

    /// Whether the character at `at` is not a word character.
    fn boundary(&self, at: usize) -> bool {
        !self.cur.char_at(at).is_some_and(is_word)
    }

    fn initial(&mut self, c: char) -> bool {
        let src = self.cur.src;
        let p = self.cur.p;
        if is_space(c) {
            self.cur.p = self.cur.space_end(p);
            return true;
        }
        match c {
            'b' => {
                if self.cur.starts_with("block", p) {
                    if self.cur.keyword("block-beta") {
                        return self.emit(T::Key, p + 10);
                    }
                    if self.cur.char_at(p + 5) == Some(':') {
                        return self.emit(T::IdBlock, p + 6);
                    }
                    if self.boundary(p + 5) {
                        return self.emit(T::Key, p + 5);
                    }
                }
            }
            'c' => {
                if self.cur.starts_with("columns", p) {
                    let q = self.cur.space_end(p + 7);
                    if q > p + 7 {
                        if self.cur.starts_with("auto", q) && self.boundary(q + 4) {
                            return self.put(T::Columns, "-1", q + 4);
                        }
                        let e = digits_end(src, q);
                        if e > q {
                            return self.put(T::Columns, &src[q..e], e);
                        }
                    }
                } else if self.cur.starts_with("class", p) {
                    let def = self.cur.starts_with("Def", p + 5);
                    let from = if def { p + 8 } else { p + 5 };
                    let q = self.cur.space_end(from);
                    if q > from {
                        self.cur.push(if def { S::Classdef } else { S::Class });
                        return self.emit(if def { T::Classdef } else { T::Class }, q);
                    }
                }
            }
            '"' => return self.quote(),
            's' => {
                if self.cur.starts_with("space", p) {
                    if self.cur.char_at(p + 5) == Some(':') {
                        let e = digits_end(src, p + 6);
                        if e > p + 6 {
                            return self.put(T::SpaceBlock, &src[p + 6..e], e);
                        }
                    }
                    if self.boundary(p + 5) {
                        return self.put(T::SpaceBlock, "1", p + 5);
                    }
                } else if self.cur.starts_with("style", p) {
                    let q = self.cur.space_end(p + 5);
                    if q > p + 5 {
                        self.cur.push(S::StyleStatement);
                        return self.emit(T::Style, q);
                    }
                }
            }
            'd' => {
                if self.cur.keyword("default") {
                    return self.emit(T::Default, p + 7);
                }
            }
            'l' => {
                if self.cur.keyword("linkStyle") {
                    return self.emit(T::Linkstyle, p + 9);
                }
            }
            'i' => {
                if self.cur.keyword("interpolate") {
                    return self.emit(T::Interpolate, p + 11);
                }
            }
            'a' => {
                if self.cur.starts_with("accTitle", p) {
                    let q = self.cur.space_end(p + 8);
                    if self.cur.char_at(q) == Some(':') {
                        self.cur.push(S::AccTitle);
                        let end = self.cur.space_end(q + 1);
                        return self.emit(T::AccTitle, end);
                    }
                } else if self.cur.starts_with("accDescr", p) {
                    let q = self.cur.space_end(p + 8);
                    match self.cur.char_at(q) {
                        Some(':') => {
                            self.cur.push(S::AccDescr);
                            let end = self.cur.space_end(q + 1);
                            return self.emit(T::AccDescr, end);
                        }
                        Some('{') => {
                            self.cur.push(S::AccDescrMultiline);
                            self.cur.p = self.cur.space_end(q + 1);
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            'e' => {
                if self.cur.keyword("end") {
                    let end = self.cur.space_end(p + 3);
                    return self.emit(T::EndKeyword, end);
                }
            }
            '-' => {
                if self.cur.char_at(p + 1) == Some(')') {
                    self.cur.push(S::Node);
                    return self.emit(T::NodeDstart, p + 2);
                }
                return self.link();
            }
            '=' => return self.link(),
            '<' => {
                if self.cur.char_at(p + 1) == Some('[') {
                    self.cur.push(S::BlockArrow);
                    return self.emit(T::BlockArrowStart, p + 2);
                }
                return self.link();
            }
            '(' => {
                let len = match (self.cur.char_at(p + 1), self.cur.char_at(p + 2)) {
                    (Some('-' | '['), _) => 2,
                    (Some('('), Some('(')) => 3,
                    (Some('('), _) => 2,
                    _ => 1,
                };
                self.cur.push(S::Node);
                return self.emit(T::NodeDstart, p + len);
            }
            ')' | '{' => {
                self.cur.push(S::Node);
                let end = if self.cur.char_at(p + 1) == Some(c) {
                    p + 2
                } else {
                    p + 1
                };
                return self.emit(T::NodeDstart, end);
            }
            '>' => {
                self.cur.push(S::Node);
                return self.emit(T::NodeDstart, p + 1);
            }
            '[' => {
                self.cur.push(S::Node);
                let end = if matches!(self.cur.char_at(p + 1), Some('[' | '|' | '(' | '\\' | '/')) {
                    p + 2
                } else {
                    p + 1
                };
                return self.emit(T::NodeDstart, end);
            }
            ':' => {
                let e = digits_end(src, p + 1);
                if e > p + 1 {
                    return self.put(T::Size, &src[p + 1..e], e);
                }
                return false;
            }
            _ => {}
        }

        let q = src[p..].find(ends_id).map_or(src.len(), |at| p + at);
        q > p && self.emit(T::NodeId, q)
    }

    fn node_end(&mut self, c: char) -> bool {
        let p = self.cur.p;
        let d = self.cur.char_at(p + 1);
        let e = self.cur.char_at(p + 2);
        let len = match (c, d) {
            ('"', _) => return self.quote(),
            ('(', Some('-')) => 2,
            ('(', Some('(')) => {
                if e == Some('(') {
                    3
                } else {
                    2
                }
            }
            ('(', _) => 1,
            (')', Some(')')) => {
                if e == Some(')') {
                    3
                } else {
                    2
                }
            }
            (')', Some(']')) => 2,
            (')', _) => 1,
            ('}', Some('}')) => 2,
            ('}', _) => 1,
            ('-', Some(')')) => 2,
            (']', Some(']' | ')' | '>')) => 2,
            (']', _) => 1,
            ('\\' | '/', Some(']')) => 2,
            _ => return false,
        };
        self.cur.pop();
        self.emit(T::NodeDend, p + len)
    }

    fn arrow_dir(&mut self, c: char) -> bool {
        let p = self.cur.p;
        if c == ')' {
            self.cur.pop();
            self.cur.pop();
            let end = self.cur.space_end(p + 1);
            return self.put(T::BlockArrowEnd, "]>", end);
        }
        let comma = c == ',';
        let from = self.cur.space_end(if comma { p + 1 } else { p });
        for dir in DIRECTIONS {
            if self.cur.starts_with(dir, from) {
                let e = self.cur.space_end(from + dir.len());
                let text = &self.cur.src[if comma { from } else { p }..e];
                return self.put(T::Dir, text, e);
            }
        }
        false
    }

    fn step(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match self.cur.state {
            S::Initial => return self.initial(c),
            S::String => {
                if c == '"' {
                    self.cur.pop();
                    self.cur.p += 1;
                } else {
                    let e = self.cur.find('"', p);
                    self.emit(T::Str, e);
                }
            }
            S::MdString => match c {
                '`' => {
                    if self.cur.char_at(p + 1) != Some('"') {
                        return false;
                    }
                    self.cur.pop();
                    self.cur.p += 2;
                }
                '"' => return false,
                _ => {
                    let from = self.cur.after(p);
                    let q = self.cur.src[from..]
                        .find(['`', '"'])
                        .map_or(self.cur.src.len(), |at| from + at);
                    self.emit(T::MdStr, q);
                }
            },
            S::Node => return self.node_end(c),
            S::BlockArrow => {
                if c == '"' {
                    self.quote();
                } else if c == ']' && self.cur.char_at(p + 1) == Some('>') {
                    let q = self.cur.space_end(p + 2);
                    if self.cur.char_at(q) != Some('(') {
                        return false;
                    }
                    self.cur.push(S::ArrowDir);
                    self.cur.p = q + 1;
                } else {
                    return false;
                }
            }
            S::ArrowDir => return self.arrow_dir(c),
            S::Llabel => {
                if c == '"' {
                    if self.cur.char_at(p + 1) == Some('`') {
                        self.quote();
                    } else {
                        self.emit(T::LinkLabel, p + 1);
                        self.cur.push(S::String);
                    }
                } else {
                    let Some(e) = self.link_end(self.cur.space_end(p)) else {
                        return false;
                    };
                    self.cur.pop();
                    let end = self.cur.space_end(e);
                    self.emit(T::Link, end);
                }
            }
            S::Classdef => {
                let mut kind = T::DefaultClassdefId;
                let mut k = if self.cur.starts_with("DEFAULT", p) {
                    p + 7
                } else {
                    p
                };
                let mut q = self.cur.space_end(k);
                if k == p || q == k {
                    kind = T::ClassdefId;
                    k = self.cur.word_end(p);
                    q = self.cur.space_end(k);
                }
                if !(k > p && q > k) {
                    return false;
                }
                self.cur.state = S::ClassdefId;
                self.emit(kind, q);
            }
            S::ClassdefId => {
                self.line_state(T::ClassdefStyleopts);
            }
            S::Class => return self.id_list(T::ClassentityIds, S::ClassStyle),
            S::ClassStyle => {
                self.line_state(T::Styleclass);
            }
            S::StyleStatement => return self.id_list(T::StyleEntityIds, S::StyleDefinition),
            S::StyleDefinition => {
                self.line_state(T::StyleDefinitionData);
            }
            S::AccTitle => {
                self.line_state(T::AccTitleValue);
            }
            S::AccDescr => {
                self.line_state(T::AccDescrValue);
            }
            S::AccDescrMultiline => {
                if c == '}' {
                    self.cur.pop();
                    self.cur.p += 1;
                } else {
                    let e = self.cur.find('}', p);
                    self.emit(T::AccDescrMultilineValue, e);
                }
            }
        }
        true
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lexer = Lexer {
        cur: Cursor::new(src, S::Initial),
    };

    loop {
        let Some(c) = lexer.cur.char_at(lexer.cur.p) else {
            let kind = match lexer.cur.state {
                S::Initial => Some(T::Eof),
                S::String => Some(T::Str),
                S::ClassdefId => Some(T::ClassdefStyleopts),
                S::ClassStyle => Some(T::Styleclass),
                S::StyleDefinition => Some(T::StyleDefinitionData),
                S::AccTitle => Some(T::AccTitleValue),
                S::AccDescr => Some(T::AccDescrValue),
                S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
                _ => None,
            };
            lexer.cur.p = n;
            if let Some(kind) = kind {
                lexer.put(kind, "", n);
            }
            break;
        };
        if !lexer.step(c) {
            let p = lexer.cur.p;
            let mut tokens = lexer.cur.tokens;
            tokens.push(Token::new(T::Error, "", p));
            return tokens;
        }
    }

    let mut tokens = lexer.cur.tokens;
    tokens.push(Token::new(T::End, "", n));
    tokens
}
