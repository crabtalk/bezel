//! The productions flowchart and agentflow share.

use super::{
    tokens::T,
    types::{DocItem, FlowText, LabelType, LinkInfo, Vertex},
};
use crate::{
    detect::Kind,
    error::Error,
    js,
    token::{Stream, Token, expecting},
};

const ID: [T; 11] = [
    T::Num,
    T::NodeString,
    T::Down,
    T::Minus,
    T::Default,
    T::Comma,
    T::Colon,
    T::Amp,
    T::Brkt,
    T::Mult,
    T::UnicodeText,
];
const ALPHANUM: [T; 11] = [
    T::Num,
    T::UnicodeText,
    T::NodeString,
    T::Dir,
    T::Down,
    T::Minus,
    T::Comma,
    T::Colon,
    T::Amp,
    T::Brkt,
    T::Mult,
];
const TEXT: [T; 4] = [T::Text, T::Tagstart, T::Tagend, T::UnicodeText];
const STYLE_PART: [T; 6] = [T::Num, T::NodeString, T::Colon, T::Space, T::Brkt, T::Style];
const TEXT_TOKENS: &str = "'UNICODE_TEXT', 'TEXT', 'TAGSTART'";

/// The graph operations the shared productions perform.
pub trait FlowGraph {
    fn add_vertex(&mut self, id: &str, vertex: Vertex<'_>) -> Result<(), Error>;
    fn destruct_link(&self, end: &str, start: Option<&str>) -> LinkInfo;
    fn update_link_interpolate(
        &mut self,
        positions: &[String],
        interpolate: &str,
    ) -> Result<(), Error>;
    fn update_link(&mut self, positions: &[String], style: Vec<String>) -> Result<(), Error>;
    fn add_class(&mut self, ids: &str, styles: &[String]);
    fn set_direction(&mut self, dir: &str);
    fn set_class(&mut self, ids: &str, class_name: &str);
    fn set_tooltip(&mut self, ids: &str, tooltip: &str);
    fn set_link(&mut self, ids: &str, link: &str, target: Option<&str>);
    fn set_click_event(&mut self, ids: &str);
    fn set_acc_title(&mut self, text: &str);
    fn set_acc_description(&mut self, text: &str);
}

/// Where one diagram's grammar departs from the other's.
pub struct Dialect {
    pub diagram: Kind,
    /// Kinds that may come before the `graph` keyword.
    pub blank: &'static [T],
    /// Kinds that end the `graph` line, in the order errors list them.
    /// These and `EOF` end every other statement.
    pub line_ends: &'static [T],
    /// Kinds an unquoted block id reads.
    pub no_tags: &'static [T],
    /// The error at a token no statement starts with.
    pub statement_start: &'static str,
}

/// A vertex id and the shape written after it.
pub struct Node<'a> {
    pub id: String,
    /// The label and shape name, when a shape follows the id.
    pub shape: Option<(FlowText, &'static str)>,
    pub prop: Option<(&'a str, &'a str)>,
}

impl Node<'_> {
    pub fn vertex(&self) -> Vertex<'_> {
        Vertex {
            label: self.shape.as_ref().map(|(label, _)| label),
            shape: self.shape.as_ref().map(|&(_, shape)| shape),
            prop: self.prop,
            ..Vertex::default()
        }
    }
}

/// A cursor over a flowchart-family token list, and the graph it fills.
pub struct Reader<'a, D> {
    pub s: Stream<'a, 'a, T>,
    pub db: &'a mut D,
    dialect: &'static Dialect,
}

