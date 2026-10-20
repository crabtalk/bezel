//! Pele's `diagrams/class/lexer.ts`.

use super::tokens::T;
use crate::{
    js::Lazy,
    token::Token,
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
const AT_END: [T; 13] = [
    T::Eof,
    T::Str,
    T::Eof,
    T::Generictype,
    T::CallbackName,
    T::CallbackArgs,
    T::AccTitleValue,
    T::AccDescrValue,
    T::AccDescrMultilineValue,
    T::Eof,
    T::EofInStruct,
    T::Eof,
    T::EofInStruct,
];

static RE_DIRECTION: [(T, Lazy); 4] = [
    (T::DirectionTb, Lazy::new(r".*direction\s+TB[^\n]*", "y")),
    (T::DirectionBt, Lazy::new(r".*direction\s+BT[^\n]*", "y")),
    (T::DirectionRl, Lazy::new(r".*direction\s+RL[^\n]*", "y")),
    (T::DirectionLr, Lazy::new(r".*direction\s+LR[^\n]*", "y")),
];

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    tokens: Vec<Token<'s, T>>,
    stack: Vec<usize>,
    state: usize,
    p: usize,
    dir_next: Option<usize>,
    line_end: Option<usize>,
    dir_failed: usize,
}

impl<'s> Lexer<'s> {
    fn emit(&mut self, kind: T, s: usize, e: usize) -> bool {
        self.tokens.push(Token::new(kind, &self.src[s..e], s));
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
        self.emit(T::Newline, self.p, k + 1);
    }

