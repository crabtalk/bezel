//! Pele's `diagrams/block/parser.ts`.

use super::{
    db::{
        edge_str_to_edge_data, edge_str_to_edge_start_data, edge_str_to_pattern,
        edge_str_to_thickness, type_str_to_type,
    },
    lexer::tokenize,
    tokens::*,
    types::{Block, BlockModel, Statement},
};
use crate::mermaid::{Error, Kind, js};

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

struct Parser<'s, 'm> {
    src: &'s str,
    types: Vec<u8>,
    texts: Vec<&'s str>,
    starts: Vec<usize>,
    i: usize,
    db: &'m mut BlockModel,
}

impl Parser<'_, '_> {
    fn fail(&self, expected: &str) -> Error {
        let t = self.types[self.i];
        if t == ERROR {
            return Error::at(Kind::Block, self.src, self.starts[self.i], "", true);
        }
        Error::at(
            Kind::Block,
            self.src,
            self.starts[self.i],
            &format!(
                "Expecting {expected}, got '{}'",
                TOKEN_NAMES[usize::from(t)]
            ),
            false,
        )
    }

    fn expect(&mut self, kind: u8) -> Result<String, Error> {
        if self.types[self.i] != kind {
            return Err(self.fail(&format!("'{}'", TOKEN_NAMES[usize::from(kind)])));
        }
        self.i += 1;
        Ok(self.texts[self.i - 1].to_string())
    }

