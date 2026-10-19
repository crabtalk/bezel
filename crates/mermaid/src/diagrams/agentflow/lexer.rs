//! Pele's `diagrams/agentflow/lexer.ts`. Positions are byte offsets; the
//! columns in `locs` count UTF-16 units, as Jison does.

use super::tokens::T;
use crate::{
    js::{self, Lazy},
    util::{
        chars::{code_at, is_space, is_word, width_at},
        unicode::is_unicode_letter,
    },
};

const NODE_CHARS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!\"#$%&'*+.`?\\_/";

static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle\s*:\s*", "y");
static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr\s*:\s*", "y");
static RE_ACC_DESCR_ML: Lazy = Lazy::new(r"accDescr\s*\{\s*", "y");
static RE_CALL: Lazy = Lazy::new(r"call\s+", "y");
static RE_CLICK: Lazy = Lazy::new(r"click\s+", "y");
static RE_NODIR: Lazy = Lazy::new(r"(?:\r?\n)*\s*\n", "y");
static RE_DIR: Lazy = Lazy::new(r"\s*(?:LR|RL|TB|BT|TD|BR)\b", "y");
static RE_DIR_SYM: Lazy = Lazy::new(r"\s*[<>^]", "y");
static RE_DIR_V: Lazy = Lazy::new(r"\s*v\b", "y");
static RE_DIRECTION: [Lazy; 5] = [
    Lazy::new(r".*direction\s+TB[^\n]*", "y"),
    Lazy::new(r".*direction\s+BT[^\n]*", "y"),
    Lazy::new(r".*direction\s+RL[^\n]*", "y"),
    Lazy::new(r".*direction\s+LR[^\n]*", "y"),
    Lazy::new(r".*direction\s+TD[^\n]*", "y"),
];
static RE_LINE_END: Lazy = Lazy::new(r"[\n\r  ]", "g");
static RE_EMPTY_CALL: Lazy = Lazy::new(r"\(\s*\)", "y");
static RE_BR: Lazy = Lazy::new(r"\n\s*", "g");

pub struct Tokens {
    pub types: Vec<usize>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
    /// Four numbers per token, counted the way Jison counts them: first
    /// line, first column, last line, last column. Lines start at 1, columns
    /// at 0.
    pub locs: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    String,
    MdString,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Dir,
    Text,
    EllipseText,
    TrapText,
    EdgeText,
    Click,
    Callbackname,
    Callbackargs,
    ShapeData,
    ShapeDataStr,
}

fn is_node_char(ch: u32) -> bool {
    ch < 128 && NODE_CHARS.contains(&(ch as u8))
}

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    out: Tokens,
    stack: Vec<S>,
    state: S,
    p: usize,
    line: usize,
    col: usize,
    first_graph: bool,
    dir_next: Option<usize>,
    at_next: Option<usize>,
    line_end: Option<usize>,
    dir_failed: usize,
    run_end: usize,
    run_at: Option<usize>,
    /// The last run of whitespace measured, from and to.
    ws: Option<(usize, usize)>,
    /// The last run of spaces and tabs measured, from and to.
    blank: Option<(usize, usize)>,
    no_link: Option<usize>,
    no_arrow: Option<usize>,
}

impl<'s> Lexer<'s> {
    fn code(&self, at: usize) -> u32 {
        code_at(self.src, at)
    }

    /// Moves over one match. Jison counts \r\n, \r and \n as one line break
    /// each, and after a break its column stops at the first U+2028 or
    /// U+2029.
    fn advance(&mut self, e: usize) {
        let bytes = self.src.as_bytes();
        let mut brk: Option<usize> = None;
        let mut k = self.p;
        while k < e {
            let ch = bytes[k];
            if ch == b'\n' {
                self.line += 1;
                brk = Some(k);
            } else if ch == b'\r' {
                if k + 1 < e && bytes[k + 1] == b'\n' {
                    k += 1;
                }
                self.line += 1;
                brk = Some(k);
            }
            k += 1;
        }
        match brk {
            None => self.col += js::len(&self.src[self.p..e]),
            Some(brk) => {
                let rest = &self.src[brk + 1..e];
                let stop = rest.find(['\u{2028}', '\u{2029}']).unwrap_or(rest.len());
                self.col = js::len(&rest[..stop]);
            }
        }
        self.p = e;
    }

