//! The indented outline grammar Mermaid's mindmap and kanban share. Kanban
//! adds `@{ … }` metadata after a node and keeps `@` out of node ids. Pele's
//! `diagrams/common/outline.ts`.

use serde_repr::Serialize_repr;
use std::borrow::Cow;

use crate::{
    detect::Kind,
    error::Error,
    svg::shapes::Shape,
    token::{Cursor, Grammar, Stream, Token, grammar},
    util::chars::{is_space, width_at},
};

grammar! {
    /// The outline lexer's token kinds.
    pub enum T {
        End = "$end",
        Error = "INVALID",
        Spaceline = "SPACELINE",
        Nl = "NL",
        Eof = "EOF",
        Mindmap = "MINDMAP",
        Kanban = "KANBAN",
        Spacelist = "SPACELIST",
        Icon = "ICON",
        Class = "CLASS",
        NodeDstart = "NODE_DSTART",
        NodeId = "NODE_ID",
        NodeDescr = "NODE_DESCR",
        NodeDend = "NODE_DEND",
        ShapeData = "SHAPE_DATA",
    }

    fn got(self, _text, _previous) {
        // The generated parser has no name for the end of input and prints
        // its number.
        match self {
            Self::End => "1",
            _ => self.name(),
        }
    }
}

impl T {
    fn is_stop(self) -> bool {
        matches!(self, Self::Spaceline | Self::Nl | Self::Eof)
    }

    fn starts_statement(self) -> bool {
        matches!(
            self,
            Self::Spaceline
                | Self::Spacelist
                | Self::Icon
                | Self::Class
                | Self::NodeDstart
                | Self::NodeId
        )
    }
}

const STOP: &str = "'SPACELINE', 'NL', 'EOF'";

/// What `.` does not match.
const LINE_ENDS: &[char] = &['\n', '\r', '\u{2028}', '\u{2029}'];

/// A node's type, by the delimiters around its text, numbered as in Mermaid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum NodeType {
    /// No delimiters.
    #[default]
    Default = 0,
    RoundedRect = 1,
    Rect = 2,
    Circle = 3,
    Cloud = 4,
    Bang = 5,
    Hexagon = 6,
}

impl NodeType {
    /// Any opening delimiter pairs with any closing one, as in Mermaid.
    pub fn of_delimiters(start: &str, end: &str) -> Self {
        match start {
            "[" => NodeType::Rect,
            "(" if end == ")" => NodeType::RoundedRect,
            "(" | ")" => NodeType::Cloud,
            "((" => NodeType::Circle,
            "))" => NodeType::Bang,
            "{{" => NodeType::Hexagon,
            _ => NodeType::Default,
        }
    }

    /// The shape a node of this type is drawn as, `None` for no delimiters.
    pub fn shape(self) -> Option<Shape> {
        Some(match self {
            NodeType::Default => return None,
            NodeType::RoundedRect => Shape::Rounded,
            NodeType::Rect => Shape::Rect,
            NodeType::Circle => Shape::Circle,
            NodeType::Cloud => Shape::Cloud,
            NodeType::Bang => Shape::Bang,
            NodeType::Hexagon => Shape::Hexagon,
        })
    }
}

/// What the grammar's actions call.
pub trait OutlineDb {
    fn add_node(
        &mut self,
        level: usize,
        id: &str,
        descr: &str,
        kind: NodeType,
        shape_data: Option<&str>,
    ) -> Result<(), Error>;
    fn decorate_node(&mut self, icon: Option<&str>, class: Option<&str>);
}

/// Jison's start conditions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum S {
    Initial,
    Class,
    Icon,
    Node,
    Nstr,
    Nstr2,
    Data,
    DataStr,
}

/// The diagram an outline is read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    Mindmap,
    Kanban,
}

impl Outline {
    fn diagram(self) -> Kind {
        match self {
            Self::Mindmap => Kind::Mindmap,
            Self::Kanban => Kind::Kanban,
        }
    }

    /// The keyword the outline opens with, and its token.
    fn keyword(self) -> (&'static str, T) {
        match self {
            Self::Mindmap => ("mindmap", T::Mindmap),
            Self::Kanban => ("kanban", T::Kanban),
        }
    }

