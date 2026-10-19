//! Pele's `diagrams/state/lexer.ts`: the tokens of Mermaid's state grammar,
//! lexed by hand.

use super::tokens::*;
use crate::{
    js::{self, Lazy},
    util::chars::{code_at, is_space, is_word, width_at},
};

static RE_DIRECTION: Lazy = Lazy::new(r"direction", "gi");
static RE_STEREOTYPE: Lazy = Lazy::new(
    r"<<(?:fork|join|choice)>>|\[\[(?:fork|join|choice)\]\]",
    "gi",
);

pub struct Tokens {
    pub types: Vec<u8>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
    /// Set when lexing stopped on the grammar's own error rather than on
    /// unrecognized text.
    pub error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Struct,
    State,
    StateString,
    StateId,
    Scale,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Classdef,
    ClassdefId,
    Class,
    ClassStyle,
    Style,
    StyledefStyles,
    Note,
    NoteId,
    NoteText,
    FloatingNote,
    FloatingNoteId,
}

/// Finds the next match of a pattern at or after a position, remembering the
/// answer so that asking at each token costs one scan of the text in total.
struct Finder<'s> {
    src: &'s str,
    search: fn(&str, usize) -> Option<usize>,
    from: usize,
    /// -2 before the first scan, -1 for no match.
    next: i64,
}

impl<'s> Finder<'s> {
    fn new(src: &'s str, search: fn(&str, usize) -> Option<usize>) -> Self {
        Self {
            src,
            search,
            from: 0,
            next: -2,
        }
    }

    fn at(&mut self, q: usize) -> i64 {
        if self.next == -2 || q < self.from || (self.next != -1 && q as i64 > self.next) {
            self.next = (self.search)(self.src, q).map_or(-1, |at| at as i64);
            self.from = q;
        }
        self.next
    }
}

fn find_direction(src: &str, q: usize) -> Option<usize> {
    RE_DIRECTION.exec_at(src, q).map(|m| m.start())
}

fn find_stereotype(src: &str, q: usize) -> Option<usize> {
    RE_STEREOTYPE.exec_at(src, q).map(|m| m.start())
}

fn find_line_end(src: &str, q: usize) -> Option<usize> {
    src.get(q..)?
        .find(['\n', '\r', '\u{2028}', '\u{2029}'])
        .map(|at| q + at)
}

fn find_percent(src: &str, q: usize) -> Option<usize> {
    src.get(q..)?.find("%%").map(|at| q + at)
}

fn find_brace(src: &str, q: usize) -> Option<usize> {
    src.get(q..)?.find('{').map(|at| q + at)
}

/// The byte `units` UTF-16 units after `at`, a whole character at a time.
fn skip_units(src: &str, at: usize, units: usize) -> usize {
    let mut p = at;
    let mut left = units;
    while left > 0 && p < src.len() {
        let width = width_at(src, p);
        left = left.saturating_sub(if width == 4 { 2 } else { 1 });
        p += width;
    }
    p
}

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    types: Vec<u8>,
    texts: Vec<String>,
    starts: Vec<usize>,
    stack: Vec<S>,
    state: S,
    p: usize,
    error: Option<String>,
    next_direction: Finder<'s>,
    next_stereotype: Finder<'s>,
    next_line_end: Finder<'s>,
    next_percent: Finder<'s>,
    next_brace: Finder<'s>,
    // The last `direction <dir>` of each kind on the line being lexed, and
    // where its keyword ends.
    dir_from: i64,
    dir_to: i64,
    dir_at: [i64; 4],
    dir_end: [usize; 4],
    // The same for <<fork>>, <<join>>, <<choice>> and their [[ ]] spellings.
    stereo_from: i64,
    stereo_to: i64,
    stereo_at: [i64; 6],
}

impl<'s> Lexer<'s> {
    fn code(&self, at: usize) -> u32 {
        code_at(self.src, at)
    }

    fn emit(&mut self, kind: u8, s: usize, e: usize) {
        self.types.push(kind);
        self.texts.push(self.src[s..e].to_string());
        self.starts.push(s);
        self.p = e;
    }

