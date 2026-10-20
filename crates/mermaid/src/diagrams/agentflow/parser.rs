//! Pele's `diagrams/agentflow/parser.ts`.

use super::{
    db::AgentflowDb,
    lexer::tokenize,
    types::{DocItem, FlowText, LabelType, Loc, Vertex},
};
use crate::{
    detect::Kind,
    diagrams::flowchart::{
        syntax::{Dialect, FlowGraph, Reader},
        tokens::T,
    },
    error::Error,
    js::Lazy,
};

const NO_TAGS: [T; 23] = [
    T::Num,
    T::NodeString,
    T::Space,
    T::Minus,
    T::Amp,
    T::UnicodeText,
    T::Colon,
    T::Mult,
    T::Brkt,
    T::Style,
    T::Linkstyle,
    T::Classdef,
    T::Class,
    T::Click,
    T::Graph,
    T::Dir,
    T::Flow,
    T::Connector,
    T::Global,
    T::EndKw,
    T::Down,
    T::Up,
    T::StartLink,
];

static DIALECT: Dialect = Dialect {
    diagram: Kind::Agentflow,
    blank: &[T::Space, T::Newline, T::Comment],
    line_ends: &[T::Comment, T::Semi, T::Newline],
    no_tags: &NO_TAGS,
    statement_start: "'COMMENT', 'SEMI', 'NEWLINE', 'SPACE', 'EOF', 'flow', 'global', 'connector', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'STYLE', 'LINKSTYLE', 'CLASSDEF', 'CLASS', 'CLICK', 'NODE_STRING'",
};

/// Mermaid trims spaces and tabs between a closing brace and the end of its
/// line before parsing.
static RE_BRACE_TRAIL: Lazy = Lazy::new(r"}[^\S\n]*\n", "g");

/// Shape data, and the first and last of the tokens it was read from.
struct Data {
    text: String,
    from: usize,
    to: usize,
}

struct Frame {
    global: bool,
    /// Token index of the `flow` keyword.
    at: usize,
    id: Option<FlowText>,
    title: Option<FlowText>,
    data: Option<Data>,
    list: Vec<DocItem>,
    collect: bool,
}

struct Parser<'a> {
    r: Reader<'a, AgentflowDb>,
    locs: &'a [[usize; 4]],
}

