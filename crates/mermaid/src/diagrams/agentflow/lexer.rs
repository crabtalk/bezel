//! Pele's `diagrams/agentflow/lexer.ts`. Positions are byte offsets; the
//! columns in `locs` count UTF-16 units, as Jison does.

use std::borrow::Cow;

use super::types::Loc;
use crate::{
    diagrams::flowchart::{
        lexer::{Core, Lex, S},
        tokens::T,
    },
    js,
    token::Token,
    util::chars::{is_blank, is_space},
};

pub struct Tokens<'s> {
    pub tokens: Vec<Token<'s, T>>,
    /// Each token's place, counted the way Jison counts it.
    pub locs: Vec<Loc>,
}

struct Lexer<'s> {
    core: Core<'s>,
    locs: Vec<Loc>,
    line: usize,
    col: usize,
    /// The last run of spaces and tabs measured, from and to.
    blank: Option<(usize, usize)>,
    /// Where whitespace before a link was last measured with no link after.
    no_link: Option<usize>,
    /// Where whitespace before an arrow was last measured with no arrow
    /// after.
    no_arrow: Option<usize>,
}

impl<'s> Lex<'s> for Lexer<'s> {
    const COMMENTS: bool = true;

    fn core(&self) -> &Core<'s> {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core<'s> {
        &mut self.core
    }

    fn emit(&mut self, kind: T, end: usize) {
        let text = &self.src()[self.p()..end];
        self.record(kind, text.into(), end, |l| l.skip(end));
    }

    fn emit_flat(&mut self, kind: T, end: usize) {
        let text = &self.src()[self.p()..end];
        self.record(kind, text.into(), end, |l| l.col += js::len(text));
    }

    /// Jison counts `\r\n` in such a run as two line breaks.
    fn emit_chars(&mut self, kind: T, end: usize) {
        let text = &self.src()[self.p()..end];
        self.record(kind, text.into(), end, |l| {
            for c in text.chars() {
                if c == '\n' || c == '\r' {
                    l.line += 1;
                    l.col = 0;
                } else {
                    l.col += c.len_utf16();
                }
            }
        });
    }

    fn emit_text(&mut self, kind: T, text: Cow<'s, str>, end: usize) {
        self.record(kind, text, end, |l| l.skip(end));
    }

    /// Jison counts `\r\n`, `\r` and `\n` as one line break each, and after a
    /// break its column stops at the first U+2028 or U+2029.
    fn skip(&mut self, end: usize) {
        let text = &self.src()[self.p()..end];
        let bytes = text.as_bytes();
        let mut last_break = None;
        let mut k = 0;
        while k < bytes.len() {
            match bytes[k] {
                b'\n' => {
                    self.line += 1;
                    last_break = Some(k);
                }
                b'\r' => {
                    if bytes.get(k + 1) == Some(&b'\n') {
                        k += 1;
                    }
                    self.line += 1;
                    last_break = Some(k);
                }
                _ => {}
            }
            k += 1;
        }
        match last_break {
            None => self.col += js::len(text),
            Some(at) => {
                let rest = &text[at + 1..];
                let stop = rest.find(['\u{2028}', '\u{2029}']).unwrap_or(rest.len());
                self.col = js::len(&rest[..stop]);
            }
        }
        self.core.cur.p = end;
    }

    fn close(&mut self, kind: T, at: usize) {
        self.core.cur.tokens.push(Token::new(kind, "", at));
        self.locs.push(Loc::at(self.line, self.col));
    }

    /// `%%` not followed by `{`, through the end of the line.
    fn comment_end(&self, q: usize) -> Option<usize> {
        if self.at(q) != Some('%') || self.at(q + 1) != Some('%') || self.at(q + 2) == Some('{') {
            return None;
        }
        Some(self.core.cur.line_end(q + 2))
    }

    fn own(&mut self) -> bool {
        if self.core.cur.state == S::EdgeText {
            self.edge_text()
        } else {
            self.initial()
        }
    }
}

impl<'s> Lexer<'s> {
    /// Records a token from the cursor to `end`, whose place the line and
    /// column reach by `step`.
    fn record(&mut self, kind: T, text: Cow<'s, str>, end: usize, step: impl FnOnce(&mut Self)) {
        let (line, col) = (self.line, self.col);
        let p = self.p();
        self.core.cur.tokens.push(Token::new(kind, text, p));
        step(self);
        self.core.cur.p = end;
        self.locs.push(Loc {
            last_line: self.line,
            last_column: self.col,
            ..Loc::at(line, col)
        });
    }

    /// The end of the run of spaces and tabs at `q`.
    fn blank_end(&mut self, q: usize) -> usize {
        if let Some((from, to)) = self.blank
            && (from..=to).contains(&q)
        {
            return to;
        }
        let end = self.blanks(q);
        self.blank = Some((q, end));
        end
    }

