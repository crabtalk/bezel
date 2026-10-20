//! Pele's `diagrams/c4/lexer.ts`.

use super::tokens::T;
use crate::diagrams::common::directions::Directions;
use crate::{
    js::Lazy,
    token::{Cursor, Token},
    util::chars::{is_space, is_word},
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

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Struct,
    Attribute,
    String,
    StringKv,
    StringKvKey,
    StringKvValue,
    AccTitle,
    AccDescr,
    AccDescrMultiline,
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    directions: Directions<T>,
}

impl<'s> Lexer<'s> {
    /// The end of the run of spaces at `from`.
    fn spaces_end(&self, mut from: usize) -> usize {
        while self.cur.char_at(from) == Some(' ') {
            from += 1;
        }
        from
    }

    /// `word\s[^#\n;]+`
    fn statement(&mut self, word: &str, kind: T) -> bool {
        let p = self.cur.p;
        let q = p + word.len();
        if !self.cur.starts_with(word, p) || !self.cur.char_at(q).is_some_and(is_space) {
            return false;
        }
        let from = self.cur.after(q);
        let end = self.cur.src[from..]
            .find(['#', '\n', ';'])
            .map_or(self.cur.src.len(), |at| from + at);
        if end == from {
            return false;
        }
        self.cur.emit_to(kind, end);
        true
    }

    /// `.*direction\s+XX[^\n]*`, tried before the other rules.
    fn direction(&mut self) -> bool {
        let Some((kind, end)) = self.directions.at(self.cur.src, self.cur.p) else {
            return false;
        };
        self.cur.emit_to(kind, end);
        true
    }

    /// Answers whether lexing goes on: false after text no rule matches.
    fn initial(&mut self, c: char) -> bool {
        let src = self.cur.src;
        let p = self.cur.p;
        if self.direction() {
            return true;
        }
        if c == 't' && self.statement("title", T::Title) {
            return true;
        }
        if c == 'a' {
            if self.statement("accDescription", T::AccDescription) {
                return true;
            }
            let title = self.cur.starts_with("accTitle", p);
            if title || self.cur.starts_with("accDescr", p) {
                let q = self.cur.space_end(p + 8);
                match self.cur.char_at(q) {
                    Some(':') => {
                        let end = self.cur.space_end(q + 1);
                        self.cur
                            .emit_to(if title { T::AccTitle } else { T::AccDescr }, end);
                        self.cur.state = if title { S::AccTitle } else { S::AccDescr };
                        return true;
                    }
                    Some('{') if !title => {
                        self.cur.p = self.cur.space_end(q + 1);
                        self.cur.state = S::AccDescrMultiline;
                        return true;
                    }
                    _ => {}
                }
            }
        }
        if self.cur.starts_with("%%", p) {
            // A comment takes the line breaks after it, so no NEWLINE
            // follows.
            let mut q = self.cur.find('\n', p + 2);
            while matches!(self.cur.char_at(q), Some('\n' | '\r')) {
                q += 1;
            }
            self.cur.p = q;
            return true;
        }
        if is_space(c) {
            let q = self.cur.space_end(p);
            match src[p..q].rfind('\n') {
                Some(last) => self.cur.emit_to(T::Newline, p + last + 1),
                None => self.cur.p = q,
            }
            return true;
        }
        match c {
            '{' => {
                self.cur.emit_to(T::Lbrace, p + 1);
                return true;
            }
            '}' => {
                self.cur.emit_to(T::Rbrace, p + 1);
                return true;
            }
            _ => {}
        }
        if is_word(c) {
            let q = self.cur.word_end(p);
            let word = &src[p..q];
            let find = |table: &[(&str, T)]| {
                table
                    .iter()
                    .find(|&&(name, _)| name == word)
                    .map(|&(_, kind)| kind)
            };
            if word.len() <= 19 {
                if let Some(kind) = find(&HEADERS) {
                    self.cur.emit_to(kind, q);
                    return true;
                }
                if let Some(kind) = find(&STATEMENTS) {
                    self.cur.state = S::Struct;
                    self.cur.emit_to(kind, q);
                    return true;
                }
            }
        }
        false
    }

