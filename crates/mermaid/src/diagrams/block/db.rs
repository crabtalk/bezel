//! Pele's `diagrams/block/db.ts`.

use crate::diagrams::common::style_class::StyleClass;
use indexmap::{IndexMap, IndexSet};

use super::types::{Block, BlockKind, BlockModel, Columns, Statement};
use crate::{Error, Kind, js};

const MAX_SPACES: f64 = 100000.0;

struct Frame {
    list: Vec<usize>,
    at: usize,
    parent: usize,
    children: Vec<usize>,
}

impl Frame {
    fn new(statements: Vec<Statement>, parent: usize) -> Self {
        let mut list = Vec::new();
        for statement in statements {
            match statement {
                Statement::One(at) => list.push(at),
                Statement::List(items) => list.extend(items),
            }
        }
        Self {
            list,
            at: 0,
            parent,
            children: Vec::new(),
        }
    }
}

impl BlockModel {
    pub fn new(title: Option<&str>) -> Self {
        Self {
            title: title.map(String::from),
            root: 0,
            edges: Vec::new(),
            classes: IndexMap::new(),
            by_id: IndexMap::new(),
            pending: IndexSet::new(),
            edge_count: IndexMap::new(),
            ids: 0,
            colors: 0,
            spaces: 0.0,
            nodes: vec![Block {
                id: Some("root".into()),
                kind: BlockKind::Composite,
                children: Some(Vec::new()),
                columns: Some(Columns::Auto),
                ..Block::default()
            }],
            class_lists: Vec::new(),
        }
    }

    pub fn alloc(&mut self, block: Block) -> usize {
        self.nodes.push(block);
        self.nodes.len() - 1
    }

    /// Mermaid's ids are random. These are counted, so the same text always
    /// gives the same output.
    pub fn generate_id(&mut self) -> String {
        self.ids += 1;
        format!("id-{}", self.ids)
    }

    pub fn add_style_class(&mut self, id: &str, style_attributes: &str) {
        let found = self
            .classes
            .entry(id.to_string())
            .or_insert_with(|| StyleClass::new(id));
        for attribute in style_attributes.split(',') {
            found.push_attribute(attribute);
        }
    }

    pub fn add_style_to_node(&mut self, id: &str, styles: &str) {
        let at = self.named(id);
        self.nodes[at].styles = Some(styles.split(',').map(String::from).collect());
    }

    /// Mermaid looks each id up before trimming it, so `class a, b hot`
    /// misses b. Here it is trimmed first.
    pub fn set_css_class(&mut self, item_ids: &str, css_class_name: &str) {
        for id in item_ids.split(',') {
            let at = self.named(js::trim(id));
            let list = match self.nodes[at].classes {
                Some(list) => list,
                None => {
                    self.class_lists.push(Vec::new());
                    let list = self.class_lists.len() - 1;
                    self.nodes[at].classes = Some(list);
                    list
                }
            };
            self.class_lists[list].push(css_class_name.to_string());
        }
    }

    /// Mermaid throws on a style for an unknown id, and a class for one makes
    /// the block of that id disappear when it is declared later. Here both
    /// wait for the block.
    fn named(&mut self, id: &str) -> usize {
        if let Some(&at) = self.by_id.get(id) {
            return at;
        }
        let at = self.alloc(Block {
            id: Some(id.to_string()),
            kind: BlockKind::Na,
            children: Some(Vec::new()),
            ..Block::default()
        });
        self.by_id.insert(id.to_string(), at);
        self.pending.insert(id.to_string());
        at
    }

