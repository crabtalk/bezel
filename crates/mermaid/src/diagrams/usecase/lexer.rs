//! Pele's `diagrams/usecase/lexer.ts`. Offsets count UTF-16 units, as the
//! model reports them.

use super::tokens::T;
use crate::{
    detect::Kind,
    error::Error,
    util::chars::{NONE, is_blank, is_digit, is_hex, is_letter},
};

const WORD: u8 = 1;
const PUNCTUATION: u8 = 2;
/// Each keyword, in the order of Mermaid's token list, and whether it
/// ignores case.
const KEYWORDS: [(&str, T, bool); 20] = [
    ("actor", T::Actor, false),
    ("systemBoundary", T::SystemBoundary, false),
    ("style", T::Style, false),
    ("end", T::End, false),
    ("extend", T::Extend, true),
    ("direction", T::Direction, false),
    ("TD", T::Td, false),
    ("TB", T::Tb, false),
    ("BT", T::Bt, false),
    ("LR", T::Lr, false),
    ("RL", T::Rl, false),
    ("note", T::Note, false),
    ("for", T::For, false),
    ("false", T::False, false),
    ("json", T::Json, false),
    ("classDef", T::ClassDef, false),
    ("class", T::Class, false),
    ("include", T::Include, true),
    ("true", T::True, false),
    ("usecase-beta", T::Usecase, false),
];

pub struct Tokens {
    pub types: Vec<T>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
}

/// A diagram's text as UTF-16 units, the way the lexer and the model count
/// it.
pub struct Source {
    pub text: String,
    pub units: Vec<u16>,
}

fn class_of(c: u32) -> u8 {
    match c {
        48..=57 | 65..=90 | 97..=122 | 95 => WORD,
        _ if c < 128 && b"!#$&*+/=?^|~".contains(&(c as u8)) => PUNCTUATION,
        _ => 0,
    }
}

fn is_word(c: u32) -> bool {
    c < 128 && (class_of(c) & WORD) != 0
}

impl Source {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            units: text.encode_utf16().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// `charCodeAt(at)`, or [`NONE`] past the end.
    pub fn code(&self, at: usize) -> u32 {
        self.units.get(at).map_or(NONE, |&u| u32::from(u))
    }

    pub fn slice(&self, from: usize, to: usize) -> String {
        let to = to.min(self.units.len());
        let from = from.min(to);
        String::from_utf16_lossy(&self.units[from..to])
    }

    /// `charAt(at)`.
    pub fn char_at(&self, at: usize) -> String {
        self.slice(at, at + 1)
    }

    pub fn starts_with(&self, at: usize, word: &str) -> bool {
        word.encode_utf16()
            .enumerate()
            .all(|(k, unit)| self.units.get(at + k) == Some(&unit))
    }

    /// `indexOf(needle, from)`.
    pub fn index_of(&self, needle: &str, from: usize) -> Option<usize> {
        let needle: Vec<u16> = needle.encode_utf16().collect();
        if needle.is_empty() {
            return Some(from.min(self.units.len()));
        }
        (from..self.units.len().saturating_sub(needle.len() - 1))
            .find(|&at| self.units[at..at + needle.len()] == needle[..])
    }

    /// Mermaid's INCLUDE and EXTEND are the only keywords matched without
    /// regard to case.
    fn starts_with_no_case(&self, at: usize, word: &str) -> bool {
        word.bytes()
            .enumerate()
            .all(|(k, b)| (self.code(at + k) | 32) == u32::from(b))
    }

    /// The keyword that is a prefix of the text at `at`, and its length.
    fn keyword_at(&self, at: usize) -> Option<(T, usize)> {
        KEYWORDS
            .iter()
            .find(|&&(word, _, ignore_case)| {
                if ignore_case {
                    self.starts_with_no_case(at, word)
                } else {
                    self.starts_with(at, word)
                }
            })
            .map(|&(word, kind, _)| (kind, word.len()))
    }

