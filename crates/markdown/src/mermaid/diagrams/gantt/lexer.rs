//! Pele's `diagrams/gantt/lexer.ts`.

use super::tokens::*;
use crate::mermaid::{
    diagrams::common::scan::Tokens,
    js::Lazy,
    util::chars::{code_at, is_space, width_at},
};

const DAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

static RE_ACC_TITLE: Lazy = Lazy::new(r"accTitle\s*:\s*", "iy");
static RE_ACC_DESCR: Lazy = Lazy::new(r"accDescr\s*:\s*", "iy");
static RE_ACC_DESCR_ML: Lazy = Lazy::new(r"accDescr\s*\{\s*", "iy");
static RE_HREF: Lazy = Lazy::new(r#"href\s+""#, "iy");
static RE_CALL: Lazy = Lazy::new(r"call\s+", "iy");
static RE_CLICK: Lazy = Lazy::new(r"click\s+", "iy");
static RE_EMPTY_CALL: Lazy = Lazy::new(r"\(\s*\)", "y");
static RE_WEEKDAY: Lazy = Lazy::new(
    r"weekday\s+(monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
    "iy",
);
static RE_WEEKEND: Lazy = Lazy::new(r"weekend\s+(?:(friday)|saturday)\b", "iy");
static RE_DATE: Lazy = Lazy::new(r"\d\d\d\d-\d\d-\d\d\b", "y");
static RE_AXIS_FORMAT: Lazy = Lazy::new(r"axisFormat\s[^#\n;]+", "iy");
static RE_ACC_DESCRIPTION: Lazy = Lazy::new(r"accDescription\s[^#\n;]+", "iy");
static RE_GANTT: Lazy = Lazy::new(r"gantt\b", "iy");
static RE_DATE_FORMAT: Lazy = Lazy::new(r"dateFormat\s[^#\n;]+", "iy");
static RE_INCLUSIVE: Lazy = Lazy::new(r"inclusiveEndDates\b", "iy");
static RE_INCLUDES: Lazy = Lazy::new(r"includes\s[^#\n;]+", "iy");
static RE_TOP_AXIS: Lazy = Lazy::new(r"topAxis\b", "iy");
static RE_TICK_INTERVAL: Lazy = Lazy::new(r"tickInterval\s[^#\n;]+", "iy");
static RE_TODAY_MARKER: Lazy = Lazy::new(r"todayMarker\s[^\n;]+", "iy");
static RE_TITLE: Lazy = Lazy::new(r"title\s[^\n]+", "iy");
static RE_EXCLUDES: Lazy = Lazy::new(r"excludes\s[^#\n;]+", "iy");
static RE_SECTION: Lazy = Lazy::new(r"section\s[^\n]+", "iy");
/// Statements that start with a keyword, with the keyword's first letter, in
/// the grammar's rule order.
static KEYWORDS: [(u32, &Lazy, i32); 12] = [
    (97, &RE_AXIS_FORMAT, AXIS_FORMAT),
    (97, &RE_ACC_DESCRIPTION, ACC_DESCRIPTION),
    (103, &RE_GANTT, GANTT),
    (100, &RE_DATE_FORMAT, DATE_FORMAT),
    (105, &RE_INCLUSIVE, INCLUSIVE_END_DATES),
    (105, &RE_INCLUDES, INCLUDES),
    (116, &RE_TOP_AXIS, TOP_AXIS),
    (116, &RE_TICK_INTERVAL, TICK_INTERVAL),
    (116, &RE_TODAY_MARKER, TODAY_MARKER),
    (116, &RE_TITLE, TITLE),
    (101, &RE_EXCLUDES, EXCLUDES),
    (115, &RE_SECTION, SECTION),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Initial,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
    Href,
    Callbackname,
    Callbackargs,
    Click,
}

struct Lexer<'a> {
    src: &'a str,
    out: Tokens,
    state: State,
    p: usize,
}

impl Lexer<'_> {
    fn emit(&mut self, kind: i32, s: usize, e: usize) {
        self.out.types.push(kind);
        self.out.texts.push(self.src[s..e].to_string());
        self.out.starts.push(s);
        self.p = e;
    }

    fn at(&self, re: &Lazy) -> Option<usize> {
        re.match_end(self.src, self.p)
    }

    fn until(&self, ch: char, from: usize) -> usize {
        self.src
            .get(from..)
            .and_then(|rest| rest.find(ch))
            .map_or(self.src.len(), |at| from + at)
    }

    fn keyword(&mut self, c: u32) -> bool {
        let p = self.p;
        match c | 32 {
            97 => {
                if let Some(e) = self.at(&RE_ACC_TITLE) {
                    self.state = State::AccTitle;
                    self.emit(ACC_TITLE, p, e);
                    return true;
                }
                if let Some(e) = self.at(&RE_ACC_DESCR) {
                    self.state = State::AccDescr;
                    self.emit(ACC_DESCR, p, e);
                    return true;
                }
                if let Some(e) = self.at(&RE_ACC_DESCR_ML) {
                    self.state = State::AccDescrMultiline;
                    self.p = e;
                    return true;
                }
            }
            104 => {
                if let Some(e) = self.at(&RE_HREF) {
                    self.state = State::Href;
                    self.p = e;
                    return true;
                }
            }
            99 => {
                if let Some(e) = self.at(&RE_CALL) {
                    self.state = State::Callbackname;
                    self.p = e;
                    return true;
                }
                if let Some(e) = self.at(&RE_CLICK) {
                    self.state = State::Click;
                    self.p = e;
                    return true;
                }
            }
            119 => {
                if let Some(m) = RE_WEEKDAY.exec_at(self.src, p) {
                    let day = m.str(1).to_lowercase();
                    let index = DAYS.iter().position(|d| *d == day).unwrap_or(0) as i32;
                    let end = m.end();
                    self.emit(WEEKDAY_MONDAY + index, p, end);
                    return true;
                }
                if let Some(m) = RE_WEEKEND.exec_at(self.src, p) {
                    let kind = if m.get(1).is_some() {
                        WEEKEND_FRIDAY
                    } else {
                        WEEKEND_SATURDAY
                    };
                    let end = m.end();
                    self.emit(kind, p, end);
                    return true;
                }
            }
            _ => {}
        }
        for &(_, re, kind) in KEYWORDS.iter().filter(|(first, _, _)| *first == c | 32) {
            if let Some(e) = self.at(re) {
                self.emit(kind, p, e);
                return true;
            }
        }
        false
    }

    /// The UTF-16 unit after the character at `p`: a low surrogate when
    /// that character is outside the BMP.
    fn next_unit(&self, p: usize) -> u32 {
        let w = width_at(self.src, p);
        if w == 4 {
            0xdc00
        } else {
            code_at(self.src, p + w)
        }
    }
}

