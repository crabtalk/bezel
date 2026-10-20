//! Pele's `diagrams/flowchart/lexer.ts`. Positions are byte offsets.

use super::tokens::T;
use crate::{
    js::Lazy,
    token::Token,
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
static RE_DIRECTION: [(T, Lazy); 5] = [
    (T::DirectionTb, Lazy::new(r".*direction\s+TB[^\n]*", "y")),
    (T::DirectionBt, Lazy::new(r".*direction\s+BT[^\n]*", "y")),
    (T::DirectionRl, Lazy::new(r".*direction\s+RL[^\n]*", "y")),
    (T::DirectionLr, Lazy::new(r".*direction\s+LR[^\n]*", "y")),
    (T::DirectionTd, Lazy::new(r".*direction\s+TD[^\n]*", "y")),
];
static RE_LINE_END: Lazy = Lazy::new(r"[\n\r  ]", "g");
static RE_LINK: Lazy = Lazy::new(r"\s*[xo<]?--+[-xo>]\s*", "y");
static RE_START_LINK: Lazy = Lazy::new(r"\s*[xo<]?--\s*", "y");
static RE_THICK_LINK: Lazy = Lazy::new(r"\s*[xo<]?==+[=xo>]\s*", "y");
static RE_THICK_START: Lazy = Lazy::new(r"\s*[xo<]?==\s*", "y");
static RE_DOTTED_LINK: Lazy = Lazy::new(r"\s*[xo<]?-?\.+-[xo>]?\s*", "y");
static RE_DOTTED_START: Lazy = Lazy::new(r"\s*[xo<]?-\.\s*", "y");
static RE_INVIS_LINK: Lazy = Lazy::new(r"\s*~~~+\s*", "y");
static RE_NEWLINE: Lazy = Lazy::new(r"(?:\r?\n)+", "y");
static RE_SPACE: Lazy = Lazy::new(r"[^\S\n\r]+", "y");
static RE_EMPTY_CALL: Lazy = Lazy::new(r"\(\s*\)", "y");
static RE_BR: Lazy = Lazy::new(r"\n\s*", "g");

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
    ThickEdgeText,
    DottedEdgeText,
    Click,
    Callbackname,
    Callbackargs,
    ShapeData,
    ShapeDataStr,
}

struct Lexer<'s> {
    src: &'s str,
    n: usize,
    out: Vec<Token<'s, T>>,
    stack: Vec<S>,
    state: S,
    p: usize,
    first_graph: bool,
    dir_next: Option<usize>,
    at_next: Option<usize>,
    line_end: Option<usize>,
    dir_failed: usize,
    run_end: usize,
    run_at: Option<usize>,
    /// The last run of whitespace stepped over, from and to.
    ws: Option<(usize, usize)>,
}

fn is_node_char(ch: u32) -> bool {
    ch < 128 && NODE_CHARS.contains(&(ch as u8))
}

impl<'s> Lexer<'s> {
    fn code(&self, at: usize) -> u32 {
        code_at(self.src, at)
    }