    fn emit_text(&mut self, kind: u8, text: String, s: usize, e: usize) {
        self.types.push(kind);
        self.texts.push(text);
        self.starts.push(s);
        self.p = e;
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

    /// Case-insensitive match of a lowercase ASCII word, as the grammar's /i
    /// flag gives.
    fn word(&self, w: &str, q: usize) -> bool {
        let bytes = self.src.as_bytes();
        w.bytes().enumerate().all(|(i, a)| match bytes.get(q + i) {
            Some(&c) => c == a || (a.is_ascii_lowercase() && c == a - 32),
            None => false,
        })
    }

    fn ws_end(&self, q: usize) -> usize {
        let mut j = q;
        while j < self.n && is_space(self.code(j)) {
            j += width_at(self.src, j);
        }
        j
    }

    fn word_end(&self, q: usize) -> usize {
        let mut j = q;
        while j < self.n && is_word(self.code(j)) {
            j += 1;
        }
        j
    }

    fn eol(&self, q: usize) -> usize {
        self.src
            .get(q..)
            .and_then(|rest| rest.find('\n'))
            .map_or(self.n, |k| q + k)
    }

    fn line_end(&mut self, q: usize) -> usize {
        let k = self.next_line_end.at(q);
        if k == -1 { self.n } else { k as usize }
    }

    /// Rules `.*direction\s+TB[^\n]*` and its three siblings: the first kind,
    /// in grammar order, found between q and the end of its line.
    fn direction(&mut self, q: usize) -> i64 {
        let qi = q as i64;
        if qi < self.dir_from || qi > self.dir_to {
            let first = self.next_direction.at(q);
            let end = if first == -1 {
                self.n
            } else {
                self.line_end(q)
            };
            self.dir_from = qi;
            self.dir_to = end as i64;
            self.dir_at = [-1; 4];
            let mut d = first;
            while d != -1 && (d as usize) < end {
                let du = d as usize;
                let w = self.ws_end(du + 9);
                if w != du + 9 {
                    let kind = if self.word("tb", w) {
                        0
                    } else if self.word("bt", w) {
                        1
                    } else if self.word("rl", w) {
                        2
                    } else if self.word("lr", w) {
                        3
                    } else {
                        -1
                    };
                    if kind != -1 {
                        self.dir_at[kind as usize] = d;
                        self.dir_end[kind as usize] = w + 2;
                    }
                }
                d = self.next_direction.at(du + 1);
            }
        }
        (0..4)
            .find(|&kind| self.dir_at[kind] >= qi)
            .map_or(-1, |kind| kind as i64)
    }

    fn stereotype(&mut self, q: usize) -> i64 {
        let qi = q as i64;
        if qi < self.stereo_from || qi > self.stereo_to {
            let first = self.next_stereotype.at(q);
            let end = if first == -1 {
                self.n
            } else {
                self.line_end(q)
            };
            self.stereo_from = qi;
            self.stereo_to = end as i64;
            self.stereo_at = [-1; 6];
            let mut d = first;
            while d != -1 && (d as usize) < end {
                let du = d as usize;
                let letter = self.code(du + 2) | 32;
                let base = if self.code(du) == 60 { 0 } else { 3 };
                let kind = if letter == 102 {
                    0
                } else if letter == 106 {
                    1
                } else {
                    2
                };
                self.stereo_at[base + kind] = d;
                d = self.next_stereotype.at(du + 1);
            }
        }
        (0..6)
            .find(|&kind| self.stereo_at[kind] >= qi)
            .map_or(-1, |kind| kind as i64)
    }

    /// The grammar's processId(): an id ends at an inline `%%`, and a text
    /// that starts with one is dropped.
    fn emit_id(&mut self, s: usize, e: usize) -> bool {
        let at = self.next_percent.at(s);
        if at == -1 || at as usize + 2 > e {
            self.emit(ID, s, e);
            return true;
        }
        let at = at as usize;
        if at == s {
            self.p = e;
            return false;
        }
        self.emit(ID, s, at);
        true
    }

    fn finish(mut self, kind: u8) -> Tokens {
        self.types.push(kind);
        self.texts.push(String::new());
        self.starts.push(self.p);
        Tokens {
            types: self.types,
            texts: self.texts,
            starts: self.starts,
            error: self.error,
        }
    }

    fn is_comment(&self, c: u32) -> bool {
        let p = self.p;
        c == 35 || (c == 37 && self.code(p + 1) == 37 && self.code(p + 2) != 123)
    }
}

pub fn tokenize(src: &str) -> Tokens {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        types: Vec::new(),
        texts: Vec::new(),
        starts: Vec::new(),
        stack: Vec::new(),
        state: S::Initial,
        p: 0,
        error: None,
        next_direction: Finder::new(src, find_direction),
        next_stereotype: Finder::new(src, find_stereotype),
        next_line_end: Finder::new(src, find_line_end),
        next_percent: Finder::new(src, find_percent),
        next_brace: Finder::new(src, find_brace),
        dir_from: -1,
        dir_to: -1,
        dir_at: [-1; 4],
        dir_end: [0; 4],
        stereo_from: -1,
        stereo_to: -1,
        stereo_at: [-1; 6],
    };

