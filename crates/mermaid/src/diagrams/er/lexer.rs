//! Pele's `diagrams/er/lexer.ts`.

use super::tokens::T;
use crate::diagrams::common::directions::Directions;
use crate::{
    js::Lazy,
    token::{Cursor, Token},
    util::chars::{digits_end, is_space, is_word, starts_with_caseless},
};

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Block,
    BlockBacktick,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Style,
}

static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle\s*:\s*", "iy");
static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr\s*:\s*", "iy");
static RE_ACC_DESCR_ML: Lazy = Lazy::new(r"accDescr\s*\{\s*", "iy");
static RE_DIRECTION: [(T, Lazy); 4] = [
    (T::DirectionTb, Lazy::new(r".*direction\s+TB[^\n]*", "iy")),
    (T::DirectionBt, Lazy::new(r".*direction\s+BT[^\n]*", "iy")),
    (T::DirectionRl, Lazy::new(r".*direction\s+RL[^\n]*", "iy")),
    (T::DirectionLr, Lazy::new(r".*direction\s+LR[^\n]*", "iy")),
];

/// The runs of characters the lexer's rules read as one token.
#[derive(Clone, Copy)]
enum Run {
    /// A name outside a block.
    Text,
    StyleText,
    /// The first character of an attribute word.
    AttrStart,
    AttrRest,
}

impl Run {
    fn has_ascii(self, c: char) -> bool {
        match self {
            Self::Text => is_word(c) || matches!(c, '-' | '*' | '.'),
            Self::StyleText => is_word(c) || matches!(c, '-' | '*'),
            Self::AttrStart => c.is_ascii_alphabetic() || matches!(c, '_' | '*'),
            Self::AttrRest => {
                is_word(c) || matches!(c, '-' | '*' | '.' | '[' | ']' | '(' | ')' | ',')
            }
        }
    }

    /// Outside a block every character past ASCII is in the run.
    fn has(self, c: Option<char>) -> bool {
        c.is_some_and(|c| !c.is_ascii() || self.has_ascii(c))
    }

    /// Mermaid's attribute pattern is case-insensitive, which lets the micro
    /// sign into its À-￿ range.
    fn has_in_attribute(self, c: Option<char>) -> bool {
        c.is_some_and(|c| {
            if c.is_ascii() {
                self.has_ascii(c)
            } else {
                c >= '\u{c0}' || c == '\u{b5}'
            }
        })
    }
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    directions: Directions<T>,
    /// Where the next `~` is, once searched for.
    tilde_next: Option<Option<usize>>,
    tilde_line: Option<usize>,
    tilde_last: Option<usize>,
    ws_next: Option<usize>,
}

impl<'s> Lexer<'s> {
    /// A one-character token at the cursor.
    fn emit_char(&mut self, kind: T) {
        let end = self.cur.after(self.cur.p);
        self.cur.emit_to(kind, end);
    }

    fn at(&self, c: char, q: usize) -> bool {
        self.cur.char_at(q) == Some(c)
    }

    fn lower(&self, q: usize) -> Option<char> {
        self.cur.char_at(q).map(|c| c.to_ascii_lowercase())
    }

    /// Whether the character at `q` is not a word character.
    fn boundary(&self, q: usize) -> bool {
        !self.cur.char_at(q).is_some_and(is_word)
    }

    /// The end of the run of characters that are not whitespace at `from`.
    fn word_end(&self, from: usize) -> usize {
        self.cur.src[from..]
            .find(is_space)
            .map_or(self.cur.src.len(), |at| from + at)
    }

    /// `.*direction\s+XX[^\n]*`, tried before the other rules.
    fn direction(&mut self) -> bool {
        let Some((kind, end)) = self.directions.at(self.cur.src, self.cur.p) else {
            return false;
        };
        self.cur.emit_to(kind, end);
        true
    }

    fn quoted(&mut self) {
        let p = self.cur.p;
        let Some(e) = self.cur.src[p + 1..].find('"').map(|at| p + 1 + at) else {
            self.emit_char(T::Char);
            return;
        };
        let name = e > p + 1
            && !self.cur.src[p + 1..e].contains(['%', '\\', '\r', '\n', '\u{b}', '\u{8}']);
        self.cur
            .emit_to(if name { T::EntityName } else { T::Word }, e + 1);
    }

