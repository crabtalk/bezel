//! Pele's `diagrams/quadrant/lexer.ts`.

use super::tokens::{QUADRANTS, T};
use crate::{
    diagrams::common::chart::{ChartRules, ChartState},
    token::{Cursor, Token},
    util::chars::{digits_end, is_line_end, is_space, is_word},
};

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Title,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    String,
    MdString,
    ClassName,
    PointStart,
    PointX,
    PointY,
}

impl S {
    /// The token the state gives for the empty rest of the input, where it
    /// gives one.
    fn at_end(self) -> Option<T> {
        Some(match self {
            Self::Initial => T::Eof,
            Self::Title => T::TitleValue,
            Self::AccTitle => T::AccTitleValue,
            Self::AccDescr => T::AccDescrValue,
            Self::AccDescrMultiline => T::AccDescrMultilineValue,
            Self::String => T::Str,
            _ => return None,
        })
    }
}

impl ChartState for S {
    const ACC_TITLE: Self = Self::AccTitle;
    const ACC_DESCR: Self = Self::AccDescr;
    const ACC_DESCR_MULTILINE: Self = Self::AccDescrMultiline;
    const STRING: Self = Self::String;
}

/// The last run scanned, so that a long run is not scanned again from each
/// of its characters.
#[derive(Default)]
struct Run {
    from: usize,
    to: usize,
}

impl Run {
    /// The end of the run at `at`, from `scan` unless `at` is inside the
    /// last run.
    fn end(&mut self, at: usize, scan: impl FnOnce(usize) -> usize) -> usize {
        if at < self.from || at >= self.to {
            self.from = at;
            self.to = scan(at);
        }
        self.to
    }
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    /// Spaces before a keyword.
    spaces: Run,
    /// Dashes of an axis arrow.
    dashes: Run,
    /// Whitespace before a point's colon.
    blank: Run,
    /// Whitespace between a point's colon and its bracket.
    gap: Run,
}

/// The end of the run of spaces at `from`.
fn spaces_end(cur: &Cursor<'_, T, S>, from: usize) -> usize {
    cur.run_end(from, |c| c == ' ')
}

