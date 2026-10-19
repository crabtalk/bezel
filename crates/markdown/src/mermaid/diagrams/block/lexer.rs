//! Pele's `diagrams/block/lexer.ts`.

use super::tokens::*;
use crate::mermaid::util::chars::{code_at, is_space, is_word, width_at};

const S_INITIAL: u8 = 0;
const S_STRING: u8 = 1;
const S_MD_STRING: u8 = 2;
const S_NODE: u8 = 3;
const S_BLOCK_ARROW: u8 = 4;
const S_ARROW_DIR: u8 = 5;
const S_LLABEL: u8 = 6;
const S_CLASSDEF: u8 = 7;
const S_CLASSDEFID: u8 = 8;
const S_CLASS: u8 = 9;
const S_CLASS_STYLE: u8 = 10;
const S_STYLE_STMNT: u8 = 11;
const S_STYLE_DEFINITION: u8 = 12;
const S_ACC_TITLE: u8 = 13;
const S_ACC_DESCR: u8 = 14;
const S_ACC_DESCR_MULTILINE: u8 = 15;

const DIRECTIONS: [&str; 6] = ["right", "left", "x", "y", "up", "down"];

/// Characters that end a node id: ( ) [ { } < > - : = and whitespace.
const ID_STOP: [bool; 128] = {
    let mut table = [false; 128];
    let stops: [u8; 16] = [
        9, 10, 11, 12, 13, 32, 40, 41, 45, 58, 60, 61, 62, 91, 123, 125,
    ];
    let mut k = 0;
    while k < stops.len() {
        table[stops[k] as usize] = true;
        k += 1;
    }
    table
};

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
}

impl<'s> Lexer<'s> {
    fn code(&self, at: usize) -> u32 {
        code_at(self.src, at)
    }

    fn starts_with(&self, text: &str, at: usize) -> bool {
        self.src
            .as_bytes()
            .get(at..)
            .is_some_and(|rest| rest.starts_with(text.as_bytes()))
    }

    fn index_of(&self, c: char, from: usize) -> Option<usize> {
        self.src[from..].find(c).map(|e| from + e)
    }

    fn put(&mut self, kind: u8, text: &'s str, end: usize) -> bool {
        self.tokens.types.push(kind);
        self.tokens.texts.push(text);
        self.tokens.starts.push(self.p);
        self.p = end;
        true
    }

    fn emit(&mut self, kind: u8, end: usize) -> bool {
        let text = &self.src[self.p..end];
        self.put(kind, text, end)
    }

    fn push(&mut self, s: u8) {
        self.stack.push(self.state);
        self.state = s;
    }

