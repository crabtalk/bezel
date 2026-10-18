//! Pele's `diagrams/class/lexer.ts`.

use super::tokens::*;
use crate::mermaid::{
    js::Lazy,
    util::{
        chars::{code_at, is_space, is_word, width_at},
        unicode::is_unicode_letter,
    },
};

const INITIAL: usize = 0;
const STRING: usize = 1;
const BQSTRING: usize = 2;
const GENERIC: usize = 3;
const CALLBACK_NAME_STATE: usize = 4;
const CALLBACK_ARGS_STATE: usize = 5;
const ACC_TITLE_STATE: usize = 6;
const ACC_DESCR_STATE: usize = 7;
const ACC_DESCR_MULTILINE: usize = 8;
const CLS: usize = 9;
const CLASS_BODY: usize = 10;
const NAMESPACE_STATE: usize = 11;
const NAMESPACE_BODY: usize = 12;

/// The token each state gives at the end of input, before the parser sees the
/// end itself.
const AT_END: [u8; 13] = [
    EOF,
    STR,
    EOF,
    GENERICTYPE,
    CALLBACK_NAME,
    CALLBACK_ARGS,
    ACC_TITLE_VALUE,
    ACC_DESCR_VALUE,
    ACC_DESCR_MULTILINE_VALUE,
    EOF,
    EOF_IN_STRUCT,
    EOF,
    EOF_IN_STRUCT,
];

static RE_DIRECTION: [Lazy; 4] = [
    Lazy::new(r".*direction\s+TB[^\n]*", "y"),
    Lazy::new(r".*direction\s+BT[^\n]*", "y"),
    Lazy::new(r".*direction\s+RL[^\n]*", "y"),
    Lazy::new(r".*direction\s+LR[^\n]*", "y"),
];

pub struct Tokens<'s> {
    pub types: Vec<u8>,
    pub texts: Vec<&'s str>,
    pub starts: Vec<usize>,
}

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    tokens: Tokens<'s>,
    stack: Vec<usize>,
    state: usize,
    p: usize,
    dir_next: Option<usize>,
    line_end: Option<usize>,
    dir_failed: usize,
}

impl<'s> Lexer<'s> {
    fn emit(&mut self, kind: u8, s: usize, e: usize) -> bool {
        self.tokens.types.push(kind);
        self.tokens.texts.push(&self.src[s..e]);
        self.tokens.starts.push(s);
        self.p = e;
        true
    }

    fn push(&mut self, state: usize) {
        self.stack.push(self.state);
        self.state = state;
    }

