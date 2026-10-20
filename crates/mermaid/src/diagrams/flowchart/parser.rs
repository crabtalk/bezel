//! Pele's `diagrams/flowchart/parser.ts`.

use super::{
    Flowchart,
    db::{FlowDb, destruct_link},
    lexer::tokenize,
    store::FlowStore,
    syntax::{Builder, Dialect, Reader, ShapeData},
    tokens::T,
    types::{DocItem, FlowText},
};
use crate::{
    detect::Kind,
    error::Error,
    js::{self, Lazy},
    util::chars::is_space,
};

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
    destruct_link,
};

/// Mermaid trims whitespace between a closing brace and the end of its line
/// before parsing.
static RE_BRACE_TRAIL: Lazy = Lazy::new(r"}\s*\n", "g");

struct Frame {
    id: Option<String>,
    title: Option<FlowText>,
    list: Vec<DocItem>,
    collect: bool,
}

struct Parser<'a> {
    r: Reader<'a>,
    db: &'a mut FlowDb,
}

impl Builder for FlowDb {
    type Payload = Flowchart;

    fn store(&mut self) -> &mut FlowStore<Flowchart> {
        self
    }

    fn attach(&mut self, id: &str, data: &ShapeData) -> Result<(), Error> {
        self.add_metadata(id, &data.text)
    }
}

impl Parser<'_> {
    fn subgraph_header(&mut self) -> Result<Frame, Error> {
        self.r.s.bump();
        if self.r.at_separator() {
            self.r.s.bump();
            return Ok(Frame {
                id: None,
                title: None,
                list: Vec::new(),
                collect: false,
            });
        }
        self.r.s.expect(T::Space, &self.r.separators(&[]))?;
        let text = self.r.text_no_tags()?;
        let mut title = None;
        if self.r.s.eat(T::Sqs) {
            title = Some(self.r.text()?);
            self.r.closed(T::Sqe)?;
        }
        if !self.r.at_separator() {
            return Err(self.r.s.fail(&self.r.separators(&[T::Sqs])));
        }
        self.r.s.bump();
        // Without a bracketed title, an id with whitespace is only a title.
        let id = (title.is_some() || !text.text.contains(|c: char| is_space(c)))
            .then(|| js::trim(&text.text).to_string());
        Ok(Frame {
            id,
            title: Some(title.unwrap_or(text)),
            list: Vec::new(),
            collect: false,
        })
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
                let (id, _) = self
                    .db
                    .add_sub_graph(frame.id, &list, frame.title.as_ref(), |_| true);
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
                let item = self.r.statement(self.db)?;
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
        r: Reader::new(&DIALECT, &src, &tokens),
        db,
    };
    p.r.header(p.db)?;
    p.document()?;
    if !p.r.s.is(T::End) {
        return Err(p.r.fail_statement());
    }
    Ok(())
}