    pub fn set_hierarchy(&mut self, statements: Vec<Statement>) -> Result<(), Error> {
        let mut stack = vec![Frame::new(statements, self.root)];
        while let Some(f) = stack.last_mut() {
            if f.at == f.list.len() {
                let f = stack.pop().expect("a frame");
                self.nodes[f.parent].children = Some(f.children);
                continue;
            }
            let at = f.list[f.at];
            f.at += 1;
            let parent = f.parent;
            let block = &self.nodes[at];
            match block.kind {
                BlockKind::ClassDef => {
                    let id = block.id.clone().unwrap_or_default();
                    let css = block.css.clone().unwrap_or_default();
                    self.add_style_class(&id, &css);
                    continue;
                }
                BlockKind::ApplyClass => {
                    let id = block.id.clone().unwrap_or_default();
                    let class = block.style_class.clone().unwrap_or_default();
                    self.set_css_class(&id, &class);
                    continue;
                }
                BlockKind::ApplyStyles => {
                    if let Some(styles) = block.styles_str.clone().filter(|s| !s.is_empty()) {
                        let id = block.id.clone().unwrap_or_default();
                        self.add_style_to_node(&id, &styles);
                    }
                    continue;
                }
                BlockKind::ColumnSetting => {
                    let columns = block.columns.unwrap_or(Columns::Auto);
                    self.nodes[parent].columns = Some(columns);
                    continue;
                }
                BlockKind::Edge => {
                    let key = block.id.clone().unwrap_or_default();
                    let count = self.edge_count.get(&key).copied().unwrap_or(0) + 1;
                    self.edge_count.insert(key.clone(), count);
                    self.nodes[at].id = Some(format!("{count}-{key}"));
                    self.edges.push(at);
                    continue;
                }
                _ => {}
            }

            // A composite whose header was a link statement has no id of its
            // own.
            if self.nodes[at].id.is_none() {
                let id = self.generate_id();
                self.nodes[at].id = Some(id);
            }
            let block = &mut self.nodes[at];
            let id = block.id.clone().unwrap_or_default();
            if block.label.as_deref().is_none_or(str::is_empty) {
                block.label = Some(if block.kind == BlockKind::Composite {
                    String::new()
                } else {
                    id.clone()
                });
            }
            let known = self.by_id.get(&id).copied();
            let fresh = known.is_none() || self.pending.shift_remove(&id);
            if !fresh {
                let known = known.expect("a known block");
                let (kind, label) = (self.nodes[at].kind, self.nodes[at].label.clone());
                if kind != BlockKind::Na {
                    self.nodes[known].kind = kind;
                }
                if label.as_deref() != Some(id.as_str()) {
                    self.nodes[known].label = label;
                }
            } else {
                if let Some(known) = known {
                    let (classes, styles) =
                        (self.nodes[known].classes, self.nodes[known].styles.clone());
                    self.nodes[at].classes = classes;
                    self.nodes[at].styles = styles;
                }
                if self.nodes[at].kind == BlockKind::Composite {
                    self.nodes[at].color_index = Some(self.colors as f64);
                    self.colors += 1;
                }
                self.by_id.insert(id.clone(), at);
            }

            let inner = std::mem::take(&mut self.nodes[at].statements);
            self.nodes[at].children = Some(Vec::new());
            let mut pushed = Vec::new();
            if self.nodes[at].kind == BlockKind::Space {
                let width = self.nodes[at].width.unwrap_or(1.0);
                if width.is_nan() || width > MAX_SPACES - self.spaces {
                    return Err(Error::limit(format!(
                        "A block diagram can hold at most {} space blocks.",
                        js::num_str(MAX_SPACES)
                    ))
                    .of(Kind::Block));
                }
                self.spaces += width.max(0.0);
                let mut j = 0usize;
                while (j as f64) < width {
                    let copy = Block {
                        id: Some(format!("{id}-{j}")),
                        children: Some(Vec::new()),
                        ..self.nodes[at].clone()
                    };
                    let copy_id = copy.id.clone().unwrap_or_default();
                    let copy = self.alloc(copy);
                    self.by_id.insert(copy_id, copy);
                    pushed.push(copy);
                    j += 1;
                }
            } else if fresh {
                pushed.push(at);
            }
            let f = stack.last_mut().expect("a frame");
            f.children.extend(pushed);
            if !inner.is_empty() {
                stack.push(Frame::new(inner, at));
            }
        }
        Ok(())
    }
}