    fn pop(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    fn code(&self, at: usize) -> u32 {
        code_at(self.src, at)
    }

    fn kw(&self, word: &str) -> bool {
        self.src[self.p..].starts_with(word) && !is_word(self.code(self.p + word.len()))
    }

    fn starts_with(&self, word: &str, at: usize) -> bool {
        self.src
            .get(at..)
            .is_some_and(|rest| rest.starts_with(word))
    }

    fn skip_ws(&self, mut q: usize) -> usize {
        while q < self.n && is_space(self.code(q)) {
            q += width_at(self.src, q);
        }
        q
    }

    fn until(&self, ch: char) -> usize {
        self.src[self.p..].find(ch).map_or(self.n, |e| self.p + e)
    }

    /// The first of `stops` from byte `q` on, or the end.
    fn scan(&self, mut q: usize, stops: &[u8]) -> usize {
        let bytes = self.src.as_bytes();
        while q < self.n && !stops.contains(&bytes[q]) {
            q += 1;
        }
        q
    }

    /// A run of whitespace is a NEWLINE up to its last line feed; the rest is
    /// skipped.
    fn space(&mut self, leaves: bool) {
        let mut q = self.p;
        let mut k = None;
        while q < self.n {
            let c = self.code(q);
            if !is_space(c) {
                break;
            }
            if c == 10 {
                k = Some(q);
            }
            q += width_at(self.src, q);
        }
        let Some(k) = k else {
            self.p = q;
            return;
        };
        if leaves {
            self.pop();
        }
        self.emit(NEWLINE, self.p, k + 1);
    }

    /// The keywords that every state recognizes, even inside generics and
    /// backtick names.
    fn keyword(&mut self, c: u32) -> bool {
        let p = self.p;
        match c {
            99 => {
                if self.kw("cssClass") {
                    return self.emit(CSSCLASS, p, p + 8);
                }
                if self.kw("callback") {
                    return self.emit(CALLBACK, p, p + 8);
                }
                if self.kw("click") {
                    return self.emit(CLICK, p, p + 5);
                }
            }
            108 if self.kw("link") => return self.emit(LINK, p, p + 4),
            110 => {
                if self.kw("note for") {
                    return self.emit(NOTE_FOR, p, p + 8);
                }
                if self.kw("note") {
                    return self.emit(NOTE, p, p + 4);
                }
            }
            60 if self.code(p + 1) == 60 => return self.emit(ANNOTATION_START, p, p + 2),
            62 if self.code(p + 1) == 62 => return self.emit(ANNOTATION_END, p, p + 2),
            104 if self.kw("href") => return self.emit(HREF, p, p + 4),
            _ => {}
        }
        false
    }

    fn common(&mut self, c: u32) -> bool {
        if self.keyword(c) {
            return true;
        }
        let p = self.p;
        let d = self.code(p + 1);
        match c {
            34 => {
                self.push(STRING);
                self.p += 1;
                return true;
            }
            126 => {
                self.push(GENERIC);
                self.p += 1;
                return true;
            }
            96 => {
                self.push(BQSTRING);
                self.p += 1;
                return true;
            }
            95 => {
                if self.kw("_self") {
                    return self.emit(LINK_TARGET, p, p + 5);
                }
                if self.kw("_blank") {
                    return self.emit(LINK_TARGET, p, p + 6);
                }
                if self.kw("_parent") {
                    return self.emit(LINK_TARGET, p, p + 7);
                }
                if self.kw("_top") {
                    return self.emit(LINK_TARGET, p, p + 4);
                }
            }
            60 => {
                return if d == 124 {
                    self.emit(EXTENSION, p, p + 2)
                } else {
                    self.emit(DEPENDENCY, p, p + 1)
                };
            }
            124 => return d == 62 && self.emit(EXTENSION, p, p + 2),
            62 => return self.emit(DEPENDENCY, p, p + 1),
            42 => return self.emit(COMPOSITION, p, p + 1),
            111 => {
                if !is_word(d) {
                    return self.emit(AGGREGATION, p, p + 1);
                }
            }
            40 => return d == 41 && self.emit(LOLLIPOP, p, p + 2),
            45 => {
                return if d == 45 {
                    self.emit(LINE, p, p + 2)
                } else {
                    self.emit(MINUS, p, p + 1)
                };
            }
            46 => {
                return if d == 46 {
                    self.emit(DOTTED_LINE, p, p + 2)
                } else {
                    self.emit(DOT, p, p + 1)
                };
            }
            58 => {
                let q = self.scan(p + 1, b":\n;");
                if q > p + 1 {
                    return self.emit(LABEL, p, q);
                }
                return d == 58 && self.code(p + 2) == 58 && self.emit(STYLE_SEPARATOR, p, p + 3);
            }
            43 => return self.emit(PLUS, p, p + 1),
            37 => return self.emit(PCT, p, p + 1),
            61 => return self.emit(EQUALS, p, p + 1),
            91 => return self.emit(SQS, p, p + 1),
            93 => return self.emit(SQE, p, p + 1),
            33 | 35 | 36 | 38 | 39 | 44 | 63 | 92 | 47 => {
                return self.emit(PUNCTUATION, p, p + 1);
            }
            _ => {}
        }
        if is_word(c) {
            let mut q = p + 1;
            while is_word(self.code(q)) {
                q += 1;
            }
            return self.emit(ALPHA, p, q);
        }
        // An astral character is a surrogate pair to Pele, whose halves are
        // no letters.
        let width = width_at(self.src, p);
        width < 4 && is_unicode_letter(c) && self.emit(UNICODE_TEXT, p, p + width)
    }

    fn direction(&mut self) -> bool {
        let p = self.p;
        let Some(mut dir_next) = self.dir_next else {
            return false;
        };
        if p < self.dir_failed {
            return false;
        }
        if dir_next < p {
            self.dir_next = self.src[p..].find("direction").map(|at| p + at);
            let Some(next) = self.dir_next else {
                return false;
            };
            dir_next = next;
        }
        let line_end = match self.line_end {
            Some(end) if end >= p => end,
            _ => {
                let end = self.src[p..]
                    .find(['\n', '\r', '\u{2028}', '\u{2029}'])
                    .map_or(self.n, |at| p + at);
                self.line_end = Some(end);
                end
            }
        };
        if dir_next >= line_end {
            return false;
        }
        for (i, re) in RE_DIRECTION.iter().enumerate() {
            if let Some(end) = re.match_end(self.src, p) {
                return self.emit(DIRECTION_TB + i as u8, p, end);
            }
        }
        // No direction statement starts anywhere in the rest of this line
        // either.
        self.dir_failed = line_end;
        false
    }

    fn initial(&mut self, c: u32) -> bool {
        if self.direction() {
            return true;
        }
        if is_space(c) {
            self.space(false);
            return true;
        }
        let p = self.p;
        match c {
            37 => {
                if self.code(p + 1) == 37 {
                    let mut e = self.until('\n');
                    while e < self.n && (self.code(e) == 10 || self.code(e) == 13) {
                        e += 1;
                    }
                    self.p = e;
                    return true;
                }
            }
            97 => {
                let title = self.starts_with("accTitle", p);
                if title || self.starts_with("accDescr", p) {
                    let q = self.skip_ws(p + 8);
                    let d = self.code(q);
                    if d == 58 {
                        let end = self.skip_ws(q + 1);
                        self.emit(if title { ACC_TITLE } else { ACC_DESCR }, p, end);
                        self.push(if title {
                            ACC_TITLE_STATE
                        } else {
                            ACC_DESCR_STATE
                        });
                        return true;
                    }
                    if d == 123 && !title {
                        self.p = self.skip_ws(q + 1);
                        self.push(ACC_DESCR_MULTILINE);
                        return true;
                    }
                }
            }
            99 => {
                if self.kw("classDiagram-v2") {
                    return self.emit(CLASS_DIAGRAM, p, p + 15);
                }
                if self.kw("classDiagram") {
                    return self.emit(CLASS_DIAGRAM, p, p + 12);
                }
                if self.starts_with("call", p) && is_space(self.code(p + 4)) {
                    self.p = self.skip_ws(p + 4);
                    self.push(CALLBACK_NAME_STATE);
                    return true;
                }
                if self.kw("classDef") {
                    return self.emit(CLASSDEF, p, p + 8);
                }
                if self.kw("class") {
                    self.push(CLS);
                    return self.emit(CLASS, p, p + 5);
                }
            }
            91 => {
                if self.starts_with("*]", p + 1) {
                    return self.emit(EDGE_STATE, p, p + 3);
                }
            }
            115 => {
                if self.kw("style") {
                    return self.emit(STYLE, p, p + 5);
                }
            }
            110 => {
                if self.kw("namespace") {
                    self.push(NAMESPACE_STATE);
                    return self.emit(NAMESPACE, p, p + 9);
                }
            }
            44 => return self.emit(COMMA, p, p + 1),
            35 => return self.emit(BRKT, p, p + 1),
            _ => {}
        }
        self.common(c) || (c == 58 && self.emit(COLON, p, p + 1))
    }

    /// Reads one token or skips text in the current state; false where no
    /// token matches.
    fn step(&mut self, c: u32) -> bool {
        let p = self.p;
        match self.state {
            INITIAL => return self.initial(c),
            CLS => {
                if is_space(c) {
                    self.space(true);
                } else if c == 125 {
                    self.pop();
                    self.pop();
                    self.emit(STRUCT_STOP, p, p + 1);
                } else if c == 123 {
                    self.push(CLASS_BODY);
                    self.emit(STRUCT_START, p, p + 1);
                } else {
                    return self.common(c);
                }
            }
            CLASS_BODY => {
                if c == 34 {
                    self.push(STRING);
                    self.p += 1;
                } else if c == 125 {
                    self.pop();
                    self.emit(STRUCT_STOP, p, p + 1);
                } else if c == 91 && self.starts_with("*]", p + 1) {
                    self.emit(EDGE_STATE, p, p + 3);
                } else if c == 123 {
                    self.emit(OPEN_IN_STRUCT, p, p + 1);
                } else if c == 10 {
                    self.p += 1;
                } else {
                    let q = self.scan(p + 1, b"{}\n");
                    self.emit(MEMBER, p, q);
                }
            }
            NAMESPACE_STATE => {
                if is_space(c) {
                    self.space(true);
                } else if c == 123 {
                    self.push(NAMESPACE_BODY);
                    self.emit(STRUCT_START, p, p + 1);
                } else if c == 125 {
                    // The brace is read again in the state the namespace was
                    // opened from.
                    self.pop();
                } else if c == 110 && self.kw("namespace") {
                    self.push(NAMESPACE_STATE);
                    self.emit(NAMESPACE, p, p + 9);
                } else {
                    return self.common(c);
                }
            }
            NAMESPACE_BODY => {
                if is_space(c) {
                    self.space(false);
                } else if c == 125 {
                    self.pop();
                    self.emit(STRUCT_STOP, p, p + 1);
                } else if c == 110 && self.kw("namespace") {
                    self.push(NAMESPACE_STATE);
                    self.emit(NAMESPACE, p, p + 9);
                } else if c == 91 && self.starts_with("*]", p + 1) {
                    self.emit(EDGE_STATE, p, p + 3);
                } else if c == 99 && self.kw("class") {
                    self.push(CLS);
                    self.emit(CLASS, p, p + 5);
                } else {
                    return self.common(c);
                }
            }
            STRING => {
                if c == 34 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.until('"');
                    self.emit(STR, p, e);
                }
            }
            GENERIC => {
                if c == 34 {
                    self.push(STRING);
                    self.p += 1;
                } else if self.keyword(c) {
                } else if c == 126 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.until('~');
                    self.emit(GENERICTYPE, p, e);
                }
            }
            BQSTRING => {
                if c == 34 {
                    self.push(STRING);
                    self.p += 1;
                } else if self.keyword(c) {
                } else if c == 126 {
                    self.push(GENERIC);
                    self.p += 1;
                } else if c == 96 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.until('`');
                    self.emit(BQUOTE_STR, p, e);
                }
            }
            CALLBACK_NAME_STATE => {
                if c == 40 {
                    let q = self.skip_ws(p + 1);
                    self.pop();
                    if self.code(q) == 41 {
                        self.p = q + 1;
                    } else {
                        self.push(CALLBACK_ARGS_STATE);
                        self.p += 1;
                    }
                } else {
                    let e = self.until('(');
                    self.emit(CALLBACK_NAME, p, e);
                }
            }
            CALLBACK_ARGS_STATE => {
                if c == 41 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.until(')');
                    self.emit(CALLBACK_ARGS, p, e);
                }
            }
            ACC_TITLE_STATE | ACC_DESCR_STATE => {
                let kind = if self.state == ACC_TITLE_STATE {
                    ACC_TITLE_VALUE
                } else {
                    ACC_DESCR_VALUE
                };
                let e = self.until('\n');
                self.emit(kind, p, e);
                self.pop();
            }
            _ => {
                if c == 125 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.until('}');
                    self.emit(ACC_DESCR_MULTILINE_VALUE, p, e);
                }
            }
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
        state: INITIAL,
        p: 0,
        dir_next: src.find("direction"),
        line_end: None,
        dir_failed: 0,
    };

    loop {
        if lexer.p >= n {
            lexer.tokens.types.push(AT_END[lexer.state]);
            lexer.tokens.texts.push("");
            lexer.tokens.starts.push(n);
            break;
        }
        let c = lexer.code(lexer.p);
        if !lexer.step(c) {
            let p = lexer.p;
            lexer.tokens.types.push(ERROR);
            lexer.tokens.texts.push("");
            lexer.tokens.starts.push(p);
            return lexer.tokens;
        }
    }

    lexer.tokens.types.push(END);
    lexer.tokens.texts.push("");
    lexer.tokens.starts.push(n);
    lexer.tokens
}
