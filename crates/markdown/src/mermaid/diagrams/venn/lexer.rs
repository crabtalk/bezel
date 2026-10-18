//! Pele's `diagrams/venn/lexer.ts`.

use crate::mermaid::util::chars::{code_at, is_digit, is_hex, is_space, is_word, width_at};

pub const END: i32 = 0;
pub const ERROR: i32 = 1;
pub const EOF: i32 = 2;
pub const NEWLINE: i32 = 3;
pub const VENN: i32 = 4;
pub const TITLE: i32 = 5;
pub const SET: i32 = 6;
pub const UNION: i32 = 7;
pub const TEXT: i32 = 8;
pub const INDENT_TEXT: i32 = 9;
pub const STYLE: i32 = 10;
pub const BRACKET_LABEL: i32 = 11;
pub const NUMERIC: i32 = 12;
pub const HEXCOLOR: i32 = 13;
pub const RGBCOLOR: i32 = 14;
pub const RGBACOLOR: i32 = 15;
pub const IDENTIFIER: i32 = 16;
pub const STRING: i32 = 17;
pub const COMMA: i32 = 18;
pub const COLON: i32 = 19;

pub const TOKEN_NAMES: [&str; 20] = [
    "$end",
    "INVALID",
    "EOF",
    "NEWLINE",
    "VENN",
    "TITLE",
    "SET",
    "UNION",
    "TEXT",
    "INDENT_TEXT",
    "STYLE",
    "BRACKET_LABEL",
    "NUMERIC",
    "HEXCOLOR",
    "RGBCOLOR",
    "RGBACOLOR",
    "IDENTIFIER",
    "STRING",
    "COMMA",
    "COLON",
];

const KEYWORDS: [(&str, i32); 5] = [
    ("venn-beta", VENN),
    ("set", SET),
    ("union", UNION),
    ("text", TEXT),
    ("style", STYLE),
];

/// The part of the model the lexer talks to: an indented `text` line is a
/// different token after a set or union than anywhere else, and the parser is
/// what knows which came before.
pub trait IndentHost {
    fn indent_mode(&self) -> bool;
    fn set_indent_mode(&mut self, enabled: bool);
}

/// Reads tokens one at a time, as Mermaid's Jison lexer for venn diagrams
/// does: every rule ignores case, the first rule that matches wins, and a
/// keyword must end at a word boundary.
pub struct VennLexer<'s> {
    /// The text and position of the token `next` returned last.
    pub text: &'s str,
    pub start: usize,
    src: &'s str,
    at: usize,
    line_start: bool,
    ended: bool,
}

impl<'s> VennLexer<'s> {
    pub fn new(src: &'s str) -> Self {
        Self {
            text: "",
            start: 0,
            src,
            at: 0,
            line_start: false,
            ended: false,
        }
    }

    fn word(&self, at: usize, word: &str) -> bool {
        word.bytes().enumerate().all(|(k, w)| {
            let c = code_at(self.src, at + k);
            let w = u32::from(w);
            c == w || ((65..=90).contains(&c) && c + 32 == w)
        })
    }

    fn keyword(&self, at: usize, word: &str) -> bool {
        self.word(at, word) && !is_word(code_at(self.src, at + word.len()))
    }

    /// The end of `rgb(1, 2, 3)` or `rgba(1, 2, 3, 0.5)` whose parenthesis
    /// opens at `at`.
    fn color(&self, at: usize, parts: usize) -> Option<usize> {
        let src = self.src;
        let mut i = at + 1;
        for part in 0..parts {
            while is_space(code_at(src, i)) {
                i += width_at(src, i);
            }
            let from = i;
            while is_digit(code_at(src, i)) || code_at(src, i) == 46 {
                i += 1;
            }
            if i == from {
                return None;
            }
            while is_space(code_at(src, i)) {
                i += width_at(src, i);
            }
            if code_at(src, i) != if part == parts - 1 { 41 } else { 44 } {
                return None;
            }
            i += 1;
        }
        Some(i)
    }

    fn line_end(&self, from: usize) -> usize {
        self.src[from.min(self.src.len())..]
            .find('\n')
            .map_or(self.src.len(), |end| from + end)
    }