    fn number(&mut self, c: char) {
        let p = self.cur.p;
        let d = self.cur.char_at(p + 1);
        if d == Some('+') && matches!(c, '0' | '1') {
            self.cur.emit_to(
                if c == '1' {
                    T::OneOrMore
                } else {
                    T::ZeroOrMore
                },
                p + 2,
            );
            return;
        }
        let q = digits_end(self.cur.src, p + 1);
        if self.at('.', q) && self.cur.char_at(q + 1).is_some_and(|c| c.is_ascii_digit()) {
            let e = digits_end(self.cur.src, q + 2);
            self.cur.emit_to(T::DecimalNum, e);
            return;
        }
        if c == '1' {
            match d {
                Some(d) if is_space(d) => {
                    let r = self.cur.space_end(self.cur.after(p + 1));
                    let f = self.cur.char_at(r);
                    if f.is_some_and(is_word) || matches!(f, Some('"' | '\'')) {
                        self.cur.emit_to(T::OnlyOne, p + 1);
                        return;
                    }
                }
                Some('-' | '.') => {
                    if matches!(self.cur.char_at(p + 2), Some('-' | '.')) {
                        self.cur.emit_to(T::OnlyOne, p + 1);
                        return;
                    }
                }
                _ => {}
            }
            if self.boundary(p + 1) {
                self.cur.emit_to(T::EntityOne, p + 1);
                return;
            }
        }
        self.cur.emit_to(T::Num, q);
    }

    /// The keyword or relationship symbol at the cursor, whose first
    /// character is `lc` in lower case.
    fn keyword(&mut self, lc: char) -> bool {
        let p = self.cur.p;
        let d = self.cur.char_at(p + 1);
        let kw = |word: &str| self.cur.keyword_caseless(word, self.cur.p);
        let (kind, len) = match lc {
            'e' if kw("erdiagram") => (T::ErDiagram, 9),
            'e' if kw("end") => (T::EndKw, self.cur.space_end(p + 3) - p),
            's' if kw("style") => {
                self.cur.state = S::Style;
                (T::Style, 5)
            }
            's' if kw("subgraph") => (T::Subgraph, 8),
            'c' if kw("classdef") => {
                self.cur.state = S::Style;
                (T::Classdef, 8)
            }
            'c' if kw("class") => (T::Class, 5),
            'o' if kw("one or zero") => (T::ZeroOrOne, 11),
            'o' if kw("one or more") || kw("one or many") => (T::OneOrMore, 11),
            'o' if kw("one") => (T::OnlyOne, 3),
            'o' if kw("only one") => (T::OnlyOne, 8),
            'o' if d == Some('|') => (T::ZeroOrOne, 2),
            'o' if d == Some('{') => (T::ZeroOrMore, 2),
            'o' if kw("optionally to") => (T::NonIdentifying, 13),
            'z' if kw("zero or one") => (T::ZeroOrOne, 11),
            'z' if kw("zero or more") || kw("zero or many") => (T::ZeroOrMore, 12),
            'm' if starts_with_caseless(self.cur.src, p, "many(0)") => (T::ZeroOrMore, 7),
            'm' if starts_with_caseless(self.cur.src, p, "many(1)") => (T::OneOrMore, 7),
            'm' if kw("many") => (T::ZeroOrMore, 4),
            't' if kw("to") => (T::Identifying, 2),
            'u' if matches!(d, Some('.' | '-' | '|')) => (T::MdParent, 1),
            '|' if self.lower(p + 1) == Some('o') && self.boundary(p + 2) => (T::ZeroOrOne, 2),
            '|' if d == Some('|') => (T::OnlyOne, 2),
            '|' if d == Some('{') => (T::OneOrMore, 2),
            '}' if self.lower(p + 1) == Some('o') && self.boundary(p + 2) => (T::ZeroOrMore, 2),
            '}' if d == Some('|') => (T::OneOrMore, 2),
            '.' if matches!(d, Some('.' | '-')) => (T::NonIdentifying, 2),
            '-' if d == Some('-') => (T::Identifying, 2),
            '-' if d == Some('.') => (T::NonIdentifying, 2),
            _ => return false,
        };
        self.cur.emit_to(kind, p + len);
        true
    }