impl<'a, D: FlowGraph> Reader<'a, D> {
    pub fn new(
        dialect: &'static Dialect,
        src: &'a str,
        tokens: &'a [Token<'a, T>],
        db: &'a mut D,
    ) -> Self {
        Self {
            s: Stream::new(dialect.diagram, src, tokens),
            db,
            dialect,
        }
    }

    pub fn at_separator(&self) -> bool {
        self.s.is(T::Eof) || self.s.is_any(self.dialect.line_ends)
    }

    /// The kinds that end a statement, then `also`, as errors list them.
    pub fn separators(&self, also: &[T]) -> String {
        let ends = self.dialect.line_ends.iter().chain(&[T::Eof]);
        expecting(ends.chain(also).copied())
    }

    pub fn separator(&mut self) -> Result<(), Error> {
        if !self.at_separator() {
            return Err(self.s.fail(&self.separators(&[])));
        }
        self.s.bump();
        Ok(())
    }

    pub fn at_vertex(&self) -> bool {
        self.s.is_any(&ID)
    }

    pub fn fail_statement(&self) -> Error {
        self.s.fail(self.dialect.statement_start)
    }

    /// The `graph` keyword and its direction, through the end of its line.
    pub fn header(&mut self) -> Result<(), Error> {
        while self.s.is_any(self.dialect.blank) {
            self.s.bump();
        }
        self.s
            .expect(T::Graph, &expecting(self.dialect.blank.iter().copied()))?;
        if self.s.eat(T::Nodir) {
            self.db.set_direction("TB");
            return Ok(());
        }
        let dir = self.s.expect(T::Dir, "'NODIR'")?;
        if self.s.eat(T::Space) {
            self.s.eat_all(T::Space);
            if !self.s.is(T::Newline) {
                return Err(self.s.fail("'NEWLINE', 'SPACE'"));
            }
        } else if !self.s.is_any(self.dialect.line_ends) {
            let line_ends = self.dialect.line_ends.iter().chain(&[T::Space]);
            return Err(self.s.fail_expecting(line_ends.copied()));
        }
        self.s.bump();
        self.db.set_direction(dir);
        Ok(())
    }

    pub fn id_string(&mut self) -> Result<String, Error> {
        self.s.run_of(&ID, "'NODE_STRING'")
    }

    fn alpha_num(&mut self) -> Result<String, Error> {
        self.s.run_of(&ALPHANUM, "'NODE_STRING'")
    }

    pub fn text(&mut self) -> Result<FlowText, Error> {
        let kind = match self.s.kind() {
            T::Str => LabelType::String,
            T::MdStr => LabelType::Markdown,
            t if TEXT.contains(&t) => LabelType::Text,
            _ => {
                return Err(self
                    .s
                    .fail(&format!("{TEXT_TOKENS}, 'TAGEND', 'STR', 'MD_STR'")));
            }
        };
        let mut text = self.s.take().to_string();
        while self.s.is_any(&TEXT) {
            text += self.s.take();
        }
        Ok(FlowText { text, kind })
    }

    pub fn closed(&mut self, kind: T) -> Result<(), Error> {
        self.s.expect(kind, TEXT_TOKENS).map(|_| ())
    }

    fn edge_text(&mut self) -> Result<FlowText, Error> {
        let kind = match self.s.kind() {
            T::Str => LabelType::String,
            T::MdStr => LabelType::Markdown,
            T::EdgeText | T::UnicodeText => LabelType::Text,
            _ => return Err(self.s.fail("'EDGE_TEXT', 'STR', 'MD_STR'")),
        };
        let mut text = self.s.take().to_string();
        while self.s.is_any(&[T::EdgeText, T::UnicodeText]) {
            text += self.s.take();
        }
        Ok(FlowText { text, kind })
    }

    pub fn text_no_tags(&mut self) -> Result<FlowText, Error> {
        let no_tags = self.dialect.no_tags;
        let kind = match self.s.kind() {
            T::Str => LabelType::Text,
            T::MdStr => LabelType::Markdown,
            t if no_tags.contains(&t) => LabelType::Text,
            _ => return Err(self.s.fail("'NODE_STRING', 'STR', 'MD_STR'")),
        };
        let mut text = self.s.take().to_string();
        while self.s.is_any(no_tags) {
            text += self.s.take();
        }
        Ok(FlowText { text, kind })
    }

    /// The run of `SHAPE_DATA` tokens from here, joined. The current token
    /// must be one.
    pub fn shape_data(&mut self) -> String {
        let mut data = self.s.take().to_string();
        while self.s.is(T::ShapeData) {
            data += self.s.take();
        }
        data
    }

    /// A vertex id and the shape after it, if any.
    pub fn node(&mut self) -> Result<Node<'a>, Error> {
        let id = self.id_string()?;
        let mut prop = None;
        let shape = match self.s.kind() {
            T::Sqs => Some(self.enclosed(T::Sqe, "square")?),
            T::Doublecirclestart => Some(self.enclosed(T::Doublecircleend, "doublecircle")?),
            T::Ps if self.s.peek(1) == T::Ps => {
                self.s.bump();
                let shape = self.enclosed(T::Pe, "circle")?;
                self.s.expect(T::Pe, "")?;
                Some(shape)
            }
            T::Ps => Some(self.enclosed(T::Pe, "round")?),
            T::EllipseStart => Some(self.enclosed(T::EllipseEnd, "ellipse")?),
            T::Stadiumstart => Some(self.enclosed(T::Stadiumend, "stadium")?),
            T::Subroutinestart => Some(self.enclosed(T::Subroutineend, "subroutine")?),
            T::VertexWithPropsStart => {
                self.s.bump();
                let field = self.s.expect(T::NodeString, "")?;
                self.s.expect(T::Colon, "")?;
                let value = self.s.expect(T::NodeString, "")?;
                prop = Some((field, value));
                self.s.expect(T::Pipe, "")?;
                let label = self.text()?;
                self.closed(T::Sqe)?;
                Some((label, "rect"))
            }
            T::Cylinderstart => Some(self.enclosed(T::Cylinderend, "cylinder")?),
            T::DiamondStart if self.s.peek(1) == T::DiamondStart => {
                self.s.bump();
                let shape = self.enclosed(T::DiamondStop, "hexagon")?;
                self.s.expect(T::DiamondStop, "")?;
                Some(shape)
            }
            T::DiamondStart => Some(self.enclosed(T::DiamondStop, "diamond")?),
            T::Tagend => Some(self.enclosed(T::Sqe, "odd")?),
            T::Trapstart => {
                Some(self.slanted((T::Invtrapend, "lean_right"), (T::Trapend, "trapezoid"))?)
            }
            T::Invtrapstart => {
                Some(self.slanted((T::Trapend, "lean_left"), (T::Invtrapend, "inv_trapezoid"))?)
            }
            _ => None,
        };
        Ok(Node { id, shape, prop })
    }

    /// Steps past a shape's opening token and reads its label through
    /// `close`.
    fn enclosed(
        &mut self,
        close: T,
        shape: &'static str,
    ) -> Result<(FlowText, &'static str), Error> {
        self.s.bump();
        let label = self.text()?;
        self.closed(close)?;
        Ok((label, shape))
    }

    /// A slanted shape's label, then `lean`'s closing token or else
    /// `straight`'s.
    fn slanted(
        &mut self,
        lean: (T, &'static str),
        straight: (T, &'static str),
    ) -> Result<(FlowText, &'static str), Error> {
        self.s.bump();
        let label = self.text()?;
        if self.s.eat(lean.0) {
            return Ok((label, lean.1));
        }
        self.closed(straight.0)?;
        Ok((label, straight.1))
    }

    pub fn at_link(&self) -> bool {
        self.s.is_any(&[T::Link, T::StartLink, T::LinkId])
    }

    pub fn link(&mut self) -> Result<LinkInfo, Error> {
        let id = self.s.is(T::LinkId).then(|| self.s.take().to_string());
        if self.s.is(T::Link) {
            let end = self.s.take();
            let mut info = self.db.destruct_link(end, None);
            if id.is_some() {
                info.id = id;
            }
            if self.s.eat(T::Pipe) {
                info.text = Some(self.text()?);
                self.closed(T::Pipe)?;
                self.s.eat(T::Space);
            }
            return Ok(info);
        }
        if self.s.is(T::StartLink) {
            let start = self.s.take();
            let label = self.edge_text()?;
            let end = self.s.expect(T::Link, "'EDGE_TEXT'")?;
            let mut info = self.db.destruct_link(end, Some(start));
            info.text = Some(label);
            if id.is_some() {
                info.id = id;
            }
            return Ok(info);
        }
        Err(self.s.fail("'LINK', 'START_LINK'"))
    }

    fn style(&mut self) -> Result<String, Error> {
        self.s
            .run_of(&STYLE_PART, "'NODE_STRING', 'NUM', 'COLON', 'BRKT'")
    }

    fn styles(&mut self) -> Result<Vec<String>, Error> {
        let mut styles = vec![self.style()?];
        while self.s.eat(T::Comma) {
            styles.push(self.style()?);
        }
        Ok(styles)
    }

    fn link_style(&mut self) -> Result<(), Error> {
        self.s.bump();
        self.s.expect(T::Space, "")?;
        let mut positions;
        if self.s.is(T::Default) {
            positions = vec![self.s.take().to_string()];
        } else {
            positions = vec![self.s.expect(T::Num, "'DEFAULT'")?.to_string()];
            while self.s.eat(T::Comma) {
                positions.push(self.s.expect(T::Num, "")?.to_string());
            }
        }
        self.s.expect(T::Space, "")?;
        if !self.s.eat(T::Interpolate) {
            let styles = self.styles()?;
            return self.db.update_link(&positions, styles);
        }
        self.s.expect(T::Space, "")?;
        let interpolate = self.alpha_num()?;
        if self.s.eat(T::Space) {
            let styles = self.styles()?;
            self.db.update_link_interpolate(&positions, &interpolate)?;
            self.db.update_link(&positions, styles)
        } else {
            self.db.update_link_interpolate(&positions, &interpolate)
        }
    }

    fn link_tail(&mut self, id: &str, url: &str) -> Result<(), Error> {
        if !self.s.eat(T::Space) {
            self.db.set_link(id, url, None);
            return Ok(());
        }
        if self.s.is(T::LinkTarget) {
            let target = self.s.take();
            self.db.set_link(id, url, Some(target));
            return Ok(());
        }
        let tooltip = self.s.expect(T::Str, "'LINK_TARGET'")?;
        if self.s.eat(T::Space) {
            let target = self.s.expect(T::LinkTarget, "")?;
            self.db.set_link(id, url, Some(target));
        } else {
            self.db.set_link(id, url, None);
        }
        self.db.set_tooltip(id, tooltip);
        Ok(())
    }

    fn click(&mut self) -> Result<(), Error> {
        let id = self.s.take();
        match self.s.kind() {
            T::Callbackname => {
                self.s.bump();
                self.s.eat(T::Callbackargs);
                let tooltip = if self.s.eat(T::Space) {
                    Some(self.s.expect(T::Str, "")?)
                } else {
                    None
                };
                self.db.set_click_event(id);
                if let Some(tooltip) = tooltip {
                    self.db.set_tooltip(id, tooltip);
                }
            }
            T::Href => {
                self.s.bump();
                let url = self.s.expect(T::Str, "")?;
                self.link_tail(id, url)?;
            }
            T::Str => {
                let url = self.s.take();
                self.link_tail(id, url)?;
            }
            _ => {
                self.alpha_num()?;
                let tooltip = if self.s.eat(T::Space) {
                    Some(self.s.expect(T::Str, "")?)
                } else {
                    None
                };
                self.db.set_click_event(id);
                if let Some(tooltip) = tooltip {
                    self.db.set_tooltip(id, tooltip);
                }
            }
        }
        Ok(())
    }

    /// A statement both diagrams have, through its separator; `None` at a
    /// token none of them starts with.
    pub fn statement(&mut self) -> Result<Option<DocItem>, Error> {
        let none = DocItem::List(Vec::new());
        let item = match self.s.kind() {
            T::Style => {
                self.s.bump();
                self.s.expect(T::Space, "")?;
                let id = self.id_string()?;
                self.s.expect(T::Space, "")?;
                let styles = self.styles()?;
                let vertex = Vertex {
                    styles: &styles,
                    ..Vertex::default()
                };
                self.db.add_vertex(&id, vertex)?;
                self.separator()?;
                none
            }
            T::Linkstyle => {
                self.link_style()?;
                self.separator()?;
                none
            }
            T::Classdef => {
                self.s.bump();
                self.s.expect(T::Space, "")?;
                let id = self.id_string()?;
                self.s.expect(T::Space, "")?;
                let styles = self.styles()?;
                self.db.add_class(&id, &styles);
                self.separator()?;
                none
            }
            T::Class => {
                self.s.bump();
                self.s.expect(T::Space, "")?;
                let ids = self.id_string()?;
                self.s.expect(T::Space, "")?;
                let class = self.id_string()?;
                self.db.set_class(&ids, &class);
                self.separator()?;
                none
            }
            T::Click => {
                self.click()?;
                self.separator()?;
                none
            }
            t @ (T::DirectionTb
            | T::DirectionBt
            | T::DirectionRl
            | T::DirectionLr
            | T::DirectionTd) => {
                self.s.bump();
                let dir = match t {
                    T::DirectionTb => "TB",
                    T::DirectionBt => "BT",
                    T::DirectionRl => "RL",
                    T::DirectionLr => "LR",
                    _ => "TD",
                };
                DocItem::Dir(dir.to_string())
            }
            T::AccTitle => {
                self.s.bump();
                let value = js::trim(self.s.expect(T::AccTitleValue, "")?).to_string();
                self.db.set_acc_title(&value);
                DocItem::Str(value)
            }
            T::AccDescr => {
                self.s.bump();
                let value = js::trim(self.s.expect(T::AccDescrValue, "")?).to_string();
                self.db.set_acc_description(&value);
                DocItem::Str(value)
            }
            T::AccDescrMultilineValue => {
                let value = js::trim(self.s.take()).to_string();
                self.db.set_acc_description(&value);
                DocItem::Str(value)
            }
            _ => return Ok(None),
        };
        Ok(Some(item))
    }
}