impl Parser<'_> {
    /// The span from the start of token `a` to the end of token `b`.
    fn span(&self, a: usize, b: usize) -> Option<Loc> {
        let ([first_line, first_column, ..], [.., last_line, last_column]) =
            (self.locs[a], self.locs[b]);
        Some(Loc {
            first_line,
            first_column,
            last_line,
            last_column,
        })
    }

    fn shape_data(&mut self) -> Option<Data> {
        if !self.r.s.is(T::ShapeData) {
            return None;
        }
        let from = self.r.s.index();
        let text = self.r.shape_data();
        Some(Data {
            text,
            from,
            to: self.r.s.index() - 1,
        })
    }

    fn add_data(&mut self, id: &str, data: &Data) -> Result<(), Error> {
        let vertex = Vertex {
            metadata: Some(&data.text),
            ..Vertex::default()
        };
        let loc = self.span(data.from, data.to);
        self.r.db.add_vertex_at(id, vertex, loc)
    }

    /// [`Parser::add_data`], recorded as an attachment to the vertex.
    fn attach(&mut self, id: &str, data: &Data) -> Result<(), Error> {
        self.add_data(id, data)?;
        let loc = self.span(data.from, data.to);
        self.r.db.extend_vertex_mapping(id, loc);
        Ok(())
    }

    fn vertex(&mut self) -> Result<String, Error> {
        let from = self.r.s.index();
        let node = self.r.node()?;
        self.r.db.add_vertex(&node.id, node.vertex())?;
        let loc = self.span(from, self.r.s.index() - 1);
        self.r
            .db
            .add_vertex_mapping(&node.id, node.shape.is_none(), loc);
        Ok(node.id)
    }

    fn styled_vertex(&mut self) -> Result<String, Error> {
        let id = self.vertex()?;
        if self.r.s.eat(T::StyleSeparator) {
            let class = self.r.id_string()?;
            self.r.db.set_class(&id, &class);
        }
        Ok(id)
    }

    /// A run of vertices joined by `&`, and the shape data after the last.
    fn nodes(&mut self) -> Result<(Vec<String>, Option<Data>), Error> {
        let mut ids = vec![self.styled_vertex()?];
        loop {
            let data = self.shape_data();
            if !self.r.s.eat(T::Space) {
                return Ok((ids, data));
            }
            self.r.s.eat_all(T::Space);
            if !self.r.s.eat(T::Amp) {
                if data.is_some() {
                    return Err(self.r.s.fail("'AMP'"));
                }
                return Ok((ids, None));
            }
            if !self.r.s.eat(T::Space) {
                return Err(self.r.s.fail("'SPACE'"));
            }
            self.r.s.eat_all(T::Space);
            let next = self.styled_vertex()?;
            if let Some(data) = data {
                self.attach(&ids[ids.len() - 1], &data)?;
            }
            ids.push(next);
        }
    }

    /// [`Parser::nodes`], with the shape data attached to the last vertex.
    fn node(&mut self) -> Result<Vec<String>, Error> {
        let (ids, data) = self.nodes()?;
        if let Some(data) = data {
            self.attach(&ids[ids.len() - 1], &data)?;
        }
        Ok(ids)
    }

    fn vertex_statement(&mut self) -> Result<Vec<String>, Error> {
        let from = self.r.s.index();
        let mut stmt = self.node()?;
        let mut nodes = stmt.clone();
        while self.r.at_link() {
            let info = self.r.link()?;
            let next = self.node()?;
            self.r.db.add_link(&stmt, &next, Some(&info))?;
            let loc = self.span(from, self.r.s.index() - 1);
            self.r.db.add_edge_mapping(&next, loc);
            nodes.splice(0..0, next.iter().cloned());
            stmt = next;
        }
        Ok(nodes)
    }

    fn attach_data(&mut self, id: &str, frame: &Frame) -> Result<(), Error> {
        match &frame.data {
            Some(data) => self.add_data(id, data),
            None => Ok(()),
        }
    }

    /// Reads `flow` or `connector` up to the separator: an id, an optional
    /// bracketed title, optional metadata.
    fn header(&mut self, at: usize) -> Result<Frame, Error> {
        self.r.s.expect(T::Space, &self.r.separators(&[]))?;
        let id = self.r.text_no_tags()?;
        let mut title = FlowText {
            text: String::new(),
            kind: LabelType::Text,
        };
        if self.r.s.eat(T::Sqs) {
            title = self.r.text()?;
            self.r.closed(T::Sqe)?;
        }
        let data = self.shape_data();
        if !self.r.at_separator() {
            return Err(self.r.s.fail(&self.r.separators(&[T::ShapeData])));
        }
        Ok(Frame {
            global: false,
            at,
            id: Some(id),
            title: Some(title),
            data,
            list: Vec::new(),
            collect: false,
        })
    }

    fn statement(&mut self) -> Result<DocItem, Error> {
        if let Some(item) = self.r.statement()? {
            return Ok(item);
        }
        if self.r.s.is(T::Connector) {
            let at = self.r.s.index();
            self.r.s.bump();
            let frame = self.header(at)?;
            let sep = self.r.s.index();
            self.r.s.bump();
            let id = self
                .r
                .db
                .add_connector(frame.id.as_ref(), frame.title.as_ref());
            self.attach_data(&id, &frame)?;
            let (start, end) = (self.span(frame.at, frame.at), self.span(sep, sep));
            self.r
                .db
                .add_connector_mapping(frame.id.as_ref(), start, end);
            return Ok(DocItem::Str(id));
        }
        if !self.r.at_vertex() {
            return Err(self.r.fail_statement());
        }
        let nodes = self.vertex_statement()?;
        self.r.separator()?;
        Ok(DocItem::List(nodes))
    }

    /// Blocks nest without recursion: each open one keeps the list and
    /// collect flag of its parent.
    fn document(&mut self) -> Result<(), Error> {
        let mut open: Vec<Frame> = Vec::new();
        let mut list: Vec<DocItem> = Vec::new();
        let mut collect = false;
        loop {
            let t = self.r.s.kind();
            if t == T::End {
                break;
            }
            if t == T::EndKw {
                let Some(frame) = open.pop() else {
                    break;
                };
                let at = self.r.s.index();
                self.r.s.bump();
                if frame.global {
                    self.r.db.add_global(&list);
                    list = frame.list;
                    collect = frame.collect;
                    continue;
                }
                let id = self
                    .r
                    .db
                    .add_sub_graph(frame.id.as_ref(), &list, frame.title.as_ref());
                self.attach_data(&id, &frame)?;
                let (start, end) = (self.span(frame.at, frame.at), self.span(at, at));
                self.r
                    .db
                    .add_subgraph_mapping(frame.id.as_ref(), start, end);
                list = frame.list;
                collect = frame.collect;
                if collect {
                    list.push(DocItem::Str(id));
                }
            } else if t == T::Semi || t == T::Newline || t == T::Space || t == T::Eof {
                if collect {
                    list.push(DocItem::Str(self.r.s.text().to_string()));
                }
                self.r.s.bump();
            } else if t == T::Comment {
                self.r.s.bump();
            } else if t == T::Flow || t == T::Global {
                let at = self.r.s.index();
                self.r.s.bump();
                let frame = if t == T::Global || self.r.at_separator() {
                    let frame = Frame {
                        global: t == T::Global,
                        at,
                        id: None,
                        title: None,
                        data: None,
                        list: std::mem::take(&mut list),
                        collect,
                    };
                    self.r.separator()?;
                    frame
                } else {
                    let mut frame = self.header(at)?;
                    frame.list = std::mem::take(&mut list);
                    frame.collect = collect;
                    self.r.s.bump();
                    frame
                };
                open.push(frame);
                collect = true;
            } else {
                let item = self.statement()?;
                if collect && !matches!(&item, DocItem::List(items) if items.is_empty()) {
                    list.push(item);
                }
            }
        }
        if !open.is_empty() {
            return Err(self.r.s.fail("'end'"));
        }
        Ok(())
    }
}

/// `raw` parses the text as it is, the way Mermaid's generated parser does
/// without its wrapper.
pub fn parse_agentflow(source: &str, db: &mut AgentflowDb, raw: bool) -> Result<(), Error> {
    let src = if !raw && source.contains('}') {
        RE_BRACE_TRAIL.replace(source, |_| "}\n".into())
    } else {
        source.to_string()
    };
    let lexed = tokenize(&src);
    let mut p = Parser {
        r: Reader::new(&DIALECT, &src, &lexed.tokens, db),
        locs: &lexed.locs,
    };
    p.r.header()?;
    p.document()?;
    if !p.r.s.is(T::End) {
        return Err(p.r.fail_statement());
    }
    Ok(())
}