    /// The keywords that every state recognizes, even inside generics and
    /// backtick names.
    fn keyword(&mut self, c: u32) -> bool {
        let p = self.p;
        match c {
            99 => {
                if self.kw("cssClass") {
                    return self.emit(T::Cssclass, p, p + 8);
                }
                if self.kw("callback") {
                    return self.emit(T::Callback, p, p + 8);
                }
                if self.kw("click") {
                    return self.emit(T::Click, p, p + 5);
                }
            }
            108 if self.kw("link") => return self.emit(T::Link, p, p + 4),
            110 => {
                if self.kw("note for") {
                    return self.emit(T::NoteFor, p, p + 8);
                }
                if self.kw("note") {
                    return self.emit(T::Note, p, p + 4);
                }
            }
            60 if self.code(p + 1) == 60 => return self.emit(T::AnnotationStart, p, p + 2),
            62 if self.code(p + 1) == 62 => return self.emit(T::AnnotationEnd, p, p + 2),
            104 if self.kw("href") => return self.emit(T::Href, p, p + 4),
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
                    return self.emit(T::LinkTarget, p, p + 5);
                }
                if self.kw("_blank") {
                    return self.emit(T::LinkTarget, p, p + 6);
                }
                if self.kw("_parent") {
                    return self.emit(T::LinkTarget, p, p + 7);
                }
                if self.kw("_top") {
                    return self.emit(T::LinkTarget, p, p + 4);
                }
            }
            60 => {
                return if d == 124 {
                    self.emit(T::Extension, p, p + 2)
                } else {
                    self.emit(T::Dependency, p, p + 1)
                };
            }
            124 => return d == 62 && self.emit(T::Extension, p, p + 2),
            62 => return self.emit(T::Dependency, p, p + 1),
            42 => return self.emit(T::Composition, p, p + 1),
            111 => {
                if !is_word(d) {
                    return self.emit(T::Aggregation, p, p + 1);
                }
            }
            40 => return d == 41 && self.emit(T::Lollipop, p, p + 2),
            45 => {
                return if d == 45 {
                    self.emit(T::Line, p, p + 2)
                } else {
                    self.emit(T::Minus, p, p + 1)
                };
            }
            46 => {
                return if d == 46 {
                    self.emit(T::DottedLine, p, p + 2)
                } else {
                    self.emit(T::Dot, p, p + 1)
                };
            }
            58 => {
                let q = self.scan(p + 1, b":\n;");
                if q > p + 1 {
                    return self.emit(T::Label, p, q);
                }
                return d == 58 && self.code(p + 2) == 58 && self.emit(T::StyleSeparator, p, p + 3);
            }
            43 => return self.emit(T::Plus, p, p + 1),
            37 => return self.emit(T::Pct, p, p + 1),
            61 => return self.emit(T::Equals, p, p + 1),
            91 => return self.emit(T::Sqs, p, p + 1),
            93 => return self.emit(T::Sqe, p, p + 1),
            33 | 35 | 36 | 38 | 39 | 44 | 63 | 92 | 47 => {
                return self.emit(T::Punctuation, p, p + 1);
            }
            _ => {}
        }
        if is_word(c) {
            let mut q = p + 1;
            while is_word(self.code(q)) {
                q += 1;
            }
            return self.emit(T::Alpha, p, q);
        }
        // An astral character is a surrogate pair to Pele, whose halves are
        // no letters.
        let width = width_at(self.src, p);
        width < 4 && is_unicode_letter(c) && self.emit(T::UnicodeText, p, p + width)
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
        for (kind, re) in &RE_DIRECTION {
            if let Some(end) = re.match_end(self.src, p) {
                return self.emit(*kind, p, end);
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
                        self.emit(if title { T::AccTitle } else { T::AccDescr }, p, end);
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
                    return self.emit(T::ClassDiagram, p, p + 15);
                }
                if self.kw("classDiagram") {
                    return self.emit(T::ClassDiagram, p, p + 12);
                }
                if self.starts_with("call", p) && is_space(self.code(p + 4)) {
                    self.p = self.skip_ws(p + 4);
                    self.push(CALLBACK_NAME_STATE);
                    return true;
                }
                if self.kw("classDef") {
                    return self.emit(T::Classdef, p, p + 8);
                }
                if self.kw("class") {
                    self.push(CLS);
                    return self.emit(T::Class, p, p + 5);
                }
            }
            91 => {
                if self.starts_with("*]", p + 1) {
                    return self.emit(T::EdgeState, p, p + 3);
                }
            }
            115 => {
                if self.kw("style") {
                    return self.emit(T::Style, p, p + 5);
                }
            }
            110 => {
                if self.kw("namespace") {
                    self.push(NAMESPACE_STATE);
                    return self.emit(T::Namespace, p, p + 9);
                }
            }
            44 => return self.emit(T::Comma, p, p + 1),
            35 => return self.emit(T::Brkt, p, p + 1),
            _ => {}
        }
        self.common(c) || (c == 58 && self.emit(T::Colon, p, p + 1))
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
                    self.emit(T::StructStop, p, p + 1);
                } else if c == 123 {
                    self.push(CLASS_BODY);
                    self.emit(T::StructStart, p, p + 1);
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
                    self.emit(T::StructStop, p, p + 1);
                } else if c == 91 && self.starts_with("*]", p + 1) {
                    self.emit(T::EdgeState, p, p + 3);
                } else if c == 123 {
                    self.emit(T::OpenInStruct, p, p + 1);
                } else if c == 10 {
                    self.p += 1;
                } else {
                    let q = self.scan(p + 1, b"{}\n");
                    self.emit(T::Member, p, q);
                }
            }
            NAMESPACE_STATE => {
                if is_space(c) {
                    self.space(true);
                } else if c == 123 {
                    self.push(NAMESPACE_BODY);
                    self.emit(T::StructStart, p, p + 1);
                } else if c == 125 {
                    // The brace is read again in the state the namespace was
                    // opened from.
                    self.pop();
                } else if c == 110 && self.kw("namespace") {
                    self.push(NAMESPACE_STATE);
                    self.emit(T::Namespace, p, p + 9);
                } else {
                    return self.common(c);
                }
            }
            NAMESPACE_BODY => {
                if is_space(c) {
                    self.space(false);
                } else if c == 125 {
                    self.pop();
                    self.emit(T::StructStop, p, p + 1);
                } else if c == 110 && self.kw("namespace") {
                    self.push(NAMESPACE_STATE);
                    self.emit(T::Namespace, p, p + 9);
                } else if c == 91 && self.starts_with("*]", p + 1) {
                    self.emit(T::EdgeState, p, p + 3);
                } else if c == 99 && self.kw("class") {
                    self.push(CLS);
                    self.emit(T::Class, p, p + 5);
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
                    self.emit(T::Str, p, e);
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
                    self.emit(T::Generictype, p, e);
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
                    self.emit(T::BquoteStr, p, e);
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
                    self.emit(T::CallbackName, p, e);
                }
            }
            CALLBACK_ARGS_STATE => {
                if c == 41 {
                    self.pop();
                    self.p += 1;
                } else {
                    let e = self.until(')');
                    self.emit(T::CallbackArgs, p, e);
                }
            }
            ACC_TITLE_STATE | ACC_DESCR_STATE => {
                let kind = if self.state == ACC_TITLE_STATE {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
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
                    self.emit(T::AccDescrMultilineValue, p, e);
                }
            }
        }
        true
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lexer = Lexer {
        src,
        n,
        tokens: Vec::new(),
        stack: Vec::new(),
        state: INITIAL,
        p: 0,
        dir_next: src.find("direction"),
        line_end: None,
        dir_failed: 0,
    };

    loop {
        if lexer.p >= n {
            lexer.tokens.push(Token::new(AT_END[lexer.state], "", n));
            break;
        }
        let c = lexer.code(lexer.p);
        if !lexer.step(c) {
            lexer.tokens.push(Token::new(T::Error, "", lexer.p));
            return lexer.tokens;
        }
    }

    lexer.tokens.push(Token::new(T::End, "", n));
    lexer.tokens
}