    fn pop(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s;
        }
    }

    fn ws(&self, mut q: usize) -> usize {
        while q < self.n && is_space(self.code(q)) {
            q += width_at(self.src, q);
        }
        q
    }

    fn digits(&self, mut q: usize) -> usize {
        while q < self.n {
            let c = self.code(q);
            if !(48..=57).contains(&c) {
                break;
            }
            q += 1;
        }
        q
    }

    fn word(&self, mut q: usize) -> usize {
        while q < self.n && is_word(self.code(q)) {
            q += 1;
        }
        q
    }

    fn keyword(&self, text: &str) -> bool {
        self.starts_with(text, self.p) && !is_word(self.code(self.p + text.len()))
    }

    fn quote(&mut self) -> bool {
        if self.code(self.p + 1) == 96 {
            self.push(S_MD_STRING);
            self.p += 2;
        } else {
            self.push(S_STRING);
            self.p += 1;
        }
        true
    }

    /// End of `.+-[xo>]?` at q.
    fn dotted(&self, mut q: usize) -> Option<usize> {
        if self.code(q) != 46 {
            return None;
        }
        loop {
            q += 1;
            if self.code(q) != 46 {
                break;
            }
        }
        if self.code(q) != 45 {
            return None;
        }
        let c = self.code(q + 1);
        Some(if c == 120 || c == 111 || c == 62 {
            q + 2
        } else {
            q + 1
        })
    }

    /// End of a complete link at q, before its trailing whitespace.
    fn link_end(&self, mut q: usize) -> Option<usize> {
        let c = self.code(q);
        if c == 120 || c == 111 || c == 60 {
            q += 1;
        }
        let line = self.code(q);
        if line == 46 {
            return self.dotted(q);
        }
        if line != 45 && line != 61 {
            return None;
        }
        let mut k = q + 1;
        while self.code(k) == line {
            k += 1;
        }
        if k - q == 1 {
            return if line == 45 { self.dotted(k) } else { None };
        }
        let head = self.code(k);
        if head == 120 || head == 111 || head == 62 {
            return Some(k + 1);
        }
        if k - q >= 3 { Some(k) } else { None }
    }

    fn link(&mut self) -> bool {
        if let Some(e) = self.link_end(self.p) {
            let end = self.ws(e);
            return self.emit(LINK, end);
        }
        let q = if self.code(self.p) == 60 {
            self.p + 1
        } else {
            self.p
        };
        let a = self.code(q);
        let b = self.code(q + 1);
        if (a == 45 && (b == 45 || b == 46)) || (a == 61 && b == 61) {
            self.push(S_LLABEL);
            let end = self.ws(q + 2);
            return self.emit(START_LINK, end);
        }
        false
    }

    fn line_state(&mut self, kind: u8) -> bool {
        let e = self.index_of('\n', self.p).unwrap_or(self.n);
        self.pop();
        self.emit(kind, e)
    }

    /// `\w+(,\s*\w+)*`
    fn id_list(&mut self, kind: u8, next: u8) -> bool {
        let mut k = self.word(self.p);
        if k == self.p {
            return false;
        }
        while self.code(k) == 44 {
            let from = self.ws(k + 1);
            let to = self.word(from);
            if to == from {
                break;
            }
            k = to;
        }
        self.state = next;
        self.emit(kind, k)
    }

    fn initial(&mut self, c: u32) -> bool {
        let src = self.src;
        let p = self.p;
        if is_space(c) {
            self.p = self.ws(p + width_at(src, p));
            return true;
        }
        match c {
            98 => {
                if self.starts_with("block", p) {
                    if self.keyword("block-beta") {
                        return self.emit(KEY, p + 10);
                    }
                    if self.code(p + 5) == 58 {
                        return self.emit(ID_BLOCK, p + 6);
                    }
                    if !is_word(self.code(p + 5)) {
                        return self.emit(KEY, p + 5);
                    }
                }
            }
            99 => {
                if self.starts_with("columns", p) {
                    let q = self.ws(p + 7);
                    if q > p + 7 {
                        if self.starts_with("auto", q) && !is_word(self.code(q + 4)) {
                            return self.put(COLUMNS, "-1", q + 4);
                        }
                        let e = self.digits(q);
                        if e > q {
                            return self.put(COLUMNS, &src[q..e], e);
                        }
                    }
                } else if self.starts_with("class", p) {
                    let def = self.starts_with("Def", p + 5);
                    let from = if def { p + 8 } else { p + 5 };
                    let q = self.ws(from);
                    if q > from {
                        self.push(if def { S_CLASSDEF } else { S_CLASS });
                        return self.emit(if def { CLASSDEF } else { CLASS }, q);
                    }
                }
            }
            34 => return self.quote(),
            115 => {
                if self.starts_with("space", p) {
                    if self.code(p + 5) == 58 {
                        let e = self.digits(p + 6);
                        if e > p + 6 {
                            return self.put(SPACE_BLOCK, &src[p + 6..e], e);
                        }
                    }
                    if !is_word(self.code(p + 5)) {
                        return self.put(SPACE_BLOCK, "1", p + 5);
                    }
                } else if self.starts_with("style", p) {
                    let q = self.ws(p + 5);
                    if q > p + 5 {
                        self.push(S_STYLE_STMNT);
                        return self.emit(STYLE, q);
                    }
                }
            }
            100 => {
                if self.keyword("default") {
                    return self.emit(DEFAULT, p + 7);
                }
            }
            108 => {
                if self.keyword("linkStyle") {
                    return self.emit(LINKSTYLE, p + 9);
                }
            }
            105 => {
                if self.keyword("interpolate") {
                    return self.emit(INTERPOLATE, p + 11);
                }
            }
            97 => {
                if self.starts_with("accTitle", p) {
                    let q = self.ws(p + 8);
                    if self.code(q) == 58 {
                        self.push(S_ACC_TITLE);
                        let end = self.ws(q + 1);
                        return self.emit(ACC_TITLE, end);
                    }
                } else if self.starts_with("accDescr", p) {
                    let q = self.ws(p + 8);
                    let d = self.code(q);
                    if d == 58 {
                        self.push(S_ACC_DESCR);
                        let end = self.ws(q + 1);
                        return self.emit(ACC_DESCR, end);
                    }
                    if d == 123 {
                        self.push(S_ACC_DESCR_MULTILINE);
                        self.p = self.ws(q + 1);
                        return true;
                    }
                }
            }
            101 => {
                if self.keyword("end") {
                    let end = self.ws(p + 3);
                    return self.emit(END_KEYWORD, end);
                }
            }
            45 => {
                if self.code(p + 1) == 41 {
                    self.push(S_NODE);
                    return self.emit(NODE_DSTART, p + 2);
                }
                return self.link();
            }
            61 => return self.link(),
            60 => {
                if self.code(p + 1) == 91 {
                    self.push(S_BLOCK_ARROW);
                    return self.emit(BLOCK_ARROW_START, p + 2);
                }
                return self.link();
            }
            40 => {
                let d = self.code(p + 1);
                let len = if d == 45 || d == 91 {
                    2
                } else if d == 40 {
                    if self.code(p + 2) == 40 { 3 } else { 2 }
                } else {
                    1
                };
                self.push(S_NODE);
                return self.emit(NODE_DSTART, p + len);
            }
            41 | 123 => {
                self.push(S_NODE);
                let end = if self.code(p + 1) == c { p + 2 } else { p + 1 };
                return self.emit(NODE_DSTART, end);
            }
            62 => {
                self.push(S_NODE);
                return self.emit(NODE_DSTART, p + 1);
            }
            91 => {
                let d = self.code(p + 1);
                self.push(S_NODE);
                let end = if d == 91 || d == 124 || d == 40 || d == 92 || d == 47 {
                    p + 2
                } else {
                    p + 1
                };
                return self.emit(NODE_DSTART, end);
            }
            58 => {
                let e = self.digits(p + 1);
                if e > p + 1 {
                    return self.put(SIZE, &src[p + 1..e], e);
                }
                return false;
            }
            _ => {}
        }

        let mut q = p;
        while q < self.n {
            let ch = self.code(q);
            if if ch < 128 {
                ID_STOP[ch as usize]
            } else {
                is_space(ch)
            } {
                break;
            }
            q += width_at(src, q);
        }
        q > p && self.emit(NODE_ID, q)
    }

    fn node_end(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.code(p + 1);
        let len = match c {
            34 => return self.quote(),
            40 => {
                if d == 45 {
                    2
                } else if d == 40 {
                    if self.code(p + 2) == 40 { 3 } else { 2 }
                } else {
                    1
                }
            }
            41 => {
                if d == 41 {
                    if self.code(p + 2) == 41 { 3 } else { 2 }
                } else if d == 93 {
                    2
                } else {
                    1
                }
            }
            125 => {
                if d == 125 {
                    2
                } else {
                    1
                }
            }
            45 => {
                if d == 41 {
                    2
                } else {
                    0
                }
            }
            93 => {
                if d == 93 || d == 41 || d == 62 {
                    2
                } else {
                    1
                }
            }
            92 | 47 if d == 93 => 2,
            _ => 0,
        };
        if len == 0 {
            return false;
        }
        self.pop();
        self.emit(NODE_DEND, p + len)
    }

    fn arrow_dir(&mut self, c: u32) -> bool {
        let p = self.p;
        if c == 41 {
            self.pop();
            self.pop();
            let end = self.ws(p + 1);
            return self.put(BLOCK_ARROW_END, "]>", end);
        }
        let from = self.ws(if c == 44 { p + 1 } else { p });
        for dir in DIRECTIONS {
            if self.starts_with(dir, from) {
                let e = self.ws(from + dir.len());
                let text = &self.src[if c == 44 { from } else { p }..e];
                return self.put(DIR, text, e);
            }
        }
        false
    }

    fn step(&mut self) -> bool {
        let src = self.src;
        let p = self.p;
        let c = self.code(p);
        match self.state {
            S_INITIAL => return self.initial(c),
            S_STRING => {
                if c == 34 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.index_of('"', p).unwrap_or(self.n);
                    self.emit(STR, e);
                }
            }
            S_MD_STRING => {
                if c == 96 {
                    if self.code(p + 1) != 34 {
                        return false;
                    }
                    self.pop();
                    self.p += 2;
                } else if c == 34 {
                    return false;
                } else {
                    let mut q = p + width_at(src, p);
                    while q < self.n {
                        let ch = self.code(q);
                        if ch == 96 || ch == 34 {
                            break;
                        }
                        q += width_at(src, q);
                    }
                    self.emit(MD_STR, q);
                }
            }
            S_NODE => return self.node_end(c),
            S_BLOCK_ARROW => {
                if c == 34 {
                    self.quote();
                } else if c == 93 && self.code(p + 1) == 62 {
                    let q = self.ws(p + 2);
                    if self.code(q) != 40 {
                        return false;
                    }
                    self.push(S_ARROW_DIR);
                    self.p = q + 1;
                } else {
                    return false;
                }
            }
            S_ARROW_DIR => return self.arrow_dir(c),
            S_LLABEL => {
                if c == 34 {
                    if self.code(p + 1) == 96 {
                        self.quote();
                    } else {
                        self.emit(LINK_LABEL, p + 1);
                        self.push(S_STRING);
                    }
                } else {
                    let Some(e) = self.link_end(self.ws(p)) else {
                        return false;
                    };
                    self.pop();
                    let end = self.ws(e);
                    self.emit(LINK, end);
                }
            }
            S_CLASSDEF => {
                let mut kind = DEFAULT_CLASSDEF_ID;
                let mut k = if self.starts_with("DEFAULT", p) {
                    p + 7
                } else {
                    p
                };
                let mut q = self.ws(k);
                if k == p || q == k {
                    kind = CLASSDEF_ID;
                    k = self.word(p);
                    q = self.ws(k);
                }
                if !(k > p && q > k) {
                    return false;
                }
                self.state = S_CLASSDEFID;
                self.emit(kind, q);
            }
            S_CLASSDEFID => {
                self.line_state(CLASSDEF_STYLEOPTS);
            }
            S_CLASS => return self.id_list(CLASSENTITY_IDS, S_CLASS_STYLE),
            S_CLASS_STYLE => {
                self.line_state(STYLECLASS);
            }
            S_STYLE_STMNT => return self.id_list(STYLE_ENTITY_IDS, S_STYLE_DEFINITION),
            S_STYLE_DEFINITION => {
                self.line_state(STYLE_DEFINITION_DATA);
            }
            S_ACC_TITLE => {
                self.line_state(ACC_TITLE_VALUE);
            }
            S_ACC_DESCR => {
                self.line_state(ACC_DESCR_VALUE);
            }
            S_ACC_DESCR_MULTILINE => {
                if c == 125 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.index_of('}', p).unwrap_or(self.n);
                    self.emit(ACC_DESCR_MULTILINE_VALUE, e);
                }
            }
            _ => {}
        }
        true
    }
}

