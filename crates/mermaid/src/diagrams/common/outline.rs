//! The indented outline grammar Mermaid's mindmap and kanban share. Kanban
//! adds `@{ … }` metadata after a node and keeps `@` out of node ids. Pele's
//! `diagrams/common/outline.ts`.

use crate::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
    util::chars::{code_at, is_word, width_at},
};

pub mod t {
    pub const END: i32 = 0;
    pub const ERROR: i32 = 1;
    pub const SPACELINE: i32 = 2;
    pub const NL: i32 = 3;
    pub const EOF: i32 = 4;
    pub const KEYWORD: i32 = 5;
    pub const SPACELIST: i32 = 6;
    pub const ICON: i32 = 7;
    pub const CLASS: i32 = 8;
    pub const NODE_DSTART: i32 = 9;
    pub const NODE_ID: i32 = 10;
    pub const NODE_DESCR: i32 = 11;
    pub const NODE_DEND: i32 = 12;
    pub const SHAPE_DATA: i32 = 13;
}

/// As the generated parser prints them. It has no name for the end of input
/// and prints its number.
const NAMES: [&str; 14] = [
    "1",
    "INVALID",
    "SPACELINE",
    "NL",
    "EOF",
    "",
    "SPACELIST",
    "ICON",
    "CLASS",
    "NODE_DSTART",
    "NODE_ID",
    "NODE_DESCR",
    "NODE_DEND",
    "SHAPE_DATA",
];

const STOP: &str = "'SPACELINE', 'NL', 'EOF'";

pub mod node_type {
    pub const DEFAULT: i32 = 0;
    pub const NO_BORDER: i32 = 0;
    pub const ROUNDED_RECT: i32 = 1;
    pub const RECT: i32 = 2;
    pub const CIRCLE: i32 = 3;
    pub const CLOUD: i32 = 4;
    pub const BANG: i32 = 5;
    pub const HEXAGON: i32 = 6;
}

const S_INITIAL: u8 = 0;
const S_CLASS: u8 = 1;
const S_ICON: u8 = 2;
const S_NODE: u8 = 3;
const S_NSTR: u8 = 4;
const S_NSTR2: u8 = 5;
const S_DATA: u8 = 6;
const S_DATA_STR: u8 = 7;

static RE_SPACE: Lazy = Lazy::new(r"\s+", "y");
static RE_LINE: Lazy = Lazy::new(r".+", "y");
static RE_ID: Lazy = Lazy::new(r"[^(\[\n){}]+", "y");
static RE_ID_KANBAN: Lazy = Lazy::new(r"[^(\[\n){}@]+", "y");
static RE_DESCR: Lazy = Lazy::new(r"[^)\](}]+", "y");
static RE_MD: Lazy = Lazy::new(r#"[^`"]+"#, "y");
static RE_DATA: Lazy = Lazy::new(r#"[^}^"]+"#, "y");
static RE_BR: Lazy = Lazy::new(r"\n\s*", "g");

pub fn token_name(kind: i32, kanban: bool) -> &'static str {
    if kind == t::KEYWORD {
        if kanban { "KANBAN" } else { "MINDMAP" }
    } else {
        NAMES[kind as usize]
    }
}

/// Any opening delimiter pairs with any closing one, as in Mermaid.
pub fn get_type(start: &str, end: &str) -> i32 {
    match start {
        "[" => node_type::RECT,
        "(" => {
            if end == ")" {
                node_type::ROUNDED_RECT
            } else {
                node_type::CLOUD
            }
        }
        "((" => node_type::CIRCLE,
        ")" => node_type::CLOUD,
        "))" => node_type::BANG,
        "{{" => node_type::HEXAGON,
        _ => node_type::DEFAULT,
    }
}

/// What the grammar's actions call.
pub trait OutlineDb {
    fn get_type(&self, start: &str, end: &str) -> i32 {
        get_type(start, end)
    }
    fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        kind: i32,
        shape_data: Option<&str>,
    ) -> Result<(), Error>;
    fn decorate_node(&mut self, icon: Option<&str>, class: Option<&str>);
}