    pub fn next(&mut self, host: &mut dyn IndentHost) -> i32 {
        let src = self.src;
        let n = src.len();
        loop {
            let mut i = self.at;
            self.start = i;
            self.text = "";
            if i >= n {
                if self.ended {
                    return END;
                }
                self.ended = true;
                return EOF;
            }
            let c = code_at(src, i);
            let w = width_at(src, i);
            // A comment runs to the end of the line. It also takes the
            // character before it along, whatever that is: a line break, or
            // the last letter of a word.
            if (c == 37 && code_at(src, i + 1) == 37 && code_at(src, i + 2) != 123)
                || (c != 125 && code_at(src, i + w) == 37 && code_at(src, i + w + 1) == 37)
            {
                self.at = self.line_end(i + w);
                continue;
            }
            if self.line_start {
                if c == 32 || c == 9 {
                    let mut j = i + 1;
                    while matches!(code_at(src, j), 32 | 9) {
                        j += 1;
                    }
                    self.at = j;
                    if self.keyword(j, "text") && host.indent_mode() {
                        // The word `text` itself is dropped; the indentation
                        // stands for it.
                        self.at = j + 4;
                        self.line_start = false;
                        self.text = &src[i..j];
                        return INDENT_TEXT;
                    }
                    continue;
                }
                if c != 10 && c != 13 {
                    host.set_indent_mode(false);
                    self.line_start = false;
                }
            }
            if c == 10 || c == 13 {
                i += 1;
                while matches!(code_at(src, i), 10 | 13) {
                    i += 1;
                }
                self.line_start = true;
                return self.token(NEWLINE, i);
            }
            if c == 37 && code_at(src, i + 1) == 37 {
                self.at = self.line_end(i);
                continue;
            }
            if c == 32 || c == 9 {
                i += 1;
                while matches!(code_at(src, i), 32 | 9) {
                    i += 1;
                }
                self.at = i;
                continue;
            }

            if ((c | 32) >= 97 && (c | 32) <= 122) || c == 95 {
                if self.word(i, "title") && is_space(code_at(src, i + 5)) {
                    let body = i + 5 + width_at(src, i + 5);
                    let mut end = body;
                    while end < n {
                        let ch = code_at(src, end);
                        if ch == 35 || ch == 10 || ch == 59 {
                            break;
                        }
                        end += width_at(src, end);
                    }
                    if end > body {
                        return self.token(TITLE, end);
                    }
                }
                for (word, kind) in KEYWORDS {
                    if self.keyword(i, word) {
                        return self.token(kind, i + word.len());
                    }
                }
                if self.word(i, "rgb") {
                    let alpha = (code_at(src, i + 3) | 32) == 97;
                    let open = i + if alpha { 4 } else { 3 };
                    let end = if code_at(src, open) == 40 {
                        self.color(open, if alpha { 4 } else { 3 })
                    } else {
                        None
                    };
                    if let Some(end) = end {
                        return self.token(if alpha { RGBACOLOR } else { RGBCOLOR }, end);
                    }
                }
                i += 1;
                while is_word(code_at(src, i)) || code_at(src, i) == 45 {
                    i += 1;
                }
                return self.token(IDENTIFIER, i);
            }
            if c == 91 {
                if code_at(src, i + 1) == 34 {
                    let quote = src[i + 2..].find('"').map(|q| i + 2 + q);
                    let Some(quote) = quote.filter(|&q| code_at(src, q + 1) == 93) else {
                        return ERROR;
                    };
                    self.at = quote + 2;
                    self.text = &src[i + 2..quote];
                    return BRACKET_LABEL;
                }
                let mut end = i + 1;
                while end < n && !matches!(code_at(src, end), 93 | 34) {
                    end += width_at(src, end);
                }
                if end == i + 1 || code_at(src, end) != 93 {
                    return ERROR;
                }
                self.at = end + 1;
                self.text = crate::mermaid::js::trim(&src[i + 1..end]);
                return BRACKET_LABEL;
            }
            if is_digit(c) || c == 46 || c == 43 || c == 45 {
                if c == 43 || c == 45 {
                    i += 1;
                }
                let from = i;
                while is_digit(code_at(src, i)) {
                    i += 1;
                }
                if i > from {
                    if code_at(src, i) == 46 && is_digit(code_at(src, i + 1)) {
                        i += 2;
                        while is_digit(code_at(src, i)) {
                            i += 1;
                        }
                    }
                } else if code_at(src, i) == 46 && is_digit(code_at(src, i + 1)) {
                    i += 2;
                    while is_digit(code_at(src, i)) {
                        i += 1;
                    }
                } else {
                    return ERROR;
                }
                return self.token(NUMERIC, i);
            }
            if c == 35 {
                let mut end = i + 1;
                while end < i + 9 && is_hex(code_at(src, end)) {
                    end += 1;
                }
                return if end >= i + 4 {
                    self.token(HEXCOLOR, end)
                } else {
                    ERROR
                };
            }
            if c == 34 {
                return match src[i + 1..].find('"') {
                    Some(quote) => self.token(STRING, i + 1 + quote + 1),
                    None => ERROR,
                };
            }
            if c == 44 {
                return self.token(COMMA, i + 1);
            }
            if c == 58 {
                return self.token(COLON, i + 1);
            }
            return ERROR;
        }
    }

    fn token(&mut self, kind: i32, end: usize) -> i32 {
        self.text = &self.src[self.at..end];
        self.at = end;
        kind
    }
}