    fn emit(&mut self, kind: T, s: usize, e: usize) {
        self.out.push(Token::new(kind, &self.src[s..e], s));
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

    fn at(&self, re: &Lazy, q: usize) -> Option<usize> {
        re.match_end(self.src, q)
    }

    fn kw(&self, word: &str) -> bool {
        self.src[self.p..].starts_with(word) && !is_word(self.code(self.p + word.len()))
    }

    /// The first character after any run of whitespace starting at `q`.
    fn skip_ws(&mut self, q: usize) -> usize {
        let mut j = q;
        match self.ws {
            Some((from, to)) if q >= from && q <= to => j = to,
            _ => {
                if is_space(self.code(q)) {
                    while j < self.n && is_space(self.code(j)) {
                        j += width_at(self.src, j);
                    }
                    self.ws = Some((q, j));
                }
            }
        }
        j
    }

    /// First character after any run of whitespace starting at `q`,
    /// skipping one optional x, o or <.
    fn link_char(&mut self, q: usize) -> u32 {
        let j = self.skip_ws(q);
        let d = self.code(j);
        if d == 120 || d == 111 || d == 60 {
            self.code(j + 1)
        } else {
            d
        }
    }

    fn invis_ahead(&mut self, q: usize) -> bool {
        let j = self.skip_ws(q);
        self.code(j) == 126
    }

    fn quote(&mut self) {
        if self.code(self.p + 1) == 96 {
            self.push(S::MdString);
            self.p += 2;
        } else {
            self.push(S::String);
            self.p += 1;
        }
    }

    fn invis_link(&mut self) -> bool {
        if !self.invis_ahead(self.p) {
            return false;
        }
        let Some(e) = self.at(&RE_INVIS_LINK, self.p) else {
            return false;
        };
        self.emit(T::Link, self.p, e);
        true
    }

    fn opener(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.code(p + 1);
        if c == 40 {
            if d == 45 {
                self.push(S::EllipseText);
                self.emit(T::EllipseStart, p, p + 2);
            } else if d == 91 {
                self.push(S::Text);
                self.emit(T::Stadiumstart, p, p + 2);
            } else if d == 40 && self.code(p + 2) == 40 {
                self.push(S::Text);
                self.emit(T::Doublecirclestart, p, p + 3);
            } else {
                self.push(S::Text);
                self.emit(T::Ps, p, p + 1);
            }
            return true;
        }
        if c == 91 {
            if d == 91 {
                self.push(S::Text);
                self.emit(T::Subroutinestart, p, p + 2);
            } else if d == 40 {
                self.push(S::Text);
                self.emit(T::Cylinderstart, p, p + 2);
            } else if d == 47 {
                self.push(S::TrapText);
                self.emit(T::Trapstart, p, p + 2);
            } else if d == 92 {
                self.push(S::TrapText);
                self.emit(T::Invtrapstart, p, p + 2);
            } else {
                self.push(S::Text);
                self.emit(T::Sqs, p, p + 1);
            }
            return true;
        }
        if c == 124 {
            self.push(S::Text);
            self.emit(T::Pipe, p, p + 1);
            return true;
        }
        if c == 123 {
            self.push(S::Text);
            self.emit(T::DiamondStart, p, p + 1);
            return true;
        }
        false
    }

    fn graph(&mut self, len: usize) {
        if self.first_graph {
            self.first_graph = false;
            self.push(S::Dir);
        }
        self.emit(T::Graph, self.p, self.p + len);
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
        self.emit(T::NodeString, p, q);
        true
    }

    fn link(&mut self, c: u32) -> bool {
        let p = self.p;
        let d = self.link_char(p);
        if d == 45 {
            if let Some(e) = self.at(&RE_LINK, p) {
                self.emit(T::Link, p, e);
                return true;
            }
            if let Some(e) = self.at(&RE_START_LINK, p) {
                self.push(S::EdgeText);
                self.emit(T::StartLink, p, e);
                return true;
            }
            if let Some(e) = self.at(&RE_DOTTED_LINK, p) {
                self.emit(T::Link, p, e);
                return true;
            }
            if let Some(e) = self.at(&RE_DOTTED_START, p) {
                self.push(S::DottedEdgeText);
                self.emit(T::StartLink, p, e);
                return true;
            }
        } else if d == 61 {
            if let Some(e) = self.at(&RE_THICK_LINK, p) {
                self.emit(T::Link, p, e);
                return true;
            }
            if let Some(e) = self.at(&RE_THICK_START, p) {
                self.push(S::ThickEdgeText);
                self.emit(T::StartLink, p, e);
                return true;
            }
        } else if d == 46
            && let Some(e) = self.at(&RE_DOTTED_LINK, p)
        {
            self.emit(T::Link, p, e);
            return true;
        }
        c != 45 && c != 61 && c != 46 && self.invis_link()
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
                        self.emit(T::AccTitle, p, e);
                        return true;
                    }
                    if let Some(e) = self.at(&RE_ACC_DESCR, p) {
                        self.push(S::AccDescr);
                        self.emit(T::AccDescr, p, e);
                        return true;
                    }
                    if let Some(e) = self.at(&RE_ACC_DESCR_ML, p) {
                        self.push(S::AccDescrMultiline);
                        self.p = e;
                        return true;
                    }
                }
            }
            64 => {
                if self.code(p + 1) == 123 {
                    self.push(S::ShapeData);
                    self.out.push(Token::new(T::ShapeData, "", p));
                    self.p += 2;
                    return true;
                }
            }
            99 => {
                if let Some(e) = self.at(&RE_CALL, p) {
                    self.push(S::Callbackname);
                    self.p = e;
                    return true;
                }
                if self.kw("classDef") {
                    self.emit(T::Classdef, p, p + 8);
                    return true;
                }
                if self.kw("class") {
                    self.emit(T::Class, p, p + 5);
                    return true;
                }
                if let Some(e) = self.at(&RE_CLICK, p) {
                    self.push(S::Click);
                    self.p = e;
                    return true;
                }
            }
            34 => {
                self.quote();
                return true;
            }
            115 => {
                if self.kw("style") {
                    self.emit(T::Style, p, p + 5);
                    return true;
                }
                if self.kw("swimlane-beta") {
                    self.graph(13);
                    return true;
                }
                if self.kw("subgraph") {
                    self.emit(T::Subgraph, p, p + 8);
                    return true;
                }
            }
            100 => {
                if self.kw("default") {
                    self.emit(T::Default, p, p + 7);
                    return true;
                }
            }
            108 => {
                if self.kw("linkStyle") {
                    self.emit(T::Linkstyle, p, p + 9);
                    return true;
                }
            }
            105 => {
                if self.kw("interpolate") {
                    self.emit(T::Interpolate, p, p + 11);
                    return true;
                }
            }
            104 => {
                if src[p..].starts_with("href") && is_space(self.code(p + 4)) {
                    self.emit(T::Href, p, p + 4 + width_at(src, p + 4));
                    return true;
                }
            }
            102 => {
                if self.kw("flowchart-elk") {
                    self.graph(13);
                    return true;
                }
                if self.kw("flowchart") {
                    self.graph(9);
                    return true;
                }
            }
            103 => {
                if self.kw("graph") {
                    self.graph(5);
                    return true;
                }
            }
            101 => {
                if self.kw("end") {
                    let mut e = p + 3;
                    while e < n && is_space(self.code(e)) {
                        e += width_at(src, e);
                    }
                    self.emit(T::EndKw, p, e);
                    return true;
                }
            }
            95 => {
                for (word, len) in [("_self", 5), ("_blank", 6), ("_parent", 7), ("_top", 4)] {
                    if self.kw(word) {
                        self.emit(T::LinkTarget, p, p + len);
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
                    for (kind, re) in &RE_DIRECTION {
                        if let Some(e) = self.at(re, p) {
                            self.emit(*kind, p, e);
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
                    self.at_next = src[p..].find('@').map(|at| at + p);
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
                self.emit(T::LinkId, p, run_at + 1);
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
            self.emit(T::Num, p, q);
            return true;
        }

        match c {
            35 => {
                self.emit(T::Brkt, p, p + 1);
                return true;
            }
            58 => {
                if self.code(p + 1) == 58 && self.code(p + 2) == 58 {
                    self.emit(T::StyleSeparator, p, p + 3);
                } else {
                    self.emit(T::Colon, p, p + 1);
                }
                return true;
            }
            38 => {
                self.emit(T::Amp, p, p + 1);
                return true;
            }
            59 => {
                self.emit(T::Semi, p, p + 1);
                return true;
            }
            44 => {
                self.emit(T::Comma, p, p + 1);
                return true;
            }
            42 => {
                self.emit(T::Mult, p, p + 1);
                return true;
            }
            120 | 111 => return self.link(c) || self.node_string(),
            60 => {
                if self.link(c) {
                    return true;
                }
                self.emit(T::Tagstart, p, p + 1);
                return true;
            }
            45 => {
                if self.link(c) || self.node_string() {
                    return true;
                }
                self.emit(T::Minus, p, p + 1);
                return true;
            }
            61 => return self.link(c),
            46 => return self.link(c) || self.node_string(),
            126 => return self.invis_link(),
            40 => return self.opener(c),
            91 => {
                if self.code(p + 1) == 124 {
                    self.emit(T::VertexWithPropsStart, p, p + 2);
                    return true;
                }
                return self.opener(c);
            }
            124 | 123 => return self.opener(c),
            62 => {
                self.push(S::Text);
                self.emit(T::Tagend, p, p + 1);
                return true;
            }
            94 => {
                self.emit(T::Up, p, p + 1);
                return true;
            }
            92 => {
                if self.code(p + 1) == 124 {
                    self.emit(T::Sep, p, p + 2);
                    return true;
                }
                return self.node_string();
            }
            118 => {
                if !is_word(self.code(p + 1)) {
                    self.emit(T::Down, p, p + 1);
                    return true;
                }
                return self.node_string();
            }
            _ => {}
        }

        if is_space(c) {
            if self.link(c) {
                return true;
            }
            if let Some(e) = self.at(&RE_NEWLINE, p) {
                self.emit(T::Newline, p, e);
                return true;
            }
            if let Some(e) = self.at(&RE_SPACE, p) {
                self.emit(T::Space, p, e);
                return true;
            }
            return false;
        }

        if c < 128 {
            return self.node_string();
        }
        if is_unicode_letter(c) {
            self.emit(T::UnicodeText, p, p + width_at(src, p));
            return true;
        }
        false
    }

    /// `b` is a second character that may start the closing link.
    fn edge_text(&mut self, c: u32, link_re: &Lazy, a: u32, b: Option<u32>) -> bool {
        if c == 34 {
            self.quote();
            return true;
        }
        let p = self.p;
        let d = self.link_char(p);
        if (d == a || Some(d) == b)
            && let Some(e) = self.at(link_re, p)
        {
            self.pop();
            self.emit(T::Link, p, e);
            return true;
        }
        if c == a && (a != 45 || self.code(p + 1) == 45) {
            return false;
        }
        let mut q = p + width_at(self.src, p);
        while q < self.n {
            let ch = self.code(q);
            if ch == 34 {
                break;
            }
            if ch == a {
                if a != 45 || self.code(q + 1) == 45 {
                    break;
                }
            } else if ch == 120 || ch == 111 || ch == 60 || Some(ch) == b || is_space(ch) {
                let dd = self.link_char(q);
                if (dd == a || Some(dd) == b) && self.at(link_re, q).is_some() {
                    break;
                }
            }
            q += 1;
        }
        self.emit(T::EdgeText, p, q);
        true
    }

    fn run(mut self) -> Vec<Token<'s, T>> {
        loop {
            let p = self.p;
            let n = self.n;
            if p >= n {
                let kind = match self.state {
                    S::Initial => Some(T::Eof),
                    S::AccTitle => Some(T::AccTitleValue),
                    S::AccDescr => Some(T::AccDescrValue),
                    S::AccDescrMultiline => Some(T::AccDescrMultilineValue),
                    S::Click => Some(T::Click),
                    S::Callbackname => Some(T::Callbackname),
                    S::Callbackargs => Some(T::Callbackargs),
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.emit(kind, n, n);
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
                            self.p += 2;
                        } else {
                            self.pop();
                            self.p += 1;
                        }
                    } else {
                        let e = src[p..].find('"').map_or(n, |at| at + p);
                        self.emit(T::Str, p, e);
                    }
                }

                S::MdString => {
                    if c == 96 {
                        if self.code(p + 1) == 34 {
                            self.pop();
                            self.p += 2;
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
                        self.emit(T::MdStr, p, q);
                    }
                }

                S::AccTitle | S::AccDescr => {
                    let e = src[p..].find('\n').map_or(n, |at| at + p);
                    let kind = if self.state == S::AccTitle {
                        T::AccTitleValue
                    } else {
                        T::AccDescrValue
                    };
                    self.pop();
                    self.emit(kind, p, e);
                }

                S::AccDescrMultiline => {
                    if c == 125 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = src[p..].find('}').map_or(n, |at| at + p);
                        self.emit(T::AccDescrMultilineValue, p, e);
                    }
                }

                S::Dir => {
                    if c == 34 {
                        self.quote();
                    } else if let Some(e) = self.at(&RE_NODIR, p) {
                        self.pop();
                        self.emit(T::Nodir, p, e);
                    } else if let Some(e) = self
                        .at(&RE_DIR, p)
                        .or_else(|| self.at(&RE_DIR_SYM, p))
                        .or_else(|| self.at(&RE_DIR_V, p))
                    {
                        self.pop();
                        self.emit(T::Dir, p, e);
                    } else {
                        ok = self.invis_link() || self.opener(c);
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
                                self.emit(T::Stadiumend, p, p + 2);
                            } else if d == 93 {
                                self.emit(T::Subroutineend, p, p + 2);
                            } else {
                                self.emit(T::Sqe, p, p + 1);
                            }
                        }
                        41 => {
                            self.pop();
                            if d == 93 {
                                self.emit(T::Cylinderend, p, p + 2);
                            } else if d == 41 && self.code(p + 2) == 41 {
                                self.emit(T::Doublecircleend, p, p + 3);
                            } else {
                                self.emit(T::Pe, p, p + 1);
                            }
                        }
                        124 => {
                            self.pop();
                            self.emit(T::Pipe, p, p + 1);
                        }
                        125 => {
                            self.pop();
                            self.emit(T::DiamondStop, p, p + 1);
                        }
                        _ => {
                            if !((c == 126 || is_space(c)) && self.invis_link()) {
                                let mut q = p + width_at(src, p);
                                while q < n {
                                    let ch = self.code(q);
                                    if matches!(ch, 91 | 93 | 40 | 41 | 123 | 125 | 124 | 34) {
                                        break;
                                    }
                                    q += 1;
                                }
                                self.emit(T::Text, p, q);
                            }
                        }
                    }
                }

                S::EllipseText => {
                    let d = self.code(p + 1);
                    if c == 34 {
                        self.quote();
                    } else if (c == 126 || is_space(c)) && self.invis_link() {
                    } else if (c == 45 || c == 47 || c == 41) && d == 41 {
                        self.pop();
                        self.emit(T::EllipseEnd, p, p + 2);
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
                            if (ch == 126 || is_space(ch))
                                && self.invis_ahead(q)
                                && self.at(&RE_INVIS_LINK, q).is_some()
                            {
                                break;
                            }
                            q += 1;
                        }
                        self.emit(T::Text, p, q);
                    }
                }

                S::TrapText => 'trap: {
                    let d = self.code(p + 1);
                    if c == 34 {
                        self.quote();
                        break 'trap;
                    }
                    if (c == 126 || is_space(c)) && self.invis_link() {
                        break 'trap;
                    }
                    match c {
                        40 => {
                            if d == 45 || d == 91 || (d == 40 && self.code(p + 2) == 40) {
                                self.opener(c);
                            } else if d == 93 {
                                self.pop();
                                self.emit(T::Trapend, p, p + 2);
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
                                self.emit(T::Trapend, p, p + 2);
                            } else {
                                self.emit(T::Text, p, p + 1);
                            }
                        }
                        47 => {
                            if d == 93 {
                                self.pop();
                                self.emit(T::Invtrapend, p, p + 2);
                            } else {
                                self.emit(T::Text, p, p + 1);
                            }
                        }
                        93 | 41 => {
                            if d == 93 {
                                self.pop();
                                self.emit(T::Trapend, p, p + 2);
                            } else {
                                ok = false;
                            }
                        }
                        125 => ok = false,
                        _ => {
                            if (c == 63 || c == 61) && d == 93 {
                                self.pop();
                                self.emit(T::Trapend, p, p + 2);
                                break 'trap;
                            }
                            let mut q = p + width_at(src, p);
                            while q < n {
                                let ch = self.code(q);
                                if matches!(ch, 92 | 91 | 93 | 40 | 41 | 123 | 125 | 47) {
                                    break;
                                }
                                q += 1;
                            }
                            self.emit(T::Text, p, q);
                        }
                    }
                }

                S::EdgeText => ok = self.edge_text(c, &RE_LINK, 45, None),
                S::ThickEdgeText => ok = self.edge_text(c, &RE_THICK_LINK, 61, None),
                S::DottedEdgeText => ok = self.edge_text(c, &RE_DOTTED_LINK, 46, Some(45)),

                S::Click => {
                    if c == 34 {
                        self.quote();
                    } else if is_space(c) {
                        self.pop();
                        self.p += width_at(src, p);
                    } else {
                        let mut q = p + width_at(src, p);
                        while q < n && !is_space(self.code(q)) {
                            q += 1;
                        }
                        self.emit(T::Click, p, q);
                    }
                }

                S::Callbackname => {
                    if c == 40 {
                        let e = self.at(&RE_EMPTY_CALL, p);
                        self.pop();
                        match e {
                            Some(e) => self.p = e,
                            None => {
                                self.push(S::Callbackargs);
                                self.p += 1;
                            }
                        }
                    } else {
                        let e = src[p..].find('(').map_or(n, |at| at + p);
                        self.emit(T::Callbackname, p, e);
                    }
                }

                S::Callbackargs => {
                    if c == 41 {
                        self.pop();
                        self.p += 1;
                    } else {
                        let e = src[p..].find(')').map_or(n, |at| at + p);
                        self.emit(T::Callbackargs, p, e);
                    }
                }

                S::ShapeData => {
                    if c == 34 {
                        self.push(S::ShapeDataStr);
                        self.emit(T::ShapeData, p, p + 1);
                    } else if c == 125 {
                        self.pop();
                        self.p += 1;
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
                        self.emit(T::ShapeData, p, q);
                    }
                }

                S::ShapeDataStr => {
                    if c == 34 {
                        self.pop();
                        self.emit(T::ShapeData, p, p + 1);
                    } else {
                        let e = src[p..].find('"').map_or(n, |at| at + p);
                        let text = RE_BR.replace(&src[p..e], |_| "<br/>".into());
                        self.out.push(Token::new(T::ShapeData, text, p));
                        self.p = e;
                    }
                }
            }

            if !ok {
                self.out.push(Token::new(T::Error, "", self.p));
                return self.out;
            }
        }

        self.out.push(Token::new(T::End, "", self.n));
        self.out
    }
}

pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    Lexer {
        src,
        n: src.len(),
        out: Vec::new(),
        stack: Vec::new(),
        state: S::Initial,
        p: 0,
        first_graph: true,
        dir_next: src.find("direction"),
        at_next: src.find('@'),
        line_end: None,
        dir_failed: 0,
        run_end: 0,
        run_at: None,
        ws: None,
    }
    .run()
}
