//! Pele's `diagrams/class/lexer.ts`.

use super::tokens::T;
use crate::{
    diagrams::common::directions::Directions,
    js::Lazy,
    token::{Cursor, Token},
    util::{
        chars::{is_space, is_word},
        unicode::is_unicode_letter,
    },
};

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    String,
    BqString,
    Generic,
    CallbackName,
    CallbackArgs,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Class,
    ClassBody,
    Namespace,
    NamespaceBody,
}

impl S {
    /// The token the condition gives at the end of the input, before the
    /// parser sees the end itself.
    fn end_token(self) -> T {
        match self {
            Self::Initial | Self::BqString | Self::Class | Self::Namespace => T::Eof,
            Self::String => T::Str,
            Self::Generic => T::Generictype,
            Self::CallbackName => T::CallbackName,
            Self::CallbackArgs => T::CallbackArgs,
            Self::AccTitle => T::AccTitleValue,
            Self::AccDescr => T::AccDescrValue,
            Self::AccDescrMultiline => T::AccDescrMultilineValue,
            Self::ClassBody | Self::NamespaceBody => T::EofInStruct,
        }
    }
}

static DIRECTIONS: [(T, Lazy); 4] = [
    (T::DirectionTb, Lazy::new(r".*direction\s+TB[^\n]*", "y")),
    (T::DirectionBt, Lazy::new(r".*direction\s+BT[^\n]*", "y")),
    (T::DirectionRl, Lazy::new(r".*direction\s+RL[^\n]*", "y")),
    (T::DirectionLr, Lazy::new(r".*direction\s+LR[^\n]*", "y")),
];

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    directions: Directions<T>,
}

impl<'s> Lexer<'s> {
    fn at(&self, q: usize) -> Option<char> {
        self.cur.char_at(q)
    }

    /// A token from the cursor to `end`.
    fn emit(&mut self, kind: T, end: usize) -> bool {
        self.cur.emit_to(kind, end);
        true
    }

    /// The first `c` from the cursor on, or the end.
    fn until(&self, c: char) -> usize {
        self.cur.until(self.cur.p, |d| d == c)
    }

    /// A run of whitespace is a NEWLINE up to its last line feed; the rest is
    /// skipped.
    fn space(&mut self, leaves: bool) {
        let p = self.cur.p;
        let end = self.cur.space_end(p);
        let Some(last) = self.cur.src[p..end].rfind('\n') else {
            self.cur.p = end;
            return;
        };
        if leaves {
            self.cur.pop();
        }
        self.emit(T::Newline, p + last + 1);
    }

    /// The keywords that every state recognizes, even inside generics and
    /// backtick names.
    fn keyword(&mut self, c: char) -> bool {
        let p = self.cur.p;
        let next = self.at(p + 1);
        match c {
            'c' => {
                if self.cur.keyword("cssClass") {
                    return self.emit(T::Cssclass, p + 8);
                }
                if self.cur.keyword("callback") {
                    return self.emit(T::Callback, p + 8);
                }
                if self.cur.keyword("click") {
                    return self.emit(T::Click, p + 5);
                }
            }
            'l' if self.cur.keyword("link") => return self.emit(T::Link, p + 4),
            'n' => {
                if self.cur.keyword("note for") {
                    return self.emit(T::NoteFor, p + 8);
                }
                if self.cur.keyword("note") {
                    return self.emit(T::Note, p + 4);
                }
            }
            '<' if next == Some('<') => return self.emit(T::AnnotationStart, p + 2),
            '>' if next == Some('>') => return self.emit(T::AnnotationEnd, p + 2),
            'h' if self.cur.keyword("href") => return self.emit(T::Href, p + 4),
            _ => {}
        }
        false
    }