    fn emit(&mut self, kind: usize, e: usize) {
        self.out.types.push(kind);
        self.out.texts.push(self.src[self.p..e].to_string());
        self.out.starts.push(self.p);
        self.out.locs.extend([self.line, self.col]);
        self.advance(e);
        self.out.locs.extend([self.line, self.col]);
    }

    /// For a token that cannot hold a line break.
    fn emit_flat(&mut self, kind: usize, e: usize) {
        self.out.types.push(kind);
        self.out.texts.push(self.src[self.p..e].to_string());
        self.out.starts.push(self.p);
        self.out.locs.extend([self.line, self.col]);
        self.col += js::len(&self.src[self.p..e]);
        self.p = e;
        self.out.locs.extend([self.line, self.col]);
    }

    /// For a run that Jison matches one character at a time, where \r\n
    /// counts as two line breaks.
    fn emit_chars(&mut self, kind: usize, e: usize) {
        self.out.types.push(kind);
        self.out.texts.push(self.src[self.p..e].to_string());
        self.out.starts.push(self.p);
        self.out.locs.extend([self.line, self.col]);
        for ch in self.src[self.p..e].chars() {
            if ch == '\n' || ch == '\r' {
                self.line += 1;
                self.col = 0;
            } else {
                self.col += ch.len_utf16();
            }
        }
        self.p = e;
        self.out.locs.extend([self.line, self.col]);
    }

    fn emit_text(&mut self, kind: usize, text: String, e: usize) {
        self.out.types.push(kind);
        self.out.texts.push(text);
        self.out.starts.push(self.p);
        self.out.locs.extend([self.line, self.col]);
        self.advance(e);
        self.out.locs.extend([self.line, self.col]);
    }

    fn push(&mut self, s: S) {
        self.stack.push(self.state);
        self.state = s;
    }

