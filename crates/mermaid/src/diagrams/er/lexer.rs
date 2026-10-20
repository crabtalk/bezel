//! Pele's `diagrams/er/lexer.ts`.

use super::tokens::T;
use crate::{
    js::Lazy,
    token::Token,
    util::chars::{NONE, code_at, is_digit, is_space, is_word, width_at},
};

const INITIAL: u8 = 0;
const BLOCK: u8 = 1;
const BLOCK_BQ: u8 = 2;
const ACC_TITLE_STATE: u8 = 3;
const ACC_DESCR_STATE: u8 = 4;
const ACC_DESCR_MULTILINE: u8 = 5;
const STYLE_STATE: u8 = 6;
const TEXT: u8 = 1;
const STYLE_CLASS: u8 = 2;
const ATTR_START: u8 = 4;
const ATTR_REST: u8 = 8;
const CLASS_TABLE: [u8; 128] = class_table();

static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle\s*:\s*", "iy");
static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr\s*:\s*", "iy");
static RE_ACC_DESCR_ML: Lazy = Lazy::new(r"accDescr\s*\{\s*", "iy");
static RE_DIRECTION: [(T, Lazy); 4] = [
    (T::DirectionTb, Lazy::new(r".*direction\s+TB[^\n]*", "iy")),
    (T::DirectionBt, Lazy::new(r".*direction\s+BT[^\n]*", "iy")),
    (T::DirectionRl, Lazy::new(r".*direction\s+RL[^\n]*", "iy")),
    (T::DirectionLr, Lazy::new(r".*direction\s+LR[^\n]*", "iy")),
];
static RE_DIR_WORD: Lazy = Lazy::new(r"direction", "gi");

const fn class_table() -> [u8; 128] {
    let mut table = [0u8; 128];
    let mut c = 0;
    while c < 128 {
        let word = (c >= 48 && c <= 57) || (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c == 95;
        let letter = word && (c < 48 || c > 57);
        if word {
            table[c] |= TEXT | STYLE_CLASS | ATTR_REST;
        }
        if letter {
            table[c] |= ATTR_START;
        }
        c += 1;
    }
    table[b'-' as usize] |= TEXT | STYLE_CLASS | ATTR_REST;
    table[b'*' as usize] |= TEXT | STYLE_CLASS | ATTR_REST;
    table[46] |= TEXT | ATTR_REST;
    table[42] |= ATTR_START;
    let rest = b"[](),";
    let mut i = 0;
    while i < rest.len() {
        table[rest[i] as usize] |= ATTR_REST;
        i += 1;
    }
    table
}

fn class_has(c: u32, kind: u8) -> bool {
    c < 128 && CLASS_TABLE[c as usize] & kind != 0
}

/// A character of a name or style run outside a block.
fn is_run(c: u32, kind: u8) -> bool {
    (c >= 128 && c != NONE) || class_has(c, kind)
}

/// Mermaid's attribute pattern is case-insensitive, which lets the micro
/// sign into its À-￿ range.
fn is_attr(c: u32, kind: u8) -> bool {
    if c < 128 {
        CLASS_TABLE[c as usize] & kind != 0
    } else {
        c != NONE && (c >= 0xc0 || c == 0xb5)
    }
}

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    tokens: Vec<Token<'s, T>>,
    state: u8,
    p: usize,
    dir_next: Option<usize>,
    dir_known: bool,
    dir_failed: usize,
    line_end: Option<usize>,
    tilde_next: Option<usize>,
    tilde_known: bool,
    tilde_line: Option<usize>,
    tilde_last: Option<usize>,
    ws_next: Option<usize>,
}

impl<'s> Lexer<'s> {
    fn c(&self, q: usize) -> u32 {
        code_at(self.src, q)
    }

    fn emit(&mut self, kind: T, s: usize, e: usize) {
        self.tokens.push(Token::new(kind, &self.src[s..e], s));
        self.p = e;
    }

    /// A one-character token at `p`.
    fn emit_char(&mut self, kind: T) {
        let p = self.p;
        self.emit(kind, p, p + width_at(self.src, p));
    }

