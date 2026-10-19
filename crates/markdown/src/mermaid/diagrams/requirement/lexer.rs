//! Pele's `diagrams/requirement/lexer.ts`.

use super::tokens::*;
use crate::mermaid::{
    js::{self, Lazy},
    util::chars::{code_at, is_line_end, is_space, is_word, width_at},
};

const INITIAL: u8 = 0;
const STRING: u8 = 1;
const STYLE_STATE: u8 = 2;
const ACC_TITLE_STATE: u8 = 3;
const ACC_DESCR_STATE: u8 = 4;
const ACC_DESCR_MULTILINE: u8 = 5;
const LONGEST_KEYWORD: usize = 22;

static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle\s*:\s*", "iy");
static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr\s*:\s*", "iy");
static RE_ACC_DESCR_ML: Lazy = Lazy::new(r"accDescr\s*\{\s*", "iy");
static RE_DIRECTION: Lazy = Lazy::new(r"direction\s+(?:(TB)|(BT)|(RL)|LR)", "gi");

/// Whether `c` ends an unquoted string: `:,{<>-=` and line breaks.
fn is_stop(c: u32) -> bool {
    matches!(c, 58 | 44 | 13 | 10 | 123 | 60 | 62 | 45 | 61)
}

pub struct Tokens<'s> {
    pub types: Vec<u8>,
    pub texts: Vec<&'s str>,
    pub starts: Vec<usize>,
}

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    tokens: Tokens<'s>,
    stack: Vec<u8>,
    state: u8,
    p: usize,
    /// Where each kind of direction statement starts and where its keyword
    /// ends, found in one pass.
    dir_starts: [Vec<usize>; 4],
    dir_ends: [Vec<usize>; 4],
    dir_seen: [usize; 4],
    any_dir: bool,
    line_end: Option<usize>,
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

    fn push(&mut self, state: u8) {
        self.stack.push(self.state);
        self.state = state;
    }

    fn pop(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    fn line_break(&self, q: usize) -> usize {
        self.src[q..].find('\n').map_or(self.n, |at| q + at)
    }

    /// `.*direction\s+XX[^\n]*`: the last such statement that starts on this
    /// line decides the token.
    fn direction(&mut self) -> bool {
        let p = self.p;
        if self.line_end.is_none_or(|end| end < p) {
            let line_end = self.src[p..]
                .find(['\n', '\r', '\u{2028}', '\u{2029}'])
                .map_or(self.n, |at| p + at);
            self.line_end = Some(line_end);
            for k in 0..4 {
                let list = &self.dir_starts[k];
                let mut j = self.dir_seen[k];
                while j < list.len() && list[j] <= line_end {
                    j += 1;
                }
                self.dir_seen[k] = j;
            }
        }
        for k in 0..4 {
            let Some(j) = self.dir_seen[k].checked_sub(1) else {
                continue;
            };
            if self.dir_starts[k][j] >= p {
                let e = self.line_break(self.dir_ends[k][j]);
                self.emit(DIRECTION_TB + k as u8, p, e);
                return true;
            }
        }
        false
    }

    fn initial(&mut self, c: u32) -> bool {
        let p = self.p;
        let n = self.n;
        let lower = c | 32;
        if lower == 116
            && self
                .src
                .get(p + 1..p + 5)
                .is_some_and(|rest| rest.eq_ignore_ascii_case("itle"))
            && is_space(self.c(p + 5))
        {
            let mut q = p + 5 + width_at(self.src, p + 5);
            let from = q;
            while q < n {
                let d = self.c(q);
                if d == 35 || d == 10 || d == 59 {
                    break;
                }
                q += width_at(self.src, q);
            }
            if q > from {
                self.emit(TITLE, p, q);
                return true;
            }
        }
        if lower == 97 && (self.c(p + 1) | 32) == 99 && (self.c(p + 2) | 32) == 99 {
            if let Some(e) = RE_ACC_TITLE.match_end(self.src, p) {
                self.push(ACC_TITLE_STATE);
                self.emit(ACC_TITLE, p, e);
                return true;
            }
            if let Some(e) = RE_ACC_DESCR.match_end(self.src, p) {
                self.push(ACC_DESCR_STATE);
                self.emit(ACC_DESCR, p, e);
                return true;
            }
            if let Some(e) = RE_ACC_DESCR_ML.match_end(self.src, p) {
                self.push(ACC_DESCR_MULTILINE);
                self.p = e;
                return true;
            }
        }
        if self.any_dir && !is_line_end(c) && self.direction() {
            return true;
        }

        if c == 10 || (c == 13 && self.c(p + 1) == 10) {
            let mut q = p;
            while q < n {
                let d = self.c(q);
                if d == 10 {
                    q += 1;
                } else if d == 13 && self.c(q + 1) == 10 {
                    q += 2;
                } else {
                    break;
                }
            }
            self.emit(NEWLINE, p, q);
            return true;
        }
        if is_space(c) {
            let mut q = p + width_at(self.src, p);
            while q < n && is_space(self.c(q)) {
                q += width_at(self.src, q);
            }
            self.p = q;
            return true;
        }
        if c == 35 || c == 37 {
            self.p = self.line_break(p);
            return true;
        }

        if is_word(c) {
            let mut q = p + 1;
            while q < n && is_word(self.c(q)) {
                q += 1;
            }
            if q - p <= LONGEST_KEYWORD {
                let word = self.src[p..q].to_ascii_lowercase();
                if let Some(&(_, keyword)) = KEYWORDS.iter().find(|(name, _)| *name == word) {
                    if keyword == STYLE || keyword == CLASSDEF || keyword == CLASS {
                        self.push(STYLE_STATE);
                    }
                    self.emit(keyword, p, q);
                    return true;
                }
            }
            while q < n {
                let d = self.c(q);
                if d < 128 && is_stop(d) {
                    break;
                }
                q += width_at(self.src, q);
            }
            self.tokens.types.push(UNQSTRING);
            self.tokens.texts.push(js::trim(&self.src[p..q]));
            self.tokens.starts.push(p);
            self.p = q;
            return true;
        }

        match c {
            123 => self.emit(STRUCT_START, p, p + 1),
            125 => self.emit(STRUCT_STOP, p, p + 1),
            58 => {
                if self.c(p + 1) == 58 && self.c(p + 2) == 58 {
                    self.emit(STYLE_SEPARATOR, p, p + 3);
                } else {
                    self.emit(COLONSEP, p, p + 1);
                }
            }
            60 => {
                if self.c(p + 1) != 45 {
                    return false;
                }
                self.emit(END_ARROW_L, p, p + 2);
            }
            45 => {
                if self.c(p + 1) == 62 {
                    self.emit(END_ARROW_R, p, p + 2);
                } else {
                    self.emit(LINE, p, p + 1);
                }
            }
            34 => {
                self.push(STRING);
                self.p += 1;
            }
            44 => self.emit(COMMA, p, p + 1),
            _ => return false,
        }
        true
    }

    fn style(&mut self, c: u32) -> bool {
        let p = self.p;
        if is_word(c) {
            let mut q = p + 1;
            while q < self.n && is_word(self.c(q)) {
                q += 1;
            }
            self.emit(ALPHA, p, q);
            return true;
        }
        match c {
            58 => self.emit(COLON, p, p + 1),
            59 => self.emit(SEMICOLON, p, p + 1),
            37 => self.emit(PERCENT, p, p + 1),
            45 => self.emit(MINUS, p, p + 1),
            35 => self.emit(BRKT, p, p + 1),
            32 => self.p += 1,
            34 => {
                self.push(STRING);
                self.p += 1;
            }
            10 => {
                self.pop();
                self.p += 1;
            }
            44 => self.emit(COMMA, p, p + 1),
            _ => return false,
        }
        true
    }
}