pub fn tokenize(src: &str) -> Tokens<'_> {
    let n = src.len();
    let mut lexer = Lexer {
        src,
        n,
        tokens: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
        },
        stack: Vec::new(),
        state: S_INITIAL,
        p: 0,
    };

    loop {
        if lexer.p >= n {
            let kind = match lexer.state {
                S_INITIAL => Some(EOF),
                S_STRING => Some(STR),
                S_CLASSDEFID => Some(CLASSDEF_STYLEOPTS),
                S_CLASS_STYLE => Some(STYLECLASS),
                S_STYLE_DEFINITION => Some(STYLE_DEFINITION_DATA),
                S_ACC_TITLE => Some(ACC_TITLE_VALUE),
                S_ACC_DESCR => Some(ACC_DESCR_VALUE),
                S_ACC_DESCR_MULTILINE => Some(ACC_DESCR_MULTILINE_VALUE),
                _ => None,
            };
            lexer.p = n;
            if let Some(kind) = kind {
                lexer.put(kind, "", n);
            }
            break;
        }
        if !lexer.step() {
            let p = lexer.p;
            let mut tokens = lexer.tokens;
            tokens.types.push(ERROR);
            tokens.texts.push("");
            tokens.starts.push(p);
            return tokens;
        }
    }

    let mut tokens = lexer.tokens;
    tokens.types.push(END);
    tokens.texts.push("");
    tokens.starts.push(n);
    tokens
}