    /// Makes the calls on `db` the grammar's actions make, in the same
    /// order, and reports errors in the state where the generated parser
    /// would.
    pub fn parse(self, src: &str, db: &mut dyn OutlineDb) -> Result<(), Error> {
        let tokens = Lexer::tokenize(src, self);
        let mut s = Stream::new(self.diagram(), src, &tokens);
        let data = if self == Self::Kanban {
            ", 'SHAPE_DATA'"
        } else {
            ""
        };
        let (_, keyword) = self.keyword();
        let name = keyword.name();
        if s.eat(T::Spaceline) {
            while s.is_any(&[T::Spaceline, T::Nl]) {
                s.bump();
            }
            if !s.is(keyword) {
                return Err(s.fail(&format!("'SPACELINE', 'NL', '{name}'")));
            }
        } else if !s.is(keyword) {
            return Err(s.fail(&format!("'SPACELINE', '{name}'")));
        }
        s.bump();
        let first = if s.eat(T::Nl) {
            "'SPACELINE', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"
        } else {
            "'SPACELINE', 'NL', 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"
        };

        loop {
            let indented = s.is(T::Spacelist);
            let level = if indented {
                s.take().chars().count()
            } else {
                0
            };
            let kind = s.kind();
            if kind == T::NodeId || kind == T::NodeDstart {
                let id = if kind == T::NodeId {
                    Some(s.take())
                } else {
                    None
                };
                let (descr, node_kind) = if s.is(T::NodeDstart) {
                    let start = s.take();
                    let descr = s.expect(T::NodeDescr, "")?;
                    let end = s.expect(T::NodeDend, "")?;
                    if !s.kind().is_stop() && !s.is(T::ShapeData) {
                        return Err(s.fail(&format!("{STOP}{data}")));
                    }
                    (descr, NodeType::of_delimiters(start, end))
                } else {
                    if !s.kind().is_stop() && !s.is(T::ShapeData) {
                        return Err(s.fail(&format!("{STOP}, 'NODE_DSTART'{data}")));
                    }
                    (id.unwrap_or_default(), NodeType::Default)
                };
                let node_id = id.unwrap_or(descr);
                let shape_data = if s.is(T::ShapeData) {
                    let mut shape_data = String::new();
                    while s.is(T::ShapeData) {
                        shape_data += s.take();
                    }
                    if !s.kind().is_stop() {
                        return Err(s.fail(&format!("{STOP}{data}")));
                    }
                    Some(shape_data)
                } else {
                    None
                };
                db.add_node(level, node_id, descr, node_kind, shape_data.as_deref())?;
            } else if kind == T::Icon || kind == T::Class {
                let text = s.take();
                if !s.kind().is_stop() {
                    return Err(s.fail(STOP));
                }
                if kind == T::Icon {
                    db.decorate_node(Some(text), None);
                } else {
                    db.decorate_node(None, Some(text));
                }
            } else if indented {
                if !kind.is_stop() {
                    return Err(s.fail(&format!(
                        "{STOP}, 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"
                    )));
                }
            } else if kind == T::Spaceline {
                s.bump();
                if !s.kind().is_stop() {
                    return Err(s.fail(STOP));
                }
            } else {
                return Err(s.fail(first));
            }

            s.bump();
            while s.is_any(&[T::Nl, T::Eof]) {
                s.bump();
            }
            if s.is(T::End) {
                return Ok(());
            }
            if !s.kind().starts_statement() {
                return Err(s.fail(&format!(
                    "{STOP}, 'SPACELIST', 'ICON', 'CLASS', 'NODE_DSTART', 'NODE_ID'"
                )));
            }
        }
    }
}

/// `text` with each line break and the whitespace after it as `<br/>`.
fn breaks(text: &str) -> Cow<'_, str> {
    if !text.contains('\n') {
        return Cow::Borrowed(text);
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('\n') {
        out.push_str(&rest[..at]);
        out.push_str("<br/>");
        rest = rest[at + 1..].trim_start_matches(|c: char| is_space(c));
    }
    out.push_str(rest);
    Cow::Owned(out)
}

struct Lexer<'s> {
    cur: Cursor<'s, T, S>,
    outline: Outline,
    /// The line feed ending the line of the last whitespace run read.
    next_break: Option<usize>,
}

