//! Pele's `diagrams/c4/lexer.ts`.

use super::tokens::T;
use crate::{
    js::Lazy,
    token::Token,
    util::chars::{code_at, is_line_end, is_space, is_word, width_at},
};

static RE_DIRECTION: [(T, Lazy); 4] = [
    (T::DirectionTb, Lazy::new(r".*direction\s+TB[^\n]*", "y")),
    (T::DirectionBt, Lazy::new(r".*direction\s+BT[^\n]*", "y")),
    (T::DirectionRl, Lazy::new(r".*direction\s+RL[^\n]*", "y")),
    (T::DirectionLr, Lazy::new(r".*direction\s+LR[^\n]*", "y")),
];

const HEADERS: [(&str, T); 5] = [
    ("C4Context", T::C4Context),
    ("C4Container", T::C4Container),
    ("C4Component", T::C4Component),
    ("C4Dynamic", T::C4Dynamic),
    ("C4Deployment", T::C4Deployment),
];

/// Keywords that open a statement's attributes.
const STATEMENTS: [(&str, T); 43] = [
    ("Person", T::Person),
    ("Person_Ext", T::PersonExt),
    ("System", T::System),
    ("SystemDb", T::SystemDb),
    ("SystemQueue", T::SystemQueue),
    ("System_Ext", T::SystemExt),
    ("SystemDb_Ext", T::SystemExtDb),
    ("SystemQueue_Ext", T::SystemExtQueue),
    ("Container", T::Container),
    ("ContainerDb", T::ContainerDb),
    ("ContainerQueue", T::ContainerQueue),
    ("Container_Ext", T::ContainerExt),
    ("ContainerDb_Ext", T::ContainerExtDb),
    ("ContainerQueue_Ext", T::ContainerExtQueue),
    ("Component", T::Component),
    ("ComponentDb", T::ComponentDb),
    ("ComponentQueue", T::ComponentQueue),
    ("Component_Ext", T::ComponentExt),
    ("ComponentDb_Ext", T::ComponentExtDb),
    ("ComponentQueue_Ext", T::ComponentExtQueue),
    ("Enterprise_Boundary", T::EnterpriseBoundary),
    ("System_Boundary", T::SystemBoundary),
    ("Boundary", T::Boundary),
    ("Container_Boundary", T::ContainerBoundary),
    ("Deployment_Node", T::Node),
    ("Node", T::Node),
    ("Node_L", T::NodeL),
    ("Node_R", T::NodeR),
    ("Rel", T::Rel),
    ("BiRel", T::Birel),
    ("Rel_Up", T::RelU),
    ("Rel_U", T::RelU),
    ("Rel_Down", T::RelD),
    ("Rel_D", T::RelD),
    ("Rel_Left", T::RelL),
    ("Rel_L", T::RelL),
    ("Rel_Right", T::RelR),
    ("Rel_R", T::RelR),
    ("Rel_Back", T::RelB),
    ("RelIndex", T::RelIndex),
    ("UpdateElementStyle", T::UpdateElStyle),
    ("UpdateRelStyle", T::UpdateRelStyle),
    ("UpdateLayoutConfig", T::UpdateLayoutConfig),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Struct,
    Attribute,
    String,
    StringKv,
    StringKvKey,
    StringKvValue,
    Acc,
    AccDescrMultiline,
}

struct Lexer<'a> {
    src: &'a str,
    n: usize,
    p: usize,
    out: Vec<Token<'a, T>>,
}

impl<'a> Lexer<'a> {
    fn emit(&mut self, ty: T, s: usize, e: usize) {
        self.out.push(Token::new(ty, &self.src[s..e], s));
        self.p = e;
    }

    fn code(&self, at: usize) -> u32 {
        code_at(self.src, at)
    }

    fn skip_ws(&self, mut q: usize) -> usize {
        while q < self.n && is_space(self.code(q)) {
            q += width_at(self.src, q);
        }
        q
    }

    fn up_to(&self, ch: char, from: usize) -> usize {
        self.src
            .get(from..)
            .and_then(|rest| rest.find(ch))
            .map_or(self.n, |q| from + q)
    }