    fn initial(&mut self, c: char) {
        let p = self.cur.p;
        if matches!(c, 'a' | 'A') && starts_with_caseless(self.cur.src, p, "acc") {
            if let Some(e) = RE_ACC_TITLE.match_end(self.cur.src, p) {
                self.cur.state = S::AccTitle;
                self.cur.emit_to(T::AccTitle, e);
                return;
            }
            if let Some(e) = RE_ACC_DESCR.match_end(self.cur.src, p) {
                self.cur.state = S::AccDescr;
                self.cur.emit_to(T::AccDescr, e);
                return;
            }
            if let Some(e) = RE_ACC_DESCR_ML.match_end(self.cur.src, p) {
                self.cur.state = S::AccDescrMultiline;
                self.cur.p = e;
                return;
            }
        }
        if self.direction() {
            return;
        }

        match c {
            ' ' | '\t' | '\r' => {
                let mut e = p + 1;
                while matches!(self.cur.char_at(e), Some(' ' | '\t' | '\r')) {
                    e += 1;
                }
                self.cur.p = e;
            }
            '\n' => {
                let e = self.newlines_end(p);
                self.cur.emit_to(T::Newline, e);
            }
            '"' => self.quoted(),
            '{' => {
                self.cur.state = S::Block;
                self.cur.emit_to(T::BlockStart, p + 1);
            }
            '#' => self.cur.emit_to(T::Brkt, p + 1),
            ',' => self.cur.emit_to(T::Comma, p + 1),
            ':' => {
                if self.at(':', p + 1) && self.at(':', p + 2) {
                    self.cur.emit_to(T::StyleSeparator, p + 3);
                } else {
                    self.cur.emit_to(T::Colon, p + 1);
                }
            }
            '[' => self.cur.emit_to(T::Sqs, p + 1),
            ']' => self.cur.emit_to(T::Sqe, p + 1),
            _ if c.is_ascii_digit() => self.number(c),
            _ if self.keyword(c.to_ascii_lowercase()) => {}
            _ if Run::Text.has(Some(c)) => {
                let mut e = self.cur.after(p);
                while Run::Text.has(self.cur.char_at(e)) {
                    e = self.cur.after(e);
                }
                self.cur.emit_to(T::UnicodeText, e);
            }
            _ => self.emit_char(T::Char),
        }
    }

    /// The end of the run of line feeds at `from`.
    fn newlines_end(&self, from: usize) -> usize {
        let mut e = from + 1;
        while self.at('\n', e) {
            e += 1;
        }
        e
    }

    /// Mermaid's generic type rule: a run of non-space characters holding a
    /// `~`, through the last `~` on the line.
    fn generic(&mut self) -> bool {
        let p = self.cur.p;
        if self.tilde_next.flatten().is_none_or(|next| next < p) {
            if self.tilde_next == Some(None) {
                return false;
            }
            self.tilde_next = Some(self.cur.src[p..].find('~').map(|at| p + at));
        }
        let Some(Some(tilde_next)) = self.tilde_next else {
            return false;
        };
        if self.ws_next.is_none_or(|ws| ws < p) {
            self.ws_next = Some(self.word_end(p));
        }
        if self.ws_next.is_some_and(|ws| tilde_next >= ws) {
            return false;
        }
        if self.tilde_line.is_none_or(|line| line < p) {
            let line = self.cur.terminator_end(p);
            self.tilde_line = Some(line);
            self.tilde_last = self.cur.src[..line].rfind('~');
        }
        let Some(tilde_last) = self.tilde_last.filter(|&last| last > tilde_next) else {
            return false;
        };
        let e = self.word_end(tilde_last + 1);
        self.cur.emit_to(T::AttributeWord, e);
        true
    }

