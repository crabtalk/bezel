//! Pele's `diagrams/requirement/lexer.ts`.

use super::tokens::{KEYWORDS, T};
use crate::{
    js::{self, Lazy},
    token::{Cursor, Token},
    util::chars::{is_line_end, is_space, is_word, starts_with_caseless},
};

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    String,
    Style,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

const LONGEST_KEYWORD: usize = 22;

static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle\s*:\s*", "iy");
static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr\s*:\s*", "iy");
static RE_ACC_DESCR_ML: Lazy = Lazy::new(r"accDescr\s*\{\s*", "iy");
static RE_DIRECTION: Lazy = Lazy::new(r"direction\s+(?:(TB)|(BT)|(RL)|LR)", "gi");
/// The direction kinds in the order of `RE_DIRECTION`'s alternatives.
const DIRECTIONS: [T; 4] = [
    T::DirectionTb,
    T::DirectionBt,
    T::DirectionRl,
    T::DirectionLr,
];

/// Whether `c` ends an unquoted string: `:,{<>-=` and line breaks.
fn is_stop(c: char) -> bool {
    matches!(c, ':' | ',' | '\r' | '\n' | '{' | '<' | '>' | '-' | '=')
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    /// Where each kind of direction statement starts and where its keyword
    /// ends, found in one pass.
    dirs: [Vec<(usize, usize)>; 4],
    /// How many of each kind start before the end of the current line.
    dir_seen: [usize; 4],
    line_end: Option<usize>,
}

impl<'s> Lexer<'s> {
    fn emit(&mut self, kind: T, end: usize) -> bool {
        self.cur.emit_to(kind, end);
        true
    }

    /// `.*direction\s+XX[^\n]*`: the last such statement that starts on this
    /// line decides the token.
    fn direction(&mut self) -> bool {
        let p = self.cur.p;
        if self.line_end.is_none_or(|end| end < p) {
            let line_end = self.cur.terminator_end(p);
            self.line_end = Some(line_end);
            for (list, seen) in self.dirs.iter().zip(&mut self.dir_seen) {
                while list.get(*seen).is_some_and(|&(start, _)| start <= line_end) {
                    *seen += 1;
                }
            }
        }
        for (k, kind) in DIRECTIONS.into_iter().enumerate() {
            let Some(j) = self.dir_seen[k].checked_sub(1) else {
                continue;
            };
            let (start, end) = self.dirs[k][j];
            if start >= p {
                let e = self.cur.line_end(end);
                return self.emit(kind, e);
            }
        }
        false
    }

    fn initial(&mut self, c: char) -> bool {
        let src = self.cur.src;
        let p = self.cur.p;
        let n = src.len();
        let lower = c.to_ascii_lowercase();
        if lower == 't'
            && src
                .get(p + 1..p + 5)
                .is_some_and(|rest| rest.eq_ignore_ascii_case("itle"))
            && self.cur.char_at(p + 5).is_some_and(is_space)
        {
            let from = self.cur.after(p + 5);
            let q = src[from..].find(['#', '\n', ';']).map_or(n, |at| from + at);
            if q > from {
                return self.emit(T::Title, q);
            }
        }
        if lower == 'a' && starts_with_caseless(src, p, "acc") {
            if let Some(e) = RE_ACC_TITLE.match_end(src, p) {
                self.cur.push(S::AccTitle);
                return self.emit(T::AccTitle, e);
            }
            if let Some(e) = RE_ACC_DESCR.match_end(src, p) {
                self.cur.push(S::AccDescr);
                return self.emit(T::AccDescr, e);
            }
            if let Some(e) = RE_ACC_DESCR_ML.match_end(src, p) {
                self.cur.push(S::AccDescrMultiline);
                self.cur.p = e;
                return true;
            }
        }
        if self.dirs.iter().any(|list| !list.is_empty()) && !is_line_end(c) && self.direction() {
            return true;
        }

        if c == '\n' || (c == '\r' && self.cur.char_at(p + 1) == Some('\n')) {
            let mut q = p;
            loop {
                if src[q..].starts_with('\n') {
                    q += 1;
                } else if src[q..].starts_with("\r\n") {
                    q += 2;
                } else {
                    break;
                }
            }
            return self.emit(T::Newline, q);
        }
        if is_space(c) {
            self.cur.p = self.cur.space_end(p);
            return true;
        }
        if matches!(c, '#' | '%') {
            self.cur.p = self.cur.line_end(p);
            return true;
        }

        if is_word(c) {
            let mut q = self.cur.word_end(p);
            if q - p <= LONGEST_KEYWORD {
                let word = src[p..q].to_ascii_lowercase();
                if let Some(&(_, keyword)) = KEYWORDS.iter().find(|(name, _)| *name == word) {
                    if matches!(keyword, T::Style | T::Classdef | T::Class) {
                        self.cur.push(S::Style);
                    }
                    return self.emit(keyword, q);
                }
            }
            q = src[q..].find(is_stop).map_or(n, |at| q + at);
            self.cur.emit_text(T::Unqstring, js::trim(&src[p..q]), p, q);
            return true;
        }

        match c {
            '{' => self.emit(T::StructStart, p + 1),
            '}' => self.emit(T::StructStop, p + 1),
            ':' if self.cur.starts_with("::", p + 1) => self.emit(T::StyleSeparator, p + 3),
            ':' => self.emit(T::Colonsep, p + 1),
            '<' if self.cur.char_at(p + 1) == Some('-') => self.emit(T::EndArrowL, p + 2),
            '-' if self.cur.char_at(p + 1) == Some('>') => self.emit(T::EndArrowR, p + 2),
            '-' => self.emit(T::Line, p + 1),
            '"' => {
                self.cur.push(S::String);
                self.cur.p += 1;
                true
            }
            ',' => self.emit(T::Comma, p + 1),
            _ => false,
        }
    }