    /// The line and column of `offset`, both from 1.
    pub fn line_column(&self, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut line_start = 0;
        let mut i = 0;
        while i < offset {
            let c = self.code(i);
            if c == 10 || c == 13 {
                if c == 13 && i + 1 < offset && self.code(i + 1) == 10 {
                    i += 1;
                }
                line += 1;
                line_start = i + 1;
            }
            i += 1;
        }
        (line, offset - line_start + 1)
    }

    pub fn location_text(&self, start: usize, end: usize) -> String {
        let (line, column) = self.line_column(start);
        format!("line {line}, column {column} [{start},{end})")
    }

    /// The end of a quoted string that opens at `at`, or `None` when the line
    /// ends first.
    fn string_end(&self, at: usize, quote: u32) -> Option<usize> {
        for k in at + 1..self.len() {
            let c = self.code(k);
            if c == quote {
                return Some(k + 1);
            }
            if c == 10 || c == 13 {
                return None;
            }
        }
        None
    }

    fn starts_token(&self, at: usize) -> bool {
        let c = self.code(at);
        if c != 34 && c != 39 {
            return true;
        }
        (c == 34 && self.code(at + 1) == 96) || self.string_end(at, c).is_some()
    }

    /// Mermaid reports the first character no token starts with, and how
    /// many characters its lexer skipped before one did.
    fn lex_error(&self, offset: usize, length: usize) -> Error {
        let (line, column) = self.line_column(offset);
        let mut error = Error::syntax(format!(
            "Error lexing usecase diagram: unexpected character: ->{}<- at offset: {offset}, skipped {length} characters. at line {line}, column {column} [{offset},{})",
            self.char_at(offset),
            offset + length
        ))
        .of(Kind::Usecase);
        error.line = line;
        error.column = column;
        error
    }