impl Lexer<'_> {
    /// Mermaid's `(1)|(0(.\d+)?)`, where the dot is any one UTF-16 unit
    /// within a line.
    fn coordinate(&self, q: usize) -> Option<usize> {
        match self.cur.char_at(q)? {
            '1' => return Some(q + 1),
            '0' => {}
            _ => return None,
        }
        let e = q + 1;
        if let Some(dot) = self.cur.char_at(e)
            && !is_line_end(dot)
            && dot.len_utf16() == 1
            && self
                .cur
                .char_at(e + dot.len_utf8())
                .is_some_and(|c| c.is_ascii_digit())
        {
            return Some(digits_end(self.cur.src, e + dot.len_utf8()));
        }
        Some(e)
    }

    /// Reads one token or skips one piece of text; false where no rule
    /// matches.
    fn step(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match self.cur.state {
            S::Initial => self.initial(c),
            S::Title => {
                self.cur.rest_of_line(T::TitleValue);
                true
            }
            S::AccTitle | S::AccDescr | S::AccDescrMultiline | S::String => self.cur.value(),
            S::MdString => {
                if self.cur.starts_with("`\"", p) {
                    self.cur.p += 2;
                    self.cur.pop();
                    return true;
                }
                if c == '`' || c == '"' {
                    return false;
                }
                let end = self.cur.until(p, |c| matches!(c, '`' | '"'));
                self.cur.emit(T::MdStr, p, end);
                true
            }
            S::ClassName => {
                if !is_word(c) {
                    return false;
                }
                let end = self.cur.word_end(p);
                self.cur.emit(T::ClassName, p, end);
                self.cur.pop();
                true
            }
            S::PointStart => {
                if let Some(end) = self.coordinate(p) {
                    self.cur.emit(T::PointX, p, end);
                    self.cur.push(S::PointX);
                    return true;
                }
                let k = self.cur.space_end(p);
                if self.cur.char_at(k) != Some(']') {
                    return false;
                }
                self.cur.p = spaces_end(&self.cur, k + 1);
                self.cur.pop();
                true
            }
            S::PointX => {
                let k = self.cur.space_end(p);
                if self.cur.char_at(k) != Some(',') {
                    return false;
                }
                self.cur.p = self.cur.space_end(k + 1);
                self.cur.state = S::PointY;
                true
            }
            S::PointY => {
                let Some(end) = self.coordinate(p) else {
                    return false;
                };
                self.cur.emit(T::PointY, p, end);
                self.cur.pop();
                true
            }
        }
    }

    /// An axis, quadrant or chart keyword or the axis arrow at `at`, and
    /// where it ends.
    fn keyword(&mut self, at: usize) -> Option<(T, usize)> {
        let cur = &self.cur;
        if cur.starts_with_caseless("x-axis", at) {
            return Some((T::XAxis, at + 6));
        }
        if cur.starts_with_caseless("y-axis", at) {
            return Some((T::YAxis, at + 6));
        }
        if cur.char_at(at) == Some('-') {
            let end = self.dashes.end(at, |q| cur.run_end(q, |c| c == '-'));
            return (end - at >= 2 && cur.char_at(end) == Some('>'))
                .then_some((T::Delimiter, end + 1));
        }
        if cur.starts_with_caseless("quadrant", at) {
            if cur.char_at(at + 8) == Some('-')
                && let Some(k @ 1..=4) = cur.char_at(at + 9).and_then(|c| c.to_digit(10))
            {
                return Some((QUADRANTS[k as usize - 1], at + 10));
            }
            if cur.starts_with_caseless("chart", at + 8) {
                return Some((T::Quadrant, at + 13));
            }
        }
        None
    }

    fn initial(&mut self, c: char) -> bool {
        let p = self.cur.p;
        if self.cur.comment() || self.cur.newlines() {
            return true;
        }
        if self.cur.keyword_caseless("title", p) {
            self.cur.emit(T::Title, p, p + 5);
            self.cur.push(S::Title);
            return true;
        }
        if self.cur.accessibility() {
            return true;
        }

        // Axis, quadrant and chart keywords and the axis arrow take the spaces
        // around them.
        let at = if c == ' ' {
            let cur = &self.cur;
            self.spaces.end(p, |q| spaces_end(cur, q))
        } else {
            p
        };
        if let Some((kind, end)) = self.keyword(at) {
            let end = spaces_end(&self.cur, end);
            self.cur.emit(kind, p, end);
            return true;
        }

        if self.cur.keyword_caseless("classdef", p) {
            self.cur.emit(T::Classdef, p, p + 8);
            return true;
        }
        if c == '"' {
            if self.cur.char_at(p + 1) == Some('`') {
                self.cur.p += 2;
                self.cur.push(S::MdString);
            } else {
                self.cur.p += 1;
                self.cur.push(S::String);
            }
            return true;
        }
        if self.cur.starts_with(":::", p) {
            self.cur.p += 3;
            self.cur.push(S::ClassName);
            return true;
        }
        // A point's `: [`, with any whitespace before, between and after, line
        // breaks included.
        if c == ':' || is_space(c) {
            let cur = &self.cur;
            let colon = if c == ':' {
                p
            } else {
                self.blank.end(p, |q| cur.space_end(q))
            };
            if cur.char_at(colon) == Some(':') {
                let bracket = self.gap.end(colon + 1, |q| cur.space_end(q));
                if cur.char_at(bracket) == Some('[') {
                    let end = cur.space_end(bracket + 1);
                    self.cur.emit(T::PointStart, p, end);
                    self.cur.push(S::PointStart);
                    return true;
                }
            }
        }

        if !c.is_ascii() {
            let end = self.cur.run_end(p, |c| !c.is_ascii());
            self.cur.emit(T::UnicodeText, p, end);
            return true;
        }
        let kind = match c {
            ' ' | '\t' | '\x0b' | '\x0c' => T::Space,
            '!' | '$' | '%' | '\'' | '`' | '?' | '\\' | '/' => T::Punctuation,
            _ => return self.cur.word(c),
        };
        self.cur.emit(kind, p, p + 1);
        true
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let mut lexer = Lexer {
        cur: Cursor::new(src, S::Initial),
        spaces: Run::default(),
        dashes: Run::default(),
        blank: Run::default(),
        gap: Run::default(),
    };
    while let Some(c) = lexer.cur.char_at(lexer.cur.p) {
        if !lexer.step(c) {
            break;
        }
    }
    let mut cur = lexer.cur;
    let n = src.len();
    if cur.p < n {
        cur.emit(T::Error, cur.p, cur.p);
        return cur.tokens;
    }
    // Jison tries the rules of the current state once more against the empty
    // rest of the input.
    if let Some(kind) = cur.state.at_end() {
        cur.emit(kind, n, n);
    }
    cur.emit(T::End, n, n);
    cur.tokens
}