#[derive(Default)]
pub struct Tokens {
    pub types: Vec<i32>,
    pub texts: Vec<String>,
    pub starts: Vec<usize>,
}

/// Jison's case-insensitive flag folds ASCII letters only.
fn letters(src: &str, p: usize, word: &str) -> bool {
    let bytes = src.as_bytes();
    word.bytes()
        .enumerate()
        .all(|(k, w)| bytes.get(p + k).is_some_and(|&c| (c | 32) == w))
}

pub fn tokenize(src: &str, kanban: bool) -> Tokens {
    let n = src.len();
    let mut out = Tokens::default();
    let keyword = if kanban { "kanban" } else { "mindmap" };
    let re_id: &Lazy = if kanban { &RE_ID_KANBAN } else { &RE_ID };
    let mut state = S_INITIAL;
    let mut p = 0;
    let mut next_break: isize = -1;

    let at = |re: &Lazy, from: usize| re.match_end(src, from);
    let up_to = |ch: char, p: usize| src[p..].find(ch).map_or(n, |e| p + e);

    macro_rules! put {
        ($kind:expr, $text:expr, $end:expr) => {{
            let end = $end;
            out.types.push($kind);
            out.texts.push($text);
            out.starts.push(p);
            p = end;
        }};
    }
    macro_rules! emit {
        ($kind:expr, $end:expr, $next:expr) => {{
            let end = $end;
            put!($kind, src[p..end].to_string(), end);
            state = $next;
        }};
    }

    while p < n {
        let c = code_at(src, p);
        let d = code_at(src, p + 1);
        if state == S_INITIAL {
            let w = if c <= 32 || c >= 160 {
                at(&RE_SPACE, p).unwrap_or(p).max(p)
            } else {
                p
            };
            if kanban && c == 64 && d == 123 {
                put!(t::SHAPE_DATA, String::new(), p + 2);
                state = S_DATA;
            } else if code_at(src, w) == 37 && code_at(src, w + 1) == 37 {
                emit!(t::SPACELINE, at(&RE_LINE, w).unwrap_or(w), S_INITIAL);
            } else if w > p {
                if next_break < p as isize {
                    next_break = up_to('\n', p) as isize;
                }
                if next_break >= w as isize {
                    emit!(t::SPACELIST, w, S_INITIAL);
                } else if let Some(e) = src[..w].rfind('\n').filter(|&e| e > p) {
                    emit!(t::SPACELINE, e + 1, S_INITIAL);
                } else {
                    emit!(t::NL, p + width_at(src, p), S_INITIAL);
                }
            } else if letters(src, p, keyword) && !is_word(code_at(src, p + keyword.len())) {
                emit!(t::KEYWORD, p + keyword.len(), S_INITIAL);
            } else if c == 58 && d == 58 && code_at(src, p + 2) == 58 {
                p += 3;
                state = S_CLASS;
            } else if c == 58 && d == 58 && letters(src, p + 2, "icon") && code_at(src, p + 6) == 40
            {
                p += 7;
                state = S_ICON;
            } else if (c == 45 && d == 41)
                || (c == 40 && (d == 45 || d == 40))
                || ((c == 41 || c == 123) && d == c)
            {
                emit!(t::NODE_DSTART, p + 2, S_NODE);
            } else if c == 41 || c == 40 || c == 91 {
                emit!(t::NODE_DSTART, p + 1, S_NODE);
            } else if let Some(e) = at(re_id, p) {
                emit!(t::NODE_ID, e, S_INITIAL);
            } else {
                break;
            }
        } else if state == S_NODE {
            if c == 34 {
                state = if d == 96 { S_NSTR2 } else { S_NSTR };
                p += if d == 96 { 2 } else { 1 };
            } else if c == 41 || c == 40 {
                let end = if d == c || (c == 40 && d == 45) {
                    p + 2
                } else {
                    p + 1
                };
                emit!(t::NODE_DEND, end, S_INITIAL);
            } else if c == 93 {
                emit!(t::NODE_DEND, p + 1, S_INITIAL);
            } else if (c == 125 && d == 125) || (c == 45 && d == 41) {
                emit!(t::NODE_DEND, p + 2, S_INITIAL);
            } else {
                // A lone closing brace takes the rest of its line as text.
                let re: &Lazy = if c == 125 { &RE_LINE } else { &RE_DESCR };
                match at(re, p) {
                    Some(end) => emit!(t::NODE_DESCR, end, S_NODE),
                    None => break,
                }
            }
        } else if state == S_NSTR {
            if c == 34 {
                p += 1;
                state = S_NODE;
            } else {
                emit!(t::NODE_DESCR, up_to('"', p), S_NSTR);
            }
        } else if state == S_NSTR2 {
            if c == 96 && d == 34 {
                p += 2;
                state = S_NODE;
            } else if let Some(e) = at(&RE_MD, p) {
                emit!(t::NODE_DESCR, e, S_NSTR2);
            } else {
                break;
            }
        } else if state == S_CLASS {
            if let Some(e) = at(&RE_LINE, p) {
                emit!(t::CLASS, e, S_INITIAL);
            } else if c == 10 {
                p += 1;
                state = S_INITIAL;
            } else {
                break;
            }
        } else if state == S_ICON {
            if c == 41 {
                p += 1;
                state = S_INITIAL;
            } else {
                emit!(t::ICON, up_to(')', p), S_ICON);
            }
        } else if state == S_DATA {
            if c == 34 {
                emit!(t::SHAPE_DATA, p + 1, S_DATA_STR);
            } else if c == 125 {
                p += 1;
                state = S_INITIAL;
            } else if let Some(e) = at(&RE_DATA, p) {
                emit!(t::SHAPE_DATA, e, S_DATA);
            } else {
                break;
            }
        } else if c == 34 {
            emit!(t::SHAPE_DATA, p + 1, S_DATA);
        } else {
            let e = up_to('"', p);
            let text = RE_BR.replace(&src[p..e], |_| "<br/>".into());
            put!(t::SHAPE_DATA, text, e);
        }
    }

    let mut close = |kind: i32, at: usize| {
        out.types.push(kind);
        out.texts.push(String::new());
        out.starts.push(at);
    };
    if p < n {
        close(t::ERROR, p);
    } else {
        if state == S_INITIAL {
            close(t::EOF, p);
        }
        close(t::END, n);
    }
    out
}

