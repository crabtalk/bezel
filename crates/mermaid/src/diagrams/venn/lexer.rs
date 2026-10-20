//! Pele's `diagrams/venn/lexer.ts`.

use crate::{
    token::{Token, grammar},
    util::chars::{code_at, is_digit, is_hex, is_space, is_word, width_at},
};

grammar! {
    /// The venn lexer's token kinds.
    pub enum T {
        End = "$end",
        Error = "INVALID",
        Eof = "EOF",
        Newline = "NEWLINE",
        Venn = "VENN",
        Title = "TITLE",
        Set = "SET",
        Union = "UNION",
        Text = "TEXT",
        IndentText = "INDENT_TEXT",
        Style = "STYLE",
        BracketLabel = "BRACKET_LABEL",
        Numeric = "NUMERIC",
        Hexcolor = "HEXCOLOR",
        Rgbcolor = "RGBCOLOR",
        Rgbacolor = "RGBACOLOR",
        Identifier = "IDENTIFIER",
        String = "STRING",
        Comma = "COMMA",
        Colon = "COLON",
    }
}

const KEYWORDS: [(&str, T); 5] = [
    ("venn-beta", T::Venn),
    ("set", T::Set),
    ("union", T::Union),
    ("text", T::Text),
    ("style", T::Style),
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
    /// The text and position of the token being read.
    text: &'s str,
    start: usize,
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

    pub fn next(&mut self, host: &mut dyn IndentHost) -> Token<'s, T> {
        let kind = self.scan(host);
        Token::new(kind, self.text, self.start)
    }

    fn scan(&mut self, host: &mut dyn IndentHost) -> T {
        let src = self.src;
        let n = src.len();
        loop {
            let mut i = self.at;
            self.start = i;
            self.text = "";
            if i >= n {
                if self.ended {
                    return T::End;
                }
                self.ended = true;
                return T::Eof;
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
                        return T::IndentText;
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
                return self.token(T::Newline, i);
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
                        return self.token(T::Title, end);
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
                        return self.token(if alpha { T::Rgbacolor } else { T::Rgbcolor }, end);
                    }
                }
                i += 1;
                while is_word(code_at(src, i)) || code_at(src, i) == 45 {
                    i += 1;
                }
                return self.token(T::Identifier, i);
            }
            if c == 91 {
                if code_at(src, i + 1) == 34 {
                    let quote = src[i + 2..].find('"').map(|q| i + 2 + q);
                    let Some(quote) = quote.filter(|&q| code_at(src, q + 1) == 93) else {
                        return T::Error;
                    };
                    self.at = quote + 2;
                    self.text = &src[i + 2..quote];
                    return T::BracketLabel;
                }
                let mut end = i + 1;
                while end < n && !matches!(code_at(src, end), 93 | 34) {
                    end += width_at(src, end);
                }
                if end == i + 1 || code_at(src, end) != 93 {
                    return T::Error;
                }
                self.at = end + 1;
                self.text = crate::js::trim(&src[i + 1..end]);
                return T::BracketLabel;
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
                    return T::Error;
                }
                return self.token(T::Numeric, i);
            }
            if c == 35 {
                let mut end = i + 1;
                while end < i + 9 && is_hex(code_at(src, end)) {
                    end += 1;
                }
                return if end >= i + 4 {
                    self.token(T::Hexcolor, end)
                } else {
                    T::Error
                };
            }
            if c == 34 {
                return match src[i + 1..].find('"') {
                    Some(quote) => self.token(T::String, i + 1 + quote + 1),
                    None => T::Error,
                };
            }
            if c == 44 {
                return self.token(T::Comma, i + 1);
            }
            if c == 58 {
                return self.token(T::Colon, i + 1);
            }
            return T::Error;
        }
    }

    fn token(&mut self, kind: T, end: usize) -> T {
        self.text = &self.src[self.at..end];
        self.at = end;
        kind
    }
}
