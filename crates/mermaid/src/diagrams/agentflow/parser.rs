//! Pele's `diagrams/agentflow/parser.ts`.

use super::{
    Agentflow,
    db::{AgentflowDb, destruct_link},
    lexer::tokenize,
    types::{DocItem, FlowText, LabelType, Loc},
};
use crate::{
    detect::Kind,
    diagrams::flowchart::{
        store::FlowStore,
        syntax::{Builder, Dialect, Reader, ShapeData},
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
    destruct_link,
};

/// Mermaid trims spaces and tabs between a closing brace and the end of its
/// line before parsing.
static RE_BRACE_TRAIL: Lazy = Lazy::new(r"}[^\S\n]*\n", "g");

struct Frame {
    global: bool,
    /// Token index of the `flow` keyword.
    at: usize,
    id: Option<FlowText>,
    title: Option<FlowText>,
    data: Option<ShapeData>,
    list: Vec<DocItem>,
    collect: bool,
}

/// The model being built, and where each token is.
struct Build<'a> {
    db: &'a mut AgentflowDb,
    locs: &'a [Loc],
}

impl Build<'_> {
    /// The span from the start of token `a` to the end of token `b`.
    fn span(&self, a: usize, b: usize) -> Loc {
        self.locs[a].to(self.locs[b])
    }

    fn add_data(&mut self, id: &str, data: &ShapeData) -> Result<(), Error> {
        let loc = self.span(data.from, data.to);
        self.db.add_metadata(id, &data.text, loc)
    }
}

impl Builder for Build<'_> {
    type Payload = Agentflow;

    fn store(&mut self) -> &mut FlowStore<Agentflow> {
        &mut self.db.store
    }

    /// Recorded as an attachment to the vertex.
    fn attach(&mut self, id: &str, data: &ShapeData) -> Result<(), Error> {
        self.add_data(id, data)?;
        let loc = self.span(data.from, data.to);
        self.db.extend_vertex_mapping(id, loc);
        Ok(())
    }

    fn vertex_read(&mut self, id: &str, bare: bool, from: usize, to: usize) {
        let loc = self.span(from, to);
        self.db.add_vertex_mapping(id, bare, loc);
    }

    fn link_read(&mut self, ends: &[String], from: usize, to: usize) {
        let loc = self.span(from, to);
        self.db.add_edge_mapping(ends, loc);
    }
}

struct Parser<'a> {
    r: Reader<'a>,
    b: Build<'a>,
}

impl Parser<'_> {
    fn attach_data(&mut self, id: &str, frame: &Frame) -> Result<(), Error> {
        match &frame.data {
            Some(data) => self.b.add_data(id, data),
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
        let data = self.r.shape_data();
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
        if !self.r.s.is(T::Connector) {
            return self.r.statement(&mut self.b);
        }
        let at = self.r.s.index();
        self.r.s.bump();
        let frame = self.header(at)?;
        let sep = self.r.s.index();
        self.r.s.bump();
        let id = self
            .b
            .db
            .add_connector(frame.id.as_ref(), frame.title.as_ref());
        self.attach_data(&id, &frame)?;
        let (start, end) = (self.b.span(frame.at, frame.at), self.b.span(sep, sep));
        self.b
            .db
            .add_connector_mapping(frame.id.as_ref(), start, end);
        Ok(DocItem::Str(id))
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
                    self.b.db.add_global(&list);
                    list = frame.list;
                    collect = frame.collect;
                    continue;
                }
                let id = self
                    .b
                    .db
                    .add_sub_graph(frame.id.as_ref(), &list, frame.title.as_ref());
                self.attach_data(&id, &frame)?;
                let (start, end) = (self.b.span(frame.at, frame.at), self.b.span(at, at));
                self.b
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
        r: Reader::new(&DIALECT, &src, &lexed.tokens),
        b: Build {
            db,
            locs: &lexed.locs,
        },
    };
    p.r.header(&mut p.b.db.store)?;
    p.document()?;
    if !p.r.s.is(T::End) {
        return Err(p.r.fail_statement());
    }
    Ok(())
}
