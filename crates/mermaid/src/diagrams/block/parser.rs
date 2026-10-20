//! Pele's `diagrams/block/parser.ts`.

use super::{
    lexer::tokenize,
    tokens::T,
    types::{Block, BlockKind, BlockModel, Columns, EdgeLine, Statement},
};
use crate::{Error, detect::Kind, js, token::Stream};

const STATEMENT_START: &str = "'NODE_ID', 'COLUMNS', 'SPACE_BLOCK', 'BLOCK_DIAGRAM_KEY', 'id-block', 'classDef', 'class', 'style'";

struct NodeInfo {
    id: String,
    label: Option<String>,
    type_str: Option<String>,
    directions: Option<Vec<String>>,
}

struct Frame {
    list: Vec<Statement>,
    header: Option<Statement>,
}

/// The grammar inlines a document's last statement when it is a list.
fn close_document(mut list: Vec<Statement>) -> Vec<Statement> {
    if let Some(Statement::List(items)) = list.last() {
        let items = items.clone();
        list.pop();
        list.extend(items.into_iter().map(Statement::One));
    }
    list
}

struct Parser<'t, 's, 'm> {
    s: Stream<'t, 's, T>,
    db: &'m mut BlockModel,
}

impl Parser<'_, '_, '_> {
    fn node(&mut self) -> Result<NodeInfo, Error> {
        let id = self.s.expect(T::NodeId, "")?.to_string();
        if self.s.is(T::NodeDstart) {
            let open = self.s.take();
            let label = self.s.expect(T::Str, "")?.to_string();
            let close = self.s.expect(T::NodeDend, "")?;
            return Ok(NodeInfo {
                id,
                label: Some(label),
                type_str: Some(format!("{open}{close}")),
                directions: None,
            });
        }
        if self.s.is(T::BlockArrowStart) {
            let open = self.s.take();
            let label = self.s.expect(T::Str, "")?.to_string();
            let mut directions = vec![self.s.expect(T::Dir, "")?.to_string()];
            while self.s.is(T::Dir) {
                directions.push(self.s.take().to_string());
            }
            let close = self.s.expect(T::BlockArrowEnd, "")?;
            return Ok(NodeInfo {
                id,
                label: Some(label),
                type_str: Some(format!("{open}{close}")),
                directions: Some(directions),
            });
        }
        Ok(NodeInfo {
            id,
            label: None,
            type_str: None,
            directions: None,
        })
    }

    fn node_statement(&mut self) -> Result<Statement, Error> {
        let first = self.node()?;
        let size = if self.s.is(T::Size) {
            js::parse_int(self.s.take(), 10)
        } else {
            1.0
        };
        let mut from = Block {
            id: Some(first.id),
            label: first.label,
            kind: BlockKind::from_brackets(first.type_str.as_deref()),
            directions: first.directions,
            width_in_columns: Some(size),
            ..Block::default()
        };
        let mut chain: Option<Vec<usize>> = None;
        while self.s.is_any(&[T::Link, T::StartLink]) {
            let mut label = String::new();
            let link = if self.s.eat(T::StartLink) {
                self.s.expect(T::LinkLabel, "")?;
                label = self.s.expect(T::Str, "")?.to_string();
                self.s.expect(T::Link, "")?
            } else {
                self.s.take()
            };
            let to = self.node()?;
            // Mermaid keeps only the last link of `A --> B --> C` and loses
            // its start. Here every link counts.
            let chain = chain.get_or_insert_with(|| {
                vec![self.db.alloc(Block {
                    id: from.id.clone(),
                    label: from.label.clone(),
                    kind: from.kind,
                    directions: from.directions.clone(),
                    ..Block::default()
                })]
            });
            from = Block {
                id: Some(to.id.clone()),
                label: to.label,
                kind: BlockKind::from_brackets(to.type_str.as_deref()),
                directions: to.directions.clone(),
                ..Block::default()
            };
            let last = self.db.nodes[*chain.last().expect("a block")]
                .id
                .clone()
                .unwrap_or_default();
            let edge = self.db.alloc(Block {
                id: Some(format!("{last}-{}", to.id)),
                start: Some(last),
                end: Some(to.id),
                label: Some(label),
                kind: BlockKind::Edge,
                line: Some(EdgeLine::of(link)),
                directions: to.directions,
                ..Block::default()
            });
            let block = self.db.alloc(from.clone());
            chain.push(edge);
            chain.push(block);
        }
        Ok(match chain {
            Some(chain) => Statement::List(chain),
            None => Statement::One(self.db.alloc(from)),
        })
    }

    fn run(&mut self) -> Result<(), Error> {
        self.s.expect(T::Key, "")?;
        // Composites nest without recursion: each open one keeps the
        // statement list of its parent.
        let mut open: Vec<Frame> = Vec::new();
        let mut list: Vec<Statement> = Vec::new();
        loop {
            match self.s.kind() {
                T::NodeId => {
                    let statement = self.node_statement()?;
                    list.push(statement);
                }
                T::Columns => {
                    // The lexer writes `auto` as `-1`.
                    let columns = match self.s.take() {
                        "-1" => Columns::Auto,
                        count => Columns::Count(js::parse_int(count, 10)),
                    };
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: BlockKind::ColumnSetting,
                        columns: Some(columns),
                        ..Block::default()
                    })));
                }
                T::SpaceBlock => {
                    let width = js::parse_int(self.s.take(), 10);
                    let id = self.db.generate_id();
                    list.push(Statement::One(self.db.alloc(Block {
                        id: Some(id),
                        kind: BlockKind::Space,
                        label: Some(String::new()),
                        width: Some(width),
                        children: Some(Vec::new()),
                        ..Block::default()
                    })));
                }
                T::Key => {
                    self.s.bump();
                    open.push(Frame {
                        list: std::mem::take(&mut list),
                        header: None,
                    });
                }
                T::IdBlock => {
                    self.s.bump();
                    if !self.s.is(T::NodeId) {
                        return Err(self.s.fail("'NODE_ID'"));
                    }
                    let header = self.node_statement()?;
                    open.push(Frame {
                        list: std::mem::take(&mut list),
                        header: Some(header),
                    });
                }
                T::EndKeyword => {
                    let Some(frame) = open.pop().filter(|_| !list.is_empty()) else {
                        return Err(self.s.fail(STATEMENT_START));
                    };
                    self.s.bump();
                    let id = self.db.generate_id();
                    let children = close_document(std::mem::take(&mut list));
                    list = frame.list;
                    let composite = match frame.header {
                        None => self.db.alloc(Block {
                            id: Some(id),
                            kind: BlockKind::Composite,
                            label: Some(String::new()),
                            statements: children,
                            ..Block::default()
                        }),
                        Some(Statement::One(header)) => {
                            let block = &mut self.db.nodes[header];
                            block.kind = BlockKind::Composite;
                            block.statements = children;
                            header
                        }
                        Some(Statement::List(items)) => self.db.alloc(Block {
                            kind: BlockKind::Composite,
                            statements: children,
                            spread: items,
                            ..Block::default()
                        }),
                    };
                    list.push(Statement::One(composite));
                }
                T::Classdef => {
                    self.s.bump();
                    let id = js::trim(self.s.expect(T::ClassdefId, "")?).to_string();
                    let css = js::trim(self.s.expect(T::ClassdefStyleopts, "")?).to_string();
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: BlockKind::ClassDef,
                        id: Some(id),
                        css: Some(css),
                        ..Block::default()
                    })));
                }
                T::Class => {
                    self.s.bump();
                    let id = js::trim(self.s.expect(T::ClassentityIds, "")?).to_string();
                    let class = js::trim(self.s.expect(T::Styleclass, "")?).to_string();
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: BlockKind::ApplyClass,
                        id: Some(id),
                        style_class: Some(class),
                        ..Block::default()
                    })));
                }
                T::Style => {
                    self.s.bump();
                    let id = js::trim(self.s.expect(T::StyleEntityIds, "")?).to_string();
                    let styles = js::trim(self.s.expect(T::StyleDefinitionData, "")?).to_string();
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: BlockKind::ApplyStyles,
                        id: Some(id),
                        styles_str: Some(styles),
                        ..Block::default()
                    })));
                }
                T::Eof => {
                    if !open.is_empty() {
                        return Err(self.s.fail(&format!("{STATEMENT_START}, 'end'")));
                    }
                    if list.is_empty() {
                        return Err(self.s.fail(STATEMENT_START));
                    }
                    return self.db.set_hierarchy(close_document(list));
                }
                _ => {
                    return Err(self.s.fail(&if list.is_empty() {
                        STATEMENT_START.to_string()
                    } else if !open.is_empty() {
                        format!("{STATEMENT_START}, 'end'")
                    } else {
                        format!("{STATEMENT_START}, 'EOF'")
                    }));
                }
            }
        }
    }
}

pub fn parse_block(src: &str, db: &mut BlockModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    Parser {
        s: Stream::new(Kind::Block, src, &tokens),
        db,
    }
    .run()
}