    fn node(&mut self) -> Result<NodeInfo, Error> {
        let id = self.expect(NODE_ID)?;
        if self.types[self.i] == NODE_DSTART {
            let open = self.texts[self.i];
            self.i += 1;
            let label = self.expect(STR)?;
            let close = self.expect(NODE_DEND)?;
            return Ok(NodeInfo {
                id,
                label: Some(label),
                type_str: Some(format!("{open}{close}")),
                directions: None,
            });
        }
        if self.types[self.i] == BLOCK_ARROW_START {
            let open = self.texts[self.i];
            self.i += 1;
            let label = self.expect(STR)?;
            let mut directions = vec![self.expect(DIR)?];
            while self.types[self.i] == DIR {
                directions.push(self.texts[self.i].to_string());
                self.i += 1;
            }
            let close = self.expect(BLOCK_ARROW_END)?;
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
        let size = if self.types[self.i] == SIZE {
            self.i += 1;
            js::parse_int(self.texts[self.i - 1], 10)
        } else {
            1.0
        };
        let mut from = Block {
            id: Some(first.id),
            label: first.label,
            kind: Some(type_str_to_type(first.type_str.as_deref()).into()),
            directions: first.directions,
            width_in_columns: Some(size),
            ..Block::default()
        };
        let mut chain: Option<Vec<usize>> = None;
        while self.types[self.i] == LINK || self.types[self.i] == START_LINK {
            let mut label = String::new();
            self.i += 1;
            if self.types[self.i - 1] == START_LINK {
                self.expect(LINK_LABEL)?;
                label = self.expect(STR)?;
                self.expect(LINK)?;
            }
            let link = self.texts[self.i - 1];
            let to = self.node()?;
            // Mermaid keeps only the last link of `A --> B --> C` and loses
            // its start. Here every link counts.
            let chain = chain.get_or_insert_with(|| {
                vec![self.db.alloc(Block {
                    id: from.id.clone(),
                    label: from.label.clone(),
                    kind: from.kind.clone(),
                    directions: from.directions.clone(),
                    ..Block::default()
                })]
            });
            from = Block {
                id: Some(to.id.clone()),
                label: to.label,
                kind: Some(type_str_to_type(to.type_str.as_deref()).into()),
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
                kind: Some("edge".into()),
                thickness: Some(edge_str_to_thickness(link).into()),
                pattern: Some(edge_str_to_pattern(link).into()),
                directions: to.directions,
                arrow_type_end: Some(edge_str_to_edge_data(link).into()),
                arrow_type_start: Some(edge_str_to_edge_start_data(link).into()),
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
        self.expect(KEY)?;
        // Composites nest without recursion: each open one keeps the
        // statement list of its parent.
        let mut open: Vec<Frame> = Vec::new();
        let mut list: Vec<Statement> = Vec::new();
        loop {
            match self.types[self.i] {
                NODE_ID => {
                    let statement = self.node_statement()?;
                    list.push(statement);
                }
                COLUMNS => {
                    let columns = js::parse_int(self.texts[self.i], 10);
                    self.i += 1;
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: Some("column-setting".into()),
                        columns: Some(columns),
                        ..Block::default()
                    })));
                }
                SPACE_BLOCK => {
                    let width = js::parse_int(self.texts[self.i], 10);
                    self.i += 1;
                    let id = self.db.generate_id();
                    list.push(Statement::One(self.db.alloc(Block {
                        id: Some(id),
                        kind: Some("space".into()),
                        label: Some(String::new()),
                        width: Some(width),
                        children: Some(Vec::new()),
                        ..Block::default()
                    })));
                }
                KEY => {
                    self.i += 1;
                    open.push(Frame {
                        list: std::mem::take(&mut list),
                        header: None,
                    });
                }
                ID_BLOCK => {
                    self.i += 1;
                    if self.types[self.i] != NODE_ID {
                        return Err(self.fail("'NODE_ID'"));
                    }
                    let header = self.node_statement()?;
                    open.push(Frame {
                        list: std::mem::take(&mut list),
                        header: Some(header),
                    });
                }
                END_KEYWORD => {
                    let Some(frame) = open.pop().filter(|_| !list.is_empty()) else {
                        return Err(self.fail(STATEMENT_START));
                    };
                    self.i += 1;
                    let id = self.db.generate_id();
                    let children = close_document(std::mem::take(&mut list));
                    list = frame.list;
                    let composite = match frame.header {
                        None => self.db.alloc(Block {
                            id: Some(id),
                            kind: Some("composite".into()),
                            label: Some(String::new()),
                            statements: children,
                            ..Block::default()
                        }),
                        Some(Statement::One(header)) => {
                            let block = &mut self.db.nodes[header];
                            block.kind = Some("composite".into());
                            block.statements = children;
                            header
                        }
                        Some(Statement::List(items)) => self.db.alloc(Block {
                            kind: Some("composite".into()),
                            statements: children,
                            spread: items,
                            ..Block::default()
                        }),
                    };
                    list.push(Statement::One(composite));
                }
                CLASSDEF => {
                    self.i += 1;
                    let id = js::trim(&self.expect(CLASSDEF_ID)?).to_string();
                    let css = js::trim(&self.expect(CLASSDEF_STYLEOPTS)?).to_string();
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: Some("classDef".into()),
                        id: Some(id),
                        css: Some(css),
                        ..Block::default()
                    })));
                }
                CLASS => {
                    self.i += 1;
                    let id = js::trim(&self.expect(CLASSENTITY_IDS)?).to_string();
                    let class = js::trim(&self.expect(STYLECLASS)?).to_string();
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: Some("applyClass".into()),
                        id: Some(id),
                        style_class: Some(class),
                        ..Block::default()
                    })));
                }
                STYLE => {
                    self.i += 1;
                    let id = js::trim(&self.expect(STYLE_ENTITY_IDS)?).to_string();
                    let styles = js::trim(&self.expect(STYLE_DEFINITION_DATA)?).to_string();
                    list.push(Statement::One(self.db.alloc(Block {
                        kind: Some("applyStyles".into()),
                        id: Some(id),
                        styles_str: Some(styles),
                        ..Block::default()
                    })));
                }
                EOF => {
                    if !open.is_empty() {
                        return Err(self.fail(&format!("{STATEMENT_START}, 'end'")));
                    }
                    if list.is_empty() {
                        return Err(self.fail(STATEMENT_START));
                    }
                    return self.db.set_hierarchy(close_document(list));
                }
                _ => {
                    return Err(self.fail(&if list.is_empty() {
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
        src,
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        i: 0,
        db,
    }
    .run()
}
