//! Pele's `diagrams/sequence/lexer.ts`.

use super::tokens::*;
use crate::{
    js,
    util::chars::{code_at, is_digit, is_space, is_word, width_at},
};

const S_INITIAL: u8 = 0;
const S_ID: u8 = 1;
const S_ALIAS: u8 = 2;
const S_LINE: u8 = 3;
const S_CONFIG: u8 = 4;
const S_ACC_TITLE: u8 = 5;
const S_ACC_DESCR: u8 = 6;
const S_ACC_DESCR_MULTILINE: u8 = 7;

const C_ACTOR_START: u8 = 1;
const C_ACTOR_REST: u8 = 2;
const C_ID: u8 = 4;
const C_ID_WIDE: u8 = 8;
const C_LINE: u8 = 16;
const C_MENU: u8 = 32;

/// Character classes of Mermaid's lexer rules, for ASCII. Other characters
/// are tested by `is_space`.
const CLASS: [u8; 128] = {
    const fn has(set: &[u8], c: u8) -> bool {
        let mut k = 0;
        while k < set.len() {
            if set[k] == c {
                return true;
            }
            k += 1;
        }
        false
    }
    let mut table = [0u8; 128];
    let mut c = 0u8;
    while c < 128 {
        let ws = c == 32 || (c >= 9 && c <= 13);
        let mut bits = 0;
        if !has(b"/\\+()<->:\n,;", c) {
            bits |= C_ACTOR_START;
        }
        if !has(b"+<->:\n,;", c) {
            bits |= C_ACTOR_REST;
        }
        if !has(b"<>:\n,;@", c) {
            bits |= C_ID_WIDE | if ws { 0 } else { C_ID };
        }
        if !has(b"#\n;", c) {
            bits |= C_LINE;
        }
        if !has(b"/\\+()<>:\n,;-", c) && !ws {
            bits |= C_MENU;
        }
        table[c as usize] = bits;
        c += 1;
    }
    table
};

#[derive(Default)]
pub struct Tokens {
    pub types: Vec<u8>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
}

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    out: Tokens,
    stack: Vec<u8>,
    state: u8,
    p: usize,
    next_nl: Option<usize>,
}

impl<'s> Lexer<'s> {
    fn c(&self, q: usize) -> u32 {
        code_at(self.src, q)
    }

    /// The byte after the character at `q`.
    fn after(&self, q: usize) -> usize {
        q + width_at(self.src, q)
    }

    fn emit(&mut self, kind: u8, s: usize, e: usize) {
        self.out.types.push(kind);
        self.out.texts.push(self.src[s..e].to_string());
        self.out.starts.push(s);
        self.p = e;
    }

    fn emit_text(&mut self, kind: u8, s: usize, e: usize, text: String) {
        self.out.types.push(kind);
        self.out.texts.push(text);
        self.out.starts.push(s);
        self.p = e;
    }

    fn push(&mut self, s: u8) {
        self.stack.push(self.state);
        self.state = s;
    }