    fn at(&self, re: &Lazy, q: usize) -> Option<usize> {
        re.match_end(self.src, q)
    }

    /// Matches a lower-case keyword at `p`, ignoring case.
    fn kw(&self, word: &str) -> bool {
        let bytes = self.src.as_bytes();
        for (i, k) in word.bytes().enumerate() {
            let Some(&c) = bytes.get(self.p + i) else {
                return false;
            };
            if c != k && (!k.is_ascii_lowercase() || (c | 32) != k) {
                return false;
            }
        }
        true
    }

    fn kwb(&self, word: &str) -> bool {
        self.kw(word) && !is_word(self.c(self.p + word.len()))
    }

    fn lower(&self, q: usize) -> u32 {
        let c = self.c(q);
        if (65..=90).contains(&c) { c + 32 } else { c }
    }

    fn next_line_end(&self, q: usize) -> usize {
        self.src[q..]
            .find(['\n', '\r', '\u{2028}', '\u{2029}'])
            .map_or(self.n, |at| q + at)
    }

    fn digits(&self, mut q: usize) -> usize {
        while is_digit(self.c(q)) {
            q += 1;
        }
        q
    }

    fn skip_spaces(&self, mut q: usize) -> usize {
        while is_space(self.c(q)) {
            q += width_at(self.src, q);
        }
        q
    }

    /// A direction statement swallows its whole line, from wherever the
    /// lexer stands on it.
    fn direction(&mut self) -> bool {
        let p = self.p;
        if self.dir_known && self.dir_next.is_none() {
            return false;
        }
        if !self.dir_known || self.dir_next.is_some_and(|next| next < p) {
            self.dir_known = true;
            self.dir_next = RE_DIR_WORD.exec_at(self.src, p).map(|m| m.start());
            if self.dir_next.is_none() {
                return false;
            }
        }
        if p < self.dir_failed {
            return false;
        }
        if self.line_end.is_none_or(|end| end < p) {
            self.line_end = Some(self.next_line_end(p));
        }
        let line_end = self.line_end.unwrap_or(self.n);
        if self.dir_next.is_some_and(|next| next >= line_end) {
            return false;
        }
        for (kind, re) in &RE_DIRECTION {
            if let Some(e) = self.at(re, p) {
                self.emit(*kind, p, e);
                return true;
            }
        }
        self.dir_failed = line_end;
        false
    }

    fn quoted(&mut self) {
        let p = self.p;
        let Some(e) = self.src[p + 1..].find('"').map(|at| p + 1 + at) else {
            self.emit_char(T::Char);
            return;
        };
        let name = e > p + 1
            && !self.src.as_bytes()[p + 1..e]
                .iter()
                .any(|&c| matches!(c, 37 | 92 | 13 | 10 | 11 | 8));
        self.emit(if name { T::EntityName } else { T::Word }, p, e + 1);
    }

    fn number(&mut self, c: u32) {
        let p = self.p;
        let d = self.c(p + 1);
        if d == 43 && (c == 49 || c == 48) {
            self.emit(if c == 49 { T::OneOrMore } else { T::ZeroOrMore }, p, p + 2);
            return;
        }
        let q = self.digits(p + 1);
        if self.c(q) == 46 && is_digit(self.c(q + 1)) {
            let e = self.digits(q + 2);
            self.emit(T::DecimalNum, p, e);
            return;
        }
        if c == 49 {
            if is_space(d) {
                let r = self.skip_spaces(p + 1 + width_at(self.src, p + 1));
                let f = self.c(r);
                if is_word(f) || f == 34 || f == 39 {
                    self.emit(T::OnlyOne, p, p + 1);
                    return;
                }
            } else if d == 45 || d == 46 {
                let f = self.c(p + 2);
                if f == 45 || f == 46 {
                    self.emit(T::OnlyOne, p, p + 1);
                    return;
                }
            }
            if !is_word(d) {
                self.emit(T::EntityOne, p, p + 1);
                return;
            }
        }
        self.emit(T::Num, p, q);
    }