    fn pop(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.state = s;
        }
    }

    fn at(&self, re: &Lazy, q: usize) -> Option<usize> {
        re.match_end(self.src, q)
    }

    fn kw(&self, word: &str) -> bool {
        self.src[self.p..].starts_with(word) && !is_word(self.code(self.p + word.len()))
    }

    fn find(&self, needle: char, from: usize) -> Option<usize> {
        self.src
            .get(from..)
            .and_then(|rest| rest.find(needle))
            .map(|at| at + from)
    }

    /// End of the run of whitespace starting at `q`. The last run is kept,
    /// so asking again from inside it costs nothing.
    fn ws_end(&mut self, q: usize) -> usize {
        if let Some((from, to)) = self.ws
            && q >= from
            && q <= to
        {
            return to;
        }
        let mut j = q;
        while j < self.n && is_space(self.code(j)) {
            j += width_at(self.src, j);
        }
        self.ws = Some((q, j));
        j
    }

    fn blank_end(&mut self, q: usize) -> usize {
        if let Some((from, to)) = self.blank
            && q >= from
            && q <= to
        {
            return to;
        }
        let j = self.blanks(q);
        self.blank = Some((q, j));
        j
    }

    /// End of `\s*--+[x>]\s*` starting at `q`.
    fn arrow_end(&mut self, q: usize) -> Option<usize> {
        let j = self.ws_end(q);
        if Some(j) == self.no_arrow || self.code(j) != 45 || self.code(j + 1) != 45 {
            return None;
        }
        let mut k = j + 2;
        while self.code(k) == 45 {
            k += 1;
        }
        let t = self.code(k);
        if t == 120 || t == 62 {
            return Some(self.ws_end(k + 1));
        }
        self.no_arrow = Some(j);
        None
    }

    fn link(&mut self) -> bool {
        let j = self.ws_end(self.p);
        if Some(j) == self.no_link || self.code(j) != 45 {
            return false;
        }
        let d = self.code(j + 1);
        let mut k = j + 2;
        if d == 45 {
            while self.code(k) == 45 {
                k += 1;
            }
            let t = self.code(k);
            if t == 120 || t == 62 {
                let e = self.ws_end(k + 1);
                self.emit(T::LINK, e);
            } else {
                self.push(S::EdgeText);
                let e = self.ws_end(j + 2);
                self.emit(T::START_LINK, e);
            }
            return true;
        }
        if d == 46 {
            while self.code(k) == 46 {
                k += 1;
            }
            if self.code(k) == 45 {
                let e = self.ws_end(k + 1);
                self.emit(T::LINK, e);
                return true;
            }
        }
        self.no_link = Some(j);
        false
    }

    /// `%%` not followed by `{`, through the end of the line.
    fn comment_end(&self, q: usize) -> Option<usize> {
        if self.code(q) != 37 || self.code(q + 1) != 37 || self.code(q + 2) == 123 {
            return None;
        }
        Some(self.find('\n', q + 2).unwrap_or(self.n))
    }

    fn quote(&mut self) {
        if self.code(self.p + 1) == 96 {
            self.push(S::MdString);
            self.advance(self.p + 2);
        } else {
            self.push(S::String);
            self.advance(self.p + 1);
        }
    }

    fn opener(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.code(p + 1);
        if c == 40 {
            if d == 45 {
                self.push(S::EllipseText);
                self.emit_flat(T::ELLIPSE_START, p + 2);
            } else if d == 91 {
                self.push(S::Text);
                self.emit_flat(T::STADIUMSTART, p + 2);
            } else if d == 40 && self.code(p + 2) == 40 {
                self.push(S::Text);
                self.emit_flat(T::DOUBLECIRCLESTART, p + 3);
            } else {
                self.push(S::Text);
                self.emit_flat(T::PS, p + 1);
            }
            return true;
        }
        if c == 91 {
            if d == 91 {
                self.push(S::Text);
                self.emit_flat(T::SUBROUTINESTART, p + 2);
            } else if d == 40 {
                self.push(S::Text);
                self.emit_flat(T::CYLINDERSTART, p + 2);
            } else if d == 47 {
                self.push(S::TrapText);
                self.emit_flat(T::TRAPSTART, p + 2);
            } else if d == 92 {
                self.push(S::TrapText);
                self.emit_flat(T::INVTRAPSTART, p + 2);
            } else {
                self.push(S::Text);
                self.emit_flat(T::SQS, p + 1);
            }
            return true;
        }
        if c == 124 {
            self.push(S::Text);
            self.emit_flat(T::PIPE, p + 1);
            return true;
        }
        if c == 123 {
            self.push(S::Text);
            self.emit_flat(T::DIAMOND_START, p + 1);
            return true;
        }
        false
    }

    fn node_string(&mut self) -> bool {
        let p = self.p;
        let mut q = p;
        while q < self.n {
            let ch = self.code(q);
            if is_node_char(ch) {
                q += 1;
            } else if ch == 45 && q + 1 < self.n {
                let d = self.code(q + 1);
                if d == 62 || d == 45 || d == 46 {
                    break;
                }
                q += 1;
            } else {
                break;
            }
        }
        if q == p {
            return false;
        }
        self.emit_flat(T::NODE_STRING, q);
        true
    }

    fn blanks(&self, from: usize) -> usize {
        let mut e = from;
        while e < self.n {
            let ch = self.code(e);
            if ch != 32 && ch != 9 {
                break;
            }
            e += 1;
        }
        e
    }

    fn initial(&mut self, c: u32) -> bool {
        let p = self.p;
        let src = self.src;
        let n = self.n;
        match c {
            97 => {
                if src[p..].starts_with("acc") {
                    if let Some(e) = self.at(&RE_ACC_TITLE, p) {
                        self.push(S::AccTitle);
                        self.emit(T::ACC_TITLE, e);
                        return true;
                    }
                    if let Some(e) = self.at(&RE_ACC_DESCR, p) {
                        self.push(S::AccDescr);
                        self.emit(T::ACC_DESCR, e);
                        return true;
                    }
                    if let Some(e) = self.at(&RE_ACC_DESCR_ML, p) {
                        self.push(S::AccDescrMultiline);
                        self.advance(e);
                        return true;
                    }
                }
                if self.kw("agentflow-beta") {
                    if self.first_graph {
                        self.first_graph = false;
                        self.push(S::Dir);
                    }
                    self.emit_flat(T::GRAPH, p + 14);
                    return true;
                }
            }
            64 => {
                if self.code(p + 1) == 123 {
                    self.push(S::ShapeData);
                    self.emit_text(T::SHAPE_DATA, String::new(), p + 2);
                    return true;
                }
            }
            99 => {
                if let Some(e) = self.at(&RE_CALL, p) {
                    self.push(S::Callbackname);
                    self.advance(e);
                    return true;
                }
                if self.kw("classDef") {
                    self.emit_flat(T::CLASSDEF, p + 8);
                    return true;
                }
                if self.kw("class") {
                    self.emit_flat(T::CLASS, p + 5);
                    return true;
                }
                if let Some(e) = self.at(&RE_CLICK, p) {
                    self.push(S::Click);
                    self.advance(e);
                    return true;
                }
                if self.kw("connector") {
                    self.emit_flat(T::CONNECTOR, p + 9);
                    return true;
                }
            }
            34 => {
                self.quote();
                return true;
            }
            115 => {
                if self.kw("style") {
                    self.emit_flat(T::STYLE, p + 5);
                    return true;
                }
            }
            100 => {
                if self.kw("default") {
                    self.emit_flat(T::DEFAULT, p + 7);
                    return true;
                }
            }
            108 => {
                if self.kw("linkStyle") {
                    self.emit_flat(T::LINKSTYLE, p + 9);
                    return true;
                }
            }
            105 => {
                if self.kw("interpolate") {
                    self.emit_flat(T::INTERPOLATE, p + 11);
                    return true;
                }
            }
            104 => {
                if src[p..].starts_with("href") && is_space(self.code(p + 4)) {
                    self.emit(T::HREF, p + 4 + width_at(src, p + 4));
                    return true;
                }
            }
            102 => {
                if self.kw("flow") {
                    self.emit_flat(T::FLOW, p + 4);
                    return true;
                }
            }
            103 => {
                if self.kw("global") {
                    let e = self.blanks(p + 6);
                    self.emit_flat(T::GLOBAL, e);
                    return true;
                }
            }
            101 => {
                if self.kw("end") {
                    let e = self.blanks(p + 3);
                    self.emit_flat(T::END_KW, e);
                    return true;
                }
            }
            95 => {
                for (word, len) in [("_self", 5), ("_blank", 6), ("_parent", 7), ("_top", 4)] {
                    if self.kw(word) {
                        self.emit_flat(T::LINK_TARGET, p + len);
                        return true;
                    }
                }
            }
            _ => {}
        }

        if let Some(mut dir_next) = self.dir_next
            && p >= self.dir_failed
        {
            if dir_next < p {
                self.dir_next = src[p..].find("direction").map(|at| at + p);
            }
            if let Some(next) = self.dir_next {
                dir_next = next;
                if self.line_end.is_none_or(|end| end < p) {
                    self.line_end = Some(RE_LINE_END.exec_at(src, p).map_or(n, |m| m.start()));
                }
                let line_end = self.line_end.unwrap_or(n);
                if dir_next < line_end {
                    for (i, re) in RE_DIRECTION.iter().enumerate() {
                        if let Some(e) = self.at(re, p) {
                            self.emit(T::DIRECTION_TB + i, e);
                            return true;
                        }
                    }
                    // No direction statement starts anywhere in the rest of
                    // this line either.
                    self.dir_failed = line_end;
                }
            }
        }

        // A link id is a run of non-space characters up to its last `@` that
        // is not followed by `{` or `"`.
        if self.at_next.is_some() && c != 34 && !is_space(c) {
            if p >= self.run_end {
                if self.at_next.is_some_and(|at| at < p) {
                    self.at_next = self.find('@', p);
                }
                let mut q = p;
                while q < n {
                    let ch = self.code(q);
                    if ch == 34 || is_space(ch) {
                        break;
                    }
                    q += 1;
                }
                self.run_end = q;
                self.run_at = None;
                if let Some(at_next) = self.at_next
                    && at_next < q
                {
                    let bytes = src.as_bytes();
                    let mut k = q.min(n - 1) as isize - 1;
                    while k >= at_next as isize {
                        let at = k as usize;
                        if bytes[at] == b'@' {
                            let after = self.code(at + 1);
                            if after != 123 && after != 34 {
                                self.run_at = Some(at);
                                break;
                            }
                        }
                        k -= 1;
                    }
                }
            }
            if let Some(run_at) = self.run_at
                && run_at > p
            {
                self.emit_flat(T::LINK_ID, run_at + 1);
                return true;
            }
        }

        if (48..=57).contains(&c) {
            let mut q = p + 1;
            while q < n {
                let ch = self.code(q);
                if !(48..=57).contains(&ch) {
                    break;
                }
                q += 1;
            }
            self.emit_flat(T::NUM, q);
            return true;
        }

        match c {
            35 => {
                self.emit_flat(T::BRKT, p + 1);
                return true;
            }
            58 => {
                if self.code(p + 1) == 58 && self.code(p + 2) == 58 {
                    self.emit_flat(T::STYLE_SEPARATOR, p + 3);
                } else {
                    self.emit_flat(T::COLON, p + 1);
                }
                return true;
            }
            38 => {
                self.emit_flat(T::AMP, p + 1);
                return true;
            }
            59 => {
                self.emit_flat(T::SEMI, p + 1);
                return true;
            }
            44 => {
                self.emit_flat(T::COMMA, p + 1);
                return true;
            }
            42 => {
                self.emit_flat(T::MULT, p + 1);
                return true;
            }
            60 => {
                self.emit_flat(T::TAGSTART, p + 1);
                return true;
            }
            45 => {
                if self.link() || self.node_string() {
                    return true;
                }
                self.emit_flat(T::MINUS, p + 1);
                return true;
            }
            40 => return self.opener(c),
            91 => {
                if self.code(p + 1) == 124 {
                    self.emit_flat(T::VERTEX_WITH_PROPS_START, p + 2);
                    return true;
                }
                return self.opener(c);
            }
            124 | 123 => return self.opener(c),
            62 => {
                self.push(S::Text);
                self.emit_flat(T::TAGEND, p + 1);
                return true;
            }
            94 => {
                self.emit_flat(T::UP, p + 1);
                return true;
            }
            92 => {
                if self.code(p + 1) == 124 {
                    self.emit_flat(T::SEP, p + 2);
                    return true;
                }
                return self.node_string();
            }
            118 => {
                if !is_word(self.code(p + 1)) {
                    self.emit_flat(T::DOWN, p + 1);
                    return true;
                }
                return self.node_string();
            }
            37 => {
                if let Some(e) = self.comment_end(p) {
                    self.emit(T::COMMENT, e);
                    return true;
                }
                return self.node_string();
            }
            _ => {}
        }

        if is_space(c) {
            if self.link() {
                return true;
            }
            if c == 32 || c == 9 {
                let from = self.blank_end(p);
                if let Some(e) = self.comment_end(from) {
                    self.emit(T::COMMENT, e);
                    return true;
                }
            }
            let mut e = p;
            loop {
                let ch = self.code(e);
                if ch == 10 {
                    e += 1;
                } else if ch == 13 && self.code(e + 1) == 10 {
                    e += 2;
                } else {
                    break;
                }
            }
            if e > p {
                self.emit(T::NEWLINE, e);
            } else {
                self.emit(T::SPACE, p + width_at(src, p));
            }
            return true;
        }

        if c < 128 {
            return self.node_string();
        }
        if is_unicode_letter(c) {
            self.emit_flat(T::UNICODE_TEXT, p + width_at(src, p));
            return true;
        }
        false
    }

    fn run(mut self) -> Tokens {
        loop {
            let p = self.p;
            let n = self.n;
            if p >= n {
                let kind = match self.state {
                    S::Initial => Some(T::EOF),
                    S::AccTitle => Some(T::ACC_TITLE_VALUE),
                    S::AccDescr => Some(T::ACC_DESCR_VALUE),
                    S::AccDescrMultiline => Some(T::ACC_DESCR_MULTILINE_VALUE),
                    S::Click => Some(T::CLICK),
                    S::Callbackname => Some(T::CALLBACKNAME),
                    S::Callbackargs => Some(T::CALLBACKARGS),
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.emit_flat(kind, n);
                }
                break;
            }

            let src = self.src;
            let c = self.code(p);
            let mut ok = true;

            match self.state {
                S::Initial => ok = self.initial(c),

                S::String => {
                    if c == 34 {
                        if self.code(p + 1) == 96 {
                            self.push(S::MdString);
                            self.advance(p + 2);
                        } else {
                            self.pop();
                            self.advance(p + 1);
                        }
                    } else {
                        let e = self.find('"', p).unwrap_or(n);
                        self.emit(T::STR, e);
                    }
                }

                S::MdString => {
                    if c == 96 {
                        if self.code(p + 1) == 34 {
                            self.pop();
                            self.advance(p + 2);
                        } else {
                            ok = false;
                        }
                    } else if c == 34 {
                        self.quote();
                    } else {
                        let mut q = p + width_at(src, p);
                        while q < n {
                            let ch = self.code(q);
                            if ch == 96 || ch == 34 {
                                break;
                            }
                            q += 1;
                        }
                        self.emit(T::MD_STR, q);
                    }
                }

                S::AccTitle | S::AccDescr => {
                    let e = self.find('\n', p).unwrap_or(n);
                    let kind = if self.state == S::AccTitle {
                        T::ACC_TITLE_VALUE
                    } else {
                        T::ACC_DESCR_VALUE
                    };
                    self.pop();
                    self.emit(kind, e);
                }

                S::AccDescrMultiline => {
                    if c == 125 {
                        self.pop();
                        self.advance(p + 1);
                    } else {
                        let e = self.find('}', p).unwrap_or(n);
                        self.emit(T::ACC_DESCR_MULTILINE_VALUE, e);
                    }
                }

                S::Dir => {
                    if c == 34 {
                        self.quote();
                    } else if let Some(e) = self.at(&RE_NODIR, p) {
                        self.pop();
                        self.emit(T::NODIR, e);
                    } else if let Some(e) = self
                        .at(&RE_DIR, p)
                        .or_else(|| self.at(&RE_DIR_SYM, p))
                        .or_else(|| self.at(&RE_DIR_V, p))
                    {
                        self.pop();
                        self.emit(T::DIR, e);
                    } else {
                        ok = self.opener(c);
                    }
                }

                S::Text => {
                    let d = self.code(p + 1);
                    match c {
                        34 => self.quote(),
                        40 | 91 | 123 => {
                            self.opener(c);
                        }
                        93 => {
                            self.pop();
                            if d == 41 {
                                self.emit_flat(T::STADIUMEND, p + 2);
                            } else if d == 93 {
                                self.emit_flat(T::SUBROUTINEEND, p + 2);
                            } else {
                                self.emit_flat(T::SQE, p + 1);
                            }
                        }
                        41 => {
                            self.pop();
                            if d == 93 {
                                self.emit_flat(T::CYLINDEREND, p + 2);
                            } else if d == 41 && self.code(p + 2) == 41 {
                                self.emit_flat(T::DOUBLECIRCLEEND, p + 3);
                            } else {
                                self.emit_flat(T::PE, p + 1);
                            }
                        }
                        124 => {
                            self.pop();
                            self.emit_flat(T::PIPE, p + 1);
                        }
                        125 => {
                            self.pop();
                            self.emit_flat(T::DIAMOND_STOP, p + 1);
                        }
                        37 => {
                            // Jison drops the lone `%` from Mermaid's text
                            // pattern, so only a comment may start with one.
                            match self.comment_end(p) {
                                Some(e) => self.advance(e),
                                None => ok = false,
                            }
                        }
                        _ => {
                            let mut q = p + width_at(src, p);
                            while q < n {
                                let ch = self.code(q);
                                if matches!(ch, 91 | 93 | 40 | 41 | 123 | 125 | 124 | 34 | 37) {
                                    break;
                                }
                                q += 1;
                            }
                            self.emit(T::TEXT, q);
                        }
                    }
                }

                S::EllipseText => {
                    let d = self.code(p + 1);
                    if c == 34 {
                        self.quote();
                    } else if let Some(e) = (c == 37).then(|| self.comment_end(p)).flatten() {
                        self.advance(e);
                    } else if (c == 45 || c == 47 || c == 41) && d == 41 {
                        self.pop();
                        self.emit_flat(T::ELLIPSE_END, p + 2);
                    } else if c == 40 || c == 91 || c == 123 {
                        self.opener(c);
                    } else if c == 41 || c == 93 || c == 125 {
                        ok = false;
                    } else {
                        let mut q = p + width_at(src, p);
                        while q < n {
                            let ch = self.code(q);
                            if matches!(ch, 34 | 40 | 41 | 91 | 93 | 123 | 125) {
                                break;
                            }
                            if (ch == 45 || ch == 47) && self.code(q + 1) == 41 {
                                break;
                            }
                            if ch == 37 && self.code(q + 1) == 37 && self.code(q + 2) != 123 {
                                break;
                            }
                            q += 1;
                        }
                        self.emit_chars(T::TEXT, q);
                    }
                }

                S::TrapText => 'trap: {
                    let d = self.code(p + 1);
                    if c == 34 {
                        self.quote();
                        break 'trap;
                    }
                    match c {
                        40 => {
                            if d == 45 || d == 91 || (d == 40 && self.code(p + 2) == 40) {
                                self.opener(c);
                            } else if d == 93 {
                                self.pop();
                                self.emit_flat(T::TRAPEND, p + 2);
                            } else {
                                self.opener(c);
                            }
                        }
                        91 | 123 => {
                            self.opener(c);
                        }
                        92 => {
                            if d == 93 {
                                self.pop();
                                self.emit_flat(T::TRAPEND, p + 2);
                            } else {
                                self.emit_flat(T::TEXT, p + 1);
                            }
                        }
                        47 => {
                            if d == 93 {
                                self.pop();
                                self.emit_flat(T::INVTRAPEND, p + 2);
                            } else {
                                self.emit_flat(T::TEXT, p + 1);
                            }
                        }
                        93 | 41 => {
                            if d == 93 {
                                self.pop();
                                self.emit_flat(T::TRAPEND, p + 2);
                            } else {
                                ok = false;
                            }
                        }
                        125 => ok = false,
                        37 => match self.comment_end(p) {
                            Some(e) => self.advance(e),
                            None => ok = false,
                        },
                        _ => {
                            if (c == 63 || c == 61) && d == 93 {
                                self.pop();
                                self.emit_flat(T::TRAPEND, p + 2);
                                break 'trap;
                            }
                            let mut q = p + width_at(src, p);
                            while q < n {
                                let ch = self.code(q);
                                if matches!(ch, 92 | 91 | 93 | 40 | 41 | 123 | 125 | 47 | 37) {
                                    break;
                                }
                                q += 1;
                            }
                            self.emit(T::TEXT, q);
                        }
                    }
                }

                S::EdgeText => 'edge: {
                    if c == 34 {
                        self.quote();
                        break 'edge;
                    }
                    if let Some(e) = self.arrow_end(p) {
                        self.pop();
                        self.emit(T::LINK, e);
                        break 'edge;
                    }
                    if c == 45 && self.code(p + 1) == 45 {
                        ok = false;
                        break 'edge;
                    }
                    let mut q = p + width_at(src, p);
                    while q < n {
                        let ch = self.code(q);
                        if ch == 34 {
                            break;
                        }
                        if ch == 45 {
                            if self.code(q + 1) == 45 {
                                break;
                            }
                        } else if is_space(ch) && self.arrow_end(q).is_some() {
                            break;
                        }
                        q += 1;
                    }
                    self.emit_chars(T::EDGE_TEXT, q);
                }

                S::Click => {
                    if c == 34 {
                        self.quote();
                    } else if is_space(c) {
                        self.pop();
                        self.advance(p + width_at(src, p));
                    } else {
                        let mut q = p + width_at(src, p);
                        while q < n && !is_space(self.code(q)) {
                            q += 1;
                        }
                        self.emit_flat(T::CLICK, q);
                    }
                }

                S::Callbackname => {
                    if c == 40 {
                        let e = self.at(&RE_EMPTY_CALL, p);
                        self.pop();
                        match e {
                            Some(e) => self.advance(e),
                            None => {
                                self.push(S::Callbackargs);
                                self.advance(p + 1);
                            }
                        }
                    } else {
                        let e = self.find('(', p).unwrap_or(n);
                        self.emit(T::CALLBACKNAME, e);
                    }
                }

                S::Callbackargs => {
                    if c == 41 {
                        self.pop();
                        self.advance(p + 1);
                    } else {
                        let e = self.find(')', p).unwrap_or(n);
                        self.emit(T::CALLBACKARGS, e);
                    }
                }

                S::ShapeData => {
                    if c == 34 {
                        self.push(S::ShapeDataStr);
                        self.emit_flat(T::SHAPE_DATA, p + 1);
                    } else if c == 125 {
                        self.pop();
                        self.advance(p + 1);
                    } else if c == 94 {
                        ok = false;
                    } else {
                        let mut q = p + width_at(src, p);
                        while q < n {
                            let ch = self.code(q);
                            if ch == 125 || ch == 94 || ch == 34 {
                                break;
                            }
                            q += 1;
                        }
                        self.emit(T::SHAPE_DATA, q);
                    }
                }

                S::ShapeDataStr => {
                    if c == 34 {
                        self.pop();
                        self.emit_flat(T::SHAPE_DATA, p + 1);
                    } else {
                        let e = self.find('"', p).unwrap_or(n);
                        let text = RE_BR.replace(&src[p..e], |_| "<br/>".into());
                        self.emit_text(T::SHAPE_DATA, text, e);
                    }
                }
            }

            if !ok {
                self.out.types.push(T::ERROR);
                self.out.texts.push(String::new());
                self.out.starts.push(self.p);
                self.out
                    .locs
                    .extend([self.line, self.col, self.line, self.col]);
                return self.out;
            }
        }

        self.out.types.push(T::END);
        self.out.texts.push(String::new());
        self.out.starts.push(self.n);
        self.out
            .locs
            .extend([self.line, self.col, self.line, self.col]);
        self.out
    }
}

pub fn tokenize(src: &str) -> Tokens {
    Lexer {
        src,
        n: src.len(),
        out: Tokens {
            types: Vec::new(),
            texts: Vec::new(),
            starts: Vec::new(),
            locs: Vec::new(),
        },
        stack: Vec::new(),
        state: S::Initial,
        p: 0,
        line: 1,
        col: 0,
        first_graph: true,
        dir_next: src.find("direction"),
        at_next: src.find('@'),
        line_end: None,
        dir_failed: 0,
        run_end: 0,
        run_at: None,
        ws: None,
        blank: None,
        no_link: None,
        no_arrow: None,
    }
    .run()
}
