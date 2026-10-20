//! Pele's `diagrams/treeview/model.ts`.

use crate::diagrams::common::db::Titles;
use serde::Serialize;

use super::{
    box_drawing::{preprocess_box_drawing, remap_error_lines},
    parser::{TreeNodeAst, TreeViewAst, parse_tree_view},
};
use crate::{
    Error,
    js::{self},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeType {
    File,
    Directory,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub id: usize,
    #[serde(serialize_with = "js::json_number")]
    pub level: f64,
    pub name: String,
    pub node_type: NodeType,
    /// An icon name as written (`pack:name`, `file`, `folder`), or `none` to
    /// hide the icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub children: Vec<TreeNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeViewModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    /// A directory named `/` at level -1 that holds the top-level entries.
    pub root: TreeNode,
    pub count: usize,
}

struct Flat {
    node: TreeNode,
    children: Vec<usize>,
}

/// Mermaid's treeView database: nodes arrive in source order with their
/// indentation.
pub struct TreeViewDb {
    pub count: usize,
    nodes: Vec<Flat>,
    stack: Vec<usize>,
}

impl TreeViewDb {
    pub fn new() -> Self {
        Self {
            count: 1,
            nodes: vec![Flat {
                node: TreeNode {
                    id: 0,
                    level: -1.0,
                    name: "/".into(),
                    node_type: NodeType::Directory,
                    icon: None,
                    css_class: None,
                    description: None,
                    children: Vec::new(),
                },
                children: Vec::new(),
            }],
            stack: vec![0],
        }
    }

    pub fn add_node(
        &mut self,
        level: f64,
        name: String,
        node_type: NodeType,
        css_class: Option<String>,
        icon: Option<String>,
        description: Option<String>,
    ) {
        while level <= self.nodes[*self.stack.last().expect("the root")].node.level {
            self.stack.pop();
        }
        let at = self.nodes.len();
        self.nodes.push(Flat {
            node: TreeNode {
                id: self.count,
                level,
                name,
                node_type,
                icon,
                css_class,
                description,
                children: Vec::new(),
            },
            children: Vec::new(),
        });
        self.count += 1;
        let parent = *self.stack.last().expect("the root");
        self.nodes[parent].children.push(at);
        self.stack.push(at);
    }

    pub fn populate(&mut self, nodes: Vec<TreeNodeAst>) {
        for node in nodes {
            let level = node.indent.map_or(0.0, |indent| indent as f64);
            let mut name = node.name;
            let is_directory = name.ends_with('/');
            if is_directory {
                name.pop();
            }
            self.add_node(
                level,
                name,
                if is_directory {
                    NodeType::Directory
                } else {
                    NodeType::File
                },
                node.class_annotation.filter(|class| !class.is_empty()),
                node.icon_annotation
                    .map(|icon| if icon.is_empty() { "none".into() } else { icon }),
                node.desc_annotation.filter(|descr| !descr.is_empty()),
            );
        }
    }

    /// The tree under the root. A node's children come after it, so building
    /// from the last node up finds every child built.
    pub fn root(self) -> TreeNode {
        let mut built: Vec<Option<TreeNode>> = Vec::with_capacity(self.nodes.len());
        let mut lists = Vec::with_capacity(self.nodes.len());
        for flat in self.nodes {
            built.push(Some(flat.node));
            lists.push(flat.children);
        }
        for at in (0..built.len()).rev() {
            let children = lists[at]
                .iter()
                .map(|&child| built[child].take().expect("a child built once"))
                .collect();
            built[at].as_mut().expect("not yet taken").children = children;
        }
        built[0].take().expect("the root")
    }
}

/// Reads the source to a syntax tree, accepting box-drawing input. Errors
/// name lines of the original text.
pub fn read_tree_view(source: &str) -> Result<TreeViewAst, Error> {
    let pre = preprocess_box_drawing(source)?;
    parse_tree_view(&pre.text).map_err(|mut error| {
        if pre.line_map.is_empty() {
            return error;
        }
        error.message = match error.message.find('\n') {
            None => remap_error_lines(&error.message, &pre.line_map),
            Some(head) => {
                remap_error_lines(&error.message[..head], &pre.line_map) + &error.message[head..]
            }
        };
        if let Some(&line) = pre.line_map.get(&error.line) {
            error.line = line;
        }
        error
    })
}

impl TreeViewModel {
    pub fn new(ast: TreeViewAst, title: Option<&str>) -> Self {
        let mut db = TreeViewDb::new();
        db.populate(ast.nodes);
        let count = db.count;
        Self {
            kind: "treeView",
            titles: ast.header.titles(title),
            root: db.root(),
            count,
        }
    }
}