    fn blanks(&self, from: usize) -> usize {
        self.core.cur.until(from, |c| !is_blank(c))
    }

    /// The end of the run of `c` at `from`.
    fn run_of(&self, from: usize, c: char) -> usize {
        self.core.cur.until(from, |d| d != c)
    }

    /// The end of `\s*--+[x>]\s*` at `q`.
    fn arrow_end(&mut self, q: usize) -> Option<usize> {
        let at = self.core.space_end(q);
        if Some(at) == self.no_arrow || self.at(at) != Some('-') || self.at(at + 1) != Some('-') {
            return None;
        }
        let head = self.run_of(at + 2, '-');
        if matches!(self.at(head), Some('x' | '>')) {
            return Some(self.core.space_end(head + 1));
        }
        self.no_arrow = Some(at);
        None
    }

    fn link(&mut self) -> bool {
        let at = self.core.space_end(self.p());
        if Some(at) == self.no_link || self.at(at) != Some('-') {
            return false;
        }
        match self.at(at + 1) {
            Some('-') => {
                let head = self.run_of(at + 2, '-');
                if matches!(self.at(head), Some('x' | '>')) {
                    let end = self.core.space_end(head + 1);
                    self.emit(T::Link, end);
                } else {
                    self.push(S::EdgeText);
                    let end = self.core.space_end(at + 2);
                    self.emit(T::StartLink, end);
                }
                return true;
            }
            Some('.') => {
                let tail = self.run_of(at + 2, '.');
                if self.at(tail) == Some('-') {
                    let end = self.core.space_end(tail + 1);
                    self.emit(T::Link, end);
                    return true;
                }
            }
            _ => {}
        }
        self.no_link = Some(at);
        false
    }

    fn agentflow_keyword(&mut self) -> bool {
        let p = self.p();
        if self.word("agentflow-beta") {
            self.graph(p + 14);
        } else if self.word("connector") {
            self.emit_flat(T::Connector, p + 9);
        } else if self.word("flow") {
            self.emit_flat(T::Flow, p + 4);
        } else if self.word("global") {
            let end = self.blanks(p + 6);
            self.emit_flat(T::Global, end);
        } else if self.word("end") {
            let end = self.blanks(p + 3);
            self.emit_flat(T::EndKw, end);
        } else {
            return false;
        }
        true
    }

    fn initial(&mut self) -> bool {
        if self.keyword()
            || self.agentflow_keyword()
            || self.direction()
            || self.link_id()
            || self.number()
        {
            return true;
        }
        let p = self.p();
        match self.at(p) {
            Some('<') => {
                self.emit_flat(T::Tagstart, p + 1);
                return true;
            }
            Some('-') => {
                if !(self.link() || self.node_string()) {
                    self.emit_flat(T::Minus, p + 1);
                }
                return true;
            }
            Some('%') => {
                let Some(end) = self.comment_end(p) else {
                    return self.node_string();
                };
                self.emit(T::Comment, end);
                return true;
            }
            Some(c) if is_space(c) => {
                if self.link() {
                    return true;
                }
                if is_blank(c) {
                    let from = self.blank_end(p);
                    if let Some(end) = self.comment_end(from) {
                        self.emit(T::Comment, end);
                        return true;
                    }
                }
                let mut end = p;
                loop {
                    match self.at(end) {
                        Some('\n') => end += 1,
                        Some('\r') if self.at(end + 1) == Some('\n') => end += 2,
                        _ => break,
                    }
                }
                if end > p {
                    self.emit(T::Newline, end);
                } else {
                    self.emit(T::Space, p + c.len_utf8());
                }
                return true;
            }
            _ => {}
        }
        if let Some(found) = self.symbol() {
            return found;
        }
        self.text_char()
    }

    /// The label of a link, up to the arrow that closes it.
    fn edge_text(&mut self) -> bool {
        let p = self.p();
        let c = self.at(p);
        if c == Some('"') {
            self.quote();
            return true;
        }
        if let Some(end) = self.arrow_end(p) {
            self.pop();
            self.emit(T::Link, end);
            return true;
        }
        if c == Some('-') && self.at(p + 1) == Some('-') {
            return false;
        }
        let mut q = p + c.map_or(1, char::len_utf8);
        while let Some(ch) = self.at(q) {
            if ch == '"'
                || (ch == '-' && self.at(q + 1) == Some('-'))
                || (is_space(ch) && self.arrow_end(q).is_some())
            {
                break;
            }
            q += ch.len_utf8();
        }
        self.emit_chars(T::EdgeText, q);
        true
    }
}

pub fn tokenize(src: &str) -> Tokens<'_> {
    let mut lexer = Lexer {
        core: Core::new(src),
        locs: Vec::new(),
        line: 1,
        col: 0,
        blank: None,
        no_link: None,
        no_arrow: None,
    };
    lexer.lex();
    Tokens {
        tokens: lexer.core.cur.tokens,
        locs: lexer.locs,
    }
}
