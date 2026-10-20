//! The productions flowchart and agentflow share.

use super::{
    store::FlowStore,
    tokens::T,
    types::{DocItem, FlowDir, FlowText, LabelType, LinkInfo, Payload, Vertex},
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
    /// The link a `LINK` token and the `START_LINK` before it, if any, draw.
    pub destruct_link: fn(&str, Option<&str>) -> LinkInfo,
}

/// A run of `SHAPE_DATA` tokens: their text, and the indices of the first
/// and the last.
pub struct ShapeData {
    pub text: String,
    pub from: usize,
    pub to: usize,
}

/// What a diagram does with the vertices and links its statements read.
pub trait Builder {
    type Payload: Payload;

    fn store(&mut self) -> &mut FlowStore<Self::Payload>;
    /// Gives the shape data after a vertex to what `id` names.
    fn attach(&mut self, id: &str, data: &ShapeData) -> Result<(), Error>;
    /// The vertex `id`, `bare` of a shape, was read from token `from`
    /// through token `to`.
    fn vertex_read(&mut self, _id: &str, _bare: bool, _from: usize, _to: usize) {}
    /// A statement read from token `from` through token `to` ends in a link
    /// to `ends`.
    fn link_read(&mut self, _ends: &[String], _from: usize, _to: usize) {}
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

/// A cursor over a flowchart-family token list.
pub struct Reader<'a> {
    pub s: Stream<'a, 'a, T>,
    dialect: &'static Dialect,
}