    fn style(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match c {
            _ if is_word(c) => {
                let q = self.cur.word_end(p);
                self.emit(T::Alpha, q)
            }
            ':' => self.emit(T::Colon, p + 1),
            ';' => self.emit(T::Semicolon, p + 1),
            '%' => self.emit(T::Percent, p + 1),
            '-' => self.emit(T::Minus, p + 1),
            '#' => self.emit(T::Brkt, p + 1),
            ' ' => {
                self.cur.p += 1;
                true
            }
            '"' => {
                self.cur.push(S::String);
                self.cur.p += 1;
                true
            }
            '\n' => {
                self.cur.pop();
                self.cur.p += 1;
                true
            }
            ',' => self.emit(T::Comma, p + 1),
            _ => false,
        }
    }

    fn step(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match self.cur.state {
            S::Initial => self.initial(c),
            S::Style => self.style(c),
            S::String => {
                if c == '"' {
                    self.cur.pop();
                    self.cur.p += 1;
                    true
                } else {
                    let e = self.cur.find('"', p);
                    self.emit(T::Qstring, e)
                }
            }
            S::AccTitle | S::AccDescr => {
                let kind = if self.cur.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                self.cur.pop();
                let e = self.cur.line_end(p);
                self.emit(kind, e)
            }
            S::AccDescrMultiline => {
                if c == '}' {
                    self.cur.pop();
                    self.cur.p += 1;
                    true
                } else {
                    let e = self.cur.find('}', p);
                    self.emit(T::AccDescrMultilineValue, e)
                }
            }
        }
    }
}

/// The token stream of Mermaid's requirement lexer. Its rules are tried in
/// order and the first match wins, so a line that holds a direction
/// statement anywhere is one token.
pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lx = Lexer {
        cur: Cursor::new(src, S::Initial),
        dirs: Default::default(),
        dir_seen: [0; 4],
        line_end: None,
    };
    for m in RE_DIRECTION.captures_iter(src) {
        let kind = (1..4)
            .find(|&group| m.get(group).is_some())
            .map_or(3, |group| group - 1);
        lx.dirs[kind].push((m.start(), m.end()));
    }

    while let Some(c) = lx.cur.char_at(lx.cur.p) {
        if !lx.step(c) {
            let mut tokens = lx.cur.tokens;
            tokens.push(Token::new(T::Error, "", lx.cur.p));
            return tokens;
        }
    }

    // At the end of input Jison still tries the rules that can match
    // nothing, once.
    let last = match lx.cur.state {
        S::Initial => Some(T::Eof),
        S::String => Some(T::Qstring),
        S::AccTitle => Some(T::AccTitleValue),
        S::AccDescr => Some(T::AccDescrValue),
        S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
        S::Style => None,
    };
    if let Some(last) = last {
        lx.cur.emit(last, n, n);
    }
    lx.cur.emit(T::End, n, n);
    lx.cur.tokens
}
