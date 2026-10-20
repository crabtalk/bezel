//! Pele's `diagrams/sankey/lexer.ts`.

use crate::{
    js,
    token::{Cursor, Token, grammar},
    util::chars::is_word,
};

grammar! {
    /// The sankey lexer's token kinds.
    pub enum T {
        End = "$end",
        /// Never lexed: every text matches some rule.
        Error = "INVALID",
        /// Not a Jison token: the reference lexer would return the same empty
        /// token forever from here.
        Stuck = "STUCK",
        Sankey = "SANKEY",
        Newline = "NEWLINE",
        Comma = "COMMA",
        Dquote = "DQUOTE",
        NonEscapedText = "NON_ESCAPED_TEXT",
        EscapedText = "ESCAPED_TEXT",
        Eof = "EOF",
    }

    fn got(self, _text, previous) {
        match self {
            Self::Stuck => previous.map_or("undefined", Self::name),
            _ => self.name(),
        }
    }
}

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Csv,
    Escaped,
}

/// RFC 4180 TEXTDATA: printable ASCII except the double quote and the comma.
fn is_text_data(c: char) -> bool {
    matches!(c, ' '..='~') && !matches!(c, '"' | ',')
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
}

impl Lexer<'_> {
    /// The end of `word`, which is ASCII, at the cursor, matched as
    /// `slice(p, p + word.length).toLowerCase() === word` with no word
    /// character after it.
    fn keyword(&self, word: &str) -> Option<usize> {
        let p = self.cur.p;
        let end = p + js::units_end(&self.cur.src[p..], word.len());
        let matched = self.cur.src[p..end].to_lowercase() == word
            && !self.cur.char_at(end).is_some_and(is_word);
        matched.then_some(end)
    }

    /// The end of the run of TEXTDATA at `from`.
    fn text_end(&self, mut from: usize) -> usize {
        while self.cur.char_at(from).is_some_and(is_text_data) {
            from += 1;
        }
        from
    }

    /// The end of the run of escaped text at `from`: TEXTDATA, commas, line
    /// breaks and doubled quotes.
    fn escaped_end(&self, mut from: usize) -> usize {
        loop {
            match self.cur.char_at(from) {
                Some(c) if is_text_data(c) || matches!(c, ',' | '\r' | '\n') => from += 1,
                Some('"') if self.cur.char_at(from + 1) == Some('"') => from += 2,
                _ => return from,
            }
        }
    }

    /// Reads one token; `false` when it is empty and the cursor has not moved.
    fn step(&mut self) -> bool {
        let p = self.cur.p;
        let c = self.cur.char_at(p);
        if self.cur.state == S::Escaped {
            if c == Some('"') && self.cur.char_at(p + 1) != Some('"') {
                self.cur.pop();
                self.cur.emit(T::Dquote, p, p + 1);
                return true;
            }
            let end = self.escaped_end(p);
            self.cur.emit(T::EscapedText, p, end);
            return end > p;
        }
        if self.cur.state == S::Initial
            && matches!(c, Some('s' | 'S'))
            && let Some(end) = self
                .keyword("sankey-beta")
                .or_else(|| self.keyword("sankey"))
        {
            self.cur.push(S::Csv);
            self.cur.emit(T::Sankey, p, end);
            return true;
        }
        match c {
            Some('\n') => self.cur.emit(T::Newline, p, p + 1),
            Some('\r') if self.cur.char_at(p + 1) == Some('\n') => {
                self.cur.emit(T::Newline, p, p + 2);
            }
            Some(',') => self.cur.emit(T::Comma, p, p + 1),
            Some('"') => {
                self.cur.push(S::Escaped);
                self.cur.emit(T::Dquote, p, p + 1);
            }
            _ => {
                let end = self.text_end(p);
                self.cur.emit(T::NonEscapedText, p, end);
                return end > p;
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
    while lexer.cur.p < n {
        // A character outside the CSV alphabet matches as empty text and is
        // never consumed.
        if !lexer.step() {
            let p = lexer.cur.p;
            lexer.cur.emit(T::Stuck, p, p);
            return lexer.cur.tokens;
        }
    }
    let last = if lexer.cur.state == S::Escaped {
        T::EscapedText
    } else {
        T::Eof
    };
    lexer.cur.emit(last, n, n);
    lexer.cur.emit(T::End, n, n);
    lexer.cur.tokens
}