pub fn tokenize(src: &str) -> Tokens {
    let n = src.len();
    let mut lx = Lexer {
        src,
        out: Tokens::default(),
        state: State::Initial,
        p: 0,
    };

    loop {
        if lx.p >= n {
            let kind = match lx.state {
                State::AccTitle => ACC_TITLE_VALUE,
                State::AccDescr => ACC_DESCR_VALUE,
                State::AccDescrMultiline => ACC_DESCR_MULTILINE_VALUE,
                State::Href => HREF,
                State::Callbackname => CALLBACKNAME,
                State::Callbackargs => CALLBACKARGS,
                State::Click => CLICK,
                State::Initial => EOF,
            };
            lx.emit(kind, n, n);
            break;
        }

        let p = lx.p;
        let c = code_at(src, p);

        match lx.state {
            State::Initial => {
                let d = lx.next_unit(p);
                let w = width_at(src, p);
                if c == 37 {
                    if d != 37 {
                        let mut e = lx.until('\n', p + 1);
                        while code_at(src, e) == 10 {
                            e += 1;
                        }
                        lx.p = e;
                    } else if code_at(src, p + 2) == 123 {
                        // The grammar enters a lexer state it never defines,
                        // so nothing can follow.
                        lx.emit(OPEN_DIRECTIVE, p, p + 3);
                        lx.emit(ERROR, p, p);
                        return lx.out;
                    } else {
                        lx.p = lx.until('\n', p + 2);
                    }
                } else if c != 125 && d == 37 {
                    // Any character but `}` followed by `%` starts a comment
                    // that runs to the end of the line.
                    lx.p = lx.until('\n', p + w + 1);
                } else if c == 10 {
                    let mut e = p + 1;
                    while code_at(src, e) == 10 {
                        e += 1;
                    }
                    lx.emit(NL, p, e);
                } else if is_space(c) {
                    lx.p += w;
                    while lx.p < n && is_space(code_at(src, lx.p)) {
                        lx.p += width_at(src, lx.p);
                    }
                } else if lx.keyword(c) {
                    continue;
                } else if (48..=57).contains(&c) && lx.at(&RE_DATE).is_some() {
                    lx.emit(DATE, p, p + 10);
                } else if c != 58 {
                    let bytes = src.as_bytes();
                    let mut e = p + w;
                    while e < n && bytes[e] != b':' && bytes[e] != b'\n' {
                        e += 1;
                    }
                    lx.emit(TASK_TXT, p, e);
                } else {
                    let bytes = src.as_bytes();
                    let mut e = p + 1;
                    while e < n && !matches!(bytes[e], b'#' | b'\n' | b';') {
                        e += 1;
                    }
                    lx.emit(if e > p + 1 { TASK_DATA } else { COLON }, p, e);
                }
            }

            State::AccTitle | State::AccDescr => {
                let kind = if lx.state == State::AccTitle {
                    ACC_TITLE_VALUE
                } else {
                    ACC_DESCR_VALUE
                };
                lx.state = State::Initial;
                let e = lx.until('\n', p);
                lx.emit(kind, p, e);
            }

            State::AccDescrMultiline => {
                if c == 125 {
                    lx.state = State::Initial;
                    lx.p += 1;
                } else {
                    let e = lx.until('}', p);
                    lx.emit(ACC_DESCR_MULTILINE_VALUE, p, e);
                }
            }

            State::Href => {
                if c == 34 {
                    lx.state = State::Initial;
                    lx.p += 1;
                } else {
                    let e = lx.until('"', p);
                    lx.emit(HREF, p, e);
                }
            }

            State::Callbackname => {
                if c == 40 {
                    if let Some(e) = lx.at(&RE_EMPTY_CALL) {
                        lx.state = State::Initial;
                        lx.p = e;
                    } else {
                        lx.state = State::Callbackargs;
                        lx.p += 1;
                    }
                } else {
                    let e = lx.until('(', p);
                    lx.emit(CALLBACKNAME, p, e);
                }
            }

            State::Callbackargs => {
                if c == 41 {
                    lx.state = State::Initial;
                    lx.p += 1;
                } else {
                    let e = lx.until(')', p);
                    lx.emit(CALLBACKARGS, p, e);
                }
            }

            State::Click => {
                if is_space(c) {
                    lx.state = State::Initial;
                    lx.p += width_at(src, p);
                } else {
                    let mut e = p + width_at(src, p);
                    while e < n && !is_space(code_at(src, e)) {
                        e += width_at(src, e);
                    }
                    lx.emit(CLICK, p, e);
                }
            }
        }
    }

    lx.out.types.push(END);
    lx.out.texts.push(String::new());
    lx.out.starts.push(n);
    lx.out
}