impl<'s> Lexer<'s> {
    fn tokenize(src: &'s str, outline: Outline) -> Vec<Token<'s, T>> {
        let mut lexer = Self {
            cur: Cursor::new(src, S::Initial),
            outline,
            next_break: None,
        };
        while let Some(c) = lexer.cur.char_at(lexer.cur.p) {
            if !lexer.step(c) {
                break;
            }
        }
        let mut cur = lexer.cur;
        let (p, n) = (cur.p, src.len());
        if p < n {
            cur.emit(T::Error, p, p);
        } else {
            if cur.state == S::Initial {
                cur.emit(T::Eof, p, p);
            }
            cur.emit(T::End, n, n);
        }
        cur.tokens
    }

    /// A `kind` token from the cursor to `end`, going on in `state`.
    fn emit(&mut self, kind: T, end: usize, state: S) -> bool {
        self.cur.emit(kind, self.cur.p, end);
        self.cur.state = state;
        true
    }

    /// A `kind` token from the cursor up to the first of `stops`, going on
    /// in `state`; false when that is empty.
    fn until(&mut self, kind: T, stops: &[char], state: S) -> bool {
        let end = self.cur.until(self.cur.p, |c| stops.contains(&c));
        end > self.cur.p && self.emit(kind, end, state)
    }

    /// Steps past `width` bytes into `state`.
    fn skip(&mut self, width: usize, state: S) -> bool {
        self.cur.p += width;
        self.cur.state = state;
        true
    }

    /// Reads one token or skips one piece of text; false where no rule
    /// matches.
    fn step(&mut self, c: char) -> bool {
        let p = self.cur.p;
        let d = self.cur.char_at(p + 1);
        match self.cur.state {
            S::Initial => self.initial(c, d),
            S::Node => match c {
                '"' if d == Some('`') => self.skip(2, S::Nstr2),
                '"' => self.skip(1, S::Nstr),
                ')' | '(' => {
                    let width = if d == Some(c) || (c == '(' && d == Some('-')) {
                        2
                    } else {
                        1
                    };
                    self.emit(T::NodeDend, p + width, S::Initial)
                }
                ']' => self.emit(T::NodeDend, p + 1, S::Initial),
                _ if self.cur.starts_with("}}", p) || self.cur.starts_with("-)", p) => {
                    self.emit(T::NodeDend, p + 2, S::Initial)
                }
                // A lone closing brace takes the rest of its line as text.
                '}' => self.until(T::NodeDescr, LINE_ENDS, S::Node),
                _ => self.until(T::NodeDescr, &[')', ']', '(', '}'], S::Node),
            },
            S::Nstr => match c {
                '"' => self.skip(1, S::Node),
                _ => {
                    let end = self.cur.until(p, |c| c == '"');
                    self.emit(T::NodeDescr, end, S::Nstr)
                }
            },
            S::Nstr2 => {
                if self.cur.starts_with("`\"", p) {
                    self.skip(2, S::Node)
                } else {
                    self.until(T::NodeDescr, &['`', '"'], S::Nstr2)
                }
            }
            S::Class => {
                self.until(T::Class, LINE_ENDS, S::Initial)
                    || (c == '\n' && self.skip(1, S::Initial))
            }
            S::Icon => match c {
                ')' => self.skip(1, S::Initial),
                _ => {
                    let end = self.cur.until(p, |c| c == ')');
                    self.emit(T::Icon, end, S::Icon)
                }
            },
            S::Data => match c {
                '"' => self.emit(T::ShapeData, p + 1, S::DataStr),
                '}' => self.skip(1, S::Initial),
                _ => self.until(T::ShapeData, &['}', '^', '"'], S::Data),
            },
            S::DataStr => match c {
                '"' => self.emit(T::ShapeData, p + 1, S::Data),
                _ => {
                    let end = self.cur.until(p, |c| c == '"');
                    let text = breaks(&self.cur.src[p..end]);
                    self.cur.emit_text(T::ShapeData, text, p, end);
                    true
                }
            },
        }
    }

    fn initial(&mut self, c: char, d: Option<char>) -> bool {
        let p = self.cur.p;
        let w = self.cur.space_end(p);
        let (word, keyword) = self.outline.keyword();
        if self.outline == Outline::Kanban && self.cur.starts_with("@{", p) {
            self.cur.emit_text(T::ShapeData, "", p, p + 2);
            self.cur.state = S::Data;
            true
        } else if self.cur.starts_with("%%", w) {
            let end = self.cur.until(w, |c| LINE_ENDS.contains(&c));
            self.emit(T::Spaceline, end, S::Initial)
        } else if w > p {
            let next_break = match self.next_break {
                Some(at) if at >= p => at,
                _ => *self.next_break.insert(self.cur.line_end(p)),
            };
            if next_break >= w {
                self.emit(T::Spacelist, w, S::Initial)
            } else if let Some(at) = self.cur.src[..w].rfind('\n').filter(|&at| at > p) {
                self.emit(T::Spaceline, at + 1, S::Initial)
            } else {
                self.emit(T::Nl, p + width_at(self.cur.src, p), S::Initial)
            }
        } else if self.cur.keyword_caseless(word, p) {
            self.emit(keyword, p + word.len(), S::Initial)
        } else if self.cur.starts_with(":::", p) {
            self.skip(3, S::Class)
        } else if self.cur.starts_with("::", p)
            && self.cur.starts_with_caseless("icon", p + 2)
            && self.cur.char_at(p + 6) == Some('(')
        {
            self.skip(7, S::Icon)
        } else if matches!((c, d), ('-', Some(')')) | ('(', Some('-' | '(')))
            || (matches!(c, ')' | '{') && d == Some(c))
        {
            self.emit(T::NodeDstart, p + 2, S::Node)
        } else if matches!(c, ')' | '(' | '[') {
            self.emit(T::NodeDstart, p + 1, S::Node)
        } else {
            let stops: &[char] = match self.outline {
                Outline::Mindmap => &['(', '[', '\n', ')', '{', '}'],
                Outline::Kanban => &['(', '[', '\n', ')', '{', '}', '@'],
            };
            self.until(T::NodeId, stops, S::Initial)
        }
    }
}
