//! Pele's `diagrams/flowchart/parser.ts`.

use super::{
    db::FlowDb,
    lexer::tokenize,
    syntax::{Dialect, FlowGraph, Reader},
    tokens::T,
    types::{DocItem, FlowText, Vertex},
};
use crate::{detect::Kind, error::Error, js::Lazy};

const NO_TAGS: [T; 21] = [
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
    T::Subgraph,
    T::EndKw,
    T::Down,
    T::Up,
    T::StartLink,
];

static DIALECT: Dialect = Dialect {
    diagram: Kind::Flowchart,
    blank: &[T::Space, T::Newline],
    line_ends: &[T::Semi, T::Newline],
    no_tags: &NO_TAGS,
    statement_start: "'SEMI', 'NEWLINE', 'SPACE', 'EOF', 'subgraph', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'STYLE', 'LINKSTYLE', 'CLASSDEF', 'CLASS', 'CLICK', 'NODE_STRING'",
};

/// Mermaid trims whitespace between a closing brace and the end of its line
/// before parsing.
static RE_BRACE_TRAIL: Lazy = Lazy::new(r"}\s*\n", "g");

struct Frame {
    id: Option<FlowText>,
    title: Option<FlowText>,
    /// The title is the id's own text.
    same: bool,
    list: Vec<DocItem>,
    collect: bool,
}

struct Parser<'a> {
    r: Reader<'a, FlowDb>,
}

impl Parser<'_> {
    fn add_data(&mut self, id: &str, data: &str) -> Result<(), Error> {
        let vertex = Vertex {
            metadata: Some(data),
            ..Vertex::default()
        };
        self.r.db.add_vertex(id, vertex)
    }

    fn vertex(&mut self) -> Result<String, Error> {
        let node = self.r.node()?;
        self.r.db.add_vertex(&node.id, node.vertex())?;
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
    fn nodes(&mut self) -> Result<(Vec<String>, Option<String>), Error> {
        let mut ids = vec![self.styled_vertex()?];
        loop {
            let data = self.r.s.is(T::ShapeData).then(|| self.r.shape_data());
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
                self.add_data(&ids[ids.len() - 1], &data)?;
            }
            ids.push(next);
        }
    }

    /// [`Parser::nodes`], with the shape data given to the last vertex.
    fn node(&mut self) -> Result<Vec<String>, Error> {
        let (ids, data) = self.nodes()?;
        if let Some(data) = data {
            self.add_data(&ids[ids.len() - 1], &data)?;
        }
        Ok(ids)
    }

    fn vertex_statement(&mut self) -> Result<Vec<String>, Error> {
        let mut stmt = self.node()?;
        let mut nodes = stmt.clone();
        while self.r.at_link() {
            let info = self.r.link()?;
            let next = self.node()?;
            self.r.db.add_link(&stmt, &next, Some(&info))?;
            nodes.splice(0..0, next.iter().cloned());
            stmt = next;
        }
        Ok(nodes)
    }

    fn subgraph_header(&mut self) -> Result<Frame, Error> {
        self.r.s.bump();
        if self.r.at_separator() {
            self.r.s.bump();
            return Ok(Frame {
                id: None,
                title: None,
                same: false,
                list: Vec::new(),
                collect: false,
            });
        }
        self.r.s.expect(T::Space, &self.r.separators(&[]))?;
        let id = self.r.text_no_tags()?;
        let mut title = None;
        if self.r.s.eat(T::Sqs) {
            title = Some(self.r.text()?);
            self.r.closed(T::Sqe)?;
        }
        if !self.r.at_separator() {
            return Err(self.r.s.fail(&self.r.separators(&[T::Sqs])));
        }
        self.r.s.bump();
        let same = title.is_none();
        Ok(Frame {
            title: Some(title.unwrap_or_else(|| id.clone())),
            id: Some(id),
            same,
            list: Vec::new(),
            collect: false,
        })
    }

    fn statement(&mut self) -> Result<DocItem, Error> {
        if let Some(item) = self.r.statement()? {
            return Ok(item);
        }
        if !self.r.at_vertex() {
            return Err(self.r.fail_statement());
        }
        let nodes = self.vertex_statement()?;
        self.r.separator()?;
        Ok(DocItem::List(nodes))
    }

    /// Subgraphs nest without recursion: each open one keeps the list and
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
                self.r.s.bump();
                let id = self.r.db.add_sub_graph(
                    frame.id.as_ref(),
                    &list,
                    frame.title.as_ref(),
                    frame.same,
                );
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
            } else if t == T::Subgraph {
                let mut frame = self.subgraph_header()?;
                frame.list = std::mem::take(&mut list);
                frame.collect = collect;
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

pub fn parse_flowchart(source: &str, db: &mut FlowDb) -> Result<(), Error> {
    let src = if source.contains('}') {
        RE_BRACE_TRAIL.replace(source, |_| "}\n".into())
    } else {
        source.to_string()
    };
    let tokens = tokenize(&src);
    let mut p = Parser {
        r: Reader::new(&DIALECT, &src, &tokens, db),
    };
    p.r.header()?;
    p.document()?;
    if !p.r.s.is(T::End) {
        return Err(p.r.fail_statement());
    }
    Ok(())
}