fn is_stop(kind: i32) -> bool {
    kind == t::SPACELINE || kind == t::NL || kind == t::EOF
}

fn starts_statement(kind: i32) -> bool {
    matches!(
        kind,
        t::SPACELINE | t::SPACELIST | t::ICON | t::CLASS | t::NODE_DSTART | t::NODE_ID
    )
}

/// Makes the calls on `db` the grammar's actions make, in the same order,
/// and reports errors in the state where the generated parser would.
pub fn parse_outline(src: &str, db: &mut dyn OutlineDb, kanban: bool) -> Result<(), Error> {
    let diagram = if kanban { Kind::Kanban } else { Kind::Mindmap };
    let Tokens {
        types,
        texts,
        starts,
    } = tokenize(src, kanban);
    let data = if kanban { ", 'SHAPE_DATA'" } else { "" };
    let mut i = 0;
    let kind_at = |i: usize| types.get(i).copied().unwrap_or(t::END);

    let fail = |i: usize, expected: &str| -> Error {
        let kind = kind_at(i);
        if kind == t::ERROR {
            return Error::at(diagram, src, starts[i], "", true);
        }
        Error::at(
            diagram,
            src,
            starts[i],
            &format!("Expecting {expected}, got '{}'", token_name(kind, kanban)),
            false,
        )
    };

    let keyword = format!("'{}'", token_name(t::KEYWORD, kanban));
    if kind_at(i) == t::SPACELINE {
        i += 1;
        while kind_at(i) == t::SPACELINE || kind_at(i) == t::NL {
            i += 1;
        }
        if kind_at(i) != t::KEYWORD {
            return Err(fail(i, &format!("'SPACELINE', 'NL', {keyword}")));
        }
    } else if kind_at(i) != t::KEYWORD {
        return Err(fail(i, &format!("'SPACELINE', {keyword}")));
    }
    i += 1;
    let mut first = "'SPACELINE', 'NL', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'";
    if kind_at(i) == t::NL {
        i += 1;
        first = "'SPACELINE', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'";
    }

    loop {
        let mut kind = kind_at(i);
        let mut level = 0;
        let indented = kind == t::SPACELIST;
        if indented {
            level = js::len(&texts[i]);
            i += 1;
            kind = kind_at(i);
        }
        if kind == t::NODE_ID || kind == t::NODE_DSTART {
            let mut id: Option<String> = None;
            let descr: String;
            let mut node_kind = 0;
            if kind == t::NODE_ID {
                id = Some(texts[i].clone());
                i += 1;
            }
            if kind_at(i) == t::NODE_DSTART {
                let start = texts[i].clone();
                i += 1;
                if kind_at(i) != t::NODE_DESCR {
                    return Err(fail(i, "'NODE_DESCR'"));
                }
                descr = texts[i].clone();
                i += 1;
                if kind_at(i) != t::NODE_DEND {
                    return Err(fail(i, "'NODE_DEND'"));
                }
                let end = texts[i].clone();
                i += 1;
                if !is_stop(kind_at(i)) && kind_at(i) != t::SHAPE_DATA {
                    return Err(fail(i, &format!("{STOP}{data}")));
                }
                node_kind = db.get_type(&start, &end);
            } else {
                if !is_stop(kind_at(i)) && kind_at(i) != t::SHAPE_DATA {
                    return Err(fail(i, &format!("{STOP}, 'NODE_DSTART'{data}")));
                }
                descr = id.clone().unwrap_or_default();
            }
            let node_id = id.unwrap_or_else(|| descr.clone());
            if kind_at(i) == t::SHAPE_DATA {
                let mut shape_data = texts[i].clone();
                i += 1;
                while kind_at(i) == t::SHAPE_DATA {
                    shape_data.push_str(&texts[i]);
                    i += 1;
                }
                if !is_stop(kind_at(i)) {
                    return Err(fail(i, &format!("{STOP}{data}")));
                }
                db.add_node(level, &node_id, &descr, node_kind, Some(&shape_data))?;
            } else {
                db.add_node(level, &node_id, &descr, node_kind, None)?;
            }
        } else if kind == t::ICON || kind == t::CLASS {
            let text = texts[i].clone();
            i += 1;
            if !is_stop(kind_at(i)) {
                return Err(fail(i, STOP));
            }
            if kind == t::ICON {
                db.decorate_node(Some(&text), None);
            } else {
                db.decorate_node(None, Some(&text));
            }
        } else if indented {
            if !is_stop(kind) {
                return Err(fail(
                    i,
                    &format!("{STOP}, 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"),
                ));
            }
        } else if kind == t::SPACELINE {
            i += 1;
            if !is_stop(kind_at(i)) {
                return Err(fail(i, STOP));
            }
        } else {
            return Err(fail(i, first));
        }

        i += 1;
        while kind_at(i) == t::NL || kind_at(i) == t::EOF {
            i += 1;
        }
        if kind_at(i) == t::END {
            return Ok(());
        }
        if !starts_statement(kind_at(i)) {
            return Err(fail(
                i,
                &format!("{STOP}, 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"),
            ));
        }
    }
}