impl<'a> Reader<'a> {
    pub fn new(dialect: &'static Dialect, src: &'a str, tokens: &'a [Token<'a, T>]) -> Self {
        Self {
            s: Stream::new(dialect.diagram, src, tokens),
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

    pub fn fail_statement(&self) -> Error {
        self.s.fail(self.dialect.statement_start)
    }

    /// The `graph` keyword and its direction, through the end of its line.
    pub fn header<P: Payload>(&mut self, store: &mut FlowStore<P>) -> Result<(), Error> {
        while self.s.is_any(self.dialect.blank) {
            self.s.bump();
        }
        self.s
            .expect(T::Graph, &expecting(self.dialect.blank.iter().copied()))?;
        if self.s.eat(T::Nodir) {
            store.direction = Some(FlowDir::Tb);
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
        store.direction = Some(FlowDir::from_header(dir));
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

    /// The run of `SHAPE_DATA` tokens from here, if any.
    pub fn shape_data(&mut self) -> Option<ShapeData> {
        if !self.s.is(T::ShapeData) {
            return None;
        }
        let from = self.s.index();
        let mut text = String::new();
        while self.s.is(T::ShapeData) {
            text += self.s.take();
        }
        Some(ShapeData {
            text,
            from,
            to: self.s.index() - 1,
        })
    }

    /// A vertex id and the shape after it, if any.
    fn node(&mut self) -> Result<Node<'a>, Error> {
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

    /// A vertex, and the class after `:::`.
    fn vertex<B: Builder>(&mut self, b: &mut B) -> Result<String, Error> {
        let from = self.s.index();
        let node = self.node()?;
        b.store().add_vertex(&node.id, node.vertex());
        b.vertex_read(&node.id, node.shape.is_none(), from, self.s.index() - 1);
        if self.s.eat(T::StyleSeparator) {
            let class = self.id_string()?;
            b.store().set_class(&node.id, &class);
        }
        Ok(node.id)
    }

    /// A run of vertices joined by `&`, the shape data after each given to
    /// it.
    fn vertices<B: Builder>(&mut self, b: &mut B) -> Result<Vec<String>, Error> {
        let mut ids = vec![self.vertex(b)?];
        loop {
            let data = self.shape_data();
            let last = &ids[ids.len() - 1];
            if !self.s.eat(T::Space) {
                if let Some(data) = data {
                    b.attach(last, &data)?;
                }
                return Ok(ids);
            }
            self.s.eat_all(T::Space);
            if !self.s.eat(T::Amp) {
                if data.is_some() {
                    return Err(self.s.fail("'AMP'"));
                }
                return Ok(ids);
            }
            if !self.s.eat(T::Space) {
                return Err(self.s.fail("'SPACE'"));
            }
            self.s.eat_all(T::Space);
            let next = self.vertex(b)?;
            if let Some(data) = data {
                b.attach(&ids[ids.len() - 1], &data)?;
            }
            ids.push(next);
        }
    }

    /// Vertices and the links between them; the ids of every vertex, the
    /// last linked first.
    fn vertex_statement<B: Builder>(&mut self, b: &mut B) -> Result<Vec<String>, Error> {
        let from = self.s.index();
        let mut stmt = self.vertices(b)?;
        let mut nodes = stmt.clone();
        while self.s.is_any(&[T::Link, T::StartLink, T::LinkId]) {
            let info = self.link()?;
            let next = self.vertices(b)?;
            b.store().add_link(&stmt, &next, Some(&info))?;
            b.link_read(&next, from, self.s.index() - 1);
            nodes.splice(0..0, next.iter().cloned());
            stmt = next;
        }
        Ok(nodes)
    }

    fn link(&mut self) -> Result<LinkInfo, Error> {
        let id = self.s.is(T::LinkId).then(|| self.s.take().to_string());
        if self.s.is(T::Link) {
            let end = self.s.take();
            let mut info = (self.dialect.destruct_link)(end, None);
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
            let mut info = (self.dialect.destruct_link)(end, Some(start));
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

    fn link_style<P: Payload>(&mut self, store: &mut FlowStore<P>) -> Result<(), Error> {
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
            return store.update_link(&positions, styles);
        }
        self.s.expect(T::Space, "")?;
        let interpolate = self.alpha_num()?;
        if self.s.eat(T::Space) {
            let styles = self.styles()?;
            store.update_link_interpolate(&positions, &interpolate)?;
            store.update_link(&positions, styles)
        } else {
            store.update_link_interpolate(&positions, &interpolate)
        }
    }

    fn link_tail<P: Payload>(
        &mut self,
        store: &mut FlowStore<P>,
        id: &str,
        url: &str,
    ) -> Result<(), Error> {
        if !self.s.eat(T::Space) {
            store.set_link(id, url, None);
            return Ok(());
        }
        if self.s.is(T::LinkTarget) {
            let target = self.s.take();
            store.set_link(id, url, Some(target));
            return Ok(());
        }
        let tooltip = self.s.expect(T::Str, "'LINK_TARGET'")?;
        if self.s.eat(T::Space) {
            let target = self.s.expect(T::LinkTarget, "")?;
            store.set_link(id, url, Some(target));
        } else {
            store.set_link(id, url, None);
        }
        store.set_tooltip(id, tooltip);
        Ok(())
    }

    fn click<P: Payload>(&mut self, store: &mut FlowStore<P>) -> Result<(), Error> {
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
                store.set_click_event(id);
                if let Some(tooltip) = tooltip {
                    store.set_tooltip(id, tooltip);
                }
            }
            T::Href => {
                self.s.bump();
                let url = self.s.expect(T::Str, "")?;
                self.link_tail(store, id, url)?;
            }
            T::Str => {
                let url = self.s.take();
                self.link_tail(store, id, url)?;
            }
            _ => {
                self.alpha_num()?;
                let tooltip = if self.s.eat(T::Space) {
                    Some(self.s.expect(T::Str, "")?)
                } else {
                    None
                };
                store.set_click_event(id);
                if let Some(tooltip) = tooltip {
                    store.set_tooltip(id, tooltip);
                }
            }
        }
        Ok(())
    }

    /// A statement both diagrams have, through its separator.
    pub fn statement<B: Builder>(&mut self, b: &mut B) -> Result<DocItem, Error> {
        let none = DocItem::List(Vec::new());
        let store = b.store();
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
                store.add_vertex(&id, vertex);
                self.separator()?;
                none
            }
            T::Linkstyle => {
                self.link_style(store)?;
                self.separator()?;
                none
            }
            T::Classdef => {
                self.s.bump();
                self.s.expect(T::Space, "")?;
                let id = self.id_string()?;
                self.s.expect(T::Space, "")?;
                let styles = self.styles()?;
                store.add_class(&id, &styles);
                self.separator()?;
                none
            }
            T::Class => {
                self.s.bump();
                self.s.expect(T::Space, "")?;
                let ids = self.id_string()?;
                self.s.expect(T::Space, "")?;
                let class = self.id_string()?;
                store.set_class(&ids, &class);
                self.separator()?;
                none
            }
            T::Click => {
                self.click(store)?;
                self.separator()?;
                none
            }
            t @ (T::DirectionTb
            | T::DirectionBt
            | T::DirectionRl
            | T::DirectionLr
            | T::DirectionTd) => {
                self.s.bump();
                DocItem::Dir(match t {
                    T::DirectionTb => FlowDir::Tb,
                    T::DirectionBt => FlowDir::Bt,
                    T::DirectionRl => FlowDir::Rl,
                    T::DirectionLr => FlowDir::Lr,
                    _ => FlowDir::Td,
                })
            }
            T::AccTitle => {
                self.s.bump();
                let value = js::trim(self.s.expect(T::AccTitleValue, "")?).to_string();
                store.titles.set_acc_title(&value);
                DocItem::Str(value)
            }
            T::AccDescr => {
                self.s.bump();
                let value = js::trim(self.s.expect(T::AccDescrValue, "")?).to_string();
                store.titles.set_acc_descr(&value);
                DocItem::Str(value)
            }
            T::AccDescrMultilineValue => {
                let value = js::trim(self.s.take()).to_string();
                store.titles.set_acc_descr(&value);
                DocItem::Str(value)
            }
            t if ID.contains(&t) => {
                let nodes = self.vertex_statement(b)?;
                self.separator()?;
                DocItem::List(nodes)
            }
            _ => return Err(self.s.fail(self.dialect.statement_start)),
        };
        Ok(item)
    }
}