    fn common(&mut self, c: char) -> bool {
        if self.keyword(c) {
            return true;
        }
        let p = self.cur.p;
        let next = self.at(p + 1);
        let opens = match c {
            '"' => Some(S::String),
            '~' => Some(S::Generic),
            '`' => Some(S::BqString),
            _ => None,
        };
        if let Some(state) = opens {
            self.cur.push(state);
            self.cur.p += 1;
            return true;
        }
        let (kind, len) = match c {
            '_' => {
                for target in ["_self", "_blank", "_parent", "_top"] {
                    if self.cur.keyword(target) {
                        return self.emit(T::LinkTarget, p + target.len());
                    }
                }
                return self.word();
            }
            '<' if next == Some('|') => (T::Extension, 2),
            '<' | '>' => (T::Dependency, 1),
            '|' if next == Some('>') => (T::Extension, 2),
            '|' => return false,
            '*' => (T::Composition, 1),
            'o' if !next.is_some_and(is_word) => (T::Aggregation, 1),
            '(' if next == Some(')') => (T::Lollipop, 2),
            '(' => return false,
            '-' if next == Some('-') => (T::Line, 2),
            '-' => (T::Minus, 1),
            '.' if next == Some('.') => (T::DottedLine, 2),
            '.' => (T::Dot, 1),
            ':' => {
                let end = self.cur.until(p + 1, |c| matches!(c, ':' | '\n' | ';'));
                if end > p + 1 {
                    return self.emit(T::Label, end);
                }
                if next == Some(':') && self.at(p + 2) == Some(':') {
                    (T::StyleSeparator, 3)
                } else {
                    return false;
                }
            }
            '+' => (T::Plus, 1),
            '%' => (T::Pct, 1),
            '=' => (T::Equals, 1),
            '[' => (T::Sqs, 1),
            ']' => (T::Sqe, 1),
            '!' | '#' | '$' | '&' | '\'' | ',' | '?' | '\\' | '/' => (T::Punctuation, 1),
            _ => return self.word(),
        };
        self.emit(kind, p + len)
    }

    /// A run of word characters, or a letter outside ASCII.
    fn word(&mut self) -> bool {
        let p = self.cur.p;
        let Some(c) = self.at(p) else {
            return false;
        };
        if is_word(c) {
            let end = self.cur.word_end(p);
            return self.emit(T::Alpha, end);
        }
        // An astral character is a surrogate pair to Pele, whose halves are
        // no letters.
        c.len_utf8() < 4
            && is_unicode_letter(u32::from(c))
            && self.emit(T::UnicodeText, p + c.len_utf8())
    }

    fn direction(&mut self) -> bool {
        let p = self.cur.p;
        let Some((kind, end)) = self.directions.at(self.cur.src, p) else {
            return false;
        };
        self.emit(kind, end)
    }