    fn block(&mut self, c: char) {
        let p = self.cur.p;
        if is_space(c) {
            self.cur.p = self.cur.space_end(p);
            return;
        }
        if matches!(c.to_ascii_lowercase(), 'p' | 'f' | 'u')
            && self.lower(p + 1) == Some('k')
            && self.boundary(p + 2)
        {
            self.cur.emit_to(T::AttributeKey, p + 2);
            return;
        }
        if self.generic() {
            return;
        }
        if Run::AttrStart.has_in_attribute(Some(c)) {
            let mut e = self.cur.after(p);
            while Run::AttrRest.has_in_attribute(self.cur.char_at(e)) {
                e = self.cur.after(e);
            }
            self.cur.emit_to(T::AttributeWord, e);
            return;
        }
        match c {
            '`' => {
                self.cur.state = S::BlockBacktick;
                self.cur.p += 1;
            }
            '"' if self.cur.src[p + 1..].contains('"') => {
                let e = self.cur.find('"', p + 1);
                self.cur.emit_to(T::Comment, e + 1);
            }
            '}' => {
                self.cur.state = S::Initial;
                self.cur.emit_to(T::BlockStop, p + 1);
            }
            _ => self.emit_char(T::Char),
        }
    }

    /// Answers whether lexing goes on: false after an unrecognised character.
    fn style(&mut self, c: char) -> bool {
        let p = self.cur.p;
        match c {
            '\n' => {
                let e = self.newlines_end(p);
                self.cur.state = S::Initial;
                self.cur.emit_to(T::Newline, e);
            }
            _ if is_space(c) => self.cur.p = self.cur.space_end(p),
            ':' => self.cur.emit_to(T::Colon, p + 1),
            ',' => self.cur.emit_to(T::Comma, p + 1),
            '#' => self.cur.emit_to(T::Brkt, p + 1),
            ';' => self.cur.emit_to(T::Semi, p + 1),
            _ if Run::StyleText.has(Some(c)) => {
                let mut e = self.cur.after(p);
                while Run::StyleText.has(self.cur.char_at(e)) {
                    e = self.cur.after(e);
                }
                self.cur.emit_to(T::StyleText, e);
            }
            _ => {
                self.cur.tokens.push(Token::new(T::Error, "", p));
                return false;
            }
        }
        true
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lx = Lexer {
        cur: Cursor::new(src, S::Initial),
        directions: Directions::caseless(src, &RE_DIRECTION),
        tilde_next: None,
        tilde_line: None,
        tilde_last: None,
        ws_next: None,
    };

    while let Some(c) = lx.cur.char_at(lx.cur.p) {
        let p = lx.cur.p;
        match lx.cur.state {
            S::Initial => lx.initial(c),
            S::Block => lx.block(c),
            S::BlockBacktick => {
                if c == '`' {
                    lx.cur.state = S::Block;
                    lx.cur.p += 1;
                } else {
                    let e = lx.cur.find('`', p);
                    lx.cur.emit_to(T::AttributeWord, e);
                }
            }
            S::AccTitle | S::AccDescr => {
                let e = lx.cur.line_end(p);
                let kind = if lx.cur.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                lx.cur.emit_to(kind, e);
                lx.cur.state = S::Initial;
            }
            S::AccDescrMultiline => {
                if c == '}' {
                    lx.cur.state = S::Initial;
                    lx.cur.p += 1;
                } else {
                    let e = lx.cur.find('}', p);
                    lx.cur.emit_to(T::AccDescrMultilineValue, e);
                }
            }
            S::Style => {
                if !lx.style(c) {
                    return lx.cur.tokens;
                }
            }
        }
    }

    // At the end of input Jison's INITIAL state yields EOF, the title and
    // description states yield an empty value, and the block and style
    // states yield nothing.
    let last = match lx.cur.state {
        S::Initial => Some(T::Eof),
        S::AccTitle => Some(T::AccTitleValue),
        S::AccDescr => Some(T::AccDescrValue),
        S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
        _ => None,
    };
    let mut tokens = lx.cur.tokens;
    if let Some(last) = last {
        tokens.push(Token::new(last, "", n));
    }
    tokens.push(Token::new(T::End, "", n));
    tokens
}
