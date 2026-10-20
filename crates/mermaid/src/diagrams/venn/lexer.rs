//! Pele's `diagrams/venn/lexer.ts`.

use crate::{
    js,
    token::{Cursor, Token, grammar},
    util::chars::{digits_end, is_word},
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
    cur: Cursor<'s, T, ()>,
    /// The text and position of the token being read.
    text: &'s str,
    start: usize,
    line_start: bool,
    ended: bool,
}

impl<'s> VennLexer<'s> {
    pub fn new(src: &'s str) -> Self {
        Self {
            cur: Cursor::new(src, ()),
            text: "",
            start: 0,
            line_start: false,
            ended: false,
        }
    }

    fn digit_at(&self, at: usize) -> bool {
        self.cur.char_at(at).is_some_and(|c| c.is_ascii_digit())
    }

    /// The end of `rgb(1, 2, 3)` or `rgba(1, 2, 3, 0.5)` whose parenthesis
    /// opens at `at`.
    fn color(&self, at: usize, parts: usize) -> Option<usize> {
        let mut i = at + 1;
        for part in 0..parts {
            i = self.cur.space_end(i);
            let from = i;
            i = self.cur.run_end(i, |c| c.is_ascii_digit() || c == '.');
            if i == from {
                return None;
            }
            i = self.cur.space_end(i);
            let close = if part == parts - 1 { ')' } else { ',' };
            if self.cur.char_at(i) != Some(close) {
                return None;
            }
            i += 1;
        }
        Some(i)
    }

    pub fn next(&mut self, host: &mut dyn IndentHost) -> Token<'s, T> {
        let kind = self.scan(host);
        Token::new(kind, self.text, self.start)
    }

    fn scan(&mut self, host: &mut dyn IndentHost) -> T {
        let src = self.cur.src;
        loop {
            let i = self.cur.p;
            self.start = i;
            self.text = "";
            let Some(c) = self.cur.char_at(i) else {
                if self.ended {
                    return T::End;
                }
                self.ended = true;
                return T::Eof;
            };
            let w = c.len_utf8();
            // A comment runs to the end of the line. It also takes the
            // character before it along, whatever that is: a line break, or
            // the last letter of a word.
            if (self.cur.starts_with("%%", i) && self.cur.char_at(i + 2) != Some('{'))
                || (c != '}' && self.cur.starts_with("%%", i + w))
            {
                self.cur.p = self.cur.line_end(i + w);
                continue;
            }
            if self.line_start {
                if matches!(c, ' ' | '\t') {
                    let j = self.cur.run_end(i, |c| matches!(c, ' ' | '\t'));
                    self.cur.p = j;
                    if self.cur.keyword_caseless("text", j) && host.indent_mode() {
                        // The word `text` itself is dropped; the indentation
                        // stands for it.
                        self.cur.p = j + 4;
                        self.line_start = false;
                        self.text = &src[i..j];
                        return T::IndentText;
                    }
                    continue;
                }
                if !matches!(c, '\n' | '\r') {
                    host.set_indent_mode(false);
                    self.line_start = false;
                }
            }
            if matches!(c, '\n' | '\r') {
                self.line_start = true;
                let end = self.cur.run_end(i, |c| matches!(c, '\n' | '\r'));
                return self.token(T::Newline, end);
            }
            if self.cur.starts_with("%%", i) {
                self.cur.p = self.cur.line_end(i);
                continue;
            }
            if matches!(c, ' ' | '\t') {
                self.cur.p = self.cur.run_end(i, |c| matches!(c, ' ' | '\t'));
                continue;
            }

            if c.is_ascii_alphabetic() || c == '_' {
                if let Some(end) = self.cur.keyword_line("title", &['#', '\n', ';']) {
                    return self.token(T::Title, end);
                }
                for (word, kind) in KEYWORDS {
                    if self.cur.keyword_caseless(word, i) {
                        return self.token(kind, i + word.len());
                    }
                }
                if self.cur.starts_with_caseless("rgb", i) {
                    let alpha = self
                        .cur
                        .char_at(i + 3)
                        .is_some_and(|c| c.eq_ignore_ascii_case(&'a'));
                    let (open, parts, kind) = if alpha {
                        (i + 4, 4, T::Rgbacolor)
                    } else {
                        (i + 3, 3, T::Rgbcolor)
                    };
                    if self.cur.char_at(open) == Some('(')
                        && let Some(end) = self.color(open, parts)
                    {
                        return self.token(kind, end);
                    }
                }
                let end = self.cur.run_end(i + 1, |c| is_word(c) || c == '-');
                return self.token(T::Identifier, end);
            }
            return match c {
                '[' => self.bracket_label(i),
                '0'..='9' | '.' | '+' | '-' => {
                    let from = if matches!(c, '+' | '-') { i + 1 } else { i };
                    let digits = digits_end(src, from);
                    let end = if self.cur.char_at(digits) == Some('.') && self.digit_at(digits + 1)
                    {
                        digits_end(src, digits + 1)
                    } else if digits > from {
                        digits
                    } else {
                        return T::Error;
                    };
                    self.token(T::Numeric, end)
                }
                '#' => {
                    let hex = src[i + 1..]
                        .bytes()
                        .take(8)
                        .take_while(u8::is_ascii_hexdigit)
                        .count();
                    if hex >= 3 {
                        self.token(T::Hexcolor, i + 1 + hex)
                    } else {
                        T::Error
                    }
                }
                '"' => match src[i + 1..].find('"') {
                    Some(quote) => self.token(T::String, i + 1 + quote + 1),
                    None => T::Error,
                },
                ',' => self.token(T::Comma, i + 1),
                ':' => self.token(T::Colon, i + 1),
                _ => T::Error,
            };
        }
    }

    /// A label in brackets at `i`, quoted or trimmed.
    fn bracket_label(&mut self, i: usize) -> T {
        let src = self.cur.src;
        if self.cur.char_at(i + 1) == Some('"') {
            let quote = src[i + 2..].find('"').map(|q| i + 2 + q);
            let Some(quote) = quote.filter(|&q| self.cur.char_at(q + 1) == Some(']')) else {
                return T::Error;
            };
            self.cur.p = quote + 2;
            self.text = &src[i + 2..quote];
            return T::BracketLabel;
        }
        let end = self.cur.until(i + 1, |c| matches!(c, ']' | '"'));
        if end == i + 1 || self.cur.char_at(end) != Some(']') {
            return T::Error;
        }
        self.cur.p = end + 1;
        self.text = js::trim(&src[i + 1..end]);
        T::BracketLabel
    }

    fn token(&mut self, kind: T, end: usize) -> T {
        self.text = &self.cur.src[self.cur.p..end];
        self.cur.p = end;
        kind
    }
}
