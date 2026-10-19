//! Pele's `diagrams/er/lexer.ts`.

use super::tokens::*;
use crate::mermaid::{
    js::Lazy,
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
static RE_DIRECTION: [Lazy; 4] = [
    Lazy::new(r".*direction\s+TB[^\n]*", "iy"),
    Lazy::new(r".*direction\s+BT[^\n]*", "iy"),
    Lazy::new(r".*direction\s+RL[^\n]*", "iy"),
    Lazy::new(r".*direction\s+LR[^\n]*", "iy"),
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

pub struct Tokens<'s> {
    pub types: Vec<u8>,
    pub texts: Vec<&'s str>,
    pub starts: Vec<usize>,
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
    tokens: Tokens<'s>,
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

    fn emit(&mut self, kind: u8, s: usize, e: usize) {
        self.tokens.types.push(kind);
        self.tokens.texts.push(&self.src[s..e]);
        self.tokens.starts.push(s);
        self.p = e;
    }

    /// A one-character token at `p`.
    fn emit_char(&mut self, kind: u8) {
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
        for (i, re) in RE_DIRECTION.iter().enumerate() {
            if let Some(e) = self.at(re, p) {
                self.emit(DIRECTION_TB + i as u8, p, e);
                return true;
            }
        }
        self.dir_failed = line_end;
        false
    }

    fn quoted(&mut self) {
        let p = self.p;
        let Some(e) = self.src[p + 1..].find('"').map(|at| p + 1 + at) else {
            self.emit_char(CHAR);
            return;
        };
        let name = e > p + 1
            && !self.src.as_bytes()[p + 1..e]
                .iter()
                .any(|&c| matches!(c, 37 | 92 | 13 | 10 | 11 | 8));
        self.emit(if name { ENTITY_NAME } else { WORD }, p, e + 1);
    }

    fn number(&mut self, c: u32) {
        let p = self.p;
        let d = self.c(p + 1);
        if d == 43 && (c == 49 || c == 48) {
            self.emit(if c == 49 { ONE_OR_MORE } else { ZERO_OR_MORE }, p, p + 2);
            return;
        }
        let q = self.digits(p + 1);
        if self.c(q) == 46 && is_digit(self.c(q + 1)) {
            let e = self.digits(q + 2);
            self.emit(DECIMAL_NUM, p, e);
            return;
        }
        if c == 49 {
            if is_space(d) {
                let r = self.skip_spaces(p + 1 + width_at(self.src, p + 1));
                let f = self.c(r);
                if is_word(f) || f == 34 || f == 39 {
                    self.emit(ONLY_ONE, p, p + 1);
                    return;
                }
            } else if d == 45 || d == 46 {
                let f = self.c(p + 2);
                if f == 45 || f == 46 {
                    self.emit(ONLY_ONE, p, p + 1);
                    return;
                }
            }
            if !is_word(d) {
                self.emit(ENTITY_ONE, p, p + 1);
                return;
            }
        }
        self.emit(NUM, p, q);
    }

    fn keyword(&mut self, lc: u32) -> bool {
        let p = self.p;
        let d = self.c(p + 1);
        match lc {
            101 => {
                if self.kwb("erdiagram") {
                    self.emit(ER_DIAGRAM, p, p + 9);
                } else if self.kwb("end") {
                    let e = self.skip_spaces(p + 3);
                    self.emit(END_KW, p, e);
                } else {
                    return false;
                }
                true
            }
            115 => {
                if self.kwb("style") {
                    self.state = STYLE_STATE;
                    self.emit(STYLE, p, p + 5);
                } else if self.kwb("subgraph") {
                    self.emit(SUBGRAPH, p, p + 8);
                } else {
                    return false;
                }
                true
            }
            99 => {
                if self.kwb("classdef") {
                    self.state = STYLE_STATE;
                    self.emit(CLASSDEF, p, p + 8);
                } else if self.kwb("class") {
                    self.emit(CLASS, p, p + 5);
                } else {
                    return false;
                }
                true
            }
            111 => {
                if self.kwb("one or zero") {
                    self.emit(ZERO_OR_ONE, p, p + 11);
                } else if self.kwb("one or more") || self.kwb("one or many") {
                    self.emit(ONE_OR_MORE, p, p + 11);
                } else if self.kwb("one") {
                    self.emit(ONLY_ONE, p, p + 3);
                } else if self.kwb("only one") {
                    self.emit(ONLY_ONE, p, p + 8);
                } else if d == 124 {
                    self.emit(ZERO_OR_ONE, p, p + 2);
                } else if d == 123 {
                    self.emit(ZERO_OR_MORE, p, p + 2);
                } else if self.kwb("optionally to") {
                    self.emit(NON_IDENTIFYING, p, p + 13);
                } else {
                    return false;
                }
                true
            }
            122 => {
                if self.kwb("zero or one") {
                    self.emit(ZERO_OR_ONE, p, p + 11);
                } else if self.kwb("zero or more") || self.kwb("zero or many") {
                    self.emit(ZERO_OR_MORE, p, p + 12);
                } else {
                    return false;
                }
                true
            }
            109 => {
                if self.kw("many(0)") {
                    self.emit(ZERO_OR_MORE, p, p + 7);
                } else if self.kw("many(1)") {
                    self.emit(ONE_OR_MORE, p, p + 7);
                } else if self.kwb("many") {
                    self.emit(ZERO_OR_MORE, p, p + 4);
                } else {
                    return false;
                }
                true
            }
            116 => {
                if !self.kwb("to") {
                    return false;
                }
                self.emit(IDENTIFYING, p, p + 2);
                true
            }
            117 => {
                if d != 46 && d != 45 && d != 124 {
                    return false;
                }
                self.emit(MD_PARENT, p, p + 1);
                true
            }
            124 => {
                if self.lower(p + 1) == 111 && !is_word(self.c(p + 2)) {
                    self.emit(ZERO_OR_ONE, p, p + 2);
                } else if d == 124 {
                    self.emit(ONLY_ONE, p, p + 2);
                } else if d == 123 {
                    self.emit(ONE_OR_MORE, p, p + 2);
                } else {
                    return false;
                }
                true
            }
            125 => {
                if self.lower(p + 1) == 111 && !is_word(self.c(p + 2)) {
                    self.emit(ZERO_OR_MORE, p, p + 2);
                } else if d == 124 {
                    self.emit(ONE_OR_MORE, p, p + 2);
                } else {
                    return false;
                }
                true
            }
            46 => {
                if d != 46 && d != 45 {
                    return false;
                }
                self.emit(NON_IDENTIFYING, p, p + 2);
                true
            }
            45 => {
                if d == 45 {
                    self.emit(IDENTIFYING, p, p + 2);
                } else if d == 46 {
                    self.emit(NON_IDENTIFYING, p, p + 2);
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
                self.emit(ACC_TITLE, p, e);
                return;
            }
            if let Some(e) = self.at(&RE_ACC_DESCR, p) {
                self.state = ACC_DESCR_STATE;
                self.emit(ACC_DESCR, p, e);
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
                self.emit(NEWLINE, p, e);
                return;
            }
            34 => {
                self.quoted();
                return;
            }
            123 => {
                self.state = BLOCK;
                self.emit(BLOCK_START, p, p + 1);
                return;
            }
            35 => {
                self.emit(BRKT, p, p + 1);
                return;
            }
            44 => {
                self.emit(COMMA, p, p + 1);
                return;
            }
            58 => {
                if self.c(p + 1) == 58 && self.c(p + 2) == 58 {
                    self.emit(STYLE_SEPARATOR, p, p + 3);
                } else {
                    self.emit(COLON, p, p + 1);
                }
                return;
            }
            91 => {
                self.emit(SQS, p, p + 1);
                return;
            }
            93 => {
                self.emit(SQE, p, p + 1);
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
            self.emit(UNICODE_TEXT, p, e);
            return;
        }
        self.emit_char(CHAR);
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
        self.emit(ATTRIBUTE_WORD, p, e);
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
            self.emit(ATTRIBUTE_KEY, p, p + 2);
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
            self.emit(ATTRIBUTE_WORD, p, e);
            return;
        }
        if c == 96 {
            self.state = BLOCK_BQ;
            self.p += 1;
            return;
        }
        if c == 34 {
            if let Some(e) = self.src[p + 1..].find('"') {
                self.emit(COMMENT, p, p + 1 + e + 1);
                return;
            }
        } else if c == 125 {
            self.state = INITIAL;
            self.emit(BLOCK_STOP, p, p + 1);
            return;
        }
        self.emit_char(CHAR);
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
            self.emit(NEWLINE, p, e);
        } else if is_space(c) {
            self.p = self.skip_spaces(p);
        } else if c == 58 {
            self.emit(COLON, p, p + 1);
        } else if c == 44 {
            self.emit(COMMA, p, p + 1);
        } else if c == 35 {
            self.emit(BRKT, p, p + 1);
        } else if c == 59 {
            self.emit(SEMI, p, p + 1);
        } else if is_run(c, STYLE_CLASS) {
            let mut e = p + width_at(self.src, p);
            while is_run(self.c(e), STYLE_CLASS) {
                e += width_at(self.src, e);
            }
            self.emit(STYLE_TEXT, p, e);
        } else {
            self.tokens.types.push(ERROR);
            self.tokens.texts.push("");
            self.tokens.starts.push(p);
            return false;
        }
        true
    }
}

pub fn tokenize(src: &str) -> Tokens<'_> {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        tokens: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
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
                    lx.emit(ATTRIBUTE_WORD, p, e);
                }
            }
            ACC_TITLE_STATE | ACC_DESCR_STATE => {
                let e = src[p..].find('\n').map_or(n, |at| p + at);
                let kind = if lx.state == ACC_TITLE_STATE {
                    ACC_TITLE_VALUE
                } else {
                    ACC_DESCR_VALUE
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
                    lx.emit(ACC_DESCR_MULTILINE_VALUE, p, e);
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
        INITIAL => Some(EOF),
        ACC_TITLE_STATE => Some(ACC_TITLE_VALUE),
        ACC_DESCR_STATE => Some(ACC_DESCR_VALUE),
        ACC_DESCR_MULTILINE => Some(ACC_DESCR_MULTILINE_VALUE),
        _ => None,
    };
    let mut tokens = lx.tokens;
    if let Some(last) = last {
        tokens.types.push(last);
        tokens.texts.push("");
        tokens.starts.push(n);
    }
    tokens.types.push(END);
    tokens.texts.push("");
    tokens.starts.push(n);
    tokens
}
