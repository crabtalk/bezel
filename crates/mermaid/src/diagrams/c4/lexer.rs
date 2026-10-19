//! Pele's `diagrams/c4/lexer.ts`.

use super::tokens::*;
use crate::{
    js::Lazy,
    util::chars::{code_at, is_line_end, is_space, is_word, width_at},
};

static RE_DIRECTION: [Lazy; 4] = [
    Lazy::new(r".*direction\s+TB[^\n]*", "y"),
    Lazy::new(r".*direction\s+BT[^\n]*", "y"),
    Lazy::new(r".*direction\s+RL[^\n]*", "y"),
    Lazy::new(r".*direction\s+LR[^\n]*", "y"),
];

const KEYWORDS: [(&str, u8); 48] = [
    ("C4Context", C4_CONTEXT),
    ("C4Container", C4_CONTAINER),
    ("C4Component", C4_COMPONENT),
    ("C4Dynamic", C4_DYNAMIC),
    ("C4Deployment", C4_DEPLOYMENT),
    ("Person", PERSON),
    ("Person_Ext", PERSON_EXT),
    ("System", SYSTEM),
    ("SystemDb", SYSTEM_DB),
    ("SystemQueue", SYSTEM_QUEUE),
    ("System_Ext", SYSTEM_EXT),
    ("SystemDb_Ext", SYSTEM_EXT_DB),
    ("SystemQueue_Ext", SYSTEM_EXT_QUEUE),
    ("Container", CONTAINER),
    ("ContainerDb", CONTAINER_DB),
    ("ContainerQueue", CONTAINER_QUEUE),
    ("Container_Ext", CONTAINER_EXT),
    ("ContainerDb_Ext", CONTAINER_EXT_DB),
    ("ContainerQueue_Ext", CONTAINER_EXT_QUEUE),
    ("Component", COMPONENT),
    ("ComponentDb", COMPONENT_DB),
    ("ComponentQueue", COMPONENT_QUEUE),
    ("Component_Ext", COMPONENT_EXT),
    ("ComponentDb_Ext", COMPONENT_EXT_DB),
    ("ComponentQueue_Ext", COMPONENT_EXT_QUEUE),
    ("Enterprise_Boundary", ENTERPRISE_BOUNDARY),
    ("System_Boundary", SYSTEM_BOUNDARY),
    ("Boundary", BOUNDARY),
    ("Container_Boundary", CONTAINER_BOUNDARY),
    ("Deployment_Node", NODE),
    ("Node", NODE),
    ("Node_L", NODE_L),
    ("Node_R", NODE_R),
    ("Rel", REL),
    ("BiRel", BIREL),
    ("Rel_Up", REL_U),
    ("Rel_U", REL_U),
    ("Rel_Down", REL_D),
    ("Rel_D", REL_D),
    ("Rel_Left", REL_L),
    ("Rel_L", REL_L),
    ("Rel_Right", REL_R),
    ("Rel_R", REL_R),
    ("Rel_Back", REL_B),
    ("RelIndex", REL_INDEX),
    ("UpdateElementStyle", UPDATE_EL_STYLE),
    ("UpdateRelStyle", UPDATE_REL_STYLE),
    ("UpdateLayoutConfig", UPDATE_LAYOUT_CONFIG),
];

/// Token text is `src[starts[i]..ends[i]]`, byte offsets; nothing is copied
/// until the parser asks for it.
#[derive(Default)]
pub struct Tokens {
    pub types: Vec<u8>,
    pub starts: Vec<usize>,
    pub ends: Vec<usize>,
}

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
    out: Tokens,
}

impl Lexer<'_> {
    fn emit(&mut self, ty: u8, s: usize, e: usize) {
        self.out.types.push(ty);
        self.out.starts.push(s);
        self.out.ends.push(e);
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
    fn statement(&mut self, word: &str, ty: u8) -> bool {
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
pub fn tokenize(src: &str) -> Tokens {
    let n = src.len();
    let mut lx = Lexer {
        src,
        n,
        p: 0,
        out: Tokens::default(),
    };
    let mut state = S::Initial;
    let mut acc_value = ACC_TITLE_VALUE;
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
            lx.emit(END, n, n);
            break;
        }
        let p = lx.p;
        if p >= n {
            done = true;
        }
        let c = lx.code(p);

        if state == S::Initial {
            if p >= n {
                lx.emit(EOF, n, n);
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
                        for (i, re) in RE_DIRECTION.iter().enumerate() {
                            if let Some(end) = re.match_end(src, p) {
                                lx.emit(DIRECTION_TB + i as u8, p, end);
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
            if c == 116 && lx.statement("title", TITLE) {
                continue;
            }
            if c == 97 {
                if lx.statement("accDescription", ACC_DESCRIPTION) {
                    continue;
                }
                let title = src[p..].starts_with("accTitle");
                if title || src[p..].starts_with("accDescr") {
                    let q = lx.skip_ws(p + 8);
                    let d = lx.code(q);
                    if d == 58 {
                        let end = lx.skip_ws(q + 1);
                        lx.emit(if title { ACC_TITLE } else { ACC_DESCR }, p, end);
                        acc_value = if title {
                            ACC_TITLE_VALUE
                        } else {
                            ACC_DESCR_VALUE
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
                    Some(last) => lx.emit(NEWLINE, p, last + 1),
                    None => lx.p = q,
                }
                continue;
            }
            if c == 123 {
                lx.emit(LBRACE, p, p + 1);
                continue;
            }
            if c == 125 {
                lx.emit(RBRACE, p, p + 1);
                continue;
            }
            if is_word(c) {
                let mut q = p + 1;
                while q < n && is_word(lx.code(q)) {
                    q += 1;
                }
                let ty = if q - p <= 19 {
                    KEYWORDS
                        .iter()
                        .find(|(word, _)| *word == &src[p..q])
                        .map(|&(_, ty)| ty)
                } else {
                    None
                };
                if let Some(ty) = ty {
                    if ty > C4_DEPLOYMENT {
                        state = S::Struct;
                    }
                    lx.emit(ty, p, q);
                    continue;
                }
            }
            lx.emit(ERROR, p, p);
            break;
        }

        match state {
            S::Initial => unreachable!(),
            S::Struct => {
                if p >= n {
                    lx.emit(EOF_IN_STRUCT, n, n);
                } else if c == 40 {
                    let mut q = p + 1;
                    while lx.code(q) == 32 {
                        q += 1;
                    }
                    if lx.code(q) == 44 {
                        lx.emit(ATTRIBUTE_EMPTY, p, q + 1);
                    } else {
                        lx.p += 1;
                    }
                    state = S::Attribute;
                } else if c == 41 {
                    lx.p += 1;
                    state = S::Initial;
                } else {
                    lx.emit(ERROR, p, p);
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
                        lx.emit(ATTRIBUTE_EMPTY, p, p + 2);
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
                            lx.emit(ATTRIBUTE_EMPTY, p, q + 2);
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
                        lx.emit(STR, p, end);
                    }
                }
            }
            S::String => {
                if c == 34 {
                    lx.p += 1;
                    state = S::Attribute;
                } else {
                    let end = lx.up_to('"', p);
                    lx.emit(STR, p, end);
                }
            }
            S::StringKv => {
                let end = lx.up_to('=', p);
                lx.emit(STR_KEY, p, end);
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
                    lx.emit(ERROR, p, p);
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
                    lx.emit(STR_VALUE, p, end);
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
                    lx.emit(ACC_DESCR_MULTILINE_VALUE, p, end);
                }
            }
        }
    }

    lx.out
}