    /// `word\s[^#\n;]+`
    fn statement(&mut self, word: &str, ty: T) -> bool {
        let p = self.p;
        let q = p + word.len();
        if !self.src[p..].starts_with(word) || !is_space(self.code(q)) {
            return false;
        }
        let from = q + width_at(self.src, q);
        let bytes = self.src.as_bytes();
        let mut q = from;
        while q < self.n {
            let c = bytes[q];
            if c == b'#' || c == b'\n' || c == b';' {
                break;
            }
            q += 1;
        }
        if q == from {
            return false;
        }
        self.emit(ty, p, q);
        true
    }

    /// Where the first line terminator at or after `from` is, or the end.
    fn line_end(&self, from: usize) -> usize {
        self.src[from..]
            .char_indices()
            .find(|&(_, c)| is_line_end(u32::from(c)))
            .map_or(self.n, |(at, _)| from + at)
    }
}

/// Follows the lexer Jison compiles from c4Diagram.jison: its rule order, its
/// start conditions, and what it returns once the input runs out in each of
/// them.
pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        p: 0,
        out: Vec::new(),
    };
    let mut state = S::Initial;
    let mut acc_value = T::AccTitleValue;
    let mut done = false;
    // Where the next `direction` is; -2 until the first search, -1 when
    // there is none.
    let mut dir_next: i64 = -2;
    let mut line_end: i64 = -1;
    let mut dir_failed: usize = 0;

    loop {
        // Jison notes the end of input one call before it reports it, so a
        // rule that matches nothing gets one last turn.
        if done {
            lx.emit(T::End, n, n);
            break;
        }
        let p = lx.p;
        if p >= n {
            done = true;
        }
        let c = lx.code(p);

        if state == S::Initial {
            if p >= n {
                lx.emit(T::Eof, n, n);
                continue;
            }
            if dir_next != -1 && p >= dir_failed {
                if dir_next < p as i64 {
                    dir_next = src[p..].find("direction").map_or(-1, |q| (p + q) as i64);
                }
                if dir_next != -1 {
                    if line_end < p as i64 {
                        line_end = lx.line_end(p) as i64;
                    }
                    if dir_next < line_end {
                        let mut matched = false;
                        for (kind, re) in &RE_DIRECTION {
                            if let Some(end) = re.match_end(src, p) {
                                lx.emit(*kind, p, end);
                                matched = true;
                                break;
                            }
                        }
                        if matched {
                            continue;
                        }
                        // No direction statement starts anywhere in the rest
                        // of this line either.
                        dir_failed = line_end as usize;
                    }
                }
            }
            if c == 116 && lx.statement("title", T::Title) {
                continue;
            }
            if c == 97 {
                if lx.statement("accDescription", T::AccDescription) {
                    continue;
                }
                let title = src[p..].starts_with("accTitle");
                if title || src[p..].starts_with("accDescr") {
                    let q = lx.skip_ws(p + 8);
                    let d = lx.code(q);
                    if d == 58 {
                        let end = lx.skip_ws(q + 1);
                        lx.emit(if title { T::AccTitle } else { T::AccDescr }, p, end);
                        acc_value = if title {
                            T::AccTitleValue
                        } else {
                            T::AccDescrValue
                        };
                        state = S::Acc;
                        continue;
                    }
                    if d == 123 && !title {
                        lx.p = lx.skip_ws(q + 1);
                        state = S::AccDescrMultiline;
                        continue;
                    }
                }
            }
            if c == 37 && lx.code(p + 1) == 37 {
                // A comment takes the line breaks after it, so no NEWLINE
                // follows.
                let mut q = lx.up_to('\n', p + 2);
                while q < n && (lx.code(q) == 10 || lx.code(q) == 13) {
                    q += 1;
                }
                lx.p = q;
                continue;
            }
            if is_space(c) {
                let mut q = p;
                let mut last = None;
                while q < n {
                    let d = lx.code(q);
                    if !is_space(d) {
                        break;
                    }
                    if d == 10 {
                        last = Some(q);
                    }
                    q += width_at(src, q);
                }
                match last {
                    Some(last) => lx.emit(T::Newline, p, last + 1),
                    None => lx.p = q,
                }
                continue;
            }
            if c == 123 {
                lx.emit(T::Lbrace, p, p + 1);
                continue;
            }
            if c == 125 {
                lx.emit(T::Rbrace, p, p + 1);
                continue;
            }
            if is_word(c) {
                let mut q = p + 1;
                while q < n && is_word(lx.code(q)) {
                    q += 1;
                }
                let find = |table: &[(&str, T)]| {
                    table
                        .iter()
                        .find(|(word, _)| *word == &src[p..q])
                        .map(|&(_, ty)| ty)
                };
                if q - p <= 19 {
                    if let Some(ty) = find(&HEADERS) {
                        lx.emit(ty, p, q);
                        continue;
                    }
                    if let Some(ty) = find(&STATEMENTS) {
                        state = S::Struct;
                        lx.emit(ty, p, q);
                        continue;
                    }
                }
            }
            lx.emit(T::Error, p, p);
            break;
        }

        match state {
            S::Initial => unreachable!(),
            S::Struct => {
                if p >= n {
                    lx.emit(T::EofInStruct, n, n);
                } else if c == 40 {
                    let mut q = p + 1;
                    while lx.code(q) == 32 {
                        q += 1;
                    }
                    if lx.code(q) == 44 {
                        lx.emit(T::AttributeEmpty, p, q + 1);
                    } else {
                        lx.p += 1;
                    }
                    state = S::Attribute;
                } else if c == 41 {
                    lx.p += 1;
                    state = S::Initial;
                } else {
                    lx.emit(T::Error, p, p);
                    break;
                }
            }
            S::Attribute => {
                if p >= n {
                    continue;
                }
                if c == 41 {
                    lx.p += 1;
                    state = S::Initial;
                } else if c == 44 {
                    if lx.code(p + 1) == 44 {
                        lx.emit(T::AttributeEmpty, p, p + 2);
                    } else {
                        lx.p += 1;
                    }
                } else {
                    let mut q = p;
                    while lx.code(q) == 32 {
                        q += 1;
                    }
                    let d = lx.code(q);
                    if d == 34 {
                        if lx.code(q + 1) == 34 {
                            lx.emit(T::AttributeEmpty, p, q + 2);
                        } else {
                            lx.p = q + 1;
                            state = S::String;
                        }
                    } else if d == 36 {
                        lx.p = q + 1;
                        state = S::StringKv;
                    } else {
                        // An unquoted value runs to the next comma, through
                        // closing parentheses and line breaks.
                        let end = lx.up_to(',', p);
                        lx.emit(T::Str, p, end);
                    }
                }
            }
            S::String => {
                if c == 34 {
                    lx.p += 1;
                    state = S::Attribute;
                } else {
                    let end = lx.up_to('"', p);
                    lx.emit(T::Str, p, end);
                }
            }
            S::StringKv => {
                let end = lx.up_to('=', p);
                lx.emit(T::StrKey, p, end);
                state = S::StringKvKey;
            }
            S::StringKvKey => {
                if p >= n {
                    continue;
                }
                let mut q = p + 1;
                if c == 61 {
                    while lx.code(q) == 32 {
                        q += 1;
                    }
                }
                if c != 61 || lx.code(q) != 34 {
                    lx.emit(T::Error, p, p);
                    break;
                }
                lx.p = q + 1;
                state = S::StringKvValue;
            }
            S::StringKvValue => {
                if p >= n {
                    continue;
                }
                if c == 34 {
                    lx.p += 1;
                    state = S::Attribute;
                } else {
                    let end = lx.up_to('"', p);
                    lx.emit(T::StrValue, p, end);
                }
            }
            S::Acc => {
                let end = lx.up_to('\n', p);
                lx.emit(acc_value, p, end);
                state = S::Initial;
            }
            S::AccDescrMultiline => {
                if c == 125 {
                    lx.p += 1;
                    state = S::Initial;
                } else {
                    let end = lx.up_to('}', p);
                    lx.emit(T::AccDescrMultilineValue, p, end);
                }
            }
        }
    }

    lx.out
}