    fn pop(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.state = state;
        }
    }

    fn eol(&mut self, q: usize) -> usize {
        if self.next_nl.is_none_or(|nl| nl < q) {
            self.next_nl = Some(self.src[q..].find('\n').map_or(self.n, |at| q + at));
        }
        self.next_nl.unwrap_or(self.n)
    }

    fn has(&self, q: usize, bit: u8) -> bool {
        let c = self.c(q);
        if c < 128 {
            return CLASS[c as usize] & bit != 0;
        }
        if bit == C_ID || bit == C_MENU {
            !is_space(c)
        } else {
            true
        }
    }

    /// Matches a lowercase literal without regard to the case of the source.
    fn ci(&self, q: usize, word: &str) -> bool {
        let bytes = self.src.as_bytes();
        word.bytes().enumerate().all(|(k, w)| {
            bytes.get(q + k).is_some_and(|&c| {
                if w.is_ascii_lowercase() {
                    c | 32 == w
                } else {
                    c == w
                }
            })
        })
    }

    fn word(&self, lit: &str) -> bool {
        self.ci(self.p, lit) && !is_word(self.c(self.p + lit.len()))
    }

    fn skip_ws(&self, mut q: usize) -> usize {
        while q < self.n && is_space(self.c(q)) {
            q = self.after(q);
        }
        q
    }

    fn skip_same_line(&mut self) -> bool {
        let mut q = self.p;
        while q < self.n {
            let c = self.c(q);
            if c == 10 || !is_space(c) {
                break;
            }
            q = self.after(q);
        }
        if q == self.p {
            return false;
        }
        self.p = q;
        true
    }

    fn line_run(&self, mut q: usize) -> usize {
        while q < self.n && self.has(q, C_LINE) {
            q = self.after(q);
        }
        q
    }

    fn begin(&mut self, kind: u8, len: usize, next: u8) {
        self.emit(kind, self.p, self.p + len);
        self.push(next);
    }

    fn keyword(&mut self, lit: &str, kind: u8, next: Option<u8>) -> bool {
        if !self.word(lit) {
            return false;
        }
        self.emit(kind, self.p, self.p + lit.len());
        if let Some(next) = next {
            self.push(next);
        }
        true
    }

    /// The actor menu keywords count only when spaces and the start of a name
    /// follow.
    fn menu(&mut self, lit: &str, kind: u8) -> bool {
        if !self.ci(self.p, lit) {
            return false;
        }
        let mut q = self.p + lit.len();
        let mut c = self.c(q);
        if c != 32 && c != 9 {
            return false;
        }
        while c == 32 || c == 9 {
            q += 1;
            c = self.c(q);
        }
        if q >= self.n || !self.has(q, C_MENU) {
            return false;
        }
        self.emit(kind, self.p, self.p + lit.len());
        true
    }

    fn arrow_ahead(&self, q: usize) -> bool {
        let c = self.c(q);
        if c == 45 {
            let d = self.c(q + 1);
            if matches!(d, 120 | 88 | 41 | 92 | 47 | 45) {
                return true;
            }
            let e = self.c(q + 2);
            return d == 124 && (e == 92 || e == 47);
        }
        if c == 47 || c == 92 {
            let d = self.c(q + 1);
            return (d == 124 || d == c) && self.c(q + 2) == 45;
        }
        c == 40 && self.c(q + 1) == 41
    }

    fn actor(&mut self) {
        let p = self.p;
        let mut q = self.after(p);
        while q < self.n && self.has(q, C_ACTOR_START) {
            q = self.after(q);
        }
        while q < self.n && !self.arrow_ahead(q) {
            let mut r = q;
            while self.c(r) == 45 {
                r += 1;
            }
            if r >= self.n || !self.has(r, C_ACTOR_REST) {
                break;
            }
            r = self.after(r);
            while r < self.n && self.has(r, C_ACTOR_REST) {
                r = self.after(r);
            }
            q = r;
        }
        let text = js::trim(&self.src[p..q]).to_string();
        self.emit_text(ACTOR, p, q, text);
    }

    fn number(&mut self) -> bool {
        let p = self.p;
        let mut q = p;
        while is_digit(self.c(q)) {
            q += 1;
        }
        let whole = q > p;
        if self.c(q) == 46 && is_digit(self.c(q + 1)) {
            let r = if is_digit(self.c(q + 2)) {
                q + 3
            } else {
                q + 2
            };
            let after = self.c(r);
            if after == 32 || after == 10 {
                self.emit(NUM, p, r);
                return true;
            }
        }
        let after = self.c(q);
        if whole && (after == 32 || after == 10) {
            self.emit(NUM, p, q);
            return true;
        }
        false
    }

    fn dash(&mut self) {
        let p = self.p;
        let d = self.c(p + 1);
        let e = self.c(p + 2);
        if d == 62 {
            if e == 62 {
                self.emit(SOLID_ARROW, p, p + 3);
            } else {
                self.emit(SOLID_OPEN_ARROW, p, p + 2);
            }
        } else if d == 120 || d == 88 {
            self.emit(SOLID_CROSS, p, p + 2);
        } else if d == 41 {
            self.emit(SOLID_POINT, p, p + 2);
        } else if d == 124 && e == 92 {
            self.emit(SOLID_ARROW_TOP, p, p + 3);
        } else if d == 124 && e == 47 {
            self.emit(SOLID_ARROW_BOTTOM, p, p + 3);
        } else if d == 92 && e == 92 {
            self.emit(STICK_ARROW_TOP, p, p + 3);
        } else if d == 47 && e == 47 {
            self.emit(STICK_ARROW_BOTTOM, p, p + 3);
        } else if d == 45 {
            let f = self.c(p + 3);
            if e == 62 {
                if f == 62 {
                    self.emit(DOTTED_ARROW, p, p + 4);
                } else {
                    self.emit(DOTTED_OPEN_ARROW, p, p + 3);
                }
            } else if e == 120 || e == 88 {
                self.emit(DOTTED_CROSS, p, p + 3);
            } else if e == 41 {
                self.emit(DOTTED_POINT, p, p + 3);
            } else if e == 124 && f == 92 {
                self.emit(SOLID_ARROW_TOP_DOTTED, p, p + 4);
            } else if e == 124 && f == 47 {
                self.emit(SOLID_ARROW_BOTTOM_DOTTED, p, p + 4);
            } else if e == 92 && f == 92 {
                self.emit(STICK_ARROW_TOP_DOTTED, p, p + 4);
            } else if e == 47 && f == 47 {
                self.emit(STICK_ARROW_BOTTOM_DOTTED, p, p + 4);
            } else {
                self.emit(MINUS, p, p + 1);
            }
        } else {
            self.emit(MINUS, p, p + 1);
        }
    }

    fn reverse(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.c(p + 1);
        if (d != 124 && d != c) || self.c(p + 2) != 45 {
            return false;
        }
        let dotted = self.c(p + 3) == 45;
        let top = c == 47;
        let kind = if d == 124 {
            match (dotted, top) {
                (true, true) => SOLID_ARROW_TOP_REVERSE_DOTTED,
                (true, false) => SOLID_ARROW_BOTTOM_REVERSE_DOTTED,
                (false, true) => SOLID_ARROW_TOP_REVERSE,
                (false, false) => SOLID_ARROW_BOTTOM_REVERSE,
            }
        } else {
            match (dotted, top) {
                (true, true) => STICK_ARROW_TOP_REVERSE_DOTTED,
                (true, false) => STICK_ARROW_BOTTOM_REVERSE_DOTTED,
                (false, true) => STICK_ARROW_TOP_REVERSE,
                (false, false) => STICK_ARROW_BOTTOM_REVERSE,
            }
        };
        self.emit(kind, p, p + if dotted { 4 } else { 3 });
        true
    }

    fn title(&mut self) -> bool {
        if !self.ci(self.p, "title") {
            return false;
        }
        let mut q = self.p + 5;
        let legacy = self.c(q) == 58;
        if legacy {
            q += 1;
        }
        if !is_space(self.c(q)) {
            return false;
        }
        let next = self.after(q);
        if next >= self.n || !self.has(next, C_LINE) {
            return false;
        }
        let end = self.line_run(next);
        self.emit(if legacy { LEGACY_TITLE } else { TITLE }, self.p, end);
        true
    }

    fn acc(&mut self) -> bool {
        let descr = self.ci(self.p, "accdescr");
        if !descr && !self.ci(self.p, "acctitle") {
            return false;
        }
        let q = self.skip_ws(self.p + 8);
        let c = self.c(q);
        if c == 58 {
            let end = self.skip_ws(q + 1);
            self.emit(if descr { ACC_DESCR } else { ACC_TITLE }, self.p, end);
            self.push(if descr { S_ACC_DESCR } else { S_ACC_TITLE });
            return true;
        }
        if descr && c == 123 {
            self.p = self.skip_ws(q + 1);
            self.push(S_ACC_DESCR_MULTILINE);
            return true;
        }
        false
    }

    fn keywords(&mut self, c: u32) -> bool {
        match c | 32 {
            97 => {
                self.keyword("actor", PARTICIPANT_ACTOR, Some(S_ID))
                    || self.keyword("alt", ALT, Some(S_LINE))
                    || self.keyword("and", AND, Some(S_LINE))
                    || self.keyword("activate", ACTIVATE, Some(S_ID))
                    || self.acc()
                    || self.keyword("autonumber", AUTONUMBER, None)
            }
            98 => {
                self.keyword("box", BOX, Some(S_LINE)) || self.keyword("break", BREAK, Some(S_LINE))
            }
            99 => {
                self.keyword("create", CREATE, None)
                    || self.keyword("critical", CRITICAL, Some(S_LINE))
            }
            100 => {
                self.keyword("destroy", DESTROY, Some(S_ID))
                    || self.menu("details", DETAILS)
                    || self.keyword("deactivate", DEACTIVATE, Some(S_ID))
            }
            101 => self.keyword("else", ELSE, Some(S_LINE)) || self.keyword("end", END, None),
            108 => {
                self.keyword("loop", LOOP, Some(S_LINE))
                    || self.keyword("left of", LEFT_OF, None)
                    || self.menu("links", LINKS)
                    || self.menu("link", LINK)
            }
            110 => self.keyword("note", NOTE, None),
            111 => {
                self.keyword("opt", OPT, Some(S_LINE))
                    || self.keyword("option", OPTION, Some(S_LINE))
                    || self.keyword("over", OVER, None)
                    || self.keyword("off", OFF, None)
            }
            112 => {
                self.keyword("participant", PARTICIPANT, Some(S_ID))
                    || self.keyword("par", PAR, Some(S_LINE))
                    || self.keyword("par_over", PAR_OVER, Some(S_LINE))
                    || self.menu("properties", PROPERTIES)
            }
            114 => {
                self.keyword("rect", RECT, Some(S_LINE)) || self.keyword("right of", RIGHT_OF, None)
            }
            115 => self.keyword("sequencediagram", SD, None),
            116 => self.title(),
            _ => false,
        }
    }

    fn initial(&mut self) {
        let p = self.p;
        let c = self.c(p);
        if c == 10 {
            let mut q = p + 1;
            while self.c(q) == 10 {
                q += 1;
            }
            self.emit(NEWLINE, p, q);
            return;
        }
        if is_space(c) {
            self.p = self.skip_ws(self.after(p));
            return;
        }
        if c == 35 || (c == 37 && self.c(p + 1) != 123) {
            self.p = self.eol(p);
            return;
        }
        // An astral character's second unit is no `%`.
        if c != 125 && width_at(self.src, p) < 4 {
            let next = self.after(p);
            if self.c(next) == 37 && self.c(next + 1) == 37 {
                self.p = self.eol(p);
                return;
            }
        }
        if (is_digit(c) || c == 46) && self.number() {
            return;
        }
        if c < 128 && (c | 32) >= 97 && (c | 32) <= 122 && self.keywords(c) {
            return;
        }
        if c == 44 {
            self.emit(COMMA, p, p + 1);
        } else if c == 59 {
            self.emit(NEWLINE, p, p + 1);
        } else if self.has(p, C_ACTOR_START) {
            self.actor();
        } else if c == 45 {
            self.dash();
        } else if c == 60 && self.src[p..].starts_with("<<->>") {
            self.emit(BIDIRECTIONAL_SOLID_ARROW, p, p + 5);
        } else if c == 60 && self.src[p..].starts_with("<<-->>") {
            self.emit(BIDIRECTIONAL_DOTTED_ARROW, p, p + 6);
        } else if (c == 47 || c == 92) && self.reverse(c) {
        } else if c == 58 {
            let end = self.line_run(p + 1);
            self.emit(TXT, p, end);
        } else if c == 43 {
            self.emit(PLUS, p, p + 1);
        } else if c == 40 && self.c(p + 1) == 41 {
            self.emit(CENTRAL, p, p + 2);
        } else {
            self.emit(INVALID, p, p + 1);
        }
    }

    /// A participant name, up to an alias, a config object, or the end of the
    /// statement.
    fn id(&mut self) -> bool {
        if self.skip_same_line() {
            return true;
        }
        let p = self.p;
        let n = self.n;
        let c = self.c(p);
        if c == 35 {
            self.p = self.eol(p);
            return true;
        }
        if c == 64 && self.c(p + 1) == 123 {
            self.begin(CONFIG_START, 2, S_CONFIG);
            return true;
        }
        let mut a = p;
        while a < n && self.has(a, C_ID) {
            a = self.after(a);
        }
        if a > p {
            let w = self.skip_ws(a);
            if self.c(w) == 64 && self.c(w + 1) == 123 {
                self.emit(ACTOR, p, a);
                return true;
            }
            if w > a && self.ci(w, "as") && is_space(self.c(w + 2)) {
                self.emit(ACTOR, p, a);
                self.push(S_ALIAS);
                return true;
            }
        }
        let mut b = a;
        while b < n && self.has(b, C_ID_WIDE) {
            b = self.after(b);
        }
        if b > p {
            let end = self.c(b);
            // Before any other delimiter the name must be followed by a
            // comment, so it ends at the last one.
            let mut cut = b;
            if b < n && end != 10 && end != 59 {
                cut -= 1;
                while cut > p && self.src.as_bytes()[cut] != b'#' {
                    cut -= 1;
                }
            }
            if cut > p {
                let text = js::trim(&self.src[p..cut]).to_string();
                self.emit_text(ACTOR, p, cut, text);
                self.pop();
                return true;
            }
        }
        if self.c(b) == 60 {
            let e = self.eol(p);
            self.emit(INVALID, p, e);
            self.pop();
            return true;
        }
        if c != 10 {
            let e = self.eol(p);
            let text = js::trim(&self.src[p..e]).to_string();
            self.emit_text(INVALID, p, e, text);
            self.pop();
            return true;
        }
        false
    }

    fn step(&mut self) -> bool {
        let p = self.p;
        match self.state {
            S_INITIAL => {
                self.initial();
                true
            }
            S_ID => self.id(),
            S_ALIAS => {
                if self.skip_same_line() {
                    return true;
                }
                if self.c(p) == 35 {
                    self.p = self.eol(p);
                    return true;
                }
                self.pop();
                self.pop();
                if self.word("as") {
                    self.begin(AS, 2, S_LINE);
                } else {
                    self.emit(NEWLINE, p, p);
                }
                true
            }
            S_LINE => {
                if self.skip_same_line() {
                    return true;
                }
                let p = self.p;
                if self.c(p) == 35 {
                    self.p = self.eol(p);
                    return true;
                }
                let end = self.line_run(p);
                self.emit(REST_OF_LINE, p, end);
                self.pop();
                true
            }
            S_CONFIG => {
                if self.c(p) != 125 {
                    let q = self.src[p..].find('}').map_or(self.n, |at| p + at);
                    self.emit(CONFIG_CONTENT, p, q);
                } else {
                    let w = self.skip_ws(p + 1);
                    let alias = w > p + 1 && self.ci(w, "as") && is_space(self.c(w + 2));
                    self.emit(CONFIG_END, p, p + 1);
                    self.pop();
                    if alias {
                        self.push(S_ALIAS);
                    } else {
                        self.pop();
                    }
                }
                true
            }
            S_ACC_TITLE | S_ACC_DESCR => {
                let kind = if self.state == S_ACC_TITLE {
                    ACC_TITLE_VALUE
                } else {
                    ACC_DESCR_VALUE
                };
                let e = self.eol(p);
                self.emit(kind, p, e);
                self.pop();
                true
            }
            _ => {
                if self.c(p) == 125 {
                    self.p += 1;
                    self.pop();
                } else {
                    let q = self.src[p..].find('}').map_or(self.n, |at| p + at);
                    self.emit(ACC_DESCR_MULTILINE_VALUE, p, q);
                }
                true
            }
        }
    }
}

pub fn tokenize(src: &str) -> Tokens {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        out: Tokens::default(),
        stack: Vec::new(),
        state: S_INITIAL,
        p: 0,
        next_nl: None,
    };
    while lx.p < n {
        if !lx.step() {
            lx.out.types.push(ERROR);
            lx.out.texts.push(String::new());
            lx.out.starts.push(lx.p);
            return lx.out;
        }
    }

    // Mermaid's lexer tries its rules once more on the empty remainder before
    // reporting the end.
    let at_end = match lx.state {
        S_INITIAL | S_ALIAS => Some(NEWLINE),
        S_LINE => Some(REST_OF_LINE),
        S_ACC_TITLE => Some(ACC_TITLE_VALUE),
        S_ACC_DESCR => Some(ACC_DESCR_VALUE),
        S_ACC_DESCR_MULTILINE => Some(ACC_DESCR_MULTILINE_VALUE),
        _ => None,
    };
    if let Some(kind) = at_end {
        lx.emit(kind, n, n);
    }
    lx.out.types.push(EOF);
    lx.out.texts.push(String::new());
    lx.out.starts.push(n);
    lx.out
}