    fn initial(&mut self, c: char) -> bool {
        if self.direction() {
            return true;
        }
        if is_space(c) {
            self.space(false);
            return true;
        }
        let p = self.cur.p;
        match c {
            '%' if self.at(p + 1) == Some('%') => {
                let mut end = self.until('\n');
                while matches!(self.at(end), Some('\n' | '\r')) {
                    end += 1;
                }
                self.cur.p = end;
                return true;
            }
            'a' => {
                let title = self.cur.starts_with("accTitle", p);
                if title || self.cur.starts_with("accDescr", p) {
                    let q = self.cur.space_end(p + 8);
                    match self.at(q) {
                        Some(':') => {
                            let end = self.cur.space_end(q + 1);
                            self.emit(if title { T::AccTitle } else { T::AccDescr }, end);
                            self.cur.push(if title { S::AccTitle } else { S::AccDescr });
                            return true;
                        }
                        Some('{') if !title => {
                            self.cur.p = self.cur.space_end(q + 1);
                            self.cur.push(S::AccDescrMultiline);
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            'c' => {
                if self.cur.keyword("classDiagram-v2") {
                    return self.emit(T::ClassDiagram, p + 15);
                }
                if self.cur.keyword("classDiagram") {
                    return self.emit(T::ClassDiagram, p + 12);
                }
                if self.cur.starts_with("call", p) && self.at(p + 4).is_some_and(is_space) {
                    self.cur.p = self.cur.space_end(p + 4);
                    self.cur.push(S::CallbackName);
                    return true;
                }
                if self.cur.keyword("classDef") {
                    return self.emit(T::Classdef, p + 8);
                }
                if self.cur.keyword("class") {
                    self.cur.push(S::Class);
                    return self.emit(T::Class, p + 5);
                }
            }
            '[' if self.cur.starts_with("*]", p + 1) => return self.emit(T::EdgeState, p + 3),
            's' if self.cur.keyword("style") => return self.emit(T::Style, p + 5),
            'n' if self.cur.keyword("namespace") => {
                self.cur.push(S::Namespace);
                return self.emit(T::Namespace, p + 9);
            }
            ',' => return self.emit(T::Comma, p + 1),
            '#' => return self.emit(T::Brkt, p + 1),
            _ => {}
        }
        self.common(c) || (c == ':' && self.emit(T::Colon, p + 1))
    }

    /// Reads one token or skips text in the current state; false where no
    /// token matches.
    fn step(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match self.cur.state {
            S::Initial => return self.initial(c),
            S::Class => match c {
                c if is_space(c) => self.space(true),
                '}' => {
                    self.cur.pop();
                    self.cur.pop();
                    self.emit(T::StructStop, p + 1);
                }
                '{' => {
                    self.cur.push(S::ClassBody);
                    self.emit(T::StructStart, p + 1);
                }
                _ => return self.common(c),
            },
            S::ClassBody => match c {
                '"' => {
                    self.cur.push(S::String);
                    self.cur.p += 1;
                }
                '}' => {
                    self.cur.pop();
                    self.emit(T::StructStop, p + 1);
                }
                '[' if self.cur.starts_with("*]", p + 1) => {
                    self.emit(T::EdgeState, p + 3);
                }
                '{' => {
                    self.emit(T::OpenInStruct, p + 1);
                }
                '\n' => self.cur.p += 1,
                _ => {
                    let end = self.cur.until(p + 1, |c| matches!(c, '{' | '}' | '\n'));
                    self.emit(T::Member, end);
                }
            },
            S::Namespace => match c {
                c if is_space(c) => self.space(true),
                '{' => {
                    self.cur.push(S::NamespaceBody);
                    self.emit(T::StructStart, p + 1);
                }
                // The brace is read again in the state the namespace was
                // opened from.
                '}' => self.cur.pop(),
                'n' if self.cur.keyword("namespace") => {
                    self.cur.push(S::Namespace);
                    self.emit(T::Namespace, p + 9);
                }
                _ => return self.common(c),
            },
            S::NamespaceBody => match c {
                c if is_space(c) => self.space(false),
                '}' => {
                    self.cur.pop();
                    self.emit(T::StructStop, p + 1);
                }
                'n' if self.cur.keyword("namespace") => {
                    self.cur.push(S::Namespace);
                    self.emit(T::Namespace, p + 9);
                }
                '[' if self.cur.starts_with("*]", p + 1) => {
                    self.emit(T::EdgeState, p + 3);
                }
                'c' if self.cur.keyword("class") => {
                    self.cur.push(S::Class);
                    self.emit(T::Class, p + 5);
                }
                _ => return self.common(c),
            },
            S::String => {
                if c == '"' {
                    self.cur.pop();
                    self.cur.p += 1;
                } else {
                    let end = self.until('"');
                    self.emit(T::Str, end);
                }
            }
            S::Generic => {
                if c == '"' {
                    self.cur.push(S::String);
                    self.cur.p += 1;
                } else if self.keyword(c) {
                } else if c == '~' {
                    self.cur.pop();
                    self.cur.p += 1;
                } else {
                    let end = self.until('~');
                    self.emit(T::Generictype, end);
                }
            }
            S::BqString => {
                if c == '"' {
                    self.cur.push(S::String);
                    self.cur.p += 1;
                } else if self.keyword(c) {
                } else if c == '~' {
                    self.cur.push(S::Generic);
                    self.cur.p += 1;
                } else if c == '`' {
                    self.cur.pop();
                    self.cur.p += 1;
                } else {
                    let end = self.until('`');
                    self.emit(T::BquoteStr, end);
                }
            }
            S::CallbackName => {
                if c == '(' {
                    let q = self.cur.space_end(p + 1);
                    self.cur.pop();
                    if self.at(q) == Some(')') {
                        self.cur.p = q + 1;
                    } else {
                        self.cur.push(S::CallbackArgs);
                        self.cur.p += 1;
                    }
                } else {
                    let end = self.until('(');
                    self.emit(T::CallbackName, end);
                }
            }
            S::CallbackArgs => {
                if c == ')' {
                    self.cur.pop();
                    self.cur.p += 1;
                } else {
                    let end = self.until(')');
                    self.emit(T::CallbackArgs, end);
                }
            }
            S::AccTitle | S::AccDescr => {
                let kind = if self.cur.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                let end = self.until('\n');
                self.emit(kind, end);
                self.cur.pop();
            }
            S::AccDescrMultiline => {
                if c == '}' {
                    self.cur.pop();
                    self.cur.p += 1;
                } else {
                    let end = self.until('}');
                    self.emit(T::AccDescrMultilineValue, end);
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
        directions: Directions::new(src, &DIRECTIONS),
    };

    while let Some(c) = lexer.at(lexer.cur.p) {
        if !lexer.step(c) {
            let p = lexer.cur.p;
            let mut tokens = lexer.cur.tokens;
            tokens.push(Token::new(T::Error, "", p));
            return tokens;
        }
    }

    let mut tokens = lexer.cur.tokens;
    tokens.push(Token::new(lexer.cur.state.end_token(), "", n));
    tokens.push(Token::new(T::End, "", n));
    tokens
}
