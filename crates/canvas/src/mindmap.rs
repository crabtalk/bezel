//! A mindmap is a canvas whose edges form a tree: a node's children are the
//! edges leaving it, in edge order.
//!
//! Layout writes positions back into the [`Canvas`], so the file stays plain
//! JSON Canvas that any other viewer opens as it was last laid out.

use std::collections::HashSet;

use crate::model::{Canvas, Edge, End, GROUP, Node, Side, TEXT};

/// Between a parent's right side and its children's left.
pub const GAP_X: i64 = 64;
/// Between two sibling subtrees.
pub const GAP_Y: i64 = 16;
/// A new node's box, until its content is measured.
pub const NODE_WIDTH: i64 = 200;
pub const NODE_HEIGHT: i64 = 40;

/// Tree motion from a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    Parent,
    FirstChild,
    PrevSibling,
    NextSibling,
}

pub fn parent<'a>(canvas: &'a Canvas, id: &str) -> Option<&'a str> {
    canvas
        .edges
        .iter()
        .find(|edge| edge.to_node == id)
        .map(|edge| edge.from_node.as_str())
}

pub fn children<'a>(canvas: &'a Canvas, id: &'a str) -> impl Iterator<Item = &'a str> {
    canvas
        .edges
        .iter()
        .filter(move |edge| edge.from_node == id)
        .map(|edge| edge.to_node.as_str())
}

/// Nodes nothing points at. Groups are frames, not branches.
pub fn roots(canvas: &Canvas) -> impl Iterator<Item = &Node> {
    canvas
        .nodes
        .iter()
        .filter(|node| node.kind != GROUP && parent(canvas, &node.id).is_none())
}

pub fn step(canvas: &Canvas, id: &str, toward: Toward) -> Option<String> {
    let sibling = |offset: isize| {
        let siblings: Vec<&str> = children(canvas, parent(canvas, id)?).collect();
        let at = siblings.iter().position(|s| *s == id)?;
        siblings
            .get(at.checked_add_signed(offset)?)
            .map(|s| s.to_string())
    };
    match toward {
        Toward::Parent => parent(canvas, id).map(str::to_owned),
        Toward::FirstChild => children(canvas, id).next().map(str::to_owned),
        Toward::PrevSibling => sibling(-1),
        Toward::NextSibling => sibling(1),
    }
}

/// Lay every tree out to the right of its root, which stays where it is.
///
/// A node reached twice hangs off the first parent that reaches it, so a stray
/// cycle or a cross link cannot loop the layout.
pub fn layout(canvas: &mut Canvas) {
    let roots: Vec<usize> = roots(canvas)
        .filter_map(|node| canvas.index_of(&node.id))
        .collect();
    let mut seen = HashSet::new();
    let trees: Vec<Branch> = roots
        .into_iter()
        .map(|root| branch(canvas, root, &mut seen))
        .collect();
    for tree in &trees {
        let root = &canvas.nodes[tree.ix];
        let (x, center) = (root.x, root.y + root.height / 2);
        place(canvas, tree, x, center);
    }
}

/// A new empty text node with no parent.
pub fn add_root(canvas: &mut Canvas, x: i64, y: i64) -> String {
    push_node(canvas, x, y)
}

/// A new empty text node as `parent`'s last child, beside it.
///
/// Edits place and do not lay out: a free canvas keeps every other node where
/// it was, and a mindmap calls [`layout`] after.
pub fn add_child(canvas: &mut Canvas, parent: &str) -> Option<String> {
    let at = canvas.node(parent)?;
    let (x, y) = (at.x + at.width + GAP_X, at.y);
    let id = push_node(canvas, x, y);
    let edge = canvas.mint();
    canvas.edges.push(tree_edge(edge, parent, &id));
    Some(id)
}

/// A new empty text node right after `of`, under the same parent.
pub fn add_sibling(canvas: &mut Canvas, of: &str) -> Option<String> {
    let parent = parent(canvas, of)?.to_owned();
    let at = canvas.edges.iter().position(|edge| edge.to_node == of)? + 1;
    let below = canvas.node(of)?;
    let (x, y) = (below.x, below.y + below.height + GAP_Y);
    let id = push_node(canvas, x, y);
    let edge = canvas.mint();
    canvas.edges.insert(at, tree_edge(edge, &parent, &id));
    Some(id)
}

/// Remove a node and its branch. Answers what to select next: the previous
/// sibling, else the next, else the parent.
pub fn remove(canvas: &mut Canvas, id: &str) -> Option<String> {
    let root = canvas.index_of(id)?;
    let next = step(canvas, id, Toward::PrevSibling)
        .or_else(|| step(canvas, id, Toward::NextSibling))
        .or_else(|| step(canvas, id, Toward::Parent));
    let mut seen = HashSet::new();
    let mut doomed = Vec::new();
    branch(canvas, root, &mut seen).collect(&mut doomed);
    let doomed: HashSet<String> = doomed
        .into_iter()
        .map(|ix| canvas.nodes[ix].id.clone())
        .collect();
    canvas.nodes.retain(|node| !doomed.contains(&node.id));
    canvas
        .edges
        .retain(|edge| !doomed.contains(&edge.from_node) && !doomed.contains(&edge.to_node));
    next
}

struct Branch {
    ix: usize,
    children: Vec<Branch>,
}

impl Branch {
    fn collect(&self, into: &mut Vec<usize>) {
        into.push(self.ix);
        for child in &self.children {
            child.collect(into);
        }
    }
}

fn branch(canvas: &Canvas, ix: usize, seen: &mut HashSet<usize>) -> Branch {
    seen.insert(ix);
    let mut kids = Vec::new();
    for to in children(canvas, &canvas.nodes[ix].id) {
        if let Some(kid) = canvas.index_of(to)
            && seen.insert(kid)
        {
            kids.push(kid);
        }
    }
    Branch {
        ix,
        children: kids
            .into_iter()
            .map(|kid| branch(canvas, kid, seen))
            .collect(),
    }
}

/// The height a branch takes: its node, or its children stacked, whichever is
/// taller.
fn span(canvas: &Canvas, branch: &Branch) -> i64 {
    canvas.nodes[branch.ix]
        .height
        .max(stack(canvas, &branch.children))
}

fn stack(canvas: &Canvas, branches: &[Branch]) -> i64 {
    let gaps = GAP_Y * (branches.len() as i64 - 1).max(0);
    branches.iter().map(|b| span(canvas, b)).sum::<i64>() + gaps
}

fn place(canvas: &mut Canvas, branch: &Branch, x: i64, center: i64) {
    let node = &mut canvas.nodes[branch.ix];
    node.x = x;
    node.y = center - node.height / 2;
    let column = x + node.width + GAP_X;
    let mut top = center - stack(canvas, &branch.children) / 2;
    for child in &branch.children {
        let span = span(canvas, child);
        place(canvas, child, column, top + span / 2);
        top += span + GAP_Y;
    }
}

fn push_node(canvas: &mut Canvas, x: i64, y: i64) -> String {
    let id = canvas.mint();
    canvas.nodes.push(Node {
        id: id.clone(),
        kind: TEXT.into(),
        x,
        y,
        width: NODE_WIDTH,
        height: NODE_HEIGHT,
        text: Some(String::new()),
        ..Node::default()
    });
    id
}

fn tree_edge(id: String, from: &str, to: &str) -> Edge {
    Edge {
        from_side: Some(Side::Right),
        to_side: Some(Side::Left),
        // The spec's default end is an arrow; a branch is not a direction.
        to_end: Some(End::None),
        ..Edge::new(id, from, to)
    }
}