    /// Produces the tokens of Mermaid's Chevrotain lexer for use case
    /// diagrams: the first pattern in its list that matches wins, a keyword
    /// gives way to a longer identifier, and `json id@` and `<<` switch to
    /// modes that read a JSON object and a stereotype.
    pub fn tokenize(&self) -> Result<Tokens, Error> {
        let mut types: Vec<T> = Vec::new();
        let mut starts: Vec<usize> = Vec::new();
        let mut ends: Vec<usize> = Vec::new();
        let n = self.len();
        let mut i = 0;
        // Whether only blanks came before on this line, where comments and
        // accessibility lines may start.
        let mut line_start = true;
        let mut no_brace = false;

        'tokens: while i < n {
            let c = self.code(i);
            if c == 32 || c == 9 {
                i += 1;
                continue;
            }
            let mut kind: T;
            let mut end = i + 1;
            let mut fresh = false;

            if is_word(c) {
                let mut line = None;
                if c == 97 && line_start && self.starts_with(i, "acc") {
                    let descr = self.starts_with(i + 3, "Descr");
                    if descr || self.starts_with(i + 3, "Title") {
                        let mut j = i + 8;
                        while is_blank(self.code(j)) {
                            j += 1;
                        }
                        let next = self.code(j);
                        if next == 58 {
                            line = Some(if descr {
                                T::AccDescrLine
                            } else {
                                T::AccTitleLine
                            });
                            end = j + 1;
                            while end < n && self.code(end) != 10 && self.code(end) != 13 {
                                end += 1;
                            }
                        } else if next == 123 && descr && !no_brace {
                            match self.index_of("}", j + 1) {
                                None => no_brace = true,
                                Some(close) => {
                                    line = Some(T::AccDescrBlock);
                                    end = close + 1;
                                }
                            }
                        }
                    }
                } else if c == 106 && self.starts_with(i, "json") && is_blank(self.code(i + 4)) {
                    let mut j = i + 5;
                    while is_blank(self.code(j)) {
                        j += 1;
                    }
                    let id = j;
                    while is_word(self.code(j)) {
                        j += 1;
                    }
                    if j > id {
                        while is_blank(self.code(j)) {
                            j += 1;
                        }
                        if self.code(j) == 64 {
                            j += 1;
                            while is_blank(self.code(j)) {
                                j += 1;
                            }
                            if self.code(j) == 123 {
                                types.push(T::JsonDeclarationStart);
                                starts.push(i);
                                ends.push(j);
                                let mut depth = 0i64;
                                let mut quoted = false;
                                let mut escaped = false;
                                let mut k = j;
                                while k < n {
                                    let ch = self.code(k);
                                    if quoted {
                                        if escaped {
                                            escaped = false;
                                        } else if ch == 92 {
                                            escaped = true;
                                        } else if ch == 34 {
                                            quoted = false;
                                        }
                                    } else if ch == 34 {
                                        quoted = true;
                                    } else if ch == 123 {
                                        depth += 1;
                                    } else if ch == 125 {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    k += 1;
                                }
                                types.push(if k < n {
                                    T::JsonObjectLiteral
                                } else {
                                    T::UnclosedJsonObjectLiteral
                                });
                                starts.push(j);
                                i = if k < n { k + 1 } else { n };
                                ends.push(i);
                                line_start = false;
                                continue 'tokens;
                            }
                        }
                    }
                }
                if let Some(line) = line {
                    kind = line;
                } else {
                    while end < n && is_word(self.code(end)) {
                        end += 1;
                    }
                    if let Some((keyword, length)) = self.keyword_at(i) {
                        // A keyword that is only the start of a longer word
                        // is an identifier.
                        if keyword == T::Usecase || end - i == length {
                            kind = keyword;
                            end = i + length;
                        } else {
                            kind = T::Identifier;
                        }
                    } else if (c == 111 || c == 120)
                        && self.code(i + 1) == 45
                        && self.code(i + 2) == 45
                    {
                        kind = if c == 111 {
                            T::BackwardCircle
                        } else {
                            T::BackwardCross
                        };
                        end = i + 3;
                    } else if is_digit(c) {
                        // `1mg` is an identifier, and `1.5px` is a number
                        // because the identifier stops at the dot.
                        let mut digits = i + 1;
                        while is_digit(self.code(digits)) {
                            digits += 1;
                        }
                        if self.code(digits) == 46 && is_digit(self.code(digits + 1)) {
                            kind = T::Number;
                            end = digits + 2;
                            while is_digit(self.code(end)) {
                                end += 1;
                            }
                            while is_letter(self.code(end)) {
                                end += 1;
                            }
                        } else {
                            kind = T::Identifier;
                        }
                    } else {
                        kind = T::Identifier;
                        while self.code(end) == 45 && is_word(self.code(end + 1)) {
                            kind = T::CssIdentifier;
                            end += 2;
                            while is_word(self.code(end)) {
                                end += 1;
                            }
                        }
                    }
                }
            } else if c == 10 {
                kind = T::Newline;
                fresh = true;
            } else if c == 13 {
                kind = T::Newline;
                fresh = true;
                if self.code(end) == 10 {
                    end += 1;
                }
            } else if c == 34 || c == 39 {
                if c == 34 && self.code(i + 1) == 96 {
                    let close = self.index_of("`\"", i + 2);
                    kind = if close.is_none() {
                        T::UnclosedMarkdownString
                    } else {
                        T::MarkdownString
                    };
                    end = close.map_or(n, |close| close + 2);
                } else {
                    kind = T::PlainString;
                    match self.string_end(i, c) {
                        Some(e) => end = e,
                        None => {
                            let mut resume = i + 1;
                            while resume < n && !self.starts_token(resume) {
                                resume += 1;
                            }
                            return Err(self.lex_error(i, resume - i));
                        }
                    }
                }
            } else if c == 37 {
                if line_start && self.code(end) == 37 {
                    kind = T::Comment;
                    while end < n && self.code(end) != 10 && self.code(end) != 13 {
                        end += 1;
                    }
                } else {
                    kind = T::Percent;
                }
            } else if c == 45 {
                if self.code(end) != 45 {
                    kind = T::Dash;
                } else {
                    while self.code(end) == 45 {
                        end += 1;
                    }
                    let next = self.code(end);
                    let two = end - i == 2;
                    if two && next == 124 && self.code(end + 1) == 62 {
                        kind = T::Generalization;
                        end += 2;
                    } else if next == 62 {
                        kind = T::ForwardSolid;
                        end += 1;
                    } else if two && (next == 111 || next == 120) {
                        kind = if next == 111 {
                            T::ForwardCircle
                        } else {
                            T::ForwardCross
                        };
                        end += 1;
                    } else {
                        kind = T::MarkerlessSolid;
                    }
                }
            } else if c == 46 {
                let next = self.code(end);
                if next == 46 && self.code(end + 1) == 62 {
                    kind = T::DependencyArrow;
                    end += 2;
                } else if is_digit(next) {
                    kind = T::Number;
                    while is_digit(self.code(end)) {
                        end += 1;
                    }
                    while is_letter(self.code(end)) {
                        end += 1;
                    }
                } else {
                    kind = T::Dot;
                }
            } else if c == 60 {
                if self.code(end) == 60 {
                    types.push(T::StereotypeStart);
                    starts.push(i);
                    ends.push(i + 2);
                    i += 2;
                    if i >= n {
                        break;
                    }
                    if self.code(i) == 62 && self.code(i + 1) == 62 {
                        kind = T::StereotypeEnd;
                        end = i + 2;
                    } else {
                        let mut close = i;
                        let mut found = false;
                        while close < n {
                            let ch = self.code(close);
                            if ch == 10 || ch == 13 {
                                break;
                            }
                            if ch == 62 && self.code(close + 1) == 62 {
                                found = true;
                                break;
                            }
                            close += 1;
                        }
                        if found && !crate::js::trim(&self.slice(i, close)).is_empty() {
                            types.push(T::StereotypeText);
                            starts.push(i);
                            ends.push(close);
                            kind = T::StereotypeEnd;
                            i = close;
                            end = close + 2;
                        } else {
                            while close < n && self.code(close) != 10 && self.code(close) != 13 {
                                close += 1;
                            }
                            if close == i {
                                while close < n
                                    && (self.code(close) == 10 || self.code(close) == 13)
                                {
                                    close += 1;
                                }
                                return Err(self.lex_error(i, close - i));
                            }
                            kind = T::UnclosedStereotypeText;
                            end = close;
                        }
                    }
                } else if self.code(end) == 45 && self.code(end + 1) == 45 {
                    kind = T::BackwardSolid;
                    end += 2;
                    while self.code(end) == 45 {
                        end += 1;
                    }
                } else {
                    kind = T::LabelPunctuation;
                }
            } else if c == 58 {
                if self.code(end) == 58 && self.code(end + 1) == 58 {
                    kind = T::ClassSeparator;
                    end += 2;
                } else {
                    kind = T::Colon;
                }
            } else if c == 64 {
                if self.code(end) == 123 {
                    kind = T::MetadataStart;
                    end += 1;
                } else {
                    kind = T::At;
                }
            } else if c == 123 {
                kind = T::Lbrace;
            } else if c == 125 {
                kind = T::Rbrace;
            } else if c == 91 {
                kind = T::Lbracket;
            } else if c == 93 {
                kind = T::Rbracket;
            } else if c == 40 {
                kind = T::Lparen;
            } else if c == 41 {
                kind = T::Rparen;
            } else if c == 44 {
                kind = T::Comma;
            } else if c == 35 {
                while is_hex(self.code(end)) {
                    end += 1;
                }
                kind = if end > i + 1 {
                    T::HashColor
                } else {
                    T::CssPunctuation
                };
            } else if c == 92 {
                if self.code(end) == 44 {
                    kind = T::CssEscapedComma;
                    end += 1;
                } else {
                    kind = T::LabelPunctuation;
                }
            } else if c < 127 && c > 32 {
                kind = if (class_of(c) & PUNCTUATION) != 0 {
                    T::CssPunctuation
                } else {
                    T::LabelPunctuation
                };
            } else {
                kind = T::LabelSymbol;
                while end < n {
                    let ch = self.code(end);
                    if (32..127).contains(&ch) || ch == 9 || ch == 10 || ch == 13 {
                        break;
                    }
                    end += 1;
                }
            }

            types.push(kind);
            starts.push(i);
            ends.push(end);
            i = end;
            line_start = fresh;
        }

        for _ in 0..3 {
            types.push(T::Eof);
            starts.push(n);
            ends.push(n);
        }
        Ok(Tokens {
            types,
            starts,
            ends,
        })
    }
}
