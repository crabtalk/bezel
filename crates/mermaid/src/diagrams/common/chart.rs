//! The Jison lexer rules Mermaid's quadrantChart and xychart grammars share:
//! comments, line breaks, `accTitle` and `accDescr`, strings, and words,
//! numbers and punctuation.

use crate::{token::Cursor, util::chars::digits_end};

/// The token kinds the shared rules make.
pub trait ChartKind: Copy + PartialEq {
    const NEWLINE: Self;
    const ACC_TITLE: Self;
    const ACC_TITLE_VALUE: Self;
    const ACC_DESCR: Self;
    const ACC_DESCR_VALUE: Self;
    const ACC_DESCR_MULTILINE_VALUE: Self;
    const STR: Self;
    const ALPHA: Self;
    const NUM: Self;
    const COLON: Self;
    const PLUS: Self;
    const COMMA: Self;
    const EQUALS: Self;
    const MULT: Self;
    const BRKT: Self;
    const UNDERSCORE: Self;
    const DOT: Self;
    const AMP: Self;
    const MINUS: Self;
    const SEMI: Self;
    const EOF: Self;

    /// A newline, a semicolon or the end of the text.
    fn is_eol(self) -> bool {
        self == Self::NEWLINE || self == Self::SEMI || self == Self::EOF
    }

    /// The token of the single character `c`, if it is one of the shared
    /// punctuation tokens.
    fn punctuation(c: char) -> Option<Self> {
        Some(match c {
            ':' => Self::COLON,
            '+' => Self::PLUS,
            ',' => Self::COMMA,
            '=' => Self::EQUALS,
            '*' => Self::MULT,
            '#' => Self::BRKT,
            '_' => Self::UNDERSCORE,
            '.' => Self::DOT,
            '&' => Self::AMP,
            '-' => Self::MINUS,
            ';' => Self::SEMI,
            _ => return None,
        })
    }
}

/// The start conditions the shared rules enter.
pub trait ChartState: Copy + Eq {
    const ACC_TITLE: Self;
    const ACC_DESCR: Self;
    const ACC_DESCR_MULTILINE: Self;
    const STRING: Self;
}

/// The shared rules, each reading at the cursor and answering whether it
/// matched.
pub trait ChartRules<K> {
    /// Skips a comment.
    fn comment(&mut self) -> bool;
    /// A NEWLINE token of the run of line breaks.
    fn newlines(&mut self) -> bool;
    /// `accTitle` or `accDescr` with a colon, or `accDescr` with a brace,
    /// entering the start condition of its value.
    fn accessibility(&mut self) -> bool;
    /// The rules of the start conditions of [`ChartState`]; false in any
    /// other.
    fn value(&mut self) -> bool;
    /// A `kind` token of the rest of the line, leaving the start condition.
    fn rest_of_line(&mut self, kind: K);
    /// An ALPHA, NUM or shared punctuation token at `c`, the character at
    /// the cursor.
    fn word(&mut self, c: char) -> bool;
}

impl<K: ChartKind, S: ChartState> ChartRules<K> for Cursor<'_, K, S> {
    fn comment(&mut self) -> bool {
        let p = self.p;
        let Some(c) = self.char_at(p) else {
            return false;
        };
        let from = if self.starts_with("%%", p) {
            p + 2
        // Mermaid's second comment rule takes any one UTF-16 unit but `}`
        // before `%%` with it, a line break included.
        } else if c != '}' && c.len_utf16() == 1 && self.starts_with("%%", p + c.len_utf8()) {
            p + c.len_utf8() + 2
        } else {
            return false;
        };
        self.p = self.line_end(from);
        true
    }

    fn newlines(&mut self) -> bool {
        let p = self.p;
        let end = self.run_end(p, |c| matches!(c, '\n' | '\r'));
        if end == p {
            return false;
        }
        self.emit(K::NEWLINE, p, end);
        true
    }

    fn accessibility(&mut self) -> bool {
        let p = self.p;
        let descr = self.starts_with_caseless("accdescr", p);
        if !descr && !self.starts_with_caseless("acctitle", p) {
            return false;
        }
        let k = self.space_end(p + 8);
        match self.char_at(k) {
            Some(':') => {
                let end = self.space_end(k + 1);
                let (kind, state) = if descr {
                    (K::ACC_DESCR, S::ACC_DESCR)
                } else {
                    (K::ACC_TITLE, S::ACC_TITLE)
                };
                self.emit(kind, p, end);
                self.push(state);
            }
            Some('{') if descr => {
                self.p = self.space_end(k + 1);
                self.push(S::ACC_DESCR_MULTILINE);
            }
            _ => return false,
        }
        true
    }

    fn value(&mut self) -> bool {
        let state = self.state;
        if state == S::ACC_TITLE || state == S::ACC_DESCR {
            self.rest_of_line(if state == S::ACC_TITLE {
                K::ACC_TITLE_VALUE
            } else {
                K::ACC_DESCR_VALUE
            });
            return true;
        }
        let (close, kind) = if state == S::ACC_DESCR_MULTILINE {
            ('}', K::ACC_DESCR_MULTILINE_VALUE)
        } else if state == S::STRING {
            ('"', K::STR)
        } else {
            return false;
        };
        let p = self.p;
        if self.char_at(p) == Some(close) {
            self.p += 1;
            self.pop();
        } else {
            let end = self.until(p, |c| c == close);
            self.emit(kind, p, end);
        }
        true
    }

    fn rest_of_line(&mut self, kind: K) {
        let end = self.line_end(self.p);
        self.emit(kind, self.p, end);
        self.pop();
    }

    fn word(&mut self, c: char) -> bool {
        let p = self.p;
        let (kind, end) = if c.is_ascii_alphabetic() {
            (K::ALPHA, self.run_end(p, |c| c.is_ascii_alphabetic()))
        } else if c.is_ascii_digit() {
            (K::NUM, digits_end(self.src, p))
        } else if let Some(kind) = K::punctuation(c) {
            (kind, p + 1)
        } else {
            return false;
        };
        self.emit(kind, p, end);
        true
    }
}