    fn keyword(&mut self, lc: u32) -> bool {
        let p = self.p;
        let d = self.c(p + 1);
        match lc {
            101 => {
                if self.kwb("erdiagram") {
                    self.emit(T::ErDiagram, p, p + 9);
                } else if self.kwb("end") {
                    let e = self.skip_spaces(p + 3);
                    self.emit(T::EndKw, p, e);
                } else {
                    return false;
                }
                true
            }
            115 => {
                if self.kwb("style") {
                    self.state = STYLE_STATE;
                    self.emit(T::Style, p, p + 5);
                } else if self.kwb("subgraph") {
                    self.emit(T::Subgraph, p, p + 8);
                } else {
                    return false;
                }
                true
            }
            99 => {
                if self.kwb("classdef") {
                    self.state = STYLE_STATE;
                    self.emit(T::Classdef, p, p + 8);
                } else if self.kwb("class") {
                    self.emit(T::Class, p, p + 5);
                } else {
                    return false;
                }
                true
            }
            111 => {
                if self.kwb("one or zero") {
                    self.emit(T::ZeroOrOne, p, p + 11);
                } else if self.kwb("one or more") || self.kwb("one or many") {
                    self.emit(T::OneOrMore, p, p + 11);
                } else if self.kwb("one") {
                    self.emit(T::OnlyOne, p, p + 3);
                } else if self.kwb("only one") {
                    self.emit(T::OnlyOne, p, p + 8);
                } else if d == 124 {
                    self.emit(T::ZeroOrOne, p, p + 2);
                } else if d == 123 {
                    self.emit(T::ZeroOrMore, p, p + 2);
                } else if self.kwb("optionally to") {
                    self.emit(T::NonIdentifying, p, p + 13);
                } else {
                    return false;
                }
                true
            }
            122 => {
                if self.kwb("zero or one") {
                    self.emit(T::ZeroOrOne, p, p + 11);
                } else if self.kwb("zero or more") || self.kwb("zero or many") {
                    self.emit(T::ZeroOrMore, p, p + 12);
                } else {
                    return false;
                }
                true
            }
            109 => {
                if self.kw("many(0)") {
                    self.emit(T::ZeroOrMore, p, p + 7);
                } else if self.kw("many(1)") {
                    self.emit(T::OneOrMore, p, p + 7);
                } else if self.kwb("many") {
                    self.emit(T::ZeroOrMore, p, p + 4);
                } else {
                    return false;
                }
                true
            }
            116 => {
                if !self.kwb("to") {
                    return false;
                }
                self.emit(T::Identifying, p, p + 2);
                true
            }
            117 => {
                if d != 46 && d != 45 && d != 124 {
                    return false;
                }
                self.emit(T::MdParent, p, p + 1);
                true
            }
            124 => {
                if self.lower(p + 1) == 111 && !is_word(self.c(p + 2)) {
                    self.emit(T::ZeroOrOne, p, p + 2);
                } else if d == 124 {
                    self.emit(T::OnlyOne, p, p + 2);
                } else if d == 123 {
                    self.emit(T::OneOrMore, p, p + 2);
                } else {
                    return false;
                }
                true
            }
            125 => {
                if self.lower(p + 1) == 111 && !is_word(self.c(p + 2)) {
                    self.emit(T::ZeroOrMore, p, p + 2);
                } else if d == 124 {
                    self.emit(T::OneOrMore, p, p + 2);
                } else {
                    return false;
                }
                true
            }
            46 => {
                if d != 46 && d != 45 {
                    return false;
                }
                self.emit(T::NonIdentifying, p, p + 2);
                true
            }
            45 => {
                if d == 45 {
                    self.emit(T::Identifying, p, p + 2);
                } else if d == 46 {
                    self.emit(T::NonIdentifying, p, p + 2);
                } else {
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    fn initial(&mut self, c: u32) {
        let p = self.p;
        if (c == 97 || c == 65) && self.kw("acc") {
            if let Some(e) = self.at(&RE_ACC_TITLE, p) {
                self.state = ACC_TITLE_STATE;
                self.emit(T::AccTitle, p, e);
                return;
            }
            if let Some(e) = self.at(&RE_ACC_DESCR, p) {
                self.state = ACC_DESCR_STATE;
                self.emit(T::AccDescr, p, e);
                return;
            }
            if let Some(e) = self.at(&RE_ACC_DESCR_ML, p) {
                self.state = ACC_DESCR_MULTILINE;
                self.p = e;
                return;
            }
        }
        if self.direction() {
            return;
        }

        match c {
            32 | 9 | 13 => {
                let mut e = p + 1;
                while matches!(self.c(e), 32 | 9 | 13) {
                    e += 1;
                }
                self.p = e;
                return;
            }
            10 => {
                let mut e = p + 1;
                while self.c(e) == 10 {
                    e += 1;
                }
                self.emit(T::Newline, p, e);
                return;
            }
            34 => {
                self.quoted();
                return;
            }
            123 => {
                self.state = BLOCK;
                self.emit(T::BlockStart, p, p + 1);
                return;
            }
            35 => {
                self.emit(T::Brkt, p, p + 1);
                return;
            }
            44 => {
                self.emit(T::Comma, p, p + 1);
                return;
            }
            58 => {
                if self.c(p + 1) == 58 && self.c(p + 2) == 58 {
                    self.emit(T::StyleSeparator, p, p + 3);
                } else {
                    self.emit(T::Colon, p, p + 1);
                }
                return;
            }
            91 => {
                self.emit(T::Sqs, p, p + 1);
                return;
            }
            93 => {
                self.emit(T::Sqe, p, p + 1);
                return;
            }
            _ => {}
        }
        if is_digit(c) {
            self.number(c);
            return;
        }
        if self.keyword(if (65..=90).contains(&c) { c + 32 } else { c }) {
            return;
        }
        if is_run(c, TEXT) {
            let mut e = p + width_at(self.src, p);
            while is_run(self.c(e), TEXT) {
                e += width_at(self.src, e);
            }
            self.emit(T::UnicodeText, p, e);
            return;
        }
        self.emit_char(T::Char);
    }

    /// Mermaid's generic type rule: a run of non-space characters holding a
    /// `~`, through the last `~` on the line.
    fn generic(&mut self) -> bool {
        let p = self.p;
        let n = self.n;
        if self.tilde_known && self.tilde_next.is_none() {
            return false;
        }
        if !self.tilde_known || self.tilde_next.is_some_and(|next| next < p) {
            self.tilde_known = true;
            self.tilde_next = self.src[p..].find('~').map(|at| p + at);
            if self.tilde_next.is_none() {
                return false;
            }
        }
        let tilde_next = self.tilde_next.unwrap_or(n);
        if self.ws_next.is_none_or(|ws| ws < p) {
            let mut q = p;
            while q < n && !is_space(self.c(q)) {
                q += width_at(self.src, q);
            }
            self.ws_next = Some(q);
        }
        if self.ws_next.is_some_and(|ws| tilde_next >= ws) {
            return false;
        }
        if self.tilde_line.is_none_or(|line| line < p) {
            let line = self.next_line_end(p);
            self.tilde_line = Some(line);
            self.tilde_last = self.src[..line].rfind('~');
        }
        let Some(tilde_last) = self.tilde_last.filter(|&last| last > tilde_next) else {
            return false;
        };
        let mut e = tilde_last + 1;
        while e < n && !is_space(self.c(e)) {
            e += width_at(self.src, e);
        }
        self.emit(T::AttributeWord, p, e);
        true
    }

    fn block(&mut self, c: u32) {
        let p = self.p;
        if is_space(c) {
            self.p = self.skip_spaces(p);
            return;
        }
        let lc = c | 32;
        if (lc == 112 || lc == 102 || lc == 117)
            && (self.c(p + 1) | 32) == 107
            && !is_word(self.c(p + 2))
        {
            self.emit(T::AttributeKey, p, p + 2);
            return;
        }
        if self.generic() {
            return;
        }
        if is_attr(c, ATTR_START) {
            let mut e = p + width_at(self.src, p);
            while e < self.n && is_attr(self.c(e), ATTR_REST) {
                e += width_at(self.src, e);
            }
            self.emit(T::AttributeWord, p, e);
            return;
        }
        if c == 96 {
            self.state = BLOCK_BQ;
            self.p += 1;
            return;
        }
        if c == 34 {
            if let Some(e) = self.src[p + 1..].find('"') {
                self.emit(T::Comment, p, p + 1 + e + 1);
                return;
            }
        } else if c == 125 {
            self.state = INITIAL;
            self.emit(T::BlockStop, p, p + 1);
            return;
        }
        self.emit_char(T::Char);
    }

    /// Answers whether lexing goes on: false after an unrecognised character.
    fn style(&mut self, c: u32) -> bool {
        let p = self.p;
        if c == 10 {
            let mut e = p + 1;
            while self.c(e) == 10 {
                e += 1;
            }
            self.state = INITIAL;
            self.emit(T::Newline, p, e);
        } else if is_space(c) {
            self.p = self.skip_spaces(p);
        } else if c == 58 {
            self.emit(T::Colon, p, p + 1);
        } else if c == 44 {
            self.emit(T::Comma, p, p + 1);
        } else if c == 35 {
            self.emit(T::Brkt, p, p + 1);
        } else if c == 59 {
            self.emit(T::Semi, p, p + 1);
        } else if is_run(c, STYLE_CLASS) {
            let mut e = p + width_at(self.src, p);
            while is_run(self.c(e), STYLE_CLASS) {
                e += width_at(self.src, e);
            }
            self.emit(T::StyleText, p, e);
        } else {
            self.tokens.push(Token::new(T::Error, "", p));
            return false;
        }
        true
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        tokens: Vec::new(),
        state: INITIAL,
        p: 0,
        dir_next: None,
        dir_known: false,
        dir_failed: 0,
        line_end: None,
        tilde_next: None,
        tilde_known: false,
        tilde_line: None,
        tilde_last: None,
        ws_next: None,
    };

    while lx.p < n {
        let p = lx.p;
        let c = code_at(src, p);
        match lx.state {
            INITIAL => lx.initial(c),
            BLOCK => lx.block(c),
            BLOCK_BQ => {
                if c == 96 {
                    lx.state = BLOCK;
                    lx.p += 1;
                } else {
                    let e = src[p..].find('`').map_or(n, |at| p + at);
                    lx.emit(T::AttributeWord, p, e);
                }
            }
            ACC_TITLE_STATE | ACC_DESCR_STATE => {
                let e = src[p..].find('\n').map_or(n, |at| p + at);
                let kind = if lx.state == ACC_TITLE_STATE {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                lx.emit(kind, p, e);
                lx.state = INITIAL;
            }
            ACC_DESCR_MULTILINE => {
                if c == 125 {
                    lx.state = INITIAL;
                    lx.p += 1;
                } else {
                    let e = src[p..].find('}').map_or(n, |at| p + at);
                    lx.emit(T::AccDescrMultilineValue, p, e);
                }
            }
            _ => {
                if !lx.style(c) {
                    return lx.tokens;
                }
            }
        }
    }

    // At the end of input Jison's INITIAL state yields EOF, the title and
    // description states yield an empty value, and the block and style
    // states yield nothing.
    let last = match lx.state {
        INITIAL => Some(T::Eof),
        ACC_TITLE_STATE => Some(T::AccTitleValue),
        ACC_DESCR_STATE => Some(T::AccDescrValue),
        ACC_DESCR_MULTILINE => Some(T::AccDescrMultilineValue),
        _ => None,
    };
    let mut tokens = lx.tokens;
    if let Some(last) = last {
        tokens.push(Token::new(last, "", n));
    }
    tokens.push(Token::new(T::End, "", n));
    tokens
}