    loop {
        let p = lx.p;
        if p >= n {
            // Jison tries the current state's rules once more on the empty
            // remainder.
            match lx.state {
                S::Initial => lx.emit(NL, n, n),
                S::AccTitle => lx.emit(ACC_TITLE_VALUE, n, n),
                S::AccDescr => lx.emit(ACC_DESCR_VALUE, n, n),
                S::AccDescrMultiline => lx.emit(ACC_DESCR_MULTILINE_VALUE, n, n),
                S::ClassdefId => lx.emit(CLASSDEF_STYLEOPTS, n, n),
                S::ClassStyle => lx.emit(STYLECLASS, n, n),
                S::StyledefStyles => lx.emit(STYLEDEF_STYLEOPTS, n, n),
                S::StateString => lx.emit(STATE_DESCR, n, n),
                S::FloatingNote => lx.emit(NOTE_TEXT, n, n),
                S::StateId | S::FloatingNoteId => lx.emit(ID, n, n),
                _ => {}
            }
            return lx.finish(END);
        }

        let c = lx.code(p);
        match lx.state {
            S::Initial | S::Struct => {
                let root = lx.state == S::Initial;
                let lc = c | 32;
                if root {
                    if lc == 99 && lx.word("click", p) && !is_word(lx.code(p + 5)) {
                        lx.emit(CLICK, p, p + 5);
                        continue;
                    }
                    if lc == 104 && lx.word("href", p) && !is_word(lx.code(p + 4)) {
                        lx.emit(HREF, p, p + 4);
                        continue;
                    }
                    if c == 34
                        && let Some(close) = src[p + 1..].find('"')
                    {
                        lx.emit(STRING, p, p + 1 + close + 1);
                        continue;
                    }
                    if lc == 100 && lx.word("default", p) && !is_word(lx.code(p + 7)) {
                        lx.emit(DEFAULT, p, p + 7);
                        continue;
                    }
                    let kind = lx.direction(p);
                    if kind != -1 {
                        let end = lx.eol(lx.dir_end[kind as usize]);
                        lx.emit(DIRECTION_TB + kind as u8, p, end);
                        continue;
                    }
                    if c == 10 {
                        let mut j = p + 1;
                        while lx.code(j) == 10 {
                            j += 1;
                        }
                        lx.emit(NL, p, j);
                        continue;
                    }
                    if is_space(c) {
                        lx.p = lx.ws_end(p);
                        continue;
                    }
                } else if c != 10 && is_space(c) {
                    let mut j = p + width_at(src, p);
                    while j < n && lx.code(j) != 10 && is_space(lx.code(j)) {
                        j += width_at(src, j);
                    }
                    lx.p = j;
                    continue;
                }
                if lx.is_comment(c) {
                    lx.p = lx.eol(p);
                    continue;
                }
                if lc == 115 {
                    if root && lx.word("scale", p) {
                        let e = lx.ws_end(p + 5);
                        if e > p + 5 {
                            lx.emit(SCALE, p, e);
                            lx.push(S::Scale);
                            continue;
                        }
                    }
                    if lx.word("style", p) {
                        let e = lx.ws_end(p + 5);
                        if e > p + 5 {
                            lx.emit(STYLE, p, e);
                            lx.push(S::Style);
                            continue;
                        }
                    }
                    if lx.word("state", p) {
                        let e = lx.ws_end(p + 5);
                        if e > p + 5 {
                            lx.p = e;
                            lx.push(S::State);
                            continue;
                        }
                        if root && lx.word("diagram", p + 5) {
                            let from = if lx.word("-v2", p + 12) {
                                p + 15
                            } else {
                                p + 12
                            };
                            let e = lx.ws_end(from);
                            if e > from {
                                lx.emit(SD, p, e);
                                continue;
                            }
                        }
                    }
                } else if lc == 99 {
                    if lx.word("class", p) {
                        let def = lx.word("def", p + 5);
                        let from = if def { p + 8 } else { p + 5 };
                        let e = lx.ws_end(from);
                        if e > from {
                            lx.emit(if def { CLASSDEF } else { CLASS }, p, e);
                            lx.push(if def { S::Classdef } else { S::Class });
                            continue;
                        }
                    }
                } else if lc == 97 && root && (lx.word("acctitle", p) || lx.word("accdescr", p)) {
                    let title = (lx.code(p + 3) | 32) == 116;
                    let q = lx.ws_end(p + 8);
                    let d = lx.code(q);
                    if d == 58 {
                        let e = lx.ws_end(q + 1);
                        lx.emit(if title { ACC_TITLE } else { ACC_DESCR }, p, e);
                        lx.push(if title { S::AccTitle } else { S::AccDescr });
                        continue;
                    }
                    if d == 123 && !title {
                        lx.p = lx.ws_end(q + 1);
                        lx.push(S::AccDescrMultiline);
                        continue;
                    }
                }
                if root {
                    if c == 123 {
                        lx.emit(STRUCT_START, p, p + 1);
                        lx.push(S::Struct);
                        continue;
                    }
                } else {
                    let kind = lx.direction(p);
                    if kind != -1 {
                        let end = lx.eol(lx.dir_end[kind as usize]);
                        lx.emit(DIRECTION_TB + kind as u8, p, end);
                        continue;
                    }
                    if c == 125 {
                        lx.emit(STRUCT_STOP, p, p + 1);
                        lx.pop();
                        continue;
                    }
                    if c == 10 {
                        lx.p = p + 1;
                        continue;
                    }
                }
                if lc == 110 && lx.word("note", p) {
                    let e = lx.ws_end(p + 4);
                    if e > p + 4 {
                        lx.emit(NOTE, p, e);
                        lx.push(S::Note);
                        continue;
                    }
                }
                if root
                    && lc == 104
                    && lx.word("hide empty description", p)
                    && !is_word(lx.code(p + 22))
                {
                    lx.emit(HIDE_EMPTY, p, p + 22);
                    continue;
                }
                if c == 91 && lx.code(p + 1) == 42 && lx.code(p + 2) == 93 {
                    lx.emit(EDGE_STATE, p, p + 3);
                    continue;
                }
                let mut j = p;
                while j < n {
                    let d = lx.code(j);
                    if d == 58 || d == 45 || d == 123 || is_space(d) {
                        break;
                    }
                    j += width_at(src, j);
                }
                if j > p {
                    lx.emit_id(p, j);
                    continue;
                }
                if c == 58 {
                    j = p + 1;
                    while j < n {
                        let d = lx.code(j);
                        if d == 10 || d == 59 {
                            break;
                        }
                        if d == 58 {
                            let after = lx.code(j + 1);
                            if j + 1 >= n || after == 58 || after == 10 || after == 59 {
                                break;
                            }
                            j += 1;
                        }
                        j += width_at(src, j);
                    }
                    if j > p + 1 {
                        let text = js::trim(&src[p..j]).to_string();
                        lx.emit_text(DESCR, text, p, j);
                        continue;
                    }
                    if lx.code(p + 1) == 58 && lx.code(p + 2) == 58 {
                        lx.emit(STYLE_SEPARATOR, p, p + 3);
                        continue;
                    }
                } else if c == 45 && lx.code(p + 1) == 45 {
                    if lx.code(p + 2) == 62 {
                        lx.emit(ARROW, p, p + 3);
                        continue;
                    }
                    if !root {
                        lx.emit(CONCURRENT, p, p + 2);
                        continue;
                    }
                }
                if !root {
                    return lx.finish(ERROR);
                }
                lx.emit(INVALID, p, p + width_at(src, p));
            }

            S::State => {
                if c != 10 && is_space(c) {
                    let mut j = p + width_at(src, p);
                    while j < n && lx.code(j) != 10 && is_space(lx.code(j)) {
                        j += width_at(src, j);
                    }
                    lx.p = j;
                    continue;
                }
                if lx.is_comment(c) {
                    lx.p = lx.eol(p);
                    continue;
                }
                let kind = lx.stereotype(p);
                if kind != -1 {
                    let at = lx.stereo_at[kind as usize] as usize;
                    let choice = kind == 2 || kind == 5;
                    let token = if choice {
                        CHOICE
                    } else if kind % 3 == 0 {
                        FORK
                    } else {
                        JOIN
                    };
                    let text = js::trim(&src[p..at]).to_string();
                    lx.emit_text(token, text, p, at + if choice { 10 } else { 8 });
                    lx.pop();
                    continue;
                }
                if c == 34 {
                    lx.p = p + 1;
                    lx.push(S::StateString);
                    continue;
                }
                let q = if c == 10 { lx.ws_end(p) } else { p };
                if lx.word("as", q) {
                    let e = lx.ws_end(q + 2);
                    if e > q + 2 {
                        lx.emit(AS, p, e);
                        lx.push(S::StateId);
                        continue;
                    }
                }
                if c == 10 {
                    lx.p = p + 1;
                    lx.pop();
                    continue;
                }
                if c == 123 {
                    lx.emit(STRUCT_START, p, p + 1);
                    lx.pop();
                    lx.push(S::Struct);
                    continue;
                }
                if is_word(c) {
                    let first = lx.word_end(p);
                    let second = lx.ws_end(first);
                    if second > first && is_word(lx.code(second)) {
                        let brace = lx.next_brace.at(second + 1);
                        if brace != -1 && brace < lx.line_end(second) as i64 {
                            let brace = brace as usize;
                            lx.error = Some(format!(
                                "Error: State name must be a single word. Found: \"{}\"",
                                js::trim(&src[p..brace + 1])
                            ));
                            return lx.finish(ERROR);
                        }
                    }
                }
                let mut j = p + width_at(src, p);
                while j < n {
                    let d = lx.code(j);
                    if d == 123 || is_space(d) {
                        break;
                    }
                    j += width_at(src, j);
                }
                lx.emit(COMPOSIT_STATE, p, j);
            }

            S::StateString => {
                if c == 34 {
                    lx.p = p + 1;
                    lx.pop();
                    continue;
                }
                let close = src[p..].find('"').map_or(n, |k| p + k);
                lx.emit(STATE_DESCR, p, close);
            }

            S::StateId => {
                let mut j = p;
                while j < n {
                    let d = lx.code(j);
                    if d == 10 || d == 123 {
                        break;
                    }
                    j += width_at(src, j);
                }
                if lx.emit_id(p, j) {
                    lx.pop();
                }
            }

            S::Scale => {
                if (48..=57).contains(&c) {
                    let mut j = p + 1;
                    while j < n && (48..=57).contains(&lx.code(j)) {
                        j += 1;
                    }
                    lx.emit(WIDTH, p, j);
                    continue;
                }
                let q = lx.ws_end(p);
                if q > p && lx.word("width", q) && !is_word(lx.code(q + 5)) {
                    lx.p = q + 5;
                    lx.pop();
                    continue;
                }
                return lx.finish(ERROR);
            }

            S::AccTitle => {
                let e = lx.eol(p);
                lx.emit(ACC_TITLE_VALUE, p, e);
                lx.pop();
            }

            S::AccDescr => {
                let e = lx.eol(p);
                lx.emit(ACC_DESCR_VALUE, p, e);
                lx.pop();
            }

            S::AccDescrMultiline => {
                if c == 125 {
                    lx.p = p + 1;
                    lx.pop();
                    continue;
                }
                let close = src[p..].find('}').map_or(n, |k| p + k);
                lx.emit(ACC_DESCR_MULTILINE_VALUE, p, close);
            }

            S::Classdef => {
                let is_default = lx.word("default", p) && lx.ws_end(p + 7) > p + 7;
                let j = if is_default { p + 7 } else { lx.word_end(p) };
                let e = lx.ws_end(j);
                if j == p || e == j {
                    return lx.finish(ERROR);
                }
                lx.emit(
                    if is_default {
                        DEFAULT_CLASSDEF_ID
                    } else {
                        CLASSDEF_ID
                    },
                    p,
                    e,
                );
                lx.pop();
                lx.push(S::ClassdefId);
            }

            S::ClassdefId => {
                let e = lx.eol(p);
                lx.emit(CLASSDEF_STYLEOPTS, p, e);
                lx.pop();
            }

            S::Class => {
                let mut j = lx.word_end(p);
                if j == p {
                    return lx.finish(ERROR);
                }
                while lx.code(j) == 44 {
                    let k = lx.ws_end(j + 1);
                    let e = lx.word_end(k);
                    if e == k {
                        break;
                    }
                    j = e;
                }
                lx.emit(CLASSENTITY_IDS, p, j);
                lx.pop();
                lx.push(S::ClassStyle);
            }

            S::ClassStyle => {
                let e = lx.eol(p);
                lx.emit(STYLECLASS, p, e);
                lx.pop();
            }

            S::Style => {
                let mut j = p;
                while j < n && (is_word(lx.code(j)) || lx.code(j) == 44) {
                    j += 1;
                }
                let e = lx.ws_end(j);
                if j == p || e == j {
                    return lx.finish(ERROR);
                }
                lx.emit(STYLE_IDS, p, e);
                lx.pop();
                lx.push(S::StyledefStyles);
            }

            S::StyledefStyles => {
                let e = lx.eol(p);
                lx.emit(STYLEDEF_STYLEOPTS, p, e);
                lx.pop();
            }

            S::Note => {
                if lx.word("left of", p) && !is_word(lx.code(p + 7)) {
                    lx.emit(LEFT_OF, p, p + 7);
                    lx.pop();
                    lx.push(S::NoteId);
                    continue;
                }
                if lx.word("right of", p) && !is_word(lx.code(p + 8)) {
                    lx.emit(RIGHT_OF, p, p + 8);
                    lx.pop();
                    lx.push(S::NoteId);
                    continue;
                }
                if c == 34 {
                    lx.p = p + 1;
                    lx.pop();
                    lx.push(S::FloatingNote);
                    continue;
                }
                return lx.finish(ERROR);
            }

            S::NoteId => {
                let q = lx.ws_end(p);
                let mut j = q;
                while j < n {
                    let d = lx.code(j);
                    if d == 58 || d == 45 || is_space(d) {
                        break;
                    }
                    j += width_at(src, j);
                }
                if j == q {
                    return lx.finish(ERROR);
                }
                if lx.emit_id(p, j) {
                    lx.pop();
                    lx.push(S::NoteText);
                }
            }

            S::NoteText => {
                let q = lx.ws_end(p);
                if lx.code(q) == 58 {
                    let mut j = q + 1;
                    while j < n {
                        let d = lx.code(j);
                        if d == 58 || d == 10 || d == 59 {
                            break;
                        }
                        j += width_at(src, j);
                    }
                    if j > q + 1 {
                        // Mermaid drops two characters here, expecting " :";
                        // with no space before the colon the first character
                        // of the note goes too.
                        let from = skip_units(src, p, 2).min(j);
                        let text = js::trim(&src[from..j]).to_string();
                        lx.emit_text(NOTE_TEXT, text, p, j);
                        lx.pop();
                        continue;
                    }
                }
                let mut k = src[p..].find('\n').map(|k| p + k);
                let mut e = 0;
                while let Some(at) = k {
                    e = lx.ws_end(at);
                    if lx.word("end note", e) && !is_word(lx.code(e + 8)) {
                        break;
                    }
                    k = src[e..].find('\n').map(|k| e + k);
                }
                if k.is_none() {
                    return lx.finish(ERROR);
                }
                let text = js::trim(&src[p..e]).to_string();
                lx.emit_text(NOTE_TEXT, text, p, e + 8);
                lx.pop();
            }

            S::FloatingNote => {
                let q = lx.ws_end(p);
                if lx.word("as", q) {
                    let e = lx.ws_end(q + 2);
                    lx.emit(AS, p, e);
                    lx.pop();
                    lx.push(S::FloatingNoteId);
                    continue;
                }
                if c == 34 {
                    lx.p = p + 1;
                    continue;
                }
                let close = src[p..].find('"').map_or(n, |k| p + k);
                lx.emit(NOTE_TEXT, p, close);
            }

            S::FloatingNoteId => {
                let e = lx.eol(p);
                if lx.emit_id(p, e) {
                    lx.pop();
                }
            }
        }
    }
}