/// The token stream of Mermaid's requirement lexer. Its rules are tried in
/// order and the first match wins, so a line that holds a direction
/// statement anywhere is one token.
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
        stack: Vec::new(),
        state: INITIAL,
        p: 0,
        dir_starts: Default::default(),
        dir_ends: Default::default(),
        dir_seen: [0; 4],
        any_dir: false,
        line_end: None,
    };
    for m in RE_DIRECTION.captures_iter(src) {
        let kind = if m.get(1).is_some() {
            0
        } else if m.get(2).is_some() {
            1
        } else if m.get(3).is_some() {
            2
        } else {
            3
        };
        lx.dir_starts[kind].push(m.start());
        lx.dir_ends[kind].push(m.end());
        lx.any_dir = true;
    }

    while lx.p < n {
        let p = lx.p;
        let c = code_at(src, p);
        let ok = match lx.state {
            INITIAL => lx.initial(c),
            STYLE_STATE => lx.style(c),
            STRING => {
                if c == 34 {
                    lx.pop();
                    lx.p += 1;
                } else {
                    let e = src[p..].find('"').map_or(n, |at| p + at);
                    lx.emit(QSTRING, p, e);
                }
                true
            }
            ACC_TITLE_STATE | ACC_DESCR_STATE => {
                let kind = if lx.state == ACC_TITLE_STATE {
                    ACC_TITLE_VALUE
                } else {
                    ACC_DESCR_VALUE
                };
                lx.pop();
                let e = lx.line_break(p);
                lx.emit(kind, p, e);
                true
            }
            _ => {
                if c == 125 {
                    lx.pop();
                    lx.p += 1;
                } else {
                    let e = src[p..].find('}').map_or(n, |at| p + at);
                    lx.emit(ACC_DESCR_MULTILINE_VALUE, p, e);
                }
                true
            }
        };
        if !ok {
            let mut tokens = lx.tokens;
            tokens.types.push(ERROR);
            tokens.texts.push("");
            tokens.starts.push(p);
            return tokens;
        }
    }

    // At the end of input Jison still tries the rules that can match
    // nothing, once.
    let last = match lx.state {
        INITIAL => Some(EOF),
        STRING => Some(QSTRING),
        ACC_TITLE_STATE => Some(ACC_TITLE_VALUE),
        ACC_DESCR_STATE => Some(ACC_DESCR_VALUE),
        ACC_DESCR_MULTILINE => Some(ACC_DESCR_MULTILINE_VALUE),
        _ => None,
    };
    if let Some(last) = last {
        lx.emit(last, n, n);
    }
    lx.emit(END, n, n);
    lx.tokens
}
