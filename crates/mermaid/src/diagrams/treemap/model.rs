//! Pele's `diagrams/treemap/model.ts`.

use crate::diagrams::common::db::Titles;
use indexmap::IndexMap;
use serde::Serialize;

use super::parser::{ItemKind, RowAst, TreemapAst};
use crate::js::{self, Lazy};

static RE_ESCAPED_COMMA: Lazy = Lazy::new(r"\\,", "g");
static RE_COMMA: Lazy = Lazy::new(r",", "g");
static RE_MARK: Lazy = Lazy::new(r"§§§", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreemapNode {
    pub name: String,
    /// Sections have a list, which may be empty. Leaves have none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreemapNode>>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "js::json_number_opt"
    )]
    pub value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css_compiled_styles: Option<Vec<String>>,
}

pub struct TreemapItem {
    pub level: usize,
    pub name: String,
    pub leaf: bool,
    pub value: Option<f64>,
    pub class_selector: Option<String>,
    pub css_compiled_styles: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreemapModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    pub roots: Vec<TreemapNode>,
    pub classes: IndexMap<String, Vec<String>>,
}

/// The node at `path`, a list of child indices from the roots.
fn node_at<'a>(roots: &'a mut [TreemapNode], path: &[usize]) -> &'a mut TreemapNode {
    let mut node = &mut roots[path[0]];
    for &i in &path[1..] {
        node = &mut node.children.as_mut().expect("a section")[i];
    }
    node
}

/// A row belongs to the nearest section above it that is indented less.
pub fn build_hierarchy(items: Vec<TreemapItem>) -> Vec<TreemapNode> {
    let mut roots: Vec<TreemapNode> = Vec::new();
    // Each open section: its path from the roots, and its level.
    let mut stack: Vec<(Vec<usize>, usize)> = Vec::new();
    for item in items {
        let node = TreemapNode {
            name: item.name,
            children: if item.leaf { None } else { Some(Vec::new()) },
            value: if item.leaf { item.value } else { None },
            class_selector: item.class_selector,
            css_compiled_styles: item.css_compiled_styles,
        };

        while stack.last().is_some_and(|(_, level)| *level >= item.level) {
            stack.pop();
        }
        let path = match stack.last() {
            None => {
                roots.push(node);
                vec![roots.len() - 1]
            }
            Some((parent, _)) => {
                let parent_path = parent.clone();
                let children = node_at(&mut roots, &parent_path)
                    .children
                    .as_mut()
                    .expect("a section");
                children.push(node);
                let mut path = parent_path;
                path.push(children.len() - 1);
                path
            }
        };
        if !item.leaf {
            stack.push((path, item.level));
        }
    }
    roots
}

impl TreemapModel {
    pub fn new(ast: TreemapAst, title: Option<&str>) -> Self {
        let mut classes: IndexMap<String, Vec<String>> = IndexMap::new();
        for row in &ast.rows {
            let RowAst::ClassDef {
                class_name,
                style_text,
            } = row
            else {
                continue;
            };
            let styles = classes.entry(class_name.clone()).or_default();
            // An escaped comma stays in the value; the others separate
            // declarations.
            let text =
                RE_ESCAPED_COMMA.replace(style_text.as_deref().unwrap_or(""), |_| "§§§".into());
            let text = RE_COMMA.replace(&text, |_| ";".into());
            let text = RE_MARK.replace(&text, |_| ",".into());
            for style in text.split(';') {
                styles.push(style.to_string());
            }
        }

        let mut items = Vec::new();
        for row in ast.rows {
            let RowAst::Row { indent, item } = row else {
                continue;
            };
            let styles = item
                .class_selector
                .as_ref()
                .filter(|selector| !selector.is_empty())
                .and_then(|selector| classes.get(selector));
            let (leaf, value) = match item.kind {
                ItemKind::Leaf(value) => (true, Some(value)),
                ItemKind::Section => (false, None),
            };
            items.push(TreemapItem {
                level: indent.unwrap_or(0),
                name: item.name,
                leaf,
                value,
                class_selector: item.class_selector,
                css_compiled_styles: styles.filter(|styles| !styles.is_empty()).cloned(),
            });
        }

        Self {
            kind: "treemap",
            titles: ast.header.titles(title),
            roots: build_hierarchy(items),
            classes,
        }
    }
}