    /// Answers whether lexing goes on: false after text no rule matches.
    /// `c` is `None` once the input has run out.
    fn step(&mut self, c: Option<char>) -> bool {
        let p = self.cur.p;
        let n = self.cur.src.len();
        match self.cur.state {
            S::Initial => match c {
                None => self.cur.emit_to(T::Eof, n),
                Some(c) => return self.initial(c),
            },
            S::Struct => match c {
                None => self.cur.emit_to(T::EofInStruct, n),
                Some('(') => {
                    let q = self.spaces_end(p + 1);
                    if self.cur.char_at(q) == Some(',') {
                        self.cur.emit_to(T::AttributeEmpty, q + 1);
                    } else {
                        self.cur.p += 1;
                    }
                    self.cur.state = S::Attribute;
                }
                Some(')') => {
                    self.cur.p += 1;
                    self.cur.state = S::Initial;
                }
                Some(_) => return false,
            },
            S::Attribute => match c {
                None => {}
                Some(')') => {
                    self.cur.p += 1;
                    self.cur.state = S::Initial;
                }
                Some(',') => {
                    if self.cur.char_at(p + 1) == Some(',') {
                        self.cur.emit_to(T::AttributeEmpty, p + 2);
                    } else {
                        self.cur.p += 1;
                    }
                }
                Some(_) => {
                    let q = self.spaces_end(p);
                    match self.cur.char_at(q) {
                        Some('"') if self.cur.char_at(q + 1) == Some('"') => {
                            self.cur.emit_to(T::AttributeEmpty, q + 2);
                        }
                        Some('"') => {
                            self.cur.p = q + 1;
                            self.cur.state = S::String;
                        }
                        Some('$') => {
                            self.cur.p = q + 1;
                            self.cur.state = S::StringKv;
                        }
                        _ => {
                            // An unquoted value runs to the next comma,
                            // through closing parentheses and line breaks.
                            let end = self.cur.find(',', p);
                            self.cur.emit_to(T::Str, end);
                        }
                    }
                }
            },
            S::String => {
                if c == Some('"') {
                    self.cur.p += 1;
                    self.cur.state = S::Attribute;
                } else {
                    let end = self.cur.find('"', p);
                    self.cur.emit_to(T::Str, end);
                }
            }
            S::StringKv => {
                let end = self.cur.find('=', p);
                self.cur.emit_to(T::StrKey, end);
                self.cur.state = S::StringKvKey;
            }
            S::StringKvKey => match c {
                None => {}
                Some('=') => {
                    let q = self.spaces_end(p + 1);
                    if self.cur.char_at(q) != Some('"') {
                        return false;
                    }
                    self.cur.p = q + 1;
                    self.cur.state = S::StringKvValue;
                }
                Some(_) => return false,
            },
            S::StringKvValue => match c {
                None => {}
                Some('"') => {
                    self.cur.p += 1;
                    self.cur.state = S::Attribute;
                }
                Some(_) => {
                    let end = self.cur.find('"', p);
                    self.cur.emit_to(T::StrValue, end);
                }
            },
            S::AccTitle | S::AccDescr => {
                let kind = if self.cur.state == S::AccTitle {
                    T::AccTitleValue
                } else {
                    T::AccDescrValue
                };
                let end = self.cur.line_end(p);
                self.cur.emit_to(kind, end);
                self.cur.state = S::Initial;
            }
            S::AccDescrMultiline => {
                if c == Some('}') {
                    self.cur.p += 1;
                    self.cur.state = S::Initial;
                } else {
                    let end = self.cur.find('}', p);
                    self.cur.emit_to(T::AccDescrMultilineValue, end);
                }
            }
        }
        true
    }
}

/// Follows the lexer Jison compiles from c4Diagram.jison: its rule order, its
/// start conditions, and what it returns once the input runs out in each of
/// them.
pub fn tokenize(src: &str) -> Vec<Token<'_, T>> {
    let n = src.len();
    let mut lx = Lexer {
        cur: Cursor::new(src, S::Initial),
        directions: Directions::new(src, &RE_DIRECTION),
    };
    // Jison notes the end of input one call before it reports it, so a rule
    // that matches nothing gets one last turn.
    let mut done = false;
    while !done {
        let p = lx.cur.p;
        let c = lx.cur.char_at(p);
        done = c.is_none();
        if !lx.step(c) {
            lx.cur.emit(T::Error, p, p);
            return lx.cur.tokens;
        }
    }
    lx.cur.emit(T::End, n, n);
    lx.cur.tokens
}
